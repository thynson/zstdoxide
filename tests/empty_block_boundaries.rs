//! Empty-output block boundaries from the pinned format's mandatory sections.
#![forbid(unsafe_code)]
use zstdoxide::{
    DecodeOptions, Dictionary, Error, SliceDecoder, decompress_into_with_dictionary,
    decompress_into_with_options,
};

#[test]
fn empty_blocks_require_sections_and_respect_the_declared_window() {
    // A compressed block needs a literals header and a sequence-count header.
    // Pinned native stable streaming skips a zero-byte compressed body; native
    // one-shot and Rust reject the missing sections. With known content size
    // zero, the zero window also rules out a two-byte compressed block body.
    let cases: &[(&str, &[u8], bool)] = &[
        (
            "known raw zero",
            &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 1, 0, 0],
            true,
        ),
        (
            "known RLE zero",
            &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 3, 0, 0, 0x41],
            true,
        ),
        (
            "known compressed empty",
            &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 0x15, 0, 0, 0, 0],
            false,
        ),
        (
            "unknown compressed empty",
            &[0x28, 0xb5, 0x2f, 0xfd, 0, 0, 0x15, 0, 0, 0, 0],
            true,
        ),
        (
            "missing compressed sections",
            &[0x28, 0xb5, 0x2f, 0xfd, 0, 0, 5, 0, 0],
            false,
        ),
    ];
    let dictionary = Dictionary::raw(&[]).unwrap().into_shared();
    for &(name, input, valid) in cases {
        for persistent in [false, true] {
            let mut output = [0xa5; 80];
            let options = DecodeOptions::new(16);
            let direct = if persistent {
                decompress_into_with_dictionary(input, &mut output[32..48], &dictionary, options)
            } else {
                decompress_into_with_options(input, &mut output[32..48], options)
            };
            assert_eq!(direct.is_ok(), valid, "{name}, persistent={persistent}");
            assert!(output[..32].iter().chain(&output[48..]).all(|&b| b == 0xa5));
            if valid {
                assert_eq!(direct, Ok(0));
                assert_eq!(output, [0xa5; 80]);
            }
            for chunk in [1, 7, usize::MAX] {
                output.fill(0xa5);
                let mut decoder = if persistent {
                    SliceDecoder::with_dictionary(&mut output[32..48], options, dictionary.clone())
                } else {
                    SliceDecoder::new(&mut output[32..48], options)
                };
                let result = (|| {
                    for fragment in input.chunks(chunk) {
                        let mut offset = 0;
                        while offset < fragment.len() {
                            let progress = decoder.process(&fragment[offset..])?;
                            assert!(progress.consumed > 0 || progress.produced > 0);
                            offset += progress.consumed;
                        }
                    }
                    decoder.finish()?;
                    assert_eq!(decoder.total_input(), input.len() as u64);
                    Ok::<usize, Error>(decoder.output()?.len())
                })();
                assert_eq!(
                    result.is_ok(),
                    valid,
                    "{name}, persistent={persistent}, chunk={chunk}"
                );
                if valid {
                    assert_eq!(result, Ok(0));
                } else {
                    assert_eq!(decoder.output(), Err(Error::InvalidState));
                }
                drop(decoder);
                assert!(output[..32].iter().chain(&output[48..]).all(|&b| b == 0xa5));
                if valid {
                    assert_eq!(output, [0xa5; 80]);
                }
            }
        }
    }
}
