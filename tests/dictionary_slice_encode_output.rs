extern crate alloc;

use alloc::alloc::Global;
use core::num::NonZeroU64;
use zstdoxide::{
    DecodeOptions, Dictionary, Error, FrameHeader, SharedDictionary, compress_into_with_dictionary,
    compress_into_with_dictionary_in, compress_with_dictionary, compress_with_dictionary_in,
    compression_bound, decompress_with_dictionary,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

const PREPARED: &[u8] = include_bytes!("upstream/dictionaries/formatted.dict");

#[test]
fn public_raw_and_prepared_frames_match_existing_output_and_preserve_suffix() {
    let mut dictionaries = vec![
        Dictionary::raw(b"").unwrap(),
        Dictionary::raw(b"pattern ").unwrap(),
    ];
    for id in [0, 1, 255, 256, 65_535, 65_536, u32::MAX] {
        let mut bytes = PREPARED.to_vec();
        bytes[4..8].copy_from_slice(&id.to_le_bytes());
        dictionaries.push(Dictionary::from_bytes(&bytes).unwrap());
    }
    for dictionary in dictionaries {
        let dictionary = dictionary.into_shared();
        let before = dictionary.as_bytes().to_vec();
        for input in [&b""[..], &b"pattern pattern pattern pattern"[..]] {
            let expected = compress_with_dictionary(input, &dictionary).unwrap();
            let bound = compression_bound(input.len()).unwrap();
            assert!(expected.len() <= bound);
            for capacity in [expected.len(), bound] {
                let mut output = vec![0xa5; capacity + 2];
                let size =
                    compress_into_with_dictionary(input, &mut output[1..1 + capacity], &dictionary)
                        .unwrap();
                assert_eq!(size, expected.len());
                assert_eq!(&output[1..1 + size], expected);
                assert_eq!(
                    FrameHeader::parse(&expected).unwrap().0.dictionary_id,
                    dictionary.id()
                );
                assert_eq!(output[0], 0xa5);
                assert!(output[1 + size..].iter().all(|&b| b == 0xa5));
                assert_eq!(
                    decompress_with_dictionary(
                        &expected,
                        &dictionary,
                        DecodeOptions::new(input.len())
                    )
                    .unwrap(),
                    input
                );
                assert_eq!(dictionary.as_bytes(), before);
                assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
            }
        }
    }
}

#[test]
fn public_short_destinations_preserve_guards_and_recover_on_a_fresh_call() {
    let dictionary = Dictionary::from_bytes(PREPARED).unwrap().into_shared();
    let before = dictionary.as_bytes().to_vec();
    let input = b"a dictionary backed caller output a dictionary backed caller output";
    let expected = compress_with_dictionary(input, &dictionary).unwrap();
    for capacity in 0..expected.len() {
        let mut output = vec![0xa5; capacity + 2];
        assert_eq!(
            compress_into_with_dictionary(input, &mut output[1..1 + capacity], &dictionary),
            Err(Error::ResourceLimit)
        );
        assert_eq!((output[0], output[capacity + 1]), (0xa5, 0xa5));
        assert_eq!(dictionary.as_bytes(), before);
    }
    let mut output = vec![0; expected.len()];
    assert_eq!(
        compress_into_with_dictionary(input, &mut output, &dictionary),
        Ok(expected.len())
    );
    assert_eq!(output, expected);
}

#[test]
fn public_borrowed_view_keeps_nonclone_dictionary_and_scratch_owners_independent() {
    let dictionary_owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let dictionary_monitor = dictionary_owner.monitor();
    let scratch_owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let scratch_monitor = scratch_owner.monitor();
    let dictionary = Dictionary::from_bytes_in(PREPARED, &dictionary_owner).unwrap();
    let before = dictionary_monitor.snapshot();
    let input = dictionary.content().repeat(2);
    let expected = compress_with_dictionary_in(&input, &dictionary.view(), Global).unwrap();
    let mut output = vec![0xa5; expected.len() + 2];
    scratch_monitor.set_fail_at(NonZeroU64::new(1));
    assert_eq!(
        compress_into_with_dictionary_in(
            &input,
            &mut output[1..1 + expected.len()],
            &dictionary.view(),
            &scratch_owner
        ),
        Err(Error::AllocationFailed)
    );
    scratch_monitor.assert_released();
    assert_eq!((output[0], output[expected.len() + 1]), (0xa5, 0xa5));
    assert_eq!(dictionary_monitor.snapshot(), before);
    scratch_monitor.clear_failure();
    assert_eq!(
        compress_into_with_dictionary_in(
            &input,
            &mut output[1..1 + expected.len()],
            &dictionary.view(),
            &scratch_owner
        ),
        Ok(expected.len())
    );
    assert_eq!(&output[1..1 + expected.len()], expected);
    assert_eq!(dictionary_monitor.snapshot(), before);
    assert_eq!(dictionary.as_bytes(), PREPARED);
    assert_eq!(scratch_monitor.snapshot().identities_created, 1);
    scratch_monitor.assert_released();
    drop(dictionary);
    dictionary_monitor.assert_released();
}

#[test]
fn public_header_rejection_precedes_scratch_allocation_for_every_id_width() {
    for id in [0u32, 1, 256, 65_536] {
        let mut bytes = PREPARED.to_vec();
        bytes[4..8].copy_from_slice(&id.to_le_bytes());
        let dictionary = Dictionary::from_bytes(&bytes).unwrap();
        let expected = compress_with_dictionary_in(b"", &dictionary.view(), Global).unwrap();
        let header_len = FrameHeader::parse(&expected).unwrap().1;
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut output = vec![0xa5; header_len - 1];
        assert_eq!(
            compress_into_with_dictionary_in(b"", &mut output, &dictionary.view(), owner),
            Err(Error::ResourceLimit)
        );
        assert!(output.iter().all(|&b| b == 0xa5));
        assert_eq!(monitor.snapshot().attempts, 0);
        monitor.assert_released();
    }
}

#[test]
fn public_multiblock_borrowed_selection_emits_exact_existing_bytes() {
    for dictionary in [
        Dictionary::raw(b"pattern ").unwrap(),
        Dictionary::from_bytes(PREPARED).unwrap(),
    ] {
        let content = dictionary.content();
        let input: Vec<_> = (0..262_161).map(|i| content[i % content.len()]).collect();
        let expected = compress_with_dictionary_in(&input, &dictionary.view(), Global).unwrap();
        let mut output = vec![0xa5; compression_bound(input.len()).unwrap()];
        let size =
            compress_into_with_dictionary_in(&input, &mut output, &dictionary.view(), Global)
                .unwrap();
        assert_eq!(&output[..size], expected);
        assert!(output[size..].iter().all(|&b| b == 0xa5));
    }
}
