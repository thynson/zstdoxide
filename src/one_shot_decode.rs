//! Reusable call-level dictionary selection over checked one-shot output engines.
#![forbid(unsafe_code)]
use crate::one_shot_selection::Selection;
use crate::{DecodeOptions, DictionaryHandle, DictionaryView, Result, SharedDictionary};
use alloc::alloc::{Allocator, Global};

/// Owns persistent or next-call dictionary selection for one-shot decoding.
///
/// A prefix is claimed before every decode attempt and used by all standard
/// frames in that call. It is absent from the next call, including after errors,
/// empty input, skippable-only input or invalid in-place coordinates. This is
/// call-level consumption, unlike [`crate::SliceDecoder`]'s frame-level prefix.
/// Persistent selection survives calls and errors until explicitly replaced.
///
/// No dictionary handle is cloned; shared owners and borrowed views retain their
/// own lifetimes and allocators. Input, output and scratch are never retained.
/// Scratch is freshly allocated as needed on each call; this coordinator does
/// not promise native context workspace or reset/parameter equivalence.
///
/// ```
/// use zstdoxide::{DecodeOptions, Dictionary, OneShotDecoder, compress_with_dictionary};
/// let dictionary = Dictionary::raw(b"hello dictionary hello")?.into_shared();
/// let frame = compress_with_dictionary(b"hello dictionary hello", &dictionary)?;
/// let mut joined = frame.clone();
/// joined.extend_from_slice(&frame);
/// let mut decoder = OneShotDecoder::with_prefix(DecodeOptions::new(44), dictionary);
/// let mut output = [0_u8; 44];
/// assert_eq!(decoder.decompress_into(&joined, &mut output)?, 44);
/// assert_eq!(&output, b"hello dictionary hellohello dictionary hello");
/// assert!(!decoder.prefix_pending());
/// assert!(decoder.dictionary().is_none());
/// # Ok::<(), zstdoxide::Error>(())
/// ```
///
/// A borrowed selection cannot outlive its dictionary owner:
///
/// ```compile_fail
/// use zstdoxide::{DecodeOptions, Dictionary, OneShotDecoder};
/// let mut decoder = {
///     let owner = Dictionary::raw(b"dictionary").unwrap();
///     OneShotDecoder::with_prefix(DecodeOptions::new(32), owner.view())
/// };
/// let _ = decoder.decompress_into(b"", &mut [0_u8; 32]);
/// ```
///
/// Input and exclusive destination cannot overlap in direct-output calls:
///
/// ```compile_fail
/// use zstdoxide::{DecodeOptions, OneShotDecoder};
/// let mut decoder = OneShotDecoder::new(DecodeOptions::new(32));
/// let mut bytes = [0_u8; 32];
/// let _ = decoder.decompress_into(&bytes, &mut bytes);
/// ```
pub struct OneShotDecoder<D = SharedDictionary> {
    options: DecodeOptions,
    selection: Selection<D>,
}

impl OneShotDecoder<SharedDictionary> {
    /// Creates the default shared-owner coordinator without dictionary selection.
    pub fn new(options: DecodeOptions) -> Self {
        Self::without_dictionary(options)
    }
}

impl<D: DictionaryHandle> OneShotDecoder<D> {
    /// Creates an empty coordinator whose later selections have handle type D.
    pub fn without_dictionary(options: DecodeOptions) -> Self {
        Self {
            options,
            selection: Selection::None,
        }
    }

    /// Takes persistent ownership of a shared handle or borrowed dictionary view.
    pub fn with_dictionary(options: DecodeOptions, dictionary: D) -> Self {
        Self {
            options,
            selection: Selection::Persistent(dictionary),
        }
    }

    /// Takes ownership of a selection for exactly the next attempted call.
    ///
    /// Raw versus formatted interpretation is already fixed by the handle's
    /// dictionary preparation. No implicit byte reinterpretation occurs here.
    pub fn with_prefix(options: DecodeOptions, prefix: D) -> Self {
        Self {
            options,
            selection: Selection::Prefix(prefix),
        }
    }

    /// Returns copied limits used by subsequent calls.
    pub fn options(&self) -> DecodeOptions {
        self.options
    }

    /// Replaces output/window limits without changing dictionary selection.
    pub fn set_options(&mut self, options: DecodeOptions) {
        self.options = options;
    }

    /// Replaces selection with a persistent owner, or clears it with None.
    pub fn set_dictionary(&mut self, dictionary: Option<D>) {
        self.selection = dictionary.map_or(Selection::None, Selection::Persistent);
    }

    /// Replaces selection with a next-call owner, or clears it with None.
    pub fn set_prefix(&mut self, prefix: Option<D>) {
        self.selection = prefix.map_or(Selection::None, Selection::Prefix);
    }

    /// Borrows immutable metadata/content of the currently retained selection.
    pub fn dictionary(&self) -> Option<DictionaryView<'_>> {
        self.selection.retained().map(DictionaryHandle::view)
    }

    /// Whether the next attempted call will consume the retained selection.
    pub fn prefix_pending(&self) -> bool {
        self.selection.prefix_pending()
    }

    /// Decodes one exact concatenation into independent caller-owned storage.
    ///
    /// Uses Global scratch and the bounds/provisional-output contract of
    /// [`crate::decompress_into_with_options`]. Prefix consumption is described
    /// on [`Self`]; frame/history/entropy state is fresh for every attempt.
    pub fn decompress_into(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize> {
        self.decompress_into_in(input, output, Global)
    }

    /// Decodes with a scratch allocator independent of dictionary ownership.
    ///
    /// No handle or allocator is retained or cloned by the coordinator. Delegated
    /// scratch uses allocator clones as documented by the existing output APIs;
    /// ordinary allocator cloning can allocate or panic. The claimed prefix is
    /// removed before that work and drops on success, error or unwinding.
    pub fn decompress_into_in<A: Allocator + Clone>(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        allocator: A,
    ) -> Result<usize> {
        let options = self.options;
        let call = self.selection.claim();
        match call.retained() {
            Some(dictionary) => crate::decompress_into_with_dictionary_in(
                input, output, dictionary, options, allocator,
            ),
            None => crate::decompress_into_with_options_in(input, output, options, allocator),
        }
    }

    /// Decodes an encoded suffix in the caller's exclusive shared buffer.
    ///
    /// Uses Global scratch and [`crate::decompress_in_place_with_options`]'s
    /// layout/provisional-output contract. The dictionary is independent of this
    /// buffer. Even an invalid input_start consumes a pending prefix.
    pub fn decompress_in_place(&mut self, buffer: &mut [u8], input_start: usize) -> Result<usize> {
        self.decompress_in_place_in(buffer, input_start, Global)
    }

    /// Decodes in place with an independent per-call scratch allocator.
    ///
    /// Ownership and consumption match [`Self::decompress_into_in`]; buffer bounds
    /// and unchanged successful suffix match the existing in-place engine.
    pub fn decompress_in_place_in<A: Allocator + Clone>(
        &mut self,
        buffer: &mut [u8],
        input_start: usize,
        allocator: A,
    ) -> Result<usize> {
        let options = self.options;
        let call = self.selection.claim();
        match call.retained() {
            Some(dictionary) => crate::decompress_in_place_with_dictionary_in(
                buffer,
                input_start,
                dictionary,
                options,
                allocator,
            ),
            None => {
                crate::decompress_in_place_with_options_in(buffer, input_start, options, allocator)
            }
        }
    }
}
