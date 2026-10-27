//! Public scalar-bound and borrowed-serialization contracts.
#![forbid(unsafe_code)]
use zstdoxide::{
    CompressionParameterRequest, DecodeOptions, Dictionary, Error, FrameKind, compress,
    compress_at_level, compress_with_dictionary, compress_with_parameters, compression_bound,
    decompress, decompress_with_dictionary, decompressed_size, decompression_bound, frame_info,
    skippable_payload, write_skippable_frame,
};

fn noise(size: usize) -> Vec<u8> {
    let mut state = 0x83a7_5419_u32;
    (0..size)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect()
}

#[test]
fn ordinary_and_dictionary_one_shot_outputs_fit_checked_bound() {
    let dictionaries = [
        Dictionary::raw(b"dictionary bytes repeat dictionary bytes")
            .unwrap()
            .into_shared(),
        Dictionary::from_bytes(include_bytes!("upstream/dictionaries/formatted.dict"))
            .unwrap()
            .into_shared(),
    ];
    for size in [0, 1, 255, 256, 257, 131071, 131072, 131073, 262145] {
        for input in [noise(size), vec![42; size]] {
            let bound = compression_bound(input.len()).unwrap();
            let frame = compress(&input).unwrap();
            assert!(frame.len() <= bound);
            assert_eq!(decompress(&frame, size).unwrap(), input);
            for dictionary in &dictionaries {
                let frame = compress_with_dictionary(&input, dictionary).unwrap();
                assert!(frame.len() <= bound);
                assert_eq!(
                    decompress_with_dictionary(&frame, dictionary, DecodeOptions::new(size))
                        .unwrap(),
                    input
                );
            }
        }
    }
}

#[test]
fn admitted_reference_profiles_and_small_window_raw_fallback_fit_bound() {
    for input in [noise(1024), vec![42; 1024]] {
        for level in [1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 16, 17] {
            let frame = compress_at_level(&input, level).unwrap();
            assert!(frame.len() <= compression_bound(input.len()).unwrap());
            assert_eq!(decompress(&frame, input.len()).unwrap(), input);
        }
    }
    let mut request = CompressionParameterRequest::with_level(1);
    request.set_window_log(10).unwrap();
    request.set_hash_log(10).unwrap();
    request.set_min_match(4).unwrap();
    let input = noise(131073);
    let frame = compress_with_parameters(&input, &request).unwrap();
    assert!(frame.len() <= compression_bound(input.len()).unwrap());
    assert_eq!(decompress(&frame, input.len()).unwrap(), input);
}

#[test]
fn real_guards_exact_capacity_and_all_variants_roundtrip() {
    for variant in 0..16 {
        for size in [0, 1, 7, 8, 9, 4096, 131073] {
            let payload = noise(size);
            for extra in [0, 17] {
                let capacity = size + 8 + extra;
                let mut guarded = vec![0xa5; capacity + 32];
                let written =
                    write_skippable_frame(&mut guarded[16..16 + capacity], &payload, variant)
                        .unwrap();
                assert_eq!(written, size + 8);
                assert!(guarded[..16].iter().all(|&b| b == 0xa5));
                assert!(guarded[16 + written..].iter().all(|&b| b == 0xa5));
                let frame = &guarded[16..16 + written];
                let (found, borrowed) = skippable_payload(frame).unwrap();
                assert_eq!(found, variant);
                assert_eq!(borrowed, payload);
                assert_eq!(borrowed.as_ptr(), frame[8..].as_ptr());
                assert_eq!(
                    frame_info(frame).unwrap().kind(),
                    FrameKind::Skippable {
                        variant,
                        payload_size: size as u32
                    }
                );
                assert_eq!(decompressed_size(frame), Ok(Some(0)));
                assert_eq!(decompression_bound(frame), Ok(0));
                assert!(decompress(frame, 0).unwrap().is_empty());
            }
        }
    }
}

#[test]
fn every_short_destination_and_invalid_variant_preserves_all_bytes() {
    let payload = noise(257);
    for capacity in 0..payload.len() + 8 {
        let mut output = vec![0xa5; capacity + 32];
        assert_eq!(
            write_skippable_frame(&mut output[16..16 + capacity], &payload, 15),
            Err(Error::ResourceLimit)
        );
        assert!(output.iter().all(|&b| b == 0xa5));
    }
    for variant in 16..=255 {
        for capacity in [0, 1, payload.len() + 8] {
            let mut output = vec![0xa5; capacity + 32];
            assert_eq!(
                write_skippable_frame(&mut output[16..16 + capacity], &payload, variant),
                Err(Error::InvalidParameter)
            );
            assert!(output.iter().all(|&b| b == 0xa5));
        }
    }
}

#[test]
fn skippable_writer_composes_with_standard_frame_concatenation() {
    let mut stream = compress(b"first").unwrap();
    for variant in 0..16 {
        let mut frame = [0_u8; 11];
        assert_eq!(write_skippable_frame(&mut frame, b"tag", variant), Ok(11));
        stream.extend_from_slice(&frame);
    }
    stream.extend_from_slice(&compress(b"second").unwrap());
    assert_eq!(decompressed_size(&stream), Ok(Some(11)));
    assert_eq!(decompression_bound(&stream), Ok(11));
    assert_eq!(decompress(&stream, 11).unwrap(), b"firstsecond");
}
