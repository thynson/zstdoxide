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
