//! Transactional caller-owned output for standalone Huffman operations.
//!
//! `EntropyOutput` owns reusable initialized scratch; codebooks, paired table
//! descriptions, input, and caller destinations are borrowed only for a call.
//! Scalar stream choices and size plans use one safe representation. Hot writes
//! and code lookups retain their independent BitWriter/codebook backends. This
//! module adds no pointers, unsafe operations, or C runtime dependency.

use alloc::vec::Vec;

use crate::bitwriter::BitWriter;
use crate::huffman_codebook::{BuiltStandaloneTable, HuffmanCodebook};
use crate::{Error, Result};

const SOURCE_LIMIT: usize = 256 * 1024;

/// A standalone operation's number of independent reverse-consumed streams.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntropyStreams {
    One,
    Four,
}

/// Scratch is reusable after success, zero result, and errors. Only a completed
/// successful operation copies bytes into its caller's destination.
pub(crate) struct EntropyOutput {
    writer: BitWriter,
    four: Vec<u8>,
}

impl EntropyOutput {
    pub(crate) const fn new() -> Self {
        Self {
            writer: BitWriter::new(),
            four: Vec::new(),
        }
    }

    /// Owned heap capacity, excluding this value and all borrowed destinations.
    pub(crate) fn retained_capacity(&self) -> usize {
        self.writer.retained_capacity() + self.four.capacity()
    }

    /// Encode a standalone 1X/4X stream with the low-level C capacity contract.
    ///
    /// Corresponds to `HUF_compress1X_usingCTable` and
    /// `HUF_compress4X_usingCTable` in facebook/zstd v1.5.7. `Ok(None)` is its
    /// zero result: the destination lacks native-word tail slack, four-stream
    /// input is shorter than twelve bytes, or any of its four streams exceeds
    /// 65,535 encoded bytes. The fourth-stream bound is an upstream compressor
    /// rule even though no jump field stores its size. These rules differ from
    /// the wire encoder, which accepts valid six-/nine-byte four-stream inputs.
    ///
    /// Native slack uses `size_of::<usize>()`, matching the C size_t container
    /// on the target. An encoded stream fitting exactly is insufficient: C
    /// still reserves a word and requires the whole-byte cursor before its end.
    /// Sources above 256 KiB and arithmetic overflow are resource errors;
    /// unavailable symbols are corrupt input. Source validation precedes the
    /// zero-result decision. Every error and zero result leaves all caller
    /// bytes unchanged; success changes exactly the returned prefix.
    pub(crate) fn encode(
        &mut self,
        codebook: &HuffmanCodebook,
        input: &[u8],
        output: &mut [u8],
        streams: EntropyStreams,
    ) -> Result<Option<usize>> {
        let Some(expected) = encoded_size(
            codebook,
            input,
            output.len(),
            streams,
            core::mem::size_of::<usize>(),
        )?
        else {
            return Ok(None);
        };
        let encoded = match streams {
            EntropyStreams::One => {
                codebook.encode_stream(input, &mut self.writer)?;
                self.writer.as_bytes()
            }
            EntropyStreams::Four => {
                codebook.encode_standalone_four_streams(input, &mut self.writer, &mut self.four)?;
                &self.four
            }
        };
        if encoded.len() != expected {
            return Err(Error::CorruptData);
        }
        output[..expected].copy_from_slice(encoded);
        Ok(Some(expected))
    }
}

/// Copy the exact description already paired with a standalone codebook.
///
/// Exact fit succeeds; too-small output returns `ResourceLimit` without writes.
/// This is an independent Rust serialization boundary. It does not emulate
/// `HUF_writeCTable_wksp`'s capacity-dependent attempt to compress weights, which
/// may select a different raw/FSE representation or fail despite a particular
/// precomputed description fitting. No second tree is selected during copying.
pub(crate) fn write_description(table: &BuiltStandaloneTable, output: &mut [u8]) -> Result<usize> {
    let description = table.description();
    output
        .get_mut(..description.len())
        .ok_or(Error::ResourceLimit)?
        .copy_from_slice(description);
    Ok(description.len())
}

/// C's final flush writes a complete native word, then rejects a cursor at or
/// beyond endPtr. The terminal marker is included in `bits`. Keep word width a
/// scalar argument so normal tests verify both 32-bit and 64-bit thresholds.
fn minimum_capacity(bits: usize, word_bytes: usize) -> Result<usize> {
    if bits == 0 || !matches!(word_bytes, 4 | 8) {
        return Err(Error::InvalidParameter);
    }
    (bits / 8)
        .checked_add(word_bytes)
        .and_then(|size| size.checked_add(1))
        .map(|size| size.max(8))
        .ok_or(Error::ResourceLimit)
}

fn stream_bits(codebook: &HuffmanCodebook, input: &[u8]) -> Result<usize> {
    input.iter().try_fold(1_usize, |bits, &symbol| {
        bits.checked_add(usize::from(codebook.code(symbol)?.bits))
            .ok_or(Error::ResourceLimit)
    })
}

fn encoded_size(
    codebook: &HuffmanCodebook,
    input: &[u8],
    capacity: usize,
    streams: EntropyStreams,
    word_bytes: usize,
) -> Result<Option<usize>> {
    if input.len() > SOURCE_LIMIT {
        return Err(Error::ResourceLimit);
    }
    if !codebook.covers(input) {
        return Err(Error::CorruptData);
    }
    if streams == EntropyStreams::One {
        let bits = stream_bits(codebook, input)?;
        return Ok((capacity >= minimum_capacity(bits, word_bytes)?).then_some(bits.div_ceil(8)));
    }
    if input.len() < 12 || capacity < 17 {
        return Ok(None);
    }
    let quarter = input.len().div_ceil(4);
    let third_end = quarter.checked_mul(3).ok_or(Error::ResourceLimit)?;
    if third_end > input.len() {
        return Ok(None);
    }
    let ends = [quarter, quarter * 2, third_end, input.len()];
    let mut start = 0;
    let mut position = 6_usize;
    for end in ends {
        let bits = stream_bits(codebook, &input[start..end])?;
        let bytes = bits.div_ceil(8);
        if bytes > usize::from(u16::MAX)
            || capacity.saturating_sub(position) < minimum_capacity(bits, word_bytes)?
        {
            return Ok(None);
        }
        position = position.checked_add(bytes).ok_or(Error::ResourceLimit)?;
        start = end;
    }
    Ok(Some(position))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn one_bit_table() -> BuiltStandaloneTable {
        BuiltStandaloneTable::from_description(vec![128, 0x10]).unwrap()
    }

    fn twelve_bit_table() -> BuiltStandaloneTable {
        // Twelve explicit weights 12..=1, with final implicit weight one.
        BuiltStandaloneTable::from_description(vec![139, 0xcb, 0xa9, 0x87, 0x65, 0x43, 0x21])
            .unwrap()
    }

    #[test]
    fn native_word_thresholds_cover_empty_aligned_and_partial_markers_on_both_widths() {
        for word in [4, 8] {
            for bits in [1_usize, 7, 8, 9, 15, 16, 17, 65535 * 8, usize::MAX] {
                let expected = (bits / 8 + word + 1).max(8);
                assert_eq!(minimum_capacity(bits, word), Ok(expected));
            }
        }
        assert_eq!(minimum_capacity(1, 8), Ok(9));
        assert_eq!(minimum_capacity(8, 8), Ok(10));
        assert_eq!(minimum_capacity(0, 8), Err(Error::InvalidParameter));
        assert_eq!(minimum_capacity(1, 16), Err(Error::InvalidParameter));
    }

    #[test]
    fn one_stream_capacity_sweep_preserves_all_failed_destinations_and_success_tails() {
        let table = one_bit_table();
        let mut scratch = EntropyOutput::new();
        for length in 0..=128 {
            let source: Vec<_> = (0..length).map(|index| (index & 1) as u8).collect();
            let mut expected = BitWriter::new();
            table
                .codebook()
                .encode_stream(&source, &mut expected)
                .unwrap();
            let threshold = minimum_capacity(length + 1, core::mem::size_of::<usize>()).unwrap();
            for capacity in 0..=threshold + 2 {
                let mut guarded = vec![0xa5; capacity + 2];
                let result = scratch
                    .encode(
                        table.codebook(),
                        &source,
                        &mut guarded[1..=capacity],
                        EntropyStreams::One,
                    )
                    .unwrap();
                if capacity < threshold {
                    assert_eq!(result, None);
                    assert!(guarded.iter().all(|&byte| byte == 0xa5));
                } else {
                    let size = result.unwrap();
                    assert_eq!(&guarded[1..1 + size], expected.as_bytes());
                    assert!(guarded[1 + size..].iter().all(|&byte| byte == 0xa5));
                    assert_eq!(guarded[0], 0xa5);
                }
            }
        }
    }

    #[test]
    fn four_stream_policy_preserves_small_wire_inputs_and_native_slack_distinction() {
        let table = one_bit_table();
        let mut scratch = EntropyOutput::new();
        for length in 0..=11 {
            let source = vec![0; length];
            let mut destination = [0xa5; 64];
            assert_eq!(
                scratch.encode(
                    table.codebook(),
                    &source,
                    &mut destination,
                    EntropyStreams::Four
                ),
                Ok(None)
            );
            assert_eq!(destination, [0xa5; 64]);
        }
        for length in 12..=128 {
            let source = vec![0; length];
            let mut expected = Vec::new();
            let mut writer = BitWriter::new();
            table
                .codebook()
                .encode_standalone_four_streams(&source, &mut writer, &mut expected)
                .unwrap();
            for capacity in 0..=expected.len() + 12 {
                let mut guarded = vec![0xa5; capacity + 2];
                let result = scratch
                    .encode(
                        table.codebook(),
                        &source,
                        &mut guarded[1..=capacity],
                        EntropyStreams::Four,
                    )
                    .unwrap();
                if let Some(size) = result {
                    assert_eq!(size, expected.len());
                    assert_eq!(&guarded[1..1 + size], expected);
                    assert!(guarded[1 + size..].iter().all(|&byte| byte == 0xa5));
                    assert_eq!(guarded[0], 0xa5);
                } else {
                    assert!(guarded.iter().all(|&byte| byte == 0xa5));
                }
                if capacity <= expected.len() {
                    assert_eq!(result, None);
                }
            }
        }
    }

    #[test]
    fn paired_description_copy_accepts_exact_fit_without_selecting_another_tree() {
        for table in [one_bit_table(), twelve_bit_table()] {
            for capacity in 0..=table.description().len() + 2 {
                let mut guarded = vec![0xa5; capacity + 2];
                let result = write_description(&table, &mut guarded[1..=capacity]);
                if capacity < table.description().len() {
                    assert_eq!(result, Err(Error::ResourceLimit));
                    assert!(guarded.iter().all(|&byte| byte == 0xa5));
                } else {
                    assert_eq!(result, Ok(table.description().len()));
                    assert_eq!(
                        &guarded[1..1 + table.description().len()],
                        table.description()
                    );
                    assert!(
                        guarded[1 + table.description().len()..]
                            .iter()
                            .all(|&byte| byte == 0xa5)
                    );
                    assert_eq!(guarded[0], 0xa5);
                }
            }
        }
    }

    #[test]
    fn invalid_sources_leave_destination_and_existing_scratch_available_for_reuse() {
        let table = one_bit_table();
        let mut scratch = EntropyOutput::new();
        let mut destination = [0xa5; 128];
        assert!(
            scratch
                .encode(
                    table.codebook(),
                    &[0; 128],
                    &mut destination,
                    EntropyStreams::Four
                )
                .unwrap()
                .is_some()
        );
        let retained = scratch.retained_capacity();
        for (source, error) in [
            (vec![2; 128], Error::CorruptData),
            (vec![0; SOURCE_LIMIT + 1], Error::ResourceLimit),
        ] {
            for streams in [EntropyStreams::One, EntropyStreams::Four] {
                destination.fill(0xa5);
                assert_eq!(
                    scratch.encode(table.codebook(), &source, &mut destination, streams),
                    Err(error)
                );
                assert_eq!(destination, [0xa5; 128]);
                assert_eq!(scratch.retained_capacity(), retained);
            }
        }
        assert!(
            scratch
                .encode(
                    table.codebook(),
                    &[0; 128],
                    &mut destination,
                    EntropyStreams::Four
                )
                .unwrap()
                .is_some()
        );
        assert_eq!(scratch.retained_capacity(), retained);
    }

    #[test]
    fn source_limit_and_every_quarter_size_match_the_low_level_compressor_contract() {
        let table = twelve_bit_table();
        let mut scratch = EntropyOutput::new();
        let mut output = vec![0xa5; SOURCE_LIMIT * 2];
        let short = vec![0; SOURCE_LIMIT];
        for streams in [EntropyStreams::One, EntropyStreams::Four] {
            assert!(
                scratch
                    .encode(table.codebook(), &short, &mut output, streams)
                    .unwrap()
                    .is_some()
            );
        }
        let quarter = 43690;
        let mut exact = vec![12; quarter * 4];
        for start in [0, quarter, quarter * 2, quarter * 3] {
            exact[start] = 10;
        }
        let size = scratch
            .encode(table.codebook(), &exact, &mut output, EntropyStreams::Four)
            .unwrap()
            .unwrap();
        assert_eq!(size, 6 + 4 * 65535);
        for index in 0..3 {
            assert_eq!(
                u16::from_le_bytes(output[index * 2..index * 2 + 2].try_into().unwrap()),
                65535
            );
        }
        for part in 0..4 {
            let mut oversized = exact.clone();
            oversized[part * quarter] = 12;
            output.fill(0xa5);
            assert_eq!(
                scratch.encode(
                    table.codebook(),
                    &oversized,
                    &mut output,
                    EntropyStreams::Four
                ),
                Ok(None)
            );
            assert!(output.iter().all(|&byte| byte == 0xa5));
        }
    }

    #[cfg(feature = "std")]
    mod oracle {
        use super::*;
        use crate::cursor::ByteCursor;
        use std::io::Write;
        use std::path::PathBuf;
        use std::process::{Command, Stdio};
        use std::sync::atomic::{AtomicUsize, Ordering};

        const REVISION: &str = "f8745da6ff1ad1e7bab384bd1f9d742439278e99";
        const DESTINATION_LIMIT: usize = 4 * 1024 * 1024;
        static NEXT_ORACLE: AtomicUsize = AtomicUsize::new(0);

        struct Oracle(PathBuf);

        impl Drop for Oracle {
            fn drop(&mut self) {
                // This test exclusively created the directory, outside the repo.
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        /// The only owned response data are bounded successful byte strings;
        /// metadata fields are copied scalars. No foreign memory is borrowed.
        struct Record {
            capacity: usize,
            outcome: u32,
            modified: usize,
            output: Vec<u8>,
        }

        impl Oracle {
            fn compile() -> Self {
                let upstream = PathBuf::from(
                    std::env::var_os("ZSTD_UPSTREAM").expect("set ZSTD_UPSTREAM to pinned v1.5.7"),
                );
                let revision = Command::new("git")
                    .arg("-C")
                    .arg(&upstream)
                    .args(["rev-parse", "HEAD"])
                    .output()
                    .unwrap();
                assert!(revision.status.success());
                assert_eq!(
                    std::str::from_utf8(&revision.stdout).unwrap().trim(),
                    REVISION
                );
                assert!(
                    Command::new("git")
                        .arg("-C")
                        .arg(&upstream)
                        .args(["diff", "--quiet", "HEAD", "--", "lib"])
                        .status()
                        .unwrap()
                        .success()
                );
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let directory = std::env::temp_dir().join(alloc::format!(
                    "zstdoxide-huffman-capacity-{}-{stamp}-{}",
                    std::process::id(),
                    NEXT_ORACLE.fetch_add(1, Ordering::Relaxed)
                ));
                std::fs::create_dir(&directory).unwrap();
                let oracle = Self(directory);
                let compile = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
                    .args(["-std=c99", "-O2", "-Wall", "-Wextra", "-Werror"])
                    .arg("-I")
                    .arg(upstream.join("lib/common"))
                    .arg("-I")
                    .arg(upstream.join("lib"))
                    .arg(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/scripts/oracles/huffman_capacity.c"
                    ))
                    .arg(upstream.join("lib/libzstd.a"))
                    .arg("-o")
                    .arg(oracle.0.join("oracle"))
                    .output()
                    .unwrap();
                assert!(
                    compile.status.success(),
                    "{}",
                    std::string::String::from_utf8_lossy(&compile.stderr)
                );
                oracle
            }

            fn run(
                &self,
                table: &BuiltStandaloneTable,
                source: &[u8],
                operation: usize,
                flags: usize,
                capacities: &[usize],
            ) -> (bool, Vec<Record>) {
                assert!(table.description().len() <= 512 && source.len() <= SOURCE_LIMIT);
                assert!((1..=128).contains(&capacities.len()));
                assert!(
                    capacities
                        .iter()
                        .all(|&capacity| capacity <= DESTINATION_LIMIT)
                );
                let mut request = Vec::from(&b"ZHCAP001"[..]);
                for value in [
                    table.description().len(),
                    source.len(),
                    operation,
                    flags,
                    capacities.len(),
                ] {
                    request.extend_from_slice(&u32::try_from(value).unwrap().to_le_bytes());
                }
                request.extend_from_slice(table.description());
                request.extend_from_slice(source);
                for &capacity in capacities {
                    request.extend_from_slice(&(capacity as u32).to_le_bytes());
                }
                let mut child = Command::new(self.0.join("oracle"))
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                let written = child.stdin.take().unwrap().write_all(&request);
                let response = child.wait_with_output().unwrap();
                assert!(
                    response.status.success(),
                    "source={}, operation={operation}, flags={flags}: {}",
                    source.len(),
                    std::string::String::from_utf8_lossy(&response.stderr)
                );
                written.unwrap();
                assert!(response.stdout.len() <= 20 + capacities.len() * (20 + 393_240));
                let mut cursor = ByteCursor::new(&response.stdout);
                assert_eq!(cursor.take(8).unwrap(), b"ZHCO0001");
                assert_eq!(
                    cursor.little_endian(4).unwrap() as usize,
                    core::mem::size_of::<usize>()
                );
                let bmi2 = cursor.little_endian(4).unwrap();
                assert!(bmi2 <= 1);
                assert_eq!(cursor.little_endian(4).unwrap() as usize, capacities.len());
                let mut records = Vec::new();
                for &expected_capacity in capacities {
                    let capacity = cursor.little_endian(4).unwrap() as usize;
                    let outcome = cursor.little_endian(4).unwrap() as u32;
                    let error = cursor.little_endian(4).unwrap();
                    let size = cursor.little_endian(4).unwrap() as usize;
                    let modified = cursor.little_endian(4).unwrap() as usize;
                    assert_eq!(capacity, expected_capacity);
                    assert!(outcome <= 2 && modified <= capacity && size <= capacity);
                    assert_eq!(outcome == 2, error != 0);
                    assert_eq!(outcome == 1, size != 0);
                    records.push(Record {
                        capacity,
                        outcome,
                        modified,
                        output: cursor.take(size).unwrap().to_vec(),
                    });
                }
                assert!(cursor.remaining().is_empty());
                (bmi2 != 0, records)
            }
        }

        fn compare_streams(
            oracle: &Oracle,
            table: &BuiltStandaloneTable,
            source: &[u8],
            streams: EntropyStreams,
            flags: usize,
            totals: &mut [usize; 3],
        ) -> bool {
            let operation = if streams == EntropyStreams::One { 1 } else { 4 };
            let predicted = encoded_size(
                table.codebook(),
                source,
                DESTINATION_LIMIT,
                streams,
                core::mem::size_of::<usize>(),
            )
            .unwrap();
            let mut capacities: Vec<_> = (0..=20).collect();
            if let Some(size) = predicted {
                capacities.extend(size.saturating_sub(1)..=size + 20);
            } else {
                capacities.push(DESTINATION_LIMIT);
            }
            capacities.sort_unstable();
            capacities.dedup();
            let (bmi2, records) = oracle.run(table, source, operation, flags, &capacities);
            let mut scratch = EntropyOutput::new();
            for record in records {
                let mut destination = vec![0xa5; record.capacity + 2];
                let actual = scratch
                    .encode(
                        table.codebook(),
                        source,
                        &mut destination[1..=record.capacity],
                        streams,
                    )
                    .unwrap();
                assert_ne!(
                    record.outcome, 2,
                    "valid low-level stream compression unexpectedly errored"
                );
                assert_eq!(
                    actual.is_some(),
                    record.outcome == 1,
                    "source={}, operation={operation}, flags={flags}, capacity={}",
                    source.len(),
                    record.capacity
                );
                if let Some(size) = actual {
                    assert_eq!(size, record.output.len());
                    assert_eq!(&destination[1..1 + size], record.output);
                    assert_eq!(destination[0], 0xa5);
                    assert!(destination[1 + size..].iter().all(|&byte| byte == 0xa5));
                    totals[1] += 1;
                } else {
                    assert!(destination.iter().all(|&byte| byte == 0xa5));
                    totals[0] += 1;
                    totals[2] += usize::from(record.modified != 0);
                }
            }
            bmi2
        }

        #[test]
        #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7, built lib/libzstd.a, and a C compiler"]
        fn official_capacity_oracle_matches_native_word_slack_flags_and_every_quarter_limit() {
            let oracle = Oracle::compile();
            let mut totals = [0; 3];
            let simple = one_bit_table();
            let lengths: Vec<_> = (0..=20)
                .chain([
                    31,
                    32,
                    63,
                    64,
                    127,
                    128,
                    255,
                    256,
                    1023,
                    1024,
                    65536,
                    SOURCE_LIMIT,
                ])
                .collect();
            for length in lengths {
                let source: Vec<_> = (0..length).map(|index| (index & 1) as u8).collect();
                for streams in [EntropyStreams::One, EntropyStreams::Four] {
                    compare_streams(&oracle, &simple, &source, streams, 0, &mut totals);
                }
            }
            let deep = twelve_bit_table();
            let source: Vec<_> = (0..128).map(|index| (index % 13) as u8).collect();
            let bmi2 =
                compare_streams(&oracle, &deep, &source, EntropyStreams::One, 0, &mut totals);
            let mut flag_patterns = 0;
            for flags in 0..64 {
                if flags & 1 != 0 && !bmi2 {
                    continue;
                }
                flag_patterns += 1;
                for streams in [EntropyStreams::One, EntropyStreams::Four] {
                    compare_streams(&oracle, &deep, &source, streams, flags, &mut totals);
                }
            }
            let quarter = 43690;
            let mut exact = vec![12; quarter * 4];
            for start in [0, quarter, quarter * 2, quarter * 3] {
                exact[start] = 10;
            }
            compare_streams(&oracle, &deep, &exact, EntropyStreams::Four, 0, &mut totals);
            for part in 0..4 {
                let mut oversized = exact.clone();
                oversized[part * quarter] = 12;
                compare_streams(
                    &oracle,
                    &deep,
                    &oversized,
                    EntropyStreams::Four,
                    0,
                    &mut totals,
                );
            }
            for symbol in [0, 12] {
                let source = vec![symbol; SOURCE_LIMIT];
                for streams in [EntropyStreams::One, EntropyStreams::Four] {
                    compare_streams(&oracle, &deep, &source, streams, 0, &mut totals);
                }
            }
            assert!(totals.iter().all(|&count| count > 0));
            std::eprintln!(
                "C capacity checks: {} exact successful streams, {} matching zero results ({} C calls modified bytes before zero), {flag_patterns} supported flag patterns, native word {} bytes",
                totals[1],
                totals[0],
                totals[2],
                core::mem::size_of::<usize>()
            );
        }

        #[test]
        #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7, built lib/libzstd.a, and a C compiler"]
        fn official_table_writer_capacity_outcomes_remain_distinct_from_paired_copy() {
            use crate::huffman_encode::{StandaloneDepthPolicy, build_standalone_table};
            let oracle = Oracle::compile();
            let mut counts = [1; 256];
            counts[0] = 1_000_000;
            let compressed = build_standalone_table(&counts, 12, StandaloneDepthPolicy::Requested)
                .unwrap()
                .unwrap();
            assert!(compressed.description()[0] < 128);
            let mut outcomes = [0_usize; 3];
            for table in [one_bit_table(), twelve_bit_table(), compressed] {
                let capacities: Vec<_> =
                    (0..=table.description().len() + 24).chain([512]).collect();
                let (_, records) = oracle.run(&table, &[], 0, 0, &capacities);
                for record in records {
                    let mut destination = vec![0xa5; record.capacity + 2];
                    let copied = write_description(&table, &mut destination[1..=record.capacity]);
                    assert_eq!(copied.is_ok(), record.capacity >= table.description().len());
                    if let Ok(size) = copied {
                        assert_eq!(&destination[1..1 + size], table.description());
                        assert!(destination[1 + size..].iter().all(|&byte| byte == 0xa5));
                        assert_eq!(destination[0], 0xa5);
                    } else {
                        assert!(destination.iter().all(|&byte| byte == 0xa5));
                    }
                    if record.outcome == 1 {
                        outcomes[0] += 1;
                        let restored =
                            BuiltStandaloneTable::from_description(record.output).unwrap();
                        for symbol in 0..=255 {
                            assert_eq!(
                                restored.codebook().code(symbol),
                                table.codebook().code(symbol)
                            );
                        }
                    } else {
                        assert_eq!(
                            record.outcome, 2,
                            "table writer has no zero-result contract"
                        );
                        outcomes[1] += 1;
                        outcomes[2] += usize::from(copied.is_ok());
                    }
                }
            }
            assert!(outcomes[0] > 0 && outcomes[1] > 0);
            std::eprintln!(
                "C table writer: {} successful descriptions with identical codes, {} capacity errors; {} errors occurred where the preselected Rust description copy fit (explicitly different contract)",
                outcomes[0],
                outcomes[1],
                outcomes[2]
            );
        }
    }
}
