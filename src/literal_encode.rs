//! Complete literal-section selection with unpublished table ownership.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::bitwriter::BitWriter;
use crate::fast_plan::FastExecutionPlan;
use crate::frame::MAX_BLOCK_SIZE;
use crate::huffman_codebook::HuffmanCodebook;
use crate::huffman_encode::build_sparse_literal_table_in;
use crate::literal_header::LiteralHeader;
use crate::{Error, Result};

/// Reusable owned scratch. Table lifetime belongs to the enclosing block
/// encoder, which publishes a returned codebook only when it emits this section.
pub(crate) struct LiteralEncoder<A: Allocator = Global> {
    writer: BitWriter<A>,
    streams: Vec<u8, A>,
    section: Vec<u8, A>,
}

struct FormatChoice {
    header: LiteralHeader,
    four_streams: bool,
    payload_size: usize,
    section_size: usize,
}

/// Value-only policy; both entry points retain the same scratch owners.
#[derive(Clone, Copy)]
enum SelectionPolicy {
    Named,
    Fast { raw_only: bool },
}

impl SelectionPolicy {
    fn raw_only(self) -> bool {
        matches!(self, Self::Fast { raw_only: true })
    }

    fn accepts_payload(self, input_size: usize, payload_size: usize) -> bool {
        match self {
            Self::Named => true,
            Self::Fast { .. } => input_size
                .checked_sub(FastExecutionPlan::minimum_gain(input_size))
                .is_some_and(|limit| payload_size < limit),
        }
    }
}

impl LiteralEncoder<Global> {
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "const Global convenience for direct callers")
    )]
    pub(crate) const fn new() -> Self {
        Self {
            writer: BitWriter::new(),
            streams: Vec::new(),
            section: Vec::new(),
        }
    }
}

impl<A: Allocator + Clone> LiteralEncoder<A> {
    /// Each child retains the allocator that owns its storage. Clones need not
    /// be equivalent; the original allocator belongs to the final section.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "private allocator-aware context entry point")
    )]
    pub(crate) fn new_in(allocator: A) -> Self {
        Self {
            writer: BitWriter::new_in(allocator.clone()),
            streams: Vec::new_in(allocator.clone()),
            section: Vec::new_in(allocator),
        }
    }

    /// Select by complete serialized size, including all headers and markers.
    /// Ties preserve raw/RLE first, then an existing tree, then a new tree;
    /// one stream wins a tie against four. The previous codebook is immutable.
    /// A returned table is pending: discarding this block must also discard it.
    /// Errors never publish a codebook or modify the caller's previous table.
    /// Scratch may contain partial output after an allocation failure; a later
    /// call starts a fresh selection and retains existing buffer capacities.
    pub(crate) fn select(
        &mut self,
        input: &[u8],
        previous: Option<&HuffmanCodebook>,
    ) -> Result<Option<HuffmanCodebook>> {
        self.select_with_policy(input, previous, SelectionPolicy::Named)
    }

    /// Apply Fast's raw-only and strict payload-gain rules to the independent
    /// Rust entropy selector. Raw-only precedes even uniform-literal detection.
    /// `previous` carries no C Huffman-validity state, so this does not model
    /// the reference's separate minimum-attempt or valid-table scheduling.
    /// A returned codebook remains unpublished until the enclosing block commits.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "Fast block emission integration follows")
    )]
    pub(crate) fn select_fast(
        &mut self,
        input: &[u8],
        previous: Option<&HuffmanCodebook>,
        raw_only: bool,
    ) -> Result<Option<HuffmanCodebook>> {
        self.select_with_policy(input, previous, SelectionPolicy::Fast { raw_only })
    }

    fn select_with_policy(
        &mut self,
        input: &[u8],
        previous: Option<&HuffmanCodebook>,
        policy: SelectionPolicy,
    ) -> Result<Option<HuffmanCodebook>> {
        if input.len() > MAX_BLOCK_SIZE {
            return Err(Error::CorruptData);
        }
        let repeated =
            !policy.raw_only() && input.len() > 1 && input.iter().all(|&symbol| symbol == input[0]);
        let header = LiteralHeader::raw(input.len(), repeated)?;
        self.section.clear();
        self.section
            .try_reserve_exact(input.len() + 5)
            .map_err(|_| Error::AllocationFailed)?;
        // The complete raw bound above includes the largest five-byte header.
        // Individual pushes cannot request another allocation within that bound.
        for &byte in header.as_bytes() {
            self.section.push(byte);
        }
        for &byte in if repeated { &input[..1] } else { input } {
            self.section.push(byte);
        }
        if policy.raw_only() || input.len() < 2 || repeated {
            return Ok(None);
        }
        if let Some(previous) = previous
            && previous.covers(input)
            && let Some(choice) = choose_format(previous, input, 0, true, policy)?
            && choice.section_size < self.section.len()
        {
            self.write_candidate(previous, input, &[], choice)?;
        }
        let mut frequencies = [0_u64; 256];
        for &symbol in input {
            frequencies[usize::from(symbol)] += 1;
        }
        // A fresh owner avoids borrowing an allocator inside self while the
        // selected description is copied into self.section. The complete table
        // retains this clone until its codebook is moved out or it is discarded.
        let table = build_sparse_literal_table_in(&frequencies, self.section.allocator().clone())?;
        if let Some(choice) = choose_format(
            table.codebook(),
            input,
            table.description().len(),
            false,
            policy,
        )? && choice.section_size < self.section.len()
        {
            self.write_candidate(table.codebook(), input, table.description(), choice)?;
            return Ok(Some(table.into_codebook()));
        }
        Ok(None)
    }
}

impl<A: Allocator> LiteralEncoder<A> {
    fn write_candidate(
        &mut self,
        codebook: &HuffmanCodebook,
        input: &[u8],
        description: &[u8],
        choice: FormatChoice,
    ) -> Result<()> {
        if choice.four_streams {
            codebook.encode_four_streams(input, &mut self.writer, &mut self.streams)?;
        } else {
            codebook.encode_stream(input, &mut self.writer)?;
        }
        let encoded = if choice.four_streams {
            &self.streams[..]
        } else {
            self.writer.as_bytes()
        };
        if encoded.len() + description.len() != choice.payload_size {
            return Err(Error::InvalidState);
        }
        // Every selected candidate is smaller than the raw/RLE section whose
        // initialized capacity was reserved before selection. Reserve explicitly
        // as well so future independent selection changes keep this fallible.
        self.section
            .try_reserve(choice.section_size.saturating_sub(self.section.len()))
            .map_err(|_| Error::AllocationFailed)?;
        self.section.clear();
        for &byte in choice
            .header
            .as_bytes()
            .iter()
            .chain(description)
            .chain(encoded)
        {
            self.section.push(byte);
        }
        Ok(())
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.section
    }

    pub(crate) fn retained_capacity(&self) -> usize {
        self.section.capacity() + self.streams.capacity() + self.writer.retained_capacity()
    }
}

/// Predict exact stream lengths from code widths before encoding a candidate.
/// Block bounds limit arithmetic to at most twelve bits per input byte.
fn choose_format(
    codebook: &HuffmanCodebook,
    input: &[u8],
    description_size: usize,
    reused: bool,
    policy: SelectionPolicy,
) -> Result<Option<FormatChoice>> {
    let quarter = input.len().div_ceil(4).max(1);
    let mut bits = [0_usize; 4];
    for (index, &symbol) in input.iter().enumerate() {
        bits[(index / quarter).min(3)] += usize::from(codebook.code(symbol)?.bits);
    }
    let single_size = (bits.iter().sum::<usize>() + 1).div_ceil(8) + description_size;
    let mut selected = LiteralHeader::huffman(input.len(), single_size, false, reused)
        .ok()
        .filter(|_| policy.accepts_payload(input.len(), single_size))
        .map(|header| {
            let section_size = header.as_bytes().len() + single_size;
            FormatChoice {
                header,
                four_streams: false,
                payload_size: single_size,
                section_size,
            }
        });
    if input.len() >= 6 {
        let four_size = bits
            .iter()
            .map(|&size| (size + 1).div_ceil(8))
            .sum::<usize>()
            + 6
            + description_size;
        if let Ok(header) = LiteralHeader::huffman(input.len(), four_size, true, reused)
            && policy.accepts_payload(input.len(), four_size)
        {
            let section_size = header.as_bytes().len() + four_size;
            if selected
                .as_ref()
                .is_none_or(|previous| section_size < previous.section_size)
            {
                selected = Some(FormatChoice {
                    header,
                    four_streams: true,
                    payload_size: four_size,
                    section_size,
                });
            }
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::huffman::{HuffmanTable, decode_literals};
    use alloc::vec;
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn allocator() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 32,
            fail_at: None,
        })
        .unwrap()
    }

    fn sixteen_symbol_codebook() -> HuffmanCodebook {
        let mut lengths = [0; 256];
        lengths[..16].fill(4);
        HuffmanCodebook::from_lengths(&lengths, 11).unwrap()
    }

    #[test]
    fn fast_raw_only_uses_only_the_section_owner_and_ignores_previous_coverage() {
        let owner = allocator();
        let identity = owner.identity();
        let monitor = owner.monitor();
        let mut encoder = LiteralEncoder::new_in(owner);
        let previous = sixteen_symbol_codebook();
        let original: [_; 256] = core::array::from_fn(|i| previous.code(i as u8));
        let before = monitor.snapshot();
        let source = vec![200; 4096];
        assert!(
            encoder
                .select_fast(&source, Some(&previous), true)
                .unwrap()
                .is_none()
        );
        assert_eq!(encoder.section.allocator().identity(), identity);
        assert_eq!(monitor.snapshot().attempts - before.attempts, 1);
        assert_eq!(
            monitor.snapshot().identities_created,
            before.identities_created
        );
        assert_eq!(monitor.snapshot().live_blocks, 1);
        assert_eq!(encoder.writer.retained_capacity(), 0);
        assert_eq!(encoder.streams.capacity(), 0);
        assert_eq!(&encoder.as_bytes()[..3], &[0x0c, 0, 1]);
        assert_eq!(&encoder.as_bytes()[3..], &source);
        assert_eq!(
            decode_literals(encoder.as_bytes(), &mut None, MAX_BLOCK_SIZE)
                .unwrap()
                .0,
            source
        );
        let before = monitor.snapshot();
        monitor.set_fail_at(NonZeroU64::new(before.attempts + 1));
        let mut moved = encoder;
        // This uniform input is covered by the previous tree. Raw-only still
        // does not construct a histogram/table or allocate stream scratch.
        assert!(
            moved
                .select_fast(&vec![7; 4096], Some(&previous), true)
                .unwrap()
                .is_none()
        );
        assert_eq!(moved.as_bytes()[0] & 3, 0);
        assert_eq!(monitor.snapshot().attempts, before.attempts);
        assert_eq!(
            monitor.snapshot().identities_created,
            before.identities_created
        );
        assert_eq!(
            core::array::from_fn::<_, 256, _>(|i| previous.code(i as u8)),
            original
        );
        monitor.clear_failure();
        drop(moved);
        monitor.assert_released();
    }

    #[test]
    fn fast_raw_only_failure_recovers_with_borrowed_non_clone_allocator() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        let mut encoder = LiteralEncoder::new_in(&owner);
        monitor.set_fail_at(NonZeroU64::new(1));
        assert_eq!(
            encoder.select_fast(&[9; 32], None, true),
            Err(Error::AllocationFailed)
        );
        assert_eq!(monitor.snapshot().live_blocks, 0);
        monitor.clear_failure();
        assert!(encoder.select_fast(&[9; 32], None, true).unwrap().is_none());
        assert_eq!(&encoder.as_bytes()[..2], &[0x04, 0x02]);
        assert_eq!(&encoder.as_bytes()[2..], &[9; 32]);
        let before = encoder.as_bytes().to_vec();
        let snapshot = monitor.snapshot();
        assert_eq!(
            encoder.select_fast(&vec![0; MAX_BLOCK_SIZE + 1], None, true),
            Err(Error::CorruptData)
        );
        assert_eq!(encoder.as_bytes(), before);
        assert_eq!(monitor.snapshot(), snapshot);
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(encoder);
        monitor.assert_released();
    }

    fn one_short_code_table() -> (HuffmanCodebook, HuffmanTable) {
        // One one-bit symbol and128 eight-bit symbols exactly fill the tree.
        // The final weight is inferred by the independently checked reader.
        let mut weights = [1; 128];
        weights[0] = 8;
        let payload = crate::entropy_encode::encode_weights(&weights).unwrap();
        let mut description = vec![u8::try_from(payload.len()).unwrap()];
        description.extend_from_slice(&payload);
        let (table, used) = HuffmanTable::read(&description).unwrap();
        assert_eq!(used, description.len());
        let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
        assert_eq!(codebook.code(0).unwrap().bits, 1);
        assert_eq!(codebook.code(128).unwrap().bits, 8);
        (codebook, table)
    }

    #[test]
    fn fast_gain_equality_falls_back_but_the_literal_header_is_excluded() {
        let (previous, _) = one_short_code_table();
        let mut encoder = LiteralEncoder::new();
        for (short_symbols, payload, compressed) in [(4, 61, false), (5, 60, true)] {
            let mut input = vec![0; short_symbols];
            input.extend(1..=u8::try_from(64 - short_symbols).unwrap());
            let choice = choose_format(&previous, &input, 0, true, SelectionPolicy::Named)
                .unwrap()
                .unwrap();
            assert_eq!(choice.payload_size, payload);
            assert_eq!(choice.section_size, payload + 3);
            assert_eq!(64 - FastExecutionPlan::minimum_gain(64), 61);
            // Both complete sections improve on raw's66 bytes. Only the second
            // strictly beats the Fast payload limit; its complete63-byte section
            // is deliberately larger than61, proving the header is excluded.
            assert!(encoder.select(&input, Some(&previous)).unwrap().is_none());
            assert_eq!(encoder.as_bytes()[0] & 3, 3);
            assert!(
                encoder
                    .select_fast(&input, Some(&previous), false)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(encoder.as_bytes()[0] & 3, if compressed { 3 } else { 0 });
            let (_, decoding) = one_short_code_table();
            let (decoded, consumed) =
                decode_literals(encoder.as_bytes(), &mut Some(decoding), MAX_BLOCK_SIZE).unwrap();
            assert_eq!(decoded, input);
            assert_eq!(consumed, encoder.as_bytes().len());
        }
    }

    #[test]
    fn fast_format_gain_counts_description_jumps_markers_and_checked_small_sizes() {
        let codebook = sixteen_symbol_codebook();
        let fast = SelectionPolicy::Fast { raw_only: false };
        for size in [0, 1, 2, 3] {
            assert!(
                choose_format(&codebook, &vec![0; size], 0, true, fast)
                    .unwrap()
                    .is_none()
            );
        }
        for size in [333, 4096] {
            let input: Vec<_> = (0..size).map(|i| (i % 16) as u8).collect();
            let plain = choose_format(&codebook, &input, 0, true, fast)
                .unwrap()
                .unwrap();
            let limit = size - FastExecutionPlan::minimum_gain(size);
            let description = limit - plain.payload_size;
            // Adding an exact synthetic description cost reaches equality and
            // must reject; the cost is included for each format before selection.
            assert!(
                choose_format(&codebook, &input, description, false, fast)
                    .unwrap()
                    .is_none()
            );
            let accepted = choose_format(&codebook, &input, description - 1, false, fast)
                .unwrap()
                .unwrap();
            assert_eq!(accepted.payload_size, limit - 1);
            assert_eq!(accepted.four_streams, size >= 1024);
            let stream_bytes = if size >= 1024 {
                // Four1024-byte quarters: each has4096 bits plus a marker;
                // the three little-endian jump sizes add another six bytes.
                4 * 513 + 6
            } else {
                (size * 4 + 1).div_ceil(8)
            };
            assert_eq!(plain.payload_size, stream_bytes);
        }
    }

    #[test]
    fn fast_pending_trees_are_returned_only_for_selected_new_sections() {
        let mut encoder = LiteralEncoder::new();
        let input: Vec<_> = (0..4096).map(|i| (i % 16) as u8).collect();
        let previous = encoder.select_fast(&input, None, false).unwrap().unwrap();
        let original: [_; 256] = core::array::from_fn(|i| previous.code(i as u8));
        assert_eq!(encoder.as_bytes()[0] & 3, 2);
        let mut decoding = None;
        assert_eq!(
            decode_literals(encoder.as_bytes(), &mut decoding, MAX_BLOCK_SIZE)
                .unwrap()
                .0,
            input
        );
        assert!(
            encoder
                .select_fast(&input, Some(&previous), false)
                .unwrap()
                .is_none()
        );
        assert_eq!(encoder.as_bytes()[0] & 3, 3);
        assert_eq!(
            decode_literals(encoder.as_bytes(), &mut decoding, MAX_BLOCK_SIZE)
                .unwrap()
                .0,
            input
        );
        for (source, raw_only, kind) in [(vec![200; 32], false, 1), (vec![7; 32], true, 0)] {
            assert!(
                encoder
                    .select_fast(&source, Some(&previous), raw_only)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(encoder.as_bytes()[0] & 3, kind);
            assert_eq!(
                decode_literals(encoder.as_bytes(), &mut decoding, MAX_BLOCK_SIZE)
                    .unwrap()
                    .0,
                source
            );
        }
        assert_eq!(
            core::array::from_fn::<_, 256, _>(|i| previous.code(i as u8)),
            original
        );
        assert!(
            encoder
                .select_fast(&input, Some(&previous), false)
                .unwrap()
                .is_none()
        );
        assert_eq!(encoder.as_bytes()[0] & 3, 3);
    }

    #[test]
    fn fast_allocation_failures_discard_candidates_and_recover_exact_owners() {
        let input: Vec<_> = (0..128).map(|i| (i % 16) as u8).collect();
        for warm in [false, true] {
            check_selection_failures(&input, warm, SelectionPolicy::Fast { raw_only: false });
        }
    }

    #[test]
    fn custom_context_matches_global_sections_and_pending_tables() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        let mut custom = LiteralEncoder::new_in(&owner);
        let mut global = LiteralEncoder::new();
        let mut previous = None;
        let mut decoder = None;
        let mut kinds = [false; 4];
        let mut stream_formats = [false; 2];
        for input in [
            vec![],
            vec![7],
            (0..=255).collect(),
            vec![42; 4096],
            (0..333).map(|index| (index % 16) as u8).collect(),
            (0..4096).map(|index| (index % 16) as u8).collect(),
            (0..4096).map(|index| (index % 16) as u8).collect(),
        ] {
            let expected = global.select(&input, previous.as_ref()).unwrap();
            let pending = custom.select(&input, previous.as_ref()).unwrap();
            assert_eq!(pending, expected);
            assert_eq!(custom.as_bytes(), global.as_bytes());
            let kind = usize::from(custom.as_bytes()[0] & 3);
            kinds[kind] = true;
            if kind >= 2 {
                stream_formats[usize::from(custom.as_bytes()[0] & 12 != 0)] = true;
            }
            assert_eq!(
                decode_literals(custom.as_bytes(), &mut decoder, MAX_BLOCK_SIZE)
                    .unwrap()
                    .0,
                input
            );
            if pending.is_some() {
                previous = pending;
            }
        }
        assert_eq!(kinds, [true; 4]);
        assert_eq!(stream_formats, [true; 2]);
        drop(custom);
        assert_eq!(monitor.snapshot().identities_created, 1);
        monitor.assert_released();
    }

    #[test]
    fn distinct_allocator_domains_move_with_the_complete_context() {
        let owner = allocator();
        let original = owner.identity();
        let monitor = owner.monitor();
        let mut context = LiteralEncoder::new_in(owner);
        assert_eq!(context.section.allocator().identity(), original);
        assert_ne!(context.streams.allocator().identity(), original);
        let input: Vec<_> = (0..4096).map(|index| (index % 16) as u8).collect();
        let pending = context.select(&input, None).unwrap().unwrap();
        assert!(pending.covers(&input));
        // Candidate description and decoder validation scratch have been freed;
        // the remaining allocations belong to the three reusable child buffers.
        assert_eq!(monitor.snapshot().live_blocks, 3);
        let mut moved = context;
        assert!(moved.select(&input, Some(&pending)).unwrap().is_none());
        assert_eq!(moved.as_bytes()[0] & 3, 3);
        drop(moved);
        assert!(monitor.snapshot().identities_created > 3);
        monitor.assert_released();
    }

    fn check_selection_failures(input: &[u8], warm: bool, policy: SelectionPolicy) {
        let previous = sixteen_symbol_codebook();
        let original = sixteen_symbol_codebook();
        let owner = allocator();
        let monitor = owner.monitor();
        let mut baseline = LiteralEncoder::new_in(owner);
        if warm {
            baseline
                .select_with_policy(input, Some(&previous), policy)
                .unwrap();
        }
        let before = monitor.snapshot().attempts;
        let expected_pending = baseline
            .select_with_policy(input, Some(&previous), policy)
            .unwrap();
        let expected_bytes = baseline.as_bytes().to_vec();
        let attempts = monitor.snapshot().attempts - before;
        assert!(attempts > 0);
        drop(baseline);
        monitor.assert_released();
        for ordinal in 1..=attempts {
            let owner = allocator();
            let monitor = owner.monitor();
            let mut encoder = LiteralEncoder::new_in(owner);
            if warm {
                encoder
                    .select_with_policy(input, Some(&previous), policy)
                    .unwrap();
            }
            let before = monitor.snapshot().attempts;
            monitor.set_fail_at(NonZeroU64::new(before + ordinal));
            // Even if reuse was already written to scratch before a new-tree
            // allocation fails, Err must never return a pending publication.
            assert_eq!(
                encoder.select_with_policy(input, Some(&previous), policy),
                Err(Error::AllocationFailed),
                "allocation {ordinal}, warm {warm}"
            );
            assert_eq!(previous, original);
            assert_eq!(monitor.snapshot().failures, 1);
            monitor.clear_failure();
            assert_eq!(
                encoder
                    .select_with_policy(input, Some(&previous), policy)
                    .unwrap(),
                expected_pending
            );
            assert_eq!(encoder.as_bytes(), expected_bytes);
            assert_eq!(previous, original);
            drop(encoder);
            monitor.assert_released();
        }
    }

    #[test]
    fn allocation_failures_never_publish_tables_and_retry_exactly() {
        for length in [128, 4096] {
            let input: Vec<_> = (0..length).map(|index| (index % 2) as u8).collect();
            check_selection_failures(&input, false, SelectionPolicy::Named);
        }
    }

    #[test]
    fn warmed_reuse_failures_preserve_the_published_table_and_recover() {
        let input: Vec<_> = (0..128).map(|index| (index % 16) as u8).collect();
        check_selection_failures(&input, true, SelectionPolicy::Named);
    }

    #[test]
    fn invalid_size_preserves_scratch_and_warmed_capacities_stabilize() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        let mut context = LiteralEncoder::new_in(&owner);
        let sources = [
            (0..4096).map(|index| index as u8).collect::<Vec<_>>(),
            (0..333).map(|index| (index % 16) as u8).collect(),
            (0..4096).map(|index| (index % 16) as u8).collect(),
        ];
        for source in &sources {
            context.select(source, None).unwrap();
        }
        let capacity = context.retained_capacity();
        let retained_bytes = monitor.snapshot().live_bytes;
        for source in &sources {
            context.select(source, None).unwrap();
            assert_eq!(context.retained_capacity(), capacity);
            assert_eq!(monitor.snapshot().live_bytes, retained_bytes);
        }
        let before = context.as_bytes().to_vec();
        let attempts = monitor.snapshot().attempts;
        assert_eq!(
            context.select(&vec![0; MAX_BLOCK_SIZE + 1], None),
            Err(Error::CorruptData)
        );
        assert_eq!(context.as_bytes(), before);
        assert_eq!(monitor.snapshot().attempts, attempts);
        drop(context);
        monitor.assert_released();
    }

    #[test]
    fn raw_rle_new_and_repeated_tables_round_trip_with_exact_section_costs() {
        let mut encoder = LiteralEncoder::new();
        let mut previous_decode = None;
        for input in [vec![], vec![7], (0..=255).collect(), vec![42; 4096]] {
            assert!(encoder.select(&input, None).unwrap().is_none());
            let (decoded, consumed) =
                decode_literals(encoder.as_bytes(), &mut previous_decode, MAX_BLOCK_SIZE).unwrap();
            assert_eq!(decoded, input);
            assert_eq!(consumed, encoder.as_bytes().len());
        }
        let input: Vec<_> = (0..4096).map(|index| (index % 16) as u8).collect();
        let current = encoder.select(&input, None).unwrap().unwrap();
        assert_eq!(encoder.as_bytes()[0] & 3, 2);
        assert!(encoder.as_bytes().len() < input.len() * 3 / 4);
        assert_eq!(
            decode_literals(encoder.as_bytes(), &mut previous_decode, MAX_BLOCK_SIZE)
                .unwrap()
                .0,
            input
        );
        let new_size = encoder.as_bytes().len();
        assert!(encoder.select(&input, Some(&current)).unwrap().is_none());
        assert_eq!(encoder.as_bytes()[0] & 3, 3);
        assert!(encoder.as_bytes().len() < new_size);
        assert_eq!(
            decode_literals(encoder.as_bytes(), &mut previous_decode, MAX_BLOCK_SIZE)
                .unwrap()
                .0,
            input
        );
        assert!(encoder.retained_capacity() >= encoder.as_bytes().len());
    }

    #[test]
    fn raw_fallback_leaves_the_callers_reusable_tree_unchanged() {
        let (table, _) = HuffmanTable::read(&[128, 0x10]).unwrap();
        let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
        let mut encoder = LiteralEncoder::new();
        assert!(
            encoder
                .select(&[0, 1, 0], Some(&codebook))
                .unwrap()
                .is_none()
        );
        assert_eq!(encoder.as_bytes()[0] & 3, 0);
        assert!(codebook.covers(&[0, 1]));
        assert!(!codebook.covers(&[2]));
        assert!(
            encoder
                .select(&[3; 100], Some(&codebook))
                .unwrap()
                .is_none()
        );
        assert_eq!(encoder.as_bytes()[0] & 3, 1);
        assert!(
            encoder
                .select(&vec![0; MAX_BLOCK_SIZE + 1], Some(&codebook))
                .is_err()
        );
        assert_eq!(codebook.table_log(), 1);
    }

    #[test]
    fn estimated_stream_sizes_equal_encoded_sizes_across_width_boundaries() {
        let dictionary =
            include_bytes!("../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols");
        let (table, _) = HuffmanTable::read_dictionary(&dictionary[8..]).unwrap();
        let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
        let mut writer = BitWriter::new();
        let mut streams = Vec::new();
        for size in [
            0,
            1,
            5,
            6,
            7,
            9,
            11,
            12,
            255,
            256,
            1023,
            1024,
            16383,
            16384,
            MAX_BLOCK_SIZE,
        ] {
            let source = vec![0; size];
            let choice = choose_format(&codebook, &source, 0, true, SelectionPolicy::Named)
                .unwrap()
                .unwrap();
            let actual = if choice.four_streams {
                codebook
                    .encode_four_streams(&source, &mut writer, &mut streams)
                    .unwrap();
                streams.len()
            } else {
                codebook.encode_stream(&source, &mut writer).unwrap();
                writer.as_bytes().len()
            };
            assert_eq!(choice.payload_size, actual);
            assert_eq!(choice.section_size, actual + choice.header.as_bytes().len());
        }
    }
}
