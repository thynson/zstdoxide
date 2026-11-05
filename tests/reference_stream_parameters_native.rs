//! Export/replay protocol for the external pinned streaming oracle.
#![forbid(unsafe_code)]
use std::{alloc::Global, fs, path::PathBuf};
use zstdoxide::{
    CompiledStreamOptions, CompressionParameterRequest, DecodeOptions, Dictionary, EndDirective,
    Error, FrameHeader, ReferenceStrategy, ReferenceStreamEncoder, StreamStatus, decompress,
    decompress_with_dictionary_in,
};

#[test]
#[ignore = "requires the external pinned v1.5.7 reference-stream parameter campaign"]
fn reference_stream_parameter_native_schedules() {
    let root = PathBuf::from(
        std::env::var_os("ZSTDOXIDE_REFERENCE_PARAMETER_NATIVE").expect("campaign path"),
    );
    let export = std::env::var_os("ZSTDOXIDE_REFERENCE_PARAMETER_EXPORT").is_some();
    let raw: Vec<u8> = (0..8192).map(|i| ((i * 97 + i / 43) % 251) as u8).collect();
    let prepared = include_bytes!("upstream/dictionaries/formatted.dict");
    let text = include_bytes!("upstream/dictionaries/dictionary-treeless-literals.expected");
    let raw_dict = Dictionary::raw(&raw).unwrap();
    let prepared_dict = Dictionary::from_bytes(prepared).unwrap();
    if export {
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("raw.dict"), &raw).unwrap();
        fs::write(root.join("prepared.dict"), prepared).unwrap();
    }
    let mut manifest = String::new();
    for (index, strategy) in [
        ReferenceStrategy::Fast,
        ReferenceStrategy::DoubleFast,
        ReferenceStrategy::Greedy,
        ReferenceStrategy::Lazy,
        ReferenceStrategy::Lazy2,
        ReferenceStrategy::BinaryTreeLazy2,
    ]
    .into_iter()
    .enumerate()
    {
        let mut req = CompressionParameterRequest::with_level(3);
        req.set_strategy(Some(strategy));
        req.set_window_log(17).unwrap();
        req.set_hash_log(14).unwrap();
        req.set_chain_log(14).unwrap();
        for scenario in 0..6 {
            let source = match scenario {
                0 => Vec::new(),
                1 => raw[..513].to_vec(),
                2 => b"hidden pledge spanning a size-field boundary".repeat(1600),
                3 => raw.repeat(20),
                4 => raw[6000..6513].to_vec(),
                _ => text.to_vec(),
            };
            let prefix = match scenario {
                4 => Some(raw_dict.view()),
                5 => Some(prepared_dict.view()),
                _ => None,
            };
            let options = CompiledStreamOptions {
                pledged_size: (scenario != 3).then_some(source.len() as u64),
                include_content_size: scenario != 2,
                checksum: true,
                ..Default::default()
            };
            let mut encoder = if let Some(prefix) = prefix {
                ReferenceStreamEncoder::with_prefix(req, options, prefix).unwrap()
            } else {
                ReferenceStreamEncoder::with_options(req, options).unwrap()
            };
            match scenario {
                0 => {
                    encoder.reset_parameters().unwrap();
                }
                1 => {
                    encoder.reset_all();
                }
                2 | 5 => {
                    encoder
                        .process(&[], &mut [], EndDirective::Continue)
                        .unwrap();
                    assert_eq!(encoder.set_request(req), Err(Error::InvalidState));
                    assert_eq!(encoder.reset_parameters(), Err(Error::InvalidState));
                    encoder.reset_all();
                    if let Some(prefix) = prefix {
                        encoder.set_prefix(prefix).unwrap();
                    }
                }
                3 => {
                    let mut alternate = req;
                    alternate.set_strategy(Some(ReferenceStrategy::Fast));
                    encoder.set_request(alternate).unwrap();
                }
                _ => {
                    assert!(encoder.selection().is_some());
                }
            }
            encoder.set_options(options).unwrap();
            encoder.set_request(req).unwrap();
            assert_eq!(encoder.parameters(), None);
            for session in 0..2 {
                let next = b"next frame resolves its own smaller window".repeat(31);
                let input = if session == 0 { &source } else { &next };
                let selected = if session == 0 { prefix } else { None };
                if session == 1 {
                    encoder.reset_parameters().unwrap();
                    assert_eq!(encoder.request(), CompressionParameterRequest::default());
                    assert_eq!(encoder.options(), CompiledStreamOptions::default());
                    assert!(encoder.selection().is_none());
                    encoder
                        .reset_with_options(CompiledStreamOptions {
                            pledged_size: Some(input.len() as u64),
                            checksum: true,
                            ..Default::default()
                        })
                        .unwrap();
                    encoder.set_request(req).unwrap();
                }
                let id = format!("s{}-c{scenario}-f{session}", index + 1);
                let options = encoder.options();
                let kind = if selected.is_none() {
                    0
                } else if scenario == 4 {
                    1
                } else {
                    2
                };
                if export {
                    let mut frame = Vec::new();
                    let mut position = 0;
                    // Explicit Continue initializes both implementations before
                    // Finish, preserving an unknown pledge even for empty input.
                    let mut out = [0xa5; 19];
                    let p = encoder
                        .process(&[], &mut out[1..18], EndDirective::Continue)
                        .unwrap();
                    frame.extend_from_slice(&out[1..1 + p.produced]);
                    loop {
                        let end = (position + 317).min(input.len());
                        let directive = if end == input.len() {
                            EndDirective::Finish
                        } else {
                            EndDirective::Continue
                        };
                        out.fill(0xa5);
                        let p = encoder
                            .process(&input[position..end], &mut out[1..18], directive)
                            .unwrap();
                        assert_eq!(out[0], 0xa5);
                        assert!(out[1 + p.produced..].iter().all(|&b| b == 0xa5));
                        frame.extend_from_slice(&out[1..1 + p.produced]);
                        position += p.consumed;
                        if p.status == StreamStatus::FrameFinished {
                            break;
                        }
                    }
                    assert_eq!(position, input.len());
                    let decoded = if let Some(view) = selected {
                        decompress_with_dictionary_in(
                            &frame,
                            &view,
                            DecodeOptions::new(input.len()),
                            Global,
                        )
                        .unwrap()
                    } else {
                        decompress(&frame, input.len()).unwrap()
                    };
                    assert_eq!(decoded, *input);
                    fs::write(root.join(format!("{id}.input")), input).unwrap();
                    fs::write(root.join(format!("{id}.rust")), &frame).unwrap();
                    manifest.push_str(&format!(
                        "{id}\t{}\t{}\t{}\t{kind}\t{}\n",
                        index + 1,
                        options.pledged_size.map_or(-1, |n| n as i64),
                        u8::from(options.include_content_size),
                        FrameHeader::parse(&frame).unwrap().0.window_size
                    ));
                } else {
                    let frame = fs::read(root.join("outputs").join(format!("{id}.zst"))).unwrap();
                    let decoded = if let Some(view) = selected {
                        decompress_with_dictionary_in(
                            &frame,
                            &view,
                            DecodeOptions::new(input.len()),
                            Global,
                        )
                        .unwrap()
                    } else {
                        decompress(&frame, input.len()).unwrap()
                    };
                    assert_eq!(decoded, *input, "{id}");
                    let header = FrameHeader::parse(&frame).unwrap().0;
                    assert_eq!(
                        header.content_size,
                        options
                            .pledged_size
                            .filter(|_| options.include_content_size),
                        "{id}"
                    );
                    assert!(header.has_checksum);
                }
            }
        }
    }
    if export {
        fs::write(root.join("schedule.tsv"), manifest).unwrap();
    }
}
