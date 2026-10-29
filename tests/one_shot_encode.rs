extern crate alloc;

use alloc::alloc::Global;
use core::alloc::{AllocError, Allocator, Layout};
use core::num::NonZeroU64;
use core::ptr::NonNull;
use zstdoxide::{
    Dictionary, Error, OneShotEncoder, SharedDictionary, compress, compress_with_dictionary_in,
    compression_bound,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

const PREPARED: &[u8] = include_bytes!("upstream/dictionaries/formatted.dict");

fn raw_bytes() -> Vec<u8> {
    let mut state = 0x1234_5678_u32;
    (0..1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect()
}

#[test]
fn public_prefix_emits_exact_selected_bytes_then_returns_to_ordinary_encoding() {
    for dictionary in [
        Dictionary::raw(&raw_bytes()).unwrap(),
        Dictionary::from_bytes(PREPARED).unwrap(),
    ] {
        let input = dictionary.content().repeat(2);
        let selected = compress_with_dictionary_in(&input, &dictionary.view(), Global).unwrap();
        let ordinary = compress(&input).unwrap();
        assert_ne!(
            selected, ordinary,
            "fixture must observe dictionary selection"
        );
        let mut encoder = OneShotEncoder::with_prefix(dictionary.view());
        let mut output = vec![0xa5; compression_bound(input.len()).unwrap() + 2];
        assert!(encoder.prefix_pending());
        assert_eq!(
            encoder.compress_into(&input, &mut output[1..]),
            Ok(selected.len())
        );
        assert_eq!(&output[1..1 + selected.len()], selected);
        assert_eq!(output[0], 0xa5);
        assert!(output[1 + selected.len()..].iter().all(|&b| b == 0xa5));
        assert!(!encoder.prefix_pending());
        assert!(encoder.dictionary().is_none());
        output.fill(0xa5);
        assert_eq!(
            encoder.compress_into(&input, &mut output[1..]),
            Ok(ordinary.len())
        );
        assert_eq!(&output[1..1 + ordinary.len()], ordinary);
        assert!(output[1 + ordinary.len()..].iter().all(|&b| b == 0xa5));
        let mut fresh = OneShotEncoder::default();
        assert!(fresh.dictionary().is_none());
        assert_eq!(
            fresh.compress_into(&input, &mut output[1..]),
            Ok(ordinary.len())
        );
    }
}

#[test]
fn public_every_short_attempt_consumes_prefix_preserves_guards_and_recovers() {
    let dictionary = Dictionary::from_bytes(PREPARED).unwrap();
    for input in [&b""[..], &b"dictionary prefix capacity check"[..]] {
        let expected = compress_with_dictionary_in(input, &dictionary.view(), Global).unwrap();
        let ordinary = compress(input).unwrap();
        let mut encoder = OneShotEncoder::without_dictionary();
        for capacity in 0..expected.len() {
            encoder.set_prefix(Some(dictionary.view()));
            let mut short = vec![0xa5; capacity + 2];
            assert_eq!(
                encoder.compress_into(input, &mut short[1..1 + capacity]),
                Err(Error::ResourceLimit)
            );
            assert_eq!((short[0], short[capacity + 1]), (0xa5, 0xa5));
            assert!(!encoder.prefix_pending());
            assert!(encoder.dictionary().is_none());
            let mut recovered = vec![0xa5; ordinary.len() + 2];
            assert_eq!(
                encoder.compress_into(input, &mut recovered[1..1 + ordinary.len()]),
                Ok(ordinary.len())
            );
            assert_eq!(&recovered[1..1 + ordinary.len()], ordinary);
            assert_eq!((recovered[0], recovered[ordinary.len() + 1]), (0xa5, 0xa5));
            assert_eq!(dictionary.as_bytes(), PREPARED);
        }
    }
}

#[test]
fn public_persistent_replacement_and_clear_keep_exact_shared_owners() {
    let a = Dictionary::raw(&raw_bytes()).unwrap().into_shared();
    let b = Dictionary::from_bytes(PREPARED).unwrap().into_shared();
    let mut encoder = OneShotEncoder::with_dictionary(a.clone());
    let input = a.content().repeat(2);
    let a_frame = compress_with_dictionary_in(&input, &a.view(), Global).unwrap();
    let b_frame = compress_with_dictionary_in(&input, &b.view(), Global).unwrap();
    let mut output = vec![0; compression_bound(input.len()).unwrap()];
    for _ in 0..3 {
        assert_eq!(
            encoder.compress_into(&input, &mut []),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            encoder.compress_into(&input, &mut output),
            Ok(a_frame.len())
        );
        assert_eq!(&output[..a_frame.len()], a_frame);
        assert_eq!(SharedDictionary::strong_count(&a), 2);
    }
    encoder.set_prefix(Some(b.clone()));
    assert_eq!(SharedDictionary::strong_count(&a), 1);
    assert_eq!(SharedDictionary::strong_count(&b), 2);
    assert_eq!(
        encoder.compress_into(&input, &mut output),
        Ok(b_frame.len())
    );
    assert_eq!(&output[..b_frame.len()], b_frame);
    assert_eq!(SharedDictionary::strong_count(&b), 1);
    encoder.set_prefix(Some(a.clone()));
    encoder.set_dictionary(Some(b.clone()));
    assert_eq!(SharedDictionary::strong_count(&a), 1);
    assert!(!encoder.prefix_pending());
    assert_eq!(
        encoder.compress_into(&input, &mut output),
        Ok(b_frame.len())
    );
    assert_eq!(&output[..b_frame.len()], b_frame);
    encoder.set_prefix(None);
    assert!(encoder.dictionary().is_none());
    assert_eq!(SharedDictionary::strong_count(&b), 1);
}

#[test]
fn public_every_selected_failure_keeps_dictionary_and_scratch_domains_independent() {
    for prepared in [false, true] {
        let bytes = if prepared {
            PREPARED.to_vec()
        } else {
            raw_bytes()
        };
        let dictionary_owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let dictionary_monitor = dictionary_owner.monitor();
        let dictionary = if prepared {
            Dictionary::from_bytes_in(&bytes, &dictionary_owner)
        } else {
            Dictionary::raw_in(&bytes, &dictionary_owner)
        }
        .unwrap();
        let before = dictionary_monitor.snapshot();
        let input = dictionary.content().repeat(2);
        let expected = compress_with_dictionary_in(&input, &dictionary.view(), Global).unwrap();
        for prefix in [false, true] {
            let mut encoder = if prefix {
                OneShotEncoder::with_prefix(dictionary.view())
            } else {
                OneShotEncoder::with_dictionary(dictionary.view())
            };
            let mut output = vec![0xa5; expected.len() + 2];
            let owner = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let monitor = owner.monitor();
            assert_eq!(
                encoder.compress_into_in(&input, &mut output[1..1 + expected.len()], &owner),
                Ok(expected.len())
            );
            assert_eq!(&output[1..1 + expected.len()], expected);
            let attempts = monitor.snapshot().attempts;
            assert!(attempts > 0);
            monitor.assert_released();
            for ordinal in 1..=attempts {
                if prefix {
                    encoder.set_prefix(Some(dictionary.view()));
                }
                let owner = TrackingAllocator::new(AllocatorConfig::default())
                    .unwrap()
                    .into_non_clone();
                let monitor = owner.monitor();
                monitor.set_fail_at(NonZeroU64::new(ordinal));
                output.fill(0xa5);
                assert_eq!(
                    encoder.compress_into_in(&input, &mut output[1..1 + expected.len()], &owner),
                    Err(Error::AllocationFailed)
                );
                assert_eq!(encoder.dictionary().is_some(), !prefix);
                assert!(!encoder.prefix_pending());
                assert_eq!(monitor.snapshot().attempts, ordinal);
                assert_eq!(monitor.snapshot().failures, 1);
                assert_eq!(monitor.snapshot().identities_created, 1);
                monitor.assert_released();
                assert_eq!((output[0], output[expected.len() + 1]), (0xa5, 0xa5));
                assert_eq!(dictionary_monitor.snapshot(), before);
                assert_eq!(dictionary.as_bytes(), bytes);
            }
        }
        drop(dictionary);
        dictionary_monitor.assert_released();
    }
}

struct PanicClone;
impl Clone for PanicClone {
    fn clone(&self) -> Self {
        panic!("intentional caller allocator clone panic")
    }
}

// SAFETY: Every successful allocation comes from Global and remains valid when
// this zero-sized wrapper moves. Every deallocation forwards the identical
// pointer and layout to Global. No wrapper state affects allocation ownership,
// alignment, bounds, initialization, aliasing or lifetime. Clone never returns.
unsafe impl Allocator for PanicClone {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        Global.allocate(layout)
    }
    /// # Safety
    /// The pointer must identify a live allocation returned by this allocator,
    /// with a layout fitting that block. It must satisfy Global's alignment and
    /// allocation provenance rules. The caller must end all access/borrows and
    /// must not use or free the block after this call. Contents need not be
    /// initialized; deallocation does not read them.
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: The caller supplies a live block allocated through this
        // wrapper with a fitting layout and no surviving access. allocate uses
        // Global unchanged, so the same provenance, bounds, alignment and
        // lifetime satisfy Global::deallocate; the block is released once.
        unsafe { Global.deallocate(ptr, layout) }
    }
}

#[test]
fn public_allocator_unwind_consumes_only_prefix_and_preserves_output_guards() {
    for prefix in [false, true] {
        let dictionary = Dictionary::from_bytes(PREPARED).unwrap().into_shared();
        let mut encoder = if prefix {
            OneShotEncoder::with_prefix(dictionary.clone())
        } else {
            OneShotEncoder::with_dictionary(dictionary.clone())
        };
        let mut output = vec![0xa5; 258];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = encoder.compress_into_in(b"allocator unwind", &mut output[1..257], PanicClone);
        }));
        assert!(result.is_err());
        assert!(!encoder.prefix_pending());
        assert_eq!(encoder.dictionary().is_some(), !prefix);
        assert_eq!(
            SharedDictionary::strong_count(&dictionary),
            if prefix { 1 } else { 2 }
        );
        assert_eq!((output[0], output[257]), (0xa5, 0xa5));
        assert_eq!(dictionary.as_bytes(), PREPARED);
        let expected = if prefix {
            compress(b"allocator unwind").unwrap()
        } else {
            compress_with_dictionary_in(b"allocator unwind", &dictionary.view(), Global).unwrap()
        };
        assert_eq!(
            encoder.compress_into(b"allocator unwind", &mut output[1..257]),
            Ok(expected.len())
        );
        assert_eq!(&output[1..1 + expected.len()], expected);
    }
}

#[test]
fn public_prefix_spans_all_blocks_of_one_frame_then_clears() {
    for dictionary in [
        Dictionary::raw(&raw_bytes()).unwrap(),
        Dictionary::from_bytes(PREPARED).unwrap(),
    ] {
        let content = dictionary.content();
        let input: Vec<_> = (0..262_161).map(|i| content[i % content.len()]).collect();
        let expected = compress_with_dictionary_in(&input, &dictionary.view(), Global).unwrap();
        let mut encoder = OneShotEncoder::with_prefix(dictionary.view());
        let mut output = vec![0xa5; compression_bound(input.len()).unwrap() + 2];
        let size = encoder.compress_into(&input, &mut output[1..]).unwrap();
        assert_eq!(&output[1..1 + size], expected);
        assert_eq!(output[0], 0xa5);
        assert!(output[1 + size..].iter().all(|&b| b == 0xa5));
        assert!(!encoder.prefix_pending());
        assert!(encoder.dictionary().is_none());
        assert_eq!(
            zstdoxide::decompress_with_dictionary_in(
                &expected,
                &dictionary.view(),
                zstdoxide::DecodeOptions::new(input.len()),
                Global
            )
            .unwrap(),
            input
        );
        let ordinary = compress(&input).unwrap();
        let size = encoder.compress_into(&input, &mut output[1..]).unwrap();
        assert_eq!(&output[1..1 + size], ordinary);
    }
}
