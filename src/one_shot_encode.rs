//! Attempt-scoped dictionary ownership over the bounded one-shot encoder.
#![forbid(unsafe_code)]

use crate::one_shot_selection::Selection;
use crate::{
    CompressionParameterRequest, DictionaryHandle, DictionaryView, Result, SharedDictionary,
};
use alloc::alloc::{Allocator, Global};

/// Owns persistent or next-attempt dictionary selection for caller-owned output.
///
/// A prefix is consumed before every compression attempt, including empty input,
/// short output, allocation errors and caller allocator unwinding. Persistent
/// selection survives calls and errors until explicitly replaced or cleared.
/// Raw versus formatted interpretation is fixed by dictionary preparation.
/// No handle is cloned, and no input, output or scratch is retained between calls.
/// Borrowed views retain the dictionary owner's lifetime and allocation domain.
///
/// Calls use the ordinary Greedy encoder with known content size and checksum.
/// This type exposes no reference levels, streaming, workspace reuse or native
/// context reset/parameter semantics. Frame bytes and provisional-output behavior
/// match [`crate::compress_into`] or [`crate::compress_into_with_dictionary_in`].
///
/// ```
/// use zstdoxide::{Dictionary, OneShotEncoder, compress, compression_bound};
/// let dictionary = Dictionary::raw(b"example pattern example pattern")?.into_shared();
/// let input = b"example pattern example pattern";
/// let mut encoder = OneShotEncoder::with_prefix(dictionary.view());
/// let mut output = vec![0xa5; compression_bound(input.len())?];
/// let size = encoder.compress_into(input, &mut output)?;
/// assert!(!encoder.prefix_pending());
/// assert!(encoder.dictionary().is_none());
/// assert_eq!(&output[..size], zstdoxide::compress_with_dictionary(input, &dictionary)?);
/// let next = encoder.compress_into(input, &mut output)?;
/// assert_eq!(&output[..next], compress(input)?);
/// # Ok::<(), zstdoxide::Error>(())
/// ```
///
/// A borrowed selection cannot outlive its dictionary owner:
///
/// ```compile_fail
/// use zstdoxide::{Dictionary, OneShotEncoder};
/// let mut encoder = {
///     let dictionary = Dictionary::raw(b"prefix").unwrap();
///     OneShotEncoder::with_prefix(dictionary.view())
/// };
/// let _ = encoder.compress_into(b"prefix", &mut [0_u8; 64]);
/// ```
///
/// Input and destination must have independent borrows:
///
/// ```compile_fail
/// let mut encoder = zstdoxide::OneShotEncoder::new();
/// let mut storage = [0_u8; 64];
/// let _ = encoder.compress_into(&storage[..16], &mut storage);
/// ```
pub struct OneShotEncoder<D = SharedDictionary> {
    selection: Selection<D>,
}

impl OneShotEncoder<SharedDictionary> {
    /// Creates a shared-owner coordinator without a dictionary.
    pub fn new() -> Self {
        Self::without_dictionary()
    }
}

impl Default for OneShotEncoder<SharedDictionary> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D: DictionaryHandle> OneShotEncoder<D> {
    /// Creates an empty coordinator whose later selections have handle type D.
    pub fn without_dictionary() -> Self {
        Self {
            selection: Selection::None,
        }
    }
    /// Takes persistent ownership of a shared handle or borrowed dictionary view.
    pub fn with_dictionary(dictionary: D) -> Self {
        Self {
            selection: Selection::Persistent(dictionary),
        }
    }
    /// Takes ownership of a selection for exactly the next attempted call.
    ///
    /// A raw handle remains raw content; a formatted handle keeps its identifier
    /// and prepared entropy. No implicit reinterpretation as raw prefix occurs.
    pub fn with_prefix(prefix: D) -> Self {
        Self {
            selection: Selection::Prefix(prefix),
        }
    }
    /// Replaces selection with a persistent owner, or clears it with None.
    pub fn set_dictionary(&mut self, dictionary: Option<D>) {
        self.selection = dictionary.map_or(Selection::None, Selection::Persistent);
    }
    /// Replaces selection with a next-call owner, or clears it with None.
    pub fn set_prefix(&mut self, prefix: Option<D>) {
        self.selection = prefix.map_or(Selection::None, Selection::Prefix);
    }
    /// Borrows immutable metadata/content of the retained selection.
    pub fn dictionary(&self) -> Option<DictionaryView<'_>> {
        self.selection.retained().map(DictionaryHandle::view)
    }
    /// Whether the next attempt consumes the retained selection.
    pub fn prefix_pending(&self) -> bool {
        self.selection.prefix_pending()
    }
    /// Compresses one frame with Global scratch into independent caller storage.
    ///
    /// Success returns the written length and preserves the remaining output.
    /// A short destination returns [`crate::Error::ResourceLimit`]; earlier whole
    /// appends may remain. Allocation can fail and whole-call atomicity is not
    /// promised. All writes stay within the supplied slice. Prefix consumption
    /// follows [`Self`] even when no bytes are written.
    pub fn compress_into(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize> {
        self.compress_into_in(input, output, Global)
    }
    /// Compresses with fresh scratch independent of dictionary ownership.
    ///
    /// The prefix is removed before output admission or any allocator operation.
    /// A claimed owner stays alive until the attempt returns or unwinds. No
    /// dictionary handle is cloned and no caller borrow survives the attempt.
    /// The caller owns output; one-block staging and existing search/entropy
    /// scratch use the supplied allocator and its clones and release before
    /// return. A non-Clone allocator can be passed by reference. Ordinary Clone
    /// retains the caller's own allocation/panic behavior. This coordinator
    /// itself adds no allocation or clone beyond the underlying frame engine.
    pub fn compress_into_in<A: Allocator + Clone>(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        allocator: A,
    ) -> Result<usize> {
        let call = self.selection.claim();
        match call.retained() {
            Some(dictionary) => {
                crate::compress_into_with_dictionary_in(input, output, dictionary, allocator)
            }
            None => crate::compress_into_in(input, output, allocator),
        }
    }

    /// Compresses one parameterized frame into caller storage.
    ///
    /// The per-call request is resolved against the actual source length and,
    /// when a selection is retained, the dictionary's matchable content length.
    /// A retained dictionary uses the exact combined-staging, entropy-seed and
    /// scoring subset of [`crate::compress_with_parameters_and_dictionary`];
    /// no selection uses [`crate::compress_with_parameters`]. Frame flags are
    /// the parameterized family's known size without checksum, not the ordinary
    /// checksummed coordinator frame of [`Self::compress_into`]. The request is
    /// not stored, so later calls may pass a different request or use the
    /// ordinary method again.
    ///
    /// A pending prefix is claimed before request resolution, output admission
    /// or allocator work, so a resolution or unsupported-profile error still
    /// consumes it exactly like every other attempted call. Persistent
    /// selection survives successful, short-output and failed attempts. Success
    /// returns the exact written length; a short destination returns
    /// [`crate::Error::ResourceLimit`] after earlier whole appends. Bytes outside
    /// the supplied slice are never changed.
    pub fn compress_into_with_parameters(
        &mut self,
        input: &[u8],
        request: &CompressionParameterRequest,
        output: &mut [u8],
    ) -> Result<usize> {
        self.compress_into_with_parameters_in(input, request, output, Global)
    }

    /// Compresses a parameterized frame with fresh scratch independent of the
    /// dictionary owner.
    ///
    /// Behavior and lifetime guards match [`Self::compress_into_with_parameters`].
    /// The claimed owner stays alive until the attempt returns or unwinds, no
    /// handle is cloned, and no caller borrow survives the attempt. Scratch uses
    /// independent clones of the supplied allocator and is released before
    /// return; a borrowed non-Clone allocator is accepted by reference.
    pub fn compress_into_with_parameters_in<A: Allocator + Clone>(
        &mut self,
        input: &[u8],
        request: &CompressionParameterRequest,
        output: &mut [u8],
        allocator: A,
    ) -> Result<usize> {
        let call = self.selection.claim();
        match call.retained() {
            Some(dictionary) => crate::compress_into_with_parameters_and_dictionary_in(
                input, output, request, dictionary, allocator,
            ),
            None => crate::reference_encode::compress_into_with_parameters_in(
                input, output, request, allocator,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Dictionary, Error, compress, compress_with_dictionary_in, compression_bound};
    use alloc::vec;
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    const PREPARED: &[u8] = include_bytes!("../tests/upstream/dictionaries/formatted.dict");

    #[test]
    fn persistent_selection_survives_repeated_calls_and_short_output_without_handle_clones() {
        let dictionary = Dictionary::from_bytes(PREPARED).unwrap().into_shared();
        let mut encoder = OneShotEncoder::with_dictionary(dictionary.clone());
        let input = b"dictionary dictionary pattern dictionary pattern";
        let expected = compress_with_dictionary_in(input, &dictionary.view(), Global).unwrap();
        let mut output = vec![0xa5; expected.len() + 2];
        for _ in 0..3 {
            assert_eq!(
                encoder.compress_into(input, &mut []),
                Err(Error::ResourceLimit)
            );
            assert!(!encoder.prefix_pending());
            assert_eq!(encoder.dictionary().unwrap().as_bytes(), PREPARED);
            assert_eq!(
                encoder.compress_into(input, &mut output[1..1 + expected.len()]),
                Ok(expected.len())
            );
            assert_eq!(&output[1..1 + expected.len()], expected);
            assert_eq!((output[0], output[expected.len() + 1]), (0xa5, 0xa5));
            assert_eq!(SharedDictionary::strong_count(&dictionary), 2);
        }
        encoder.set_dictionary(None);
        assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
    }

    #[test]
    fn prefix_is_consumed_by_empty_success_and_capacity_failure_before_scratch() {
        let dictionary = Dictionary::from_bytes(PREPARED).unwrap();
        let expected = compress_with_dictionary_in(b"", &dictionary.view(), Global).unwrap();
        let mut encoder = OneShotEncoder::with_prefix(dictionary.view());
        let mut output = vec![0xa5; expected.len() + 2];
        assert_eq!(
            encoder.compress_into(b"", &mut output[1..1 + expected.len()]),
            Ok(expected.len())
        );
        assert_eq!(&output[1..1 + expected.len()], expected);
        assert!(encoder.dictionary().is_none());
        assert!(!encoder.prefix_pending());
        encoder.set_prefix(Some(dictionary.view()));
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        assert_eq!(
            encoder.compress_into_in(b"", &mut [], owner),
            Err(Error::ResourceLimit)
        );
        assert_eq!(monitor.snapshot().attempts, 0);
        monitor.assert_released();
        assert!(encoder.dictionary().is_none());
        assert!(!encoder.prefix_pending());
        let ordinary = compress(b"").unwrap();
        output.fill(0xa5);
        assert_eq!(
            encoder.compress_into(b"", &mut output[1..]),
            Ok(ordinary.len())
        );
        assert_eq!(&output[1..1 + ordinary.len()], ordinary);
        assert!(output[1 + ordinary.len()..].iter().all(|&b| b == 0xa5));
    }

    #[test]
    fn replacement_and_clearing_drop_exact_owners_without_retaining_caller_storage() {
        let a = Dictionary::raw(b"first raw prefix").unwrap().into_shared();
        let b = Dictionary::from_bytes(PREPARED).unwrap().into_shared();
        let mut encoder = OneShotEncoder::with_prefix(a.clone());
        assert_eq!(SharedDictionary::strong_count(&a), 2);
        encoder.set_dictionary(Some(b.clone()));
        assert_eq!(SharedDictionary::strong_count(&a), 1);
        assert_eq!(SharedDictionary::strong_count(&b), 2);
        assert!(!encoder.prefix_pending());
        encoder.set_prefix(Some(a.clone()));
        assert_eq!(SharedDictionary::strong_count(&b), 1);
        assert!(encoder.prefix_pending());
        encoder.set_prefix(None);
        assert_eq!(SharedDictionary::strong_count(&a), 1);
        assert!(encoder.dictionary().is_none());
        let input = b"fresh output after clearing";
        let expected = compress(input).unwrap();
        let mut output = vec![0; compression_bound(input.len()).unwrap()];
        assert_eq!(
            encoder.compress_into(input, &mut output),
            Ok(expected.len())
        );
        assert_eq!(&output[..expected.len()], expected);
    }

    #[test]
    fn every_observed_scratch_failure_consumes_prefix_and_keeps_independent_dictionary_owner() {
        let dictionary_owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let dictionary_monitor = dictionary_owner.monitor();
        let dictionary = Dictionary::from_bytes_in(PREPARED, &dictionary_owner).unwrap();
        let before = dictionary_monitor.snapshot();
        let input = b"dictionary dictionary useful pattern dictionary useful pattern";
        let expected = compress_with_dictionary_in(input, &dictionary.view(), Global).unwrap();
        let mut encoder = OneShotEncoder::with_prefix(dictionary.view());
        let mut output = vec![0xa5; expected.len() + 2];
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        assert_eq!(
            encoder.compress_into_in(input, &mut output[1..1 + expected.len()], &owner),
            Ok(expected.len())
        );
        let attempts = monitor.snapshot().attempts;
        assert!(attempts > 0);
        monitor.assert_released();
        assert_eq!(dictionary_monitor.snapshot(), before);
        for ordinal in 1..=attempts {
            encoder.set_prefix(Some(dictionary.view()));
            let owner = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let monitor = owner.monitor();
            monitor.set_fail_at(NonZeroU64::new(ordinal));
            output.fill(0xa5);
            assert_eq!(
                encoder.compress_into_in(input, &mut output[1..1 + expected.len()], &owner),
                Err(Error::AllocationFailed)
            );
            assert!(!encoder.prefix_pending());
            assert!(encoder.dictionary().is_none());
            assert_eq!(monitor.snapshot().attempts, ordinal);
            assert_eq!(monitor.snapshot().failures, 1);
            assert_eq!(monitor.snapshot().identities_created, 1);
            monitor.assert_released();
            assert_eq!((output[0], output[expected.len() + 1]), (0xa5, 0xa5));
            assert_eq!(dictionary_monitor.snapshot(), before);
            assert_eq!(dictionary.as_bytes(), PREPARED);
        }
        encoder.set_dictionary(None);
        drop(dictionary);
        dictionary_monitor.assert_released();
    }
}
