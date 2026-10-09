//! Owned dictionaries with validated metadata and immutable prepared entropy.
//!
//! Layout and loader contracts follow facebook/zstd v1.5.7
//! `ZSTD_decompress_insertDictionary`, `ZSTD_loadDEntropy`, and `ZSTD_DDict`.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::cursor::ByteCursor;
use crate::fse::{FseTable, FseTableView};
use crate::huffman::{HuffmanTable, HuffmanTableView};
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
pub type SharedDictionary<A = Global> = alloc::sync::Arc<Dictionary<A>>;

/// Shared immutable ownership on targets without pointer atomics.
#[cfg(not(target_has_atomic = "ptr"))]
pub type SharedDictionary<A = Global> = alloc::rc::Rc<Dictionary<A>>;

/// Prepared decoding tables and initial repeat offsets from one dictionary.
///
/// Corresponds to `ZSTD_entropyDTables_t`. Each table owns its allocation; no
/// table borrows the serialized bytes or another field. Working frame tables
/// can override these immutable defaults without cloning them.
#[derive(Debug)]
pub(crate) struct DictionaryEntropy<A: Allocator = Global> {
    pub(crate) huffman: HuffmanTable<A>,
    pub(crate) offsets: FseTable<A>,
    pub(crate) match_lengths: FseTable<A>,
    pub(crate) literal_lengths: FseTable<A>,
    pub(crate) repeat: [usize; 3],
}

/// Validated immutable entropy borrowed independently of allocator type.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DictionaryEntropyView<'a> {
    pub(crate) huffman: HuffmanTableView<'a>,
    pub(crate) offsets: FseTableView<'a>,
    pub(crate) match_lengths: FseTableView<'a>,
    pub(crate) literal_lengths: FseTableView<'a>,
    pub(crate) repeat: [usize; 3],
}

impl<A: Allocator> DictionaryEntropy<A> {
    pub(crate) fn view(&self) -> DictionaryEntropyView<'_> {
        DictionaryEntropyView {
            huffman: self.huffman.view(),
            offsets: self.offsets.view(),
            match_lengths: self.match_lengths.view(),
            literal_lengths: self.literal_lengths.view(),
            repeat: self.repeat,
        }
    }
}

/// An allocation-free immutable borrow of a validated dictionary.
///
/// The original owner retains bytes, prepared tables, and their allocators.
/// Copying this view neither allocates nor extends that owner's lifetime. Its
/// allocator type does not appear in the view. Obtain one with [`Dictionary::view`].
///
/// Slices borrow the dictionary, so a temporary view can provide them:
///
/// ```
/// use zstdoxide::Dictionary;
/// let dictionary = Dictionary::raw(b"content")?;
/// let content = dictionary.view().content();
/// assert_eq!(content, b"content");
/// # Ok::<(), zstdoxide::Error>(())
/// ```
///
/// A view cannot outlive the dictionary that produced it:
///
/// ```compile_fail
/// use zstdoxide::Dictionary;
/// let view = {
///     let dictionary = Dictionary::raw(b"content").unwrap();
///     dictionary.view()
/// };
/// let _ = view.content();
/// ```
#[derive(Clone, Copy, Debug)]
pub struct DictionaryView<'a> {
    bytes: &'a [u8],
    content_start: usize,
    id: u32,
    entropy: Option<DictionaryEntropyView<'a>>,
    retained_capacity: usize,
}

impl<'a> DictionaryView<'a> {
    /// Formatted dictionary ID, or zero for raw content.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Matchable bytes, excluding formatted metadata.
    pub fn content(&self) -> &'a [u8] {
        &self.bytes[self.content_start..]
    }

    /// Exact original serialization borrowed from the owner.
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Whether the owner parsed a formatted dictionary.
    pub fn is_formatted(&self) -> bool {
        self.entropy.is_some()
    }

    /// Read a checked content-relative byte using the same access backend as
    /// the owning dictionary. Invalid positions report corrupt compressed data.
    pub fn content_byte(&self, index: usize) -> Result<u8> {
        read_content_byte(self.content(), index)
    }

    pub(crate) fn entropy(&self) -> Option<DictionaryEntropyView<'a>> {
        self.entropy
    }

    /// Reachable payload capacity of the original owner, not memory owned by
    /// this view. Shared control storage and inline values remain excluded.
    pub(crate) fn retained_capacity(&self) -> usize {
        self.retained_capacity
    }
}

mod sealed {
    pub trait Sealed {}
}

/// A supported immutable dictionary attachment, dispatched without trait objects.
///
/// Implementations are sealed to shared [`SharedDictionary`] owners and borrowed
/// [`DictionaryView`] values. Cloning either attachment allocates nothing; a
/// shared owner keeps the dictionary alive, while a view preserves its original
/// borrow. Cloning a shared handle never clones the dictionary's allocator.
pub trait DictionaryHandle: sealed::Sealed + Clone {
    /// Borrow validated bytes and prepared tables for the current operation.
    fn view(&self) -> DictionaryView<'_>;
}

impl<A: Allocator> sealed::Sealed for SharedDictionary<A> {}

impl<A: Allocator> DictionaryHandle for SharedDictionary<A> {
    fn view(&self) -> DictionaryView<'_> {
        self.as_ref().view()
    }
}

impl sealed::Sealed for DictionaryView<'_> {}

impl DictionaryHandle for DictionaryView<'_> {
    fn view(&self) -> DictionaryView<'_> {
        *self
    }
}

/// An owned raw dictionary or validated formatted dictionary.
///
/// Content positions are indices into the owned initialized byte vector. The
/// content start is validated before construction, and no method mutates the
/// bytes, prepared entropy, or start index. Empty and short raw content is valid.
/// This value is suitable for sharing across independent codec contexts.
#[derive(Debug)]
pub struct Dictionary<A: Allocator = Global> {
    bytes: Vec<u8, A>,
    content_start: usize,
    id: u32,
    entropy: Option<DictionaryEntropy<A>>,
}

impl Dictionary<Global> {
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
        Self::from_bytes_with_mode_in(input, mode, Global)
    }

    /// Copy raw content without interpreting a formatted header.
    pub fn raw(input: &[u8]) -> Result<Self> {
        Self::raw_in(input, Global)
    }
}

impl<A: Allocator + Clone> Dictionary<A> {
    /// Copy and prepare a dictionary using automatic format detection and the
    /// supplied allocator family. The original allocator owns serialized bytes;
    /// fresh clones own tables and scratch. Clones need not be interchangeable.
    /// Ordinary allocator cloning may itself allocate or panic; only collection
    /// allocation errors are returned as [`Error::AllocationFailed`]. A borrowed
    /// non-Clone allocator is supported without cloning its underlying state.
    pub fn from_bytes_in(input: &[u8], allocator: A) -> Result<Self> {
        Self::from_bytes_with_mode_in(input, DictionaryMode::Auto, allocator)
    }

    /// Copy and validate with explicit content interpretation and allocation.
    /// Malformed formatted input is never retried as raw content. All prepared
    /// tables and temporary weight storage use this allocator family; failure
    /// releases every partially constructed owner before returning an error.
    pub fn from_bytes_with_mode_in(
        input: &[u8],
        mode: DictionaryMode,
        allocator: A,
    ) -> Result<Self> {
        let formatted = match mode {
            DictionaryMode::Auto => input.len() >= 8 && input[..4] == MAGIC.to_le_bytes(),
            DictionaryMode::Raw => false,
            DictionaryMode::Formatted => true,
        };
        let (id, content_start, entropy) = if formatted {
            let (id, start, entropy) =
                parse_formatted_in(input, allocator.clone()).map_err(dictionary_error)?;
            (id, start, Some(entropy))
        } else {
            (0, 0, None)
        };
        let mut bytes = Vec::new_in(allocator);
        bytes
            .try_reserve_exact(input.len())
            .map_err(|_| Error::AllocationFailed)?;
        for &byte in input {
            bytes.push(byte);
        }
        Ok(Self {
            bytes,
            content_start,
            id,
            entropy,
        })
    }
}

impl<A: Allocator> Dictionary<A> {
    /// Borrow bytes and prepared entropy without copying or allocating. The
    /// returned view is independent of `A` but cannot outlive this owner.
    pub fn view(&self) -> DictionaryView<'_> {
        DictionaryView {
            bytes: &self.bytes,
            content_start: self.content_start,
            id: self.id,
            entropy: self.entropy.as_ref().map(DictionaryEntropy::view),
            retained_capacity: self.retained_capacity(),
        }
    }

    /// Copy raw content into a single allocation owner without cloning it or
    /// interpreting headers. Owned non-Clone allocators are accepted here.
    pub fn raw_in(input: &[u8], allocator: A) -> Result<Self> {
        let mut bytes = Vec::new_in(allocator);
        bytes
            .try_reserve_exact(input.len())
            .map_err(|_| Error::AllocationFailed)?;
        for &byte in input {
            bytes.push(byte);
        }
        Ok(Self::from_raw_owned(bytes))
    }

    /// Transfer initialized raw content from a fallibly allocated trainer.
    pub(crate) fn from_raw_owned(bytes: Vec<u8, A>) -> Self {
        Self {
            bytes,
            content_start: 0,
            id: 0,
            entropy: None,
        }
    }

    /// Move this prepared value into shared ownership without copying its bytes.
    ///
    /// Payload and table allocations were prepared fallibly by the constructor.
    /// The standard `Arc`/`Rc` allocation is infallible and uses Global, including
    /// both reference counts and the entire inline dictionary value. Its size
    /// includes the stored allocator values. This is an explicit exception to
    /// custom allocation; the bytes and tables retain their exact owners.
    pub fn into_shared(self) -> SharedDictionary<A> {
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
        read_content_byte(self.content(), index)
    }

    pub(crate) fn entropy(&self) -> Option<&DictionaryEntropy<A>> {
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

fn read_content_byte(content: &[u8], index: usize) -> Result<u8> {
    if index >= content.len() {
        return Err(Error::CorruptData);
    }
    #[cfg(not(feature = "unsafe-dictionary"))]
    let byte = content[index];
    #[cfg(feature = "unsafe-dictionary")]
    // SAFETY: The runtime check proves index is inside the initialized slice.
    // A shared slice guarantees valid aligned u8 storage for this access and
    // prevents mutation or destruction while borrowed. Both callers derive it
    // from the owner's validated content range; the view retains that borrow.
    // Copying a byte neither extends a reference nor transfers ownership, and
    // no pointer or mutable alias escapes this access.
    let byte = unsafe { *content.get_unchecked(index) };
    Ok(byte)
}

/// Parse the ordered entropy fields and repcodes used by `ZSTD_loadDEntropy`.
fn parse_formatted_in<A: Allocator + Clone>(
    input: &[u8],
    allocator: A,
) -> Result<(u32, usize, DictionaryEntropy<A>)> {
    let mut cursor = ByteCursor::new(input);
    if cursor.little_endian(4)? != u64::from(MAGIC) {
        return Err(Error::InvalidDictionary);
    }
    let id = cursor.little_endian(4)? as u32;
    let (huffman, size) = HuffmanTable::read_dictionary_in(cursor.remaining(), allocator.clone())?;
    cursor.take(size)?;
    let (offsets, size) =
        FseTable::read_dictionary_in(cursor.remaining(), 31, 8, allocator.clone())?;
    cursor.take(size)?;
    let (match_lengths, size) =
        FseTable::read_dictionary_in(cursor.remaining(), 52, 9, allocator.clone())?;
    cursor.take(size)?;
    let (literal_lengths, size) =
        FseTable::read_dictionary_in(cursor.remaining(), 35, 9, allocator)?;
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
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

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

    fn tracking() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig::default()).unwrap()
    }

    fn same_view(view: DictionaryView<'_>, expected: &Dictionary) {
        assert_eq!(view.id(), expected.id());
        assert_eq!(view.as_bytes(), expected.as_bytes());
        assert_eq!(view.content(), expected.content());
        assert_eq!(view.is_formatted(), expected.is_formatted());
        assert_eq!(view.retained_capacity(), expected.retained_capacity());
        for index in 0..view.content().len() {
            assert_eq!(view.content_byte(index), expected.content_byte(index));
        }
        assert_eq!(
            view.content_byte(view.content().len()),
            Err(Error::CorruptData)
        );
        assert_eq!(view.content_byte(usize::MAX), Err(Error::CorruptData));
        if let (Some(view), Some(owned)) = (view.entropy(), expected.entropy()) {
            assert_eq!(view.repeat, owned.repeat);
            assert_eq!(view.huffman.code_lengths(), owned.huffman.code_lengths());
            for (view, owned) in [
                (view.offsets, &owned.offsets),
                (view.match_lengths, &owned.match_lengths),
                (view.literal_lengths, &owned.literal_lengths),
            ] {
                assert_eq!(view.table_log(), owned.table_log());
                for state in 0..1 << view.table_log() {
                    assert_eq!(view.entry(state), owned.entry(state));
                }
            }
        }
    }

    #[test]
    fn borrowed_views_erase_allocator_type_without_requests_or_state_changes() {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        for bytes in [&b""[..], &b"plain raw"[..], FIXED] {
            let global = Dictionary::from_bytes(bytes).unwrap();
            let custom = Dictionary::from_bytes_in(bytes, &owner).unwrap();
            let before = monitor.snapshot();
            assert!(monitor.fail_next());
            let views = [global.view(), custom.view()];
            let copied = views;
            for view in copied {
                same_view(view, &global);
            }
            assert_eq!(views[1].as_bytes().as_ptr(), custom.as_bytes().as_ptr());
            assert_eq!(monitor.snapshot(), before);
            assert!(matches!(
                Dictionary::raw_in(b"request", &owner),
                Err(Error::AllocationFailed)
            ));
            same_view(views[1], &global);
            drop(custom);
            monitor.assert_released();
        }
        assert_eq!(monitor.snapshot().identities_created, 1);
    }

    fn check_handle<D: DictionaryHandle>(handle: D, expected: &Dictionary) {
        let clone = handle.clone();
        same_view(handle.view(), expected);
        drop(handle);
        same_view(clone.view(), expected);
    }

    #[test]
    fn supported_handles_clone_without_allocating_or_cloning_payload_allocators() {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let dictionary = Dictionary::from_bytes_in(FIXED, &owner).unwrap();
        let global = Dictionary::from_bytes(FIXED).unwrap();
        let before = monitor.snapshot();
        check_handle(dictionary.view(), &global);
        let shared = dictionary.into_shared();
        check_handle(shared.clone(), &global);
        assert_eq!(monitor.snapshot(), before);
        assert_eq!(monitor.snapshot().live_blocks, 5);
        drop(shared);
        monitor.assert_released();
    }

    #[test]
    fn custom_raw_owner_needs_no_clone_and_moves_without_copying_storage() {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let identity = owner.identity();
        let dictionary = Dictionary::raw_in(FIXED, owner).unwrap();
        assert_eq!(dictionary.bytes.allocator().identity(), identity);
        assert_eq!(dictionary.content(), FIXED);
        assert!(!dictionary.is_formatted());
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        let pointer = dictionary.as_bytes().as_ptr();
        let moved = dictionary;
        assert_eq!(moved.as_bytes().as_ptr(), pointer);
        assert_eq!(moved.content_byte(0), Ok(FIXED[0]));
        assert_eq!(moved.content_byte(FIXED.len()), Err(Error::CorruptData));
        assert_eq!(moved.content_byte(usize::MAX), Err(Error::CorruptData));
        drop(moved);
        monitor.assert_released();

        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let empty = Dictionary::raw_in(&[], owner).unwrap();
        assert_eq!(monitor.snapshot().attempts, 0);
        assert_eq!(empty.content_byte(0), Err(Error::CorruptData));
        drop(empty);
        monitor.assert_released();
    }

    fn same_dictionary<A: Allocator, B: Allocator>(a: &Dictionary<A>, b: &Dictionary<B>) {
        assert_eq!(a.as_bytes(), b.as_bytes());
        assert_eq!(a.content(), b.content());
        assert_eq!(a.id(), b.id());
        assert_eq!(a.is_formatted(), b.is_formatted());
        if let (Some(a), Some(b)) = (a.entropy(), b.entropy()) {
            assert_eq!(a.repeat, b.repeat);
            assert_eq!(a.huffman.code_lengths(), b.huffman.code_lengths());
            for (a, b) in [
                (&a.offsets, &b.offsets),
                (&a.match_lengths, &b.match_lengths),
                (&a.literal_lengths, &b.literal_lengths),
            ] {
                assert_eq!(a.table_log(), b.table_log());
                for state in 0..1 << a.table_log() {
                    assert_eq!(a.entry(state), b.entry(state));
                }
            }
        }
    }

    #[test]
    fn custom_formatted_tables_match_global_and_accept_borrowed_non_clone_owners() {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        for bytes in [FIXED, HTTP] {
            let custom = Dictionary::from_bytes_in(bytes, &owner).unwrap();
            let global = Dictionary::from_bytes(bytes).unwrap();
            same_dictionary(&custom, &global);
            assert_eq!(custom.bytes.allocator().identity(), owner.identity());
            // Serialized bytes and four prepared tables remain; weight scratch
            // is already gone, and the borrowed allocator created no identities.
            assert_eq!(monitor.snapshot().live_blocks, 5);
            assert_eq!(monitor.snapshot().identities_created, 1);
            drop(custom);
            monitor.assert_released();
        }
        for (start, end) in [(17, 31), (31, 81), (81, 103)] {
            let bytes = replace_section(FIXED, start, end, &[0xf0, 0x03]);
            let custom = Dictionary::from_bytes_in(&bytes, &owner).unwrap();
            same_dictionary(&custom, &Dictionary::from_bytes(&bytes).unwrap());
            drop(custom);
            monitor.assert_released();
        }
    }

    #[test]
    fn custom_dictionary_failure_sweep_releases_all_partial_owners() {
        for mode in [
            DictionaryMode::Raw,
            DictionaryMode::Auto,
            DictionaryMode::Formatted,
        ] {
            let owner = tracking();
            let monitor = owner.monitor();
            let dictionary = Dictionary::from_bytes_with_mode_in(FIXED, mode, owner).unwrap();
            let requests = monitor.snapshot().attempts;
            assert!(requests > 0);
            drop(dictionary);
            monitor.assert_released();
            for ordinal in 1..=requests {
                let owner = TrackingAllocator::new(AllocatorConfig {
                    fail_at: NonZeroU64::new(ordinal),
                    ..AllocatorConfig::default()
                })
                .unwrap();
                let monitor = owner.monitor();
                let result = Dictionary::from_bytes_with_mode_in(FIXED, mode, owner);
                assert!(
                    matches!(result, Err(Error::AllocationFailed)),
                    "request {ordinal}"
                );
                assert_eq!(monitor.snapshot().attempts, ordinal);
                drop(result);
                monitor.assert_released();
            }
        }
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        assert!(monitor.fail_next());
        assert!(matches!(
            Dictionary::raw_in(b"raw", owner),
            Err(Error::AllocationFailed)
        ));
        monitor.assert_released();
    }

    #[test]
    fn custom_malformed_dictionaries_preserve_errors_and_release_scratch() {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        for bytes in [
            INVALID_REPEAT,
            HEADER_ONLY,
            &FIXED[..8],
            &FIXED[..103],
            &FIXED[..115],
        ] {
            for mode in [DictionaryMode::Auto, DictionaryMode::Formatted] {
                assert_eq!(
                    Dictionary::from_bytes_with_mode_in(bytes, mode, &owner)
                        .err()
                        .unwrap(),
                    Error::InvalidDictionary
                );
                monitor.assert_released();
            }
        }
        let before = monitor.snapshot().attempts;
        assert!(monitor.fail_next());
        assert_eq!(
            Dictionary::from_bytes_with_mode_in(b"wrong", DictionaryMode::Formatted, &owner)
                .err()
                .unwrap(),
            Error::InvalidDictionary
        );
        assert_eq!(monitor.snapshot().attempts, before);
        assert!(matches!(
            Dictionary::raw_in(b"raw", &owner),
            Err(Error::AllocationFailed)
        ));
        monitor.assert_released();
        let valid = Dictionary::from_bytes_in(FIXED, &owner).unwrap();
        same_dictionary(&valid, &Dictionary::from_bytes(FIXED).unwrap());
        drop(valid);
        monitor.assert_released();
    }

    #[test]
    fn custom_shared_payload_retains_exact_owners_until_the_last_handle() {
        let owner = tracking();
        let identity = owner.identity();
        let monitor = owner.monitor();
        let dictionary = Dictionary::from_bytes_in(FIXED, owner).unwrap();
        assert_eq!(dictionary.bytes.allocator().identity(), identity);
        assert!(monitor.snapshot().identities_created > 1);
        assert_eq!(monitor.snapshot().live_blocks, 5);
        let pointer = dictionary.as_bytes().as_ptr();
        let before = monitor.snapshot();
        let shared = dictionary.into_shared();
        let clone = shared.clone();
        assert_eq!(shared.as_bytes().as_ptr(), pointer);
        assert_eq!(monitor.snapshot().attempts, before.attempts);
        assert_eq!(
            monitor.snapshot().identities_created,
            before.identities_created
        );
        drop(shared);
        assert_eq!(monitor.snapshot().live_blocks, 5);
        assert_eq!(clone.content(), &FIXED[115..]);
        drop(clone);
        // Only payload requests are tracked; standard Arc/Rc's Global outer
        // object is deliberately outside this ledger and public promise.
        monitor.assert_released();
    }

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
