//! Public request-backed streaming geometry, failure and reset contracts.

use zstdoxide::{
    CompiledStreamOptions, CompressionParameterRequest, EndDirective, Error, FrameHeader,
    ReferenceStrategy, ReferenceStreamEncoder, SourceSize, StreamStatus, decompress,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

fn request(strategy: ReferenceStrategy) -> CompressionParameterRequest {
    let mut request = CompressionParameterRequest::with_level(3);
    request.set_strategy(Some(strategy));
    request.set_window_log(17).unwrap();
    request.set_hash_log(14).unwrap();
    request.set_chain_log(14).unwrap();
    request
}

fn pump<A: std::alloc::Allocator + Clone>(
    encoder: &mut ReferenceStreamEncoder<'_, A>,
    input: &[u8],
    chunk: usize,
    output_chunk: usize,
) -> Vec<u8> {
    let mut frame = Vec::new();
    let mut output = vec![0xa5; output_chunk + 2];
    let mut position = 0;
    loop {
        let end = (position + chunk).min(input.len());
        let directive = if end == input.len() {
            EndDirective::Finish
        } else {
            EndDirective::Continue
        };
        let mut consumed = position;
        loop {
            output.fill(0xa5);
            let p = encoder
                .process(
                    &input[consumed..end],
                    &mut output[1..1 + output_chunk],
                    directive,
                )
                .unwrap();
            assert_eq!(output[0], 0xa5);
            assert!(output[1 + p.produced..].iter().all(|&byte| byte == 0xa5));
            consumed += p.consumed;
            frame.extend_from_slice(&output[1..1 + p.produced]);
            match p.status {
                StreamStatus::NeedOutput => {}
                StreamStatus::NeedInput => {
                    assert_eq!(consumed, end);
                    break;
                }
                StreamStatus::FrameFinished => {
                    assert_eq!(consumed, input.len());
                    return frame;
                }
                StreamStatus::Flushed => panic!("unexpected flush"),
            }
        }
        position = end;
    }
}

#[test]
fn six_families_roundtrip_multiple_blocks_and_window_wrap() {
    let source = b"reference streaming history and repeated source ".repeat(6400);
    for strategy in [
        ReferenceStrategy::Fast,
        ReferenceStrategy::DoubleFast,
        ReferenceStrategy::Greedy,
        ReferenceStrategy::Lazy,
        ReferenceStrategy::Lazy2,
        ReferenceStrategy::BinaryTreeLazy2,
    ] {
        let req = request(strategy);
        for pledge in [None, Some(source.len() as u64)] {
            let mut encoder = ReferenceStreamEncoder::with_options(
                req,
                CompiledStreamOptions {
                    checksum: true,
                    pledged_size: pledge,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(encoder.parameters(), None);
            let frame = pump(&mut encoder, &source, 7919, 31);
            assert_eq!(
                decompress(&frame, source.len()).unwrap(),
                source,
                "{strategy:?}"
            );
            assert_eq!(encoder.parameters().unwrap().strategy(), strategy);
            assert_eq!(encoder.total_input(), source.len() as u64);
            assert_eq!(encoder.total_output(), frame.len() as u64);
            let header = FrameHeader::parse(&frame).unwrap().0;
            assert_eq!(header.content_size, pledge);
            assert_eq!(
                header.window_size,
                1 << encoder.parameters().unwrap().window_log()
            );
            assert_eq!(header.dictionary_id, 0);
        }
    }
}

#[test]
fn every_reset_resolves_known_zero_unknown_and_size_boundaries() {
    let req = request(ReferenceStrategy::Fast);
    let mut encoder = ReferenceStreamEncoder::new(req).unwrap();
    for length in [0, 1, 255, 256, 1024, 1025, 65_791, 65_792, 131_073] {
        let source = vec![b'x'; length];
        for include_content_size in [false, true] {
            encoder
                .reset_with_options(CompiledStreamOptions {
                    pledged_size: Some(length as u64),
                    include_content_size,
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(encoder.parameters(), None);
            let frame = pump(&mut encoder, &source, 1031, 1);
            let profile = req.resolve(SourceSize::Known(length as u64), 0).unwrap();
            assert_eq!(encoder.parameters(), Some(profile));
            let header = FrameHeader::parse(&frame).unwrap().0;
            assert_eq!(
                header.content_size,
                include_content_size.then_some(length as u64)
            );
            assert_eq!(header.window_size, 1 << profile.window_log());
            assert_eq!(decompress(&frame, length).unwrap(), source);
        }
    }
    encoder.reset();
    assert_eq!(encoder.parameters(), None);
    assert_eq!(encoder.options().pledged_size, None);
    let frame = pump(&mut encoder, b"unknown", 7, 2);
    assert_eq!(FrameHeader::parse(&frame).unwrap().0.content_size, None);
    assert_eq!(
        encoder.parameters(),
        Some(req.resolve(SourceSize::Unknown, 0).unwrap())
    );
}

#[test]
fn source_hint_changes_geometry_without_creating_a_pledge() {
    let mut req = request(ReferenceStrategy::DoubleFast);
    req.set_source_hint(1024).unwrap();
    let mut encoder = ReferenceStreamEncoder::new(req).unwrap();
    let source = b"a source hint is not an exact pledge".repeat(500);
    let frame = pump(&mut encoder, &source, 101, 7);
    let header = FrameHeader::parse(&frame).unwrap().0;
    assert_eq!(header.window_size, 1024);
    assert_eq!(header.content_size, None);
    assert_eq!(decompress(&frame, source.len()).unwrap(), source);
}

#[test]
fn flush_zero_output_and_finish_latches_preserve_source() {
    let mut encoder = ReferenceStreamEncoder::new(request(ReferenceStrategy::Lazy)).unwrap();
    let p = encoder
        .process(b"before flush", &mut [], EndDirective::Flush)
        .unwrap();
    assert_eq!(p.consumed, 12);
    assert_eq!(p.status, StreamStatus::NeedOutput);
    let mut frame = Vec::new();
    loop {
        let mut byte = [0];
        let p = encoder
            .process(&[], &mut byte, EndDirective::Flush)
            .unwrap();
        frame.extend_from_slice(&byte[..p.produced]);
        if p.status == StreamStatus::Flushed {
            break;
        }
    }
    frame.extend(pump(&mut encoder, b"after flush", 11, 1));
    assert_eq!(decompress(&frame, 23).unwrap(), b"before flushafter flush");
    assert_eq!(
        encoder
            .process(&[], &mut [], EndDirective::Finish)
            .unwrap()
            .status,
        StreamStatus::FrameFinished
    );
    assert_eq!(
        encoder.process(b"x", &mut [], EndDirective::Finish),
        Err(Error::InvalidState)
    );
    encoder.reset();
    encoder
        .process(b"abc", &mut [], EndDirective::Finish)
        .unwrap();
    assert_eq!(
        encoder.process(&[], &mut [], EndDirective::Continue),
        Err(Error::InvalidState)
    );
    encoder.reset();
    assert_eq!(
        decompress(&pump(&mut encoder, b"recovered", 4, 2), 9).unwrap(),
        b"recovered"
    );
}

#[test]
fn pledge_mismatch_poison_and_reset_are_explicit() {
    let mut encoder = ReferenceStreamEncoder::with_options(
        request(ReferenceStrategy::Fast),
        CompiledStreamOptions {
            pledged_size: Some(3),
            ..Default::default()
        },
    )
    .unwrap();
    let mut output = [0xa5; 30];
    assert_eq!(
        encoder.process(b"ab", &mut output, EndDirective::Finish),
        Err(Error::ContentSizeMismatch)
    );
    assert_eq!(output, [0xa5; 30]);
    assert_eq!(
        encoder.process(b"abc", &mut output, EndDirective::Finish),
        Err(Error::InvalidState)
    );
    encoder
        .reset_with_options(CompiledStreamOptions {
            pledged_size: Some(3),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        encoder.process(b"abcd", &mut output, EndDirective::Continue),
        Err(Error::ContentSizeMismatch)
    );
    encoder.reset();
    assert_eq!(
        decompress(&pump(&mut encoder, b"ok", 2, 3), 2).unwrap(),
        b"ok"
    );
}

#[test]
fn unsupported_strategies_and_large_profiles_allocate_nothing() {
    let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = allocator.monitor();
    for strategy in [
        ReferenceStrategy::Optimal,
        ReferenceStrategy::Ultra,
        ReferenceStrategy::Ultra2,
    ] {
        assert!(ReferenceStreamEncoder::new_in(request(strategy), allocator.clone()).is_err());
    }
    let mut req = request(ReferenceStrategy::Fast);
    req.set_window_log(21).unwrap();
    assert!(ReferenceStreamEncoder::new_in(req, allocator.clone()).is_err());
    assert_eq!(monitor.snapshot().attempts, 0);
    monitor.assert_released();
}

#[test]
fn deferred_profile_failure_precedes_header_and_allocation() {
    let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = allocator.monitor();
    let mut req = request(ReferenceStrategy::Fast);
    req.set_window_log(21).unwrap();
    let mut encoder = ReferenceStreamEncoder::with_options_in(
        req,
        CompiledStreamOptions {
            pledged_size: Some(1),
            ..Default::default()
        },
        allocator,
    )
    .unwrap();
    encoder.reset();
    let mut output = [0xa5; 30];
    assert!(
        encoder
            .process(b"a", &mut output, EndDirective::Finish)
            .is_err()
    );
    assert_eq!(output, [0xa5; 30]);
    assert_eq!(encoder.parameters(), None);
    assert_eq!(monitor.snapshot().attempts, 0);
    encoder
        .reset_with_options(CompiledStreamOptions {
            pledged_size: Some(1),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        decompress(&pump(&mut encoder, b"a", 1, 1), 1).unwrap(),
        b"a"
    );
    drop(encoder);
    monitor.assert_released();
}

#[test]
fn every_workspace_allocation_failure_releases_and_recovers() {
    let source = b"fallible reference stream scratch and entropy".repeat(100);
    let req = request(ReferenceStrategy::Greedy);
    let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = allocator.monitor();
    let mut encoder = ReferenceStreamEncoder::new_in(req, allocator).unwrap();
    let expected = pump(&mut encoder, &source, source.len(), source.len() + 100);
    let attempts = monitor.snapshot().attempts;
    assert!(attempts > 4);
    drop(encoder);
    monitor.assert_released();
    for ordinal in 1..=attempts {
        let allocator = TrackingAllocator::new(AllocatorConfig {
            fail_at: std::num::NonZeroU64::new(ordinal),
            ..Default::default()
        })
        .unwrap();
        let monitor = allocator.monitor();
        let mut encoder = ReferenceStreamEncoder::new_in(req, allocator).unwrap();
        let mut output = vec![0xa5; source.len() + 100];
        assert_eq!(
            encoder.process(&source, &mut output, EndDirective::Finish),
            Err(Error::AllocationFailed),
            "allocation {ordinal}"
        );
        assert_eq!(
            encoder.process(&[], &mut output, EndDirective::Finish),
            Err(Error::InvalidState)
        );
        monitor.clear_failure();
        encoder.reset();
        assert_eq!(
            pump(&mut encoder, &source, source.len(), output.len()),
            expected
        );
        drop(encoder);
        monitor.assert_released();
    }
}

#[test]
fn prefix_only_construction_resolves_window_then_consumes_prefix_once() {
    use zstdoxide::{DecodeOptions, Dictionary, decompress_with_dictionary_in};
    let content: Vec<u8> = (0..8192).map(|i| ((i * 97 + i / 43) % 251) as u8).collect();
    let dictionary = Dictionary::raw(&content).unwrap();
    let source = &content[6000..6513];
    for strategy in [
        ReferenceStrategy::Fast,
        ReferenceStrategy::DoubleFast,
        ReferenceStrategy::Greedy,
        ReferenceStrategy::Lazy,
        ReferenceStrategy::Lazy2,
        ReferenceStrategy::BinaryTreeLazy2,
    ] {
        let req = request(strategy);
        let options = CompiledStreamOptions {
            pledged_size: Some(source.len() as u64),
            ..Default::default()
        };
        let mut encoder =
            ReferenceStreamEncoder::with_prefix(req, options, dictionary.view()).unwrap();
        assert!(encoder.selection().is_some());
        let frame = pump(&mut encoder, source, 57, 3);
        let expected = req
            .resolve(SourceSize::Known(source.len() as u64), content.len())
            .unwrap();
        assert_eq!(encoder.parameters(), Some(expected));
        assert_eq!(
            FrameHeader::parse(&frame).unwrap().0.window_size,
            1 << expected.window_log()
        );
        assert_eq!(
            decompress_with_dictionary_in(
                &frame,
                &dictionary.view(),
                DecodeOptions::new(source.len()),
                std::alloc::Global
            )
            .unwrap(),
            source
        );
        assert!(encoder.selection().is_none());
        encoder.reset_with_options(options).unwrap();
        let frame = pump(&mut encoder, source, 57, 3);
        assert_eq!(
            encoder.parameters(),
            Some(
                req.resolve(SourceSize::Known(source.len() as u64), 0)
                    .unwrap()
            )
        );
        assert_eq!(FrameHeader::parse(&frame).unwrap().0.window_size, 1024);
        assert_eq!(decompress(&frame, source.len()).unwrap(), source);
    }
}

#[test]
fn prefix_replacement_detach_and_pending_reset_obey_boundaries() {
    use zstdoxide::Dictionary;
    let first = Dictionary::raw(b"first prefix").unwrap();
    let second = Dictionary::raw(b"second prefix").unwrap();
    let empty = Dictionary::raw(&[]).unwrap();
    let mut encoder = ReferenceStreamEncoder::with_prefix(
        request(ReferenceStrategy::Fast),
        Default::default(),
        first.view(),
    )
    .unwrap();
    encoder.reset();
    assert_eq!(
        encoder.selection().unwrap().content(),
        first.view().content()
    );
    encoder.set_prefix(second.view()).unwrap();
    assert_eq!(
        encoder.selection().unwrap().content(),
        second.view().content()
    );
    encoder.set_prefix(empty.view()).unwrap();
    assert!(encoder.selection().is_none());
    encoder.set_prefix(first.view()).unwrap();
    encoder
        .process(b"some", &mut [], EndDirective::Continue)
        .unwrap();
    assert_eq!(encoder.set_prefix(second.view()), Err(Error::InvalidState));
    assert_eq!(
        encoder.selection().unwrap().content(),
        first.view().content()
    );
    encoder.reset();
    assert!(encoder.selection().is_none());
    assert_eq!(
        decompress(&pump(&mut encoder, b"fresh", 3, 2), 5).unwrap(),
        b"fresh"
    );
}

#[test]
fn formatted_prefix_entropy_and_identifier_are_scoped_to_one_frame() {
    use zstdoxide::{DecodeOptions, Dictionary, decompress_with_dictionary_in};
    let dictionary =
        Dictionary::from_bytes(include_bytes!("upstream/dictionaries/formatted.dict")).unwrap();
    let source = include_bytes!("upstream/dictionaries/dictionary-treeless-literals.expected");
    let req = request(ReferenceStrategy::Greedy);
    for include_dictionary_id in [false, true] {
        let mut encoder = ReferenceStreamEncoder::with_prefix(
            req,
            CompiledStreamOptions {
                pledged_size: Some(source.len() as u64),
                checksum: true,
                include_dictionary_id,
                ..Default::default()
            },
            dictionary.view(),
        )
        .unwrap();
        let frame = pump(&mut encoder, source, 117, 2);
        assert_eq!(
            FrameHeader::parse(&frame).unwrap().0.dictionary_id,
            if include_dictionary_id {
                dictionary.view().id()
            } else {
                0
            }
        );
        assert_eq!(
            decompress_with_dictionary_in(
                &frame,
                &dictionary.view(),
                DecodeOptions::new(source.len()),
                std::alloc::Global
            )
            .unwrap(),
            source
        );
        encoder.reset();
        let frame = pump(&mut encoder, source, 117, 2);
        assert_eq!(FrameHeader::parse(&frame).unwrap().0.dictionary_id, 0);
        assert_eq!(decompress(&frame, source.len()).unwrap(), source);
    }
}

#[test]
fn replacement_is_transactional_and_preserves_pending_prefix_without_allocation() {
    use zstdoxide::{DecodeOptions, Dictionary, decompress_with_dictionary_in};
    let dictionary = Dictionary::raw(b"request replacement prefix contents").unwrap();
    let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = allocator.monitor();
    let original = request(ReferenceStrategy::Fast);
    let mut encoder = ReferenceStreamEncoder::with_prefix_in(
        original,
        Default::default(),
        dictionary.view(),
        allocator,
    )
    .unwrap();
    for strategy in [ReferenceStrategy::Optimal, ReferenceStrategy::Ultra2] {
        assert!(encoder.set_request(request(strategy)).is_err());
        assert_eq!(encoder.request(), original);
        assert_eq!(encoder.options(), CompiledStreamOptions::default());
        assert_eq!(encoder.parameters(), None);
        assert_eq!(encoder.total_input(), 0);
        assert_eq!(encoder.total_output(), 0);
        assert_eq!(
            encoder.selection().unwrap().content(),
            dictionary.view().content()
        );
    }
    assert_eq!(
        encoder.set_options(CompiledStreamOptions {
            pledged_size: Some(u64::MAX),
            ..Default::default()
        }),
        Err(Error::InvalidParameter)
    );
    assert_eq!(encoder.options(), CompiledStreamOptions::default());
    let source = b"replacement prefix contents";
    let options = CompiledStreamOptions {
        pledged_size: Some(source.len() as u64),
        checksum: true,
        include_content_size: false,
        ..Default::default()
    };
    encoder.set_options(options).unwrap();
    let replacement = request(ReferenceStrategy::Lazy2);
    encoder.set_request(replacement).unwrap();
    assert_eq!(encoder.request(), replacement);
    assert_eq!(encoder.options(), options);
    assert_eq!(encoder.parameters(), None);
    assert_eq!(monitor.snapshot().attempts, 0);
    let frame = pump(&mut encoder, source, 3, 1);
    let header = FrameHeader::parse(&frame).unwrap().0;
    assert_eq!(header.content_size, None);
    assert!(header.has_checksum);
    assert_eq!(
        encoder.parameters(),
        Some(
            replacement
                .resolve(
                    SourceSize::Known(source.len() as u64),
                    dictionary.view().content().len()
                )
                .unwrap()
        )
    );
    assert_eq!(
        decompress_with_dictionary_in(
            &frame,
            &dictionary.view(),
            DecodeOptions::new(source.len()),
            std::alloc::Global
        )
        .unwrap(),
        source
    );
    drop(encoder);
    monitor.assert_released();
}

#[test]
fn replacement_rejects_new_geometry_without_poisoning_and_changes_reused_frames() {
    let mut wide = request(ReferenceStrategy::Fast);
    wide.set_window_log(21).unwrap();
    let options = CompiledStreamOptions {
        pledged_size: Some(1),
        ..Default::default()
    };
    let mut encoder = ReferenceStreamEncoder::with_options(wide, options).unwrap();
    assert!(
        encoder
            .set_options(CompiledStreamOptions::default())
            .is_err()
    );
    assert_eq!(encoder.request(), wide);
    assert_eq!(encoder.options(), options);
    assert_eq!(encoder.parameters(), None);
    assert_eq!(
        decompress(&pump(&mut encoder, b"x", 1, 1), 1).unwrap(),
        b"x"
    );
    for (strategy, length) in [
        (ReferenceStrategy::DoubleFast, 513),
        (ReferenceStrategy::Greedy, 4097),
        (ReferenceStrategy::BinaryTreeLazy2, 1025),
    ] {
        encoder.reset();
        // Repair the retained wide request before replacing its unknown pledge.
        let candidate = request(strategy);
        encoder.set_request(candidate).unwrap();
        let options = CompiledStreamOptions {
            pledged_size: Some(length as u64),
            checksum: true,
            ..Default::default()
        };
        encoder.set_options(options).unwrap();
        let source = vec![b'r'; length];
        let frame = pump(&mut encoder, &source, 47, 1);
        let mut fresh = ReferenceStreamEncoder::with_options(candidate, options).unwrap();
        assert_eq!(frame, pump(&mut fresh, &source, 47, 1));
        assert_eq!(decompress(&frame, length).unwrap(), source);
        assert_eq!(encoder.parameters().unwrap().strategy(), strategy);
    }
}

#[test]
fn replacement_stage_errors_precede_admission_and_do_not_poison() {
    for stage in 0..4 {
        let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = allocator.monitor();
        let req = request(ReferenceStrategy::Fast);
        let mut encoder = ReferenceStreamEncoder::new_in(req, allocator).unwrap();
        match stage {
            0 => {
                encoder
                    .process(b"a", &mut [], EndDirective::Continue)
                    .unwrap();
            }
            1 => {
                encoder
                    .process(b"a", &mut [], EndDirective::Finish)
                    .unwrap();
            }
            2 => {
                pump(&mut encoder, b"a", 1, 1);
            }
            _ => {
                encoder
                    .set_options(CompiledStreamOptions {
                        pledged_size: Some(2),
                        ..Default::default()
                    })
                    .unwrap();
                assert_eq!(
                    encoder.process(b"a", &mut [], EndDirective::Finish),
                    Err(Error::ContentSizeMismatch)
                );
            }
        }
        let before = (
            encoder.request(),
            encoder.options(),
            encoder.parameters(),
            encoder.total_input(),
            encoder.total_output(),
            encoder.retained_capacity(),
        );
        let allocations = monitor.snapshot();
        assert_eq!(
            encoder.set_request(request(ReferenceStrategy::Ultra2)),
            Err(Error::InvalidState)
        );
        assert_eq!(
            encoder.set_options(CompiledStreamOptions {
                pledged_size: Some(u64::MAX),
                ..Default::default()
            }),
            Err(Error::InvalidState)
        );
        assert_eq!(
            before,
            (
                encoder.request(),
                encoder.options(),
                encoder.parameters(),
                encoder.total_input(),
                encoder.total_output(),
                encoder.retained_capacity()
            )
        );
        assert_eq!(monitor.snapshot(), allocations);
        if stage < 2 {
            assert_eq!(
                encoder
                    .process(&[], &mut [0; 128], EndDirective::Finish)
                    .unwrap()
                    .status,
                StreamStatus::FrameFinished
            );
        } else if stage == 2 {
            assert_eq!(
                encoder
                    .process(&[], &mut [], EndDirective::Finish)
                    .unwrap()
                    .status,
                StreamStatus::FrameFinished
            );
        } else {
            assert_eq!(
                encoder.process(&[], &mut [], EndDirective::Finish),
                Err(Error::InvalidState)
            );
        }
        drop(encoder);
        monitor.assert_released();
    }
}

#[test]
fn parameter_reset_preserves_completed_observation_and_rejects_active_stages() {
    use zstdoxide::Dictionary;
    let raw = Dictionary::raw(b"parameter reset raw prefix").unwrap();
    let formatted =
        Dictionary::from_bytes(include_bytes!("upstream/dictionaries/formatted.dict")).unwrap();
    for dictionary in [&raw, &formatted] {
        for stage in 0..5 {
            let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = allocator.monitor();
            let req = request(ReferenceStrategy::Lazy2);
            let options = CompiledStreamOptions {
                checksum: true,
                pledged_size: Some(3),
                include_content_size: false,
                include_dictionary_id: false,
            };
            let mut encoder =
                ReferenceStreamEncoder::with_prefix_in(req, options, dictionary.view(), allocator)
                    .unwrap();
            match stage {
                0 => {}
                1 => {
                    encoder
                        .process(b"a", &mut [], EndDirective::Continue)
                        .unwrap();
                }
                2 => {
                    encoder
                        .process(b"abc", &mut [], EndDirective::Finish)
                        .unwrap();
                }
                3 => {
                    pump(&mut encoder, b"abc", 1, 1);
                }
                _ => {
                    assert_eq!(
                        encoder.process(b"a", &mut [], EndDirective::Finish),
                        Err(Error::ContentSizeMismatch)
                    );
                }
            }
            let counters = (encoder.total_input(), encoder.total_output());
            let profile = encoder.parameters();
            let capacity = encoder.retained_capacity();
            let selection = encoder.selection().map(|v| v.content().to_vec());
            let before = monitor.snapshot();
            if stage == 0 || stage == 3 {
                encoder.reset_parameters().unwrap();
                assert_eq!(encoder.request(), CompressionParameterRequest::default());
                assert_eq!(encoder.options(), CompiledStreamOptions::default());
                assert!(encoder.selection().is_none());
            } else {
                assert_eq!(encoder.reset_parameters(), Err(Error::InvalidState));
                assert_eq!(encoder.request(), req);
                assert_eq!(encoder.options(), options);
                assert_eq!(encoder.selection().map(|v| v.content().to_vec()), selection);
            }
            assert_eq!(encoder.parameters(), profile);
            assert_eq!((encoder.total_input(), encoder.total_output()), counters);
            assert_eq!(encoder.retained_capacity(), capacity);
            assert_eq!(monitor.snapshot(), before);
            if stage == 3 {
                assert_eq!(
                    encoder
                        .process(&[], &mut [], EndDirective::Finish)
                        .unwrap()
                        .status,
                    StreamStatus::FrameFinished
                );
                assert_eq!(encoder.set_request(req), Err(Error::InvalidState));
                encoder.reset();
                assert_eq!(encoder.parameters(), None);
            }
            if stage == 0 || stage == 3 {
                // Defaults are requested values, not admission of every source
                // geometry. Supply a supported known size before processing.
                let next = CompiledStreamOptions {
                    pledged_size: Some(3),
                    ..Default::default()
                };
                encoder.set_options(next).unwrap();
                let frame = pump(&mut encoder, b"xyz", 1, 1);
                let mut fresh = ReferenceStreamEncoder::with_options(
                    CompressionParameterRequest::default(),
                    next,
                )
                .unwrap();
                assert_eq!(frame, pump(&mut fresh, b"xyz", 1, 1));
                assert_eq!(decompress(&frame, 3).unwrap(), b"xyz");
            } else if stage == 1 {
                assert_eq!(
                    encoder
                        .process(b"bc", &mut [0; 128], EndDirective::Finish)
                        .unwrap()
                        .status,
                    StreamStatus::FrameFinished
                );
            } else if stage == 2 {
                assert_eq!(
                    encoder
                        .process(&[], &mut [0; 128], EndDirective::Finish)
                        .unwrap()
                        .status,
                    StreamStatus::FrameFinished
                );
            } else {
                assert_eq!(
                    encoder.process(&[], &mut [], EndDirective::Finish),
                    Err(Error::InvalidState)
                );
            }
            drop(encoder);
            monitor.assert_released();
        }
    }
}

#[test]
fn reset_all_recovers_every_phase_and_removes_prefix_without_allocating() {
    use zstdoxide::Dictionary;
    let raw = Dictionary::raw(b"reset all raw prefix").unwrap();
    let formatted =
        Dictionary::from_bytes(include_bytes!("upstream/dictionaries/formatted.dict")).unwrap();
    for dictionary in [&raw, &formatted] {
        for stage in 0..5 {
            let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = allocator.monitor();
            let mut encoder = ReferenceStreamEncoder::with_prefix_in(
                request(ReferenceStrategy::BinaryTreeLazy2),
                CompiledStreamOptions {
                    checksum: true,
                    pledged_size: Some(3),
                    ..Default::default()
                },
                dictionary.view(),
                allocator,
            )
            .unwrap();
            match stage {
                0 => {}
                1 => {
                    encoder
                        .process(b"a", &mut [], EndDirective::Continue)
                        .unwrap();
                }
                2 => {
                    encoder
                        .process(b"abc", &mut [], EndDirective::Finish)
                        .unwrap();
                }
                3 => {
                    pump(&mut encoder, b"abc", 1, 1);
                }
                _ => {
                    assert_eq!(
                        encoder.process(b"a", &mut [], EndDirective::Finish),
                        Err(Error::ContentSizeMismatch)
                    );
                }
            }
            let capacity = encoder.retained_capacity();
            let before = monitor.snapshot();
            encoder.reset_all();
            assert_eq!(encoder.request(), CompressionParameterRequest::default());
            assert_eq!(encoder.options(), CompiledStreamOptions::default());
            assert_eq!(encoder.parameters(), None);
            assert!(encoder.selection().is_none());
            assert_eq!((encoder.total_input(), encoder.total_output()), (0, 0));
            assert_eq!(encoder.retained_capacity(), capacity);
            assert_eq!(monitor.snapshot(), before);
            encoder
                .set_request(request(ReferenceStrategy::DoubleFast))
                .unwrap();
            let frame = pump(&mut encoder, b"after reset all", 2, 1);
            assert_eq!(decompress(&frame, 15).unwrap(), b"after reset all");
            drop(encoder);
            monitor.assert_released();
        }
    }
}

#[test]
fn default_reset_followed_by_allocation_failure_recovers_and_releases() {
    let allocator = TrackingAllocator::new(AllocatorConfig {
        fail_at: std::num::NonZeroU64::new(1),
        ..Default::default()
    })
    .unwrap();
    let monitor = allocator.monitor();
    let mut encoder =
        ReferenceStreamEncoder::new_in(request(ReferenceStrategy::Fast), allocator).unwrap();
    encoder.reset_parameters().unwrap();
    encoder
        .set_options(CompiledStreamOptions {
            pledged_size: Some(3),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(monitor.snapshot().attempts, 0);
    assert_eq!(
        encoder.process(b"abc", &mut [0; 128], EndDirective::Finish),
        Err(Error::AllocationFailed)
    );
    assert_eq!(encoder.reset_parameters(), Err(Error::InvalidState));
    let attempts = monitor.snapshot().attempts;
    encoder.reset_all();
    assert_eq!(monitor.snapshot().attempts, attempts);
    monitor.clear_failure();
    encoder
        .set_options(CompiledStreamOptions {
            pledged_size: Some(3),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        decompress(&pump(&mut encoder, b"abc", 1, 1), 3).unwrap(),
        b"abc"
    );
    drop(encoder);
    monitor.assert_released();
}
