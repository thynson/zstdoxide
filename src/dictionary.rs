//! Owned dictionaries with validated metadata and immutable prepared entropy.
//!
//! Layout and loader contracts follow facebook/zstd v1.5.7
//! `ZSTD_decompress_insertDictionary`, `ZSTD_loadDEntropy`, and `ZSTD_DDict`.

use alloc::vec::Vec;

use crate::cursor::ByteCursor;
use crate::fse::FseTable;
use crate::huffman::HuffmanTable;
use crate::{Error, Result};

const MAGIC: u32 = 0xec30_a437;

/// How to interpret serialized dictionary bytes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DictionaryMode {
    /// Treat buffers shorter than eight bytes or without dictionary magic as
    /// raw content. Matching magic in a longer buffer commits to full parsing.
    #[default]
    Auto,
    /// Treat every byte as content, including any apparent dictionary header.
    Raw,
    /// Require a complete formatted dictionary with valid entropy and offsets.
    Formatted,
}

/// Shared immutable ownership, using pointer atomics where the target has them.
#[cfg(target_has_atomic = "ptr")]
pub type SharedDictionary = alloc::sync::Arc<Dictionary>;

/// Shared immutable ownership on targets without pointer atomics.
#[cfg(not(target_has_atomic = "ptr"))]
pub type SharedDictionary = alloc::rc::Rc<Dictionary>;

/// Prepared decoding tables and initial repeat offsets from one dictionary.
///
/// Corresponds to `ZSTD_entropyDTables_t`. Each table owns its allocation; no
/// table borrows the serialized bytes or another field. Working frame tables
/// can override these immutable defaults without cloning them.
#[derive(Debug)]
pub(crate) struct DictionaryEntropy {
    pub(crate) huffman: HuffmanTable,
    pub(crate) offsets: FseTable,
    pub(crate) match_lengths: FseTable,
    pub(crate) literal_lengths: FseTable,
    pub(crate) repeat: [usize; 3],
}

/// An owned raw dictionary or validated formatted dictionary.
///
/// Content positions are indices into the owned initialized byte vector. The
/// content start is validated before construction, and no method mutates the
/// bytes, prepared entropy, or start index. Empty and short raw content is valid.
/// This value is suitable for sharing across independent codec contexts.
#[derive(Debug)]
pub struct Dictionary {
    bytes: Vec<u8>,
    content_start: usize,
    id: u32,
    entropy: Option<DictionaryEntropy>,
}

impl Dictionary {
    /// Copy and prepare a dictionary using automatic format detection.
    /// Corresponds to the content-type behavior of `ZSTD_createDDict`.
    pub fn from_bytes(input: &[u8]) -> Result<Self> {
        Self::from_bytes_with_mode(input, DictionaryMode::Auto)
    }

    /// Copy bytes and validate according to an explicit content interpretation.
    ///
    /// Corresponds to `ZSTD_createDDict_advanced` with by-copy ownership. Auto
    /// never retries a malformed formatted dictionary as raw content. Reserved
    /// IDs, ID zero, and duplicate valid repeat offsets are accepted, matching
    /// the reference loader. Allocation failures keep their original error.
    pub fn from_bytes_with_mode(input: &[u8], mode: DictionaryMode) -> Result<Self> {
        let formatted = match mode {
            DictionaryMode::Auto => input.len() >= 8 && input[..4] == MAGIC.to_le_bytes(),
            DictionaryMode::Raw => false,
            DictionaryMode::Formatted => true,
        };
        let (id, content_start, entropy) = if formatted {
            let (id, start, entropy) = parse_formatted(input).map_err(dictionary_error)?;
            (id, start, Some(entropy))
        } else {
            (0, 0, None)
        };
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(input.len())
            .map_err(|_| Error::AllocationFailed)?;
        bytes.extend_from_slice(input);
        Ok(Self {
            bytes,
            content_start,
            id,
            entropy,
        })
    }

    /// Copy raw content without interpreting a formatted header.
    pub fn raw(input: &[u8]) -> Result<Self> {
        Self::from_bytes_with_mode(input, DictionaryMode::Raw)
    }

    /// Move this prepared value into shared ownership without copying its bytes.
    ///
    /// Dictionary-sized allocations were prepared fallibly by the constructor.
    /// The fixed-size `Arc`/`Rc` ownership handle uses its standard allocation
    /// policy; this call is an explicit future allocator migration boundary.
    pub fn into_shared(self) -> SharedDictionary {
        SharedDictionary::new(self)
    }

    /// Formatted dictionary ID, or zero for raw content.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// The matchable dictionary content, excluding formatted metadata.
    pub fn content(&self) -> &[u8] {
        &self.bytes[self.content_start..]
    }

    /// The exact original serialization supplied to the constructor.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Whether these bytes were parsed as a formatted dictionary.
    pub fn is_formatted(&self) -> bool {
        self.entropy.is_some()
    }

    /// Read a content-relative byte after checking the requested position.
    ///
    /// Invalid history positions are corrupt compressed data, independently of
    /// whether the configured dictionary itself is formatted or raw.
    pub fn content_byte(&self, index: usize) -> Result<u8> {
        let content = self.content();
        if index >= content.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-dictionary"))]
        let byte = content[index];
        #[cfg(feature = "unsafe-dictionary")]
        // SAFETY: the runtime check above proves index < content.len(), so the
        // read is inside this initialized slice. Construction proves that the
        // content range lies inside the owned Vec, and neither bytes nor the
        // start index can change after publication. The shared self borrow
        // keeps that allocation alive and forbids mutation throughout this
        // read, including when ownership is shared through Arc or Rc. Vec's
        // u8 storage is properly aligned; copying this byte neither exposes a
        // pointer/reference nor creates mutable aliases or moves owned data.
        let byte = unsafe { *content.get_unchecked(index) };
        Ok(byte)
    }

    pub(crate) fn entropy(&self) -> Option<&DictionaryEntropy> {
        self.entropy.as_ref()
    }

    /// Owned heap capacity in bytes, excluding inline state/shared handles.
    pub(crate) fn retained_capacity(&self) -> usize {
        self.bytes.capacity()
            + self.entropy.as_ref().map_or(0, |entropy| {
                entropy.huffman.retained_capacity()
                    + entropy.offsets.retained_capacity()
                    + entropy.match_lengths.retained_capacity()
                    + entropy.literal_lengths.retained_capacity()
            })
    }
}

/// Parse the ordered entropy fields and repcodes used by `ZSTD_loadDEntropy`.
fn parse_formatted(input: &[u8]) -> Result<(u32, usize, DictionaryEntropy)> {
    let mut cursor = ByteCursor::new(input);
    if cursor.little_endian(4)? != u64::from(MAGIC) {
        return Err(Error::InvalidDictionary);
    }
    let id = cursor.little_endian(4)? as u32;
    let (huffman, size) = HuffmanTable::read_dictionary(cursor.remaining())?;
    cursor.take(size)?;
    let (offsets, size) = FseTable::read_dictionary(cursor.remaining(), 31, 8)?;
    cursor.take(size)?;
    let (match_lengths, size) = FseTable::read_dictionary(cursor.remaining(), 52, 9)?;
    cursor.take(size)?;
    let (literal_lengths, size) = FseTable::read_dictionary(cursor.remaining(), 35, 9)?;
    cursor.take(size)?;
    let mut repeat = [0; 3];
    for offset in &mut repeat {
        *offset = usize::try_from(cursor.little_endian(4)?).map_err(|_| Error::ResourceLimit)?;
    }
    let content_start = cursor.position();
    let content_size = cursor.remaining().len();
    if repeat
        .iter()
        .any(|&offset| offset == 0 || offset > content_size)
    {
        return Err(Error::InvalidDictionary);
    }
    Ok((
        id,
        content_start,
        DictionaryEntropy {
            huffman,
            offsets,
            match_lengths,
            literal_lengths,
            repeat,
        },
    ))
}

fn dictionary_error(error: Error) -> Error {
    match error {
        Error::AllocationFailed | Error::ResourceLimit => error,
        _ => Error::InvalidDictionary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Original upstream fixtures retain their attribution in LICENSE.zstd and
    // the respective tests/upstream asset manifests. Header offsets and table
    // boundaries below were independently measured with the pinned C loader.
    const HTTP: &[u8] =
        include_bytes!("../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols");
    const FIXED: &[u8] = include_bytes!("../tests/upstream/inline-regressions/fixed-training.dict");
    const INVALID_REPEAT: &[u8] =
        include_bytes!("../tests/upstream/inline-regressions/invalid-repeat-code.dict");
    const HEADER_ONLY: &[u8] =
        include_bytes!("../tests/upstream/inline-regressions/header-only.dict");

    fn assert_invalid(input: &[u8], mode: DictionaryMode) {
        assert_eq!(
            Dictionary::from_bytes_with_mode(input, mode).unwrap_err(),
            Error::InvalidDictionary
        );
    }

    fn replace_section(input: &[u8], start: usize, end: usize, replacement: &[u8]) -> Vec<u8> {
        let mut result = input[..start].to_vec();
        result.extend_from_slice(replacement);
        result.extend_from_slice(&input[end..]);
        result
    }

    #[test]
    fn official_formatted_dictionary_metadata_matches_independent_c_loader() {
        for (bytes, id, content_start) in [(HTTP, 1_386_686_301, 158), (FIXED, 99, 115)] {
            let dictionary = Dictionary::from_bytes(bytes).unwrap();
            assert!(dictionary.is_formatted());
            assert_eq!(dictionary.id(), id);
            assert_eq!(dictionary.as_bytes(), bytes);
            assert_eq!(dictionary.content(), &bytes[content_start..]);
            let entropy = dictionary.entropy().unwrap();
            assert_eq!(entropy.repeat, [1, 4, 8]);
            assert_eq!(entropy.offsets.table_log(), 8);
            assert_eq!(entropy.match_lengths.table_log(), 9);
            assert_eq!(entropy.literal_lengths.table_log(), 9);
            assert!(entropy.huffman.retained_capacity() != 0);
            assert!(dictionary.retained_capacity() > bytes.len());
            assert!(core::ptr::eq(entropy, dictionary.entropy().unwrap()));
        }
    }

    #[test]
    fn short_auto_inputs_are_raw_even_with_a_complete_magic_prefix() {
        for end in 0..8 {
            let bytes = &HEADER_ONLY[..end];
            let dictionary = Dictionary::from_bytes(bytes).unwrap();
            assert!(!dictionary.is_formatted());
            assert!(dictionary.entropy().is_none());
            assert_eq!(dictionary.id(), 0);
            assert_eq!(dictionary.content(), bytes);
            assert_eq!(dictionary.as_bytes(), bytes);
            assert_invalid(bytes, DictionaryMode::Formatted);
        }
        assert_invalid(HEADER_ONLY, DictionaryMode::Auto);
        assert_invalid(HEADER_ONLY, DictionaryMode::Formatted);
    }

    #[test]
    fn raw_mode_never_interprets_even_valid_or_corrupt_formatted_headers() {
        for bytes in [
            HTTP,
            FIXED,
            INVALID_REPEAT,
            HEADER_ONLY,
            b"ordinary content",
            b"",
        ] {
            let dictionary = Dictionary::raw(bytes).unwrap();
            assert!(!dictionary.is_formatted());
            assert_eq!(dictionary.id(), 0);
            assert_eq!(dictionary.content(), bytes);
            assert_eq!(dictionary.as_bytes(), bytes);
            assert!(dictionary.entropy().is_none());
        }
        let ordinary = Dictionary::from_bytes(b"ordinary content").unwrap();
        assert!(!ordinary.is_formatted());
        assert_invalid(b"ordinary content", DictionaryMode::Formatted);
    }

    #[test]
    fn every_formatted_truncation_obeys_metadata_and_repeat_content_bounds() {
        for (bytes, content_start) in [(HTTP, 158), (FIXED, 115)] {
            for end in 0..=bytes.len() {
                let prefix = &bytes[..end];
                assert_eq!(Dictionary::raw(prefix).unwrap().content(), prefix);
                if end < content_start + 8 {
                    assert_invalid(prefix, DictionaryMode::Formatted);
                    if end >= 8 {
                        assert_invalid(prefix, DictionaryMode::Auto);
                    } else {
                        assert!(!Dictionary::from_bytes(prefix).unwrap().is_formatted());
                    }
                } else {
                    for mode in [DictionaryMode::Auto, DictionaryMode::Formatted] {
                        let dictionary = Dictionary::from_bytes_with_mode(prefix, mode).unwrap();
                        assert_eq!(dictionary.content(), &bytes[content_start..end]);
                    }
                }
            }
        }
    }

    #[test]
    fn official_invalid_repeat_dictionary_and_header_only_fixture_are_rejected() {
        for bytes in [INVALID_REPEAT, HEADER_ONLY] {
            assert_invalid(bytes, DictionaryMode::Auto);
            assert_invalid(bytes, DictionaryMode::Formatted);
        }
        let mut corrupt_huffman = FIXED.to_vec();
        corrupt_huffman[8] = 0;
        assert_invalid(&corrupt_huffman, DictionaryMode::Auto);
    }

    #[test]
    fn repeat_offsets_must_be_positive_and_within_content_but_may_duplicate() {
        let content_start = 115;
        let content_len = FIXED.len() - content_start;
        for index in 0..3 {
            for invalid in [0, content_len as u32 + 1, u32::MAX] {
                let mut bytes = FIXED.to_vec();
                let position = content_start - 12 + 4 * index;
                bytes[position..position + 4].copy_from_slice(&invalid.to_le_bytes());
                assert_invalid(&bytes, DictionaryMode::Auto);
            }
        }
        for value in [1, content_len as u32] {
            let mut bytes = FIXED.to_vec();
            for position in (content_start - 12..content_start).step_by(4) {
                bytes[position..position + 4].copy_from_slice(&value.to_le_bytes());
            }
            assert_eq!(
                Dictionary::from_bytes(&bytes)
                    .unwrap()
                    .entropy()
                    .unwrap()
                    .repeat,
                [value as usize; 3]
            );
        }
    }

    #[test]
    fn zero_and_reserved_dictionary_ids_are_accepted_without_changing_format() {
        for id in [0_u32, 1, 32767, 32768, 1 << 31, u32::MAX] {
            let mut bytes = FIXED.to_vec();
            bytes[4..8].copy_from_slice(&id.to_le_bytes());
            let dictionary = Dictionary::from_bytes(&bytes).unwrap();
            assert_eq!(dictionary.id(), id);
            assert!(dictionary.is_formatted());
            assert_eq!(dictionary.content(), &FIXED[115..]);
        }
    }

    #[test]
    fn every_fse_table_enforces_its_dictionary_log_and_symbol_bounds() {
        // The out-of-range normalized headers were serialized independently by
        // v1.5.7 FSE_writeNCount: balanced symbols 0 and the stated maximum,
        // tableLog 5. This checks each dictionary-specific symbol ceiling.
        for (start, end, invalid_log, invalid_symbols) in [
            (17, 31, 9, &[0x10, 0xe3, 0xff, 0xff, 0xf9][..]), // OF symbol 32.
            (31, 81, 10, &[0x10, 0xe3, 0xff, 0xff, 0xff, 0x7f, 0x3e][..]), // ML 53.
            (81, 103, 10, &[0x10, 0xe3, 0xff, 0xff, 0xef, 0x03][..]), // LL 36.
        ] {
            let mut bytes = FIXED.to_vec();
            bytes[start] = (bytes[start] & 0xf0) | (invalid_log - 5);
            assert_invalid(&bytes, DictionaryMode::Formatted);
            let bytes = replace_section(FIXED, start, end, invalid_symbols);
            assert_invalid(&bytes, DictionaryMode::Formatted);
        }
    }

    #[test]
    fn single_symbol_dictionary_tables_preserve_the_normalized_state_width() {
        // Independently serialized by v1.5.7 FSE_writeNCount([32], 0, 5).
        // ZSTD_createDDict_advanced(fullDict) accepts each of these six cases.
        // A log-zero inline RLE replacement would consume the wrong initial
        // state bits when the dictionary table is reused by a frame.
        for (bytes, sections) in [
            (HTTP, [(80, 91), (91, 125), (125, 146)]),
            (FIXED, [(17, 31), (31, 81), (81, 103)]),
        ] {
            for (index, (start, end)) in sections.into_iter().enumerate() {
                let changed = replace_section(bytes, start, end, &[0xf0, 0x03]);
                let dictionary = Dictionary::from_bytes(&changed).unwrap();
                let entropy = dictionary.entropy().unwrap();
                let table = [
                    &entropy.offsets,
                    &entropy.match_lengths,
                    &entropy.literal_lengths,
                ][index];
                assert_eq!(table.table_log(), 5);
                for state in 0..32 {
                    let entry = table.entry(state).unwrap();
                    assert_eq!(entry.symbol, 0);
                    assert_eq!(entry.bits, 0);
                    assert_eq!(usize::from(entry.base), state);
                }
                assert!(table.entry(32).is_err());
            }
        }
    }

    #[test]
    fn owned_and_shared_dictionaries_outlive_original_input_without_copying_content_again() {
        let mut source = FIXED.to_vec();
        let dictionary = Dictionary::from_bytes(&source).unwrap();
        source.fill(0);
        assert_eq!(dictionary.as_bytes(), FIXED);
        let content_address = dictionary.content().as_ptr();
        let capacity = dictionary.retained_capacity();
        let shared = dictionary.into_shared();
        let clone = SharedDictionary::clone(&shared);
        assert!(SharedDictionary::ptr_eq(&shared, &clone));
        assert_eq!(clone.content().as_ptr(), content_address);
        assert_eq!(clone.retained_capacity(), capacity);
        drop(shared);
        assert_eq!(clone.as_bytes(), FIXED);
    }

    #[test]
    fn indexed_content_access_checks_relative_bounds_and_usize_max() {
        for dictionary in [
            Dictionary::from_bytes(FIXED).unwrap(),
            Dictionary::raw(b"raw bytes").unwrap(),
            Dictionary::raw(b"").unwrap(),
        ] {
            for (index, &expected) in dictionary.content().iter().enumerate() {
                assert_eq!(dictionary.content_byte(index), Ok(expected));
            }
            for index in [dictionary.content().len(), usize::MAX] {
                assert_eq!(dictionary.content_byte(index), Err(Error::CorruptData));
            }
        }
    }
}
