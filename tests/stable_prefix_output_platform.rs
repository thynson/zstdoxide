//! Foreign-width/endian checked dictionary destinations and real compressed history.
#![forbid(unsafe_code)]
use std::alloc::Global;
use zstdoxide::{DecodeOptions, Dictionary, DictionaryHandle, Error, SliceDecoder, StreamStatus};
fn stream_decode<D: DictionaryHandle + Copy>(
    input: &[u8],
    output: &mut [u8],
    dictionary: D,
    options: DecodeOptions,
    allocator: Global,
) -> Result<usize, Error> {
    let mut decoder = SliceDecoder::with_prefix_in(output, options, dictionary, allocator);
    for _ in 0..2 {
        for fragment in input.chunks(7) {
            let mut offset = 0;
            while offset < fragment.len() {
                let progress = decoder.process(&fragment[offset..])?;
                assert!(progress.consumed > 0 || progress.produced > 0);
                offset += progress.consumed;
                if progress.status == StreamStatus::FrameFinished
                    && decoder.total_input() < input.len() as u64
                {
                    decoder.set_prefix(dictionary)?;
                }
            }
        }
        decoder.finish()?;
        assert_eq!(decoder.total_input(), input.len() as u64);
        let written = decoder.output()?.len();
        assert_eq!(decoder.dictionary_retained_capacity(), 0);
        decoder.reset();
        decoder.set_prefix(dictionary)?;
        assert_eq!(decoder.total_output(), 0);
        if written == 0 {
            return Ok(0);
        }
    }
    // Decode once more so the return value reflects a completed session.
    let mut offset = 0;
    while offset < input.len() {
        let progress = decoder.process(&input[offset..])?;
        offset += progress.consumed;
        if progress.status == StreamStatus::FrameFinished && offset < input.len() {
            decoder.set_prefix(dictionary)?;
        }
    }
    decoder.finish()?;
    Ok(decoder.output()?.len())
}
macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!("upstream/dictionaries/", $name))
    };
}
#[test]
fn borrowed_prefix_matches_and_boundary_replacement_preserve_guards() {
    let cases: &[(&[u8], &[u8], &[u8])] = &[
        (
            fixture!("boundary-dictionary.raw"),
            fixture!("p0-far-dictionary.zst"),
            fixture!("p0-far-dictionary.expected"),
        ),
        (
            fixture!("boundary-dictionary.raw"),
            fixture!("dictionary-to-frame-eviction.zst"),
            fixture!("dictionary-to-frame-eviction.expected"),
        ),
        (
            fixture!("formatted.dict"),
            fixture!("dictionary-treeless-four-streams.zst"),
            fixture!("dictionary-treeless-four-streams.expected"),
        ),
    ];
    for &(dictionary, frame, expected) in cases {
        let dictionary = Dictionary::from_bytes(dictionary).unwrap();
        let mut output = vec![0xa5; expected.len() + 18];
        assert_eq!(
            stream_decode(
                frame,
                &mut output[1..expected.len() + 17],
                dictionary.view(),
                DecodeOptions::new(expected.len()),
                Global
            ),
            Ok(expected.len())
        );
        assert_eq!(&output[1..expected.len() + 1], expected);
        assert_eq!(output[0], 0xa5);
        assert!(output[expected.len() + 1..].iter().all(|&b| b == 0xa5));
        output.fill(0xa5);
        assert_eq!(
            stream_decode(
                frame,
                &mut output[1..expected.len()],
                dictionary.view(),
                DecodeOptions::new(usize::MAX),
                Global
            ),
            Err(Error::ResourceLimit)
        );
        assert_eq!(output[0], 0xa5);
        assert!(output[expected.len()..].iter().all(|&b| b == 0xa5));
        let combined = [frame, frame].concat();
        let originals = [expected, expected].concat();
        let mut output = vec![0xa5; originals.len() + 2];
        assert_eq!(
            stream_decode(
                &combined,
                &mut output[1..originals.len() + 1],
                dictionary.view(),
                DecodeOptions::new(originals.len()),
                Global
            ),
            Ok(originals.len())
        );
        assert_eq!(&output[1..originals.len() + 1], originals);
        assert_eq!(output[0], 0xa5);
        assert_eq!(output[originals.len() + 1], 0xa5);
    }
    let dictionary = Dictionary::raw(fixture!("boundary-dictionary.raw")).unwrap();
    for frame in [
        fixture!("p-window-plus-one.zst").as_slice(),
        fixture!("dictionary-out-of-bounds.zst").as_slice(),
    ] {
        let mut output = [0xa5; 4098];
        assert_eq!(
            stream_decode(
                frame,
                &mut output[1..4097],
                dictionary.view(),
                DecodeOptions::new(4096),
                Global
            ),
            Err(Error::CorruptData)
        );
        assert_eq!(output[0], 0xa5);
        assert_eq!(output[4097], 0xa5);
    }
}
