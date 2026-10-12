//! Complete private Greedy frames over fixed source recipes and exact owners.
#![forbid(unsafe_code)]

use super::*;
use crate::greedy_frame_oracle_fixture::{OracleFrameCase, cases};
use crate::greedy_plan::GreedyExecutionPlan;
use crate::{FrameHeader, RawCompressionParameters};
use alloc::alloc::Global;
use core::num::NonZeroU64;
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

fn plan(case: &OracleFrameCase<'_>) -> ReferenceSearchPlan {
    let parameters = RawCompressionParameters {
        window_log: u32::from(case.window_log),
        chain_log: u32::from(case.chain_log),
        hash_log: u32::from(case.hash_log),
        search_log: u32::from(case.search_log),
        min_match: u32::from(case.min_match),
        target_length: case.target_length,
        strategy: 3,
    }
    .validate()
    .unwrap();
    ReferenceSearchPlan::Greedy(
        GreedyExecutionPlan::from_resolved_parameters(parameters, case.source.len()).unwrap(),
    )
}

fn prefix(plan: ReferenceSearchPlan) -> Vec<u8> {
    let settings =
        ReferenceFrameSettings::new(plan.source_len(), plan.parameters().window_log()).unwrap();
    ReferenceFrameHeader::new(&settings).as_bytes().to_vec()
}

fn verify(frame: &[u8], case: &OracleFrameCase<'_>) {
    assert_eq!(
        crate::decompress(frame, case.source.len()).unwrap(),
        case.source,
        "{}",
        case.name
    );
    let (header, _) = FrameHeader::parse(frame).unwrap();
    assert_eq!(header.content_size, Some(case.source.len() as u64));
    assert_eq!(
        header.window_size,
        case.source.len().min(1usize << case.window_log) as u64
    );
    assert_eq!(header.dictionary_id, 0);
    assert!(!header.has_checksum);
}

#[test]
fn whole_and_scheduled_frames_decode_with_committed_repeat_publication() {
    let cases = cases().unwrap();
    let mut frames = 0;
    let mut changed_raw = 0;
    let mut changed_rle = 0;
    let mut kinds = [0usize; 3];
    for case in &cases {
        let plan = plan(case);
        let frame = compress_reference_in(case.source, plan, Global).unwrap();
        verify(&frame, case);
        frames += 1;
        if case.source.is_empty() {
            assert_eq!(frame, [0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 1, 0, 0]);
            continue;
        }
        let mut frame = prefix(plan);
        let mut workspace = ReferenceEncoderWorkspace::new(case.source, plan, Global).unwrap();
        let mut observed = case.blocks.iter();
        for &length in &case.schedule {
            // The workspace emits nonempty extents; empty Continue calls have
            // already been qualified at the independent search-state boundary.
            if length == 0 {
                continue;
            }
            let expected = observed.next().unwrap();
            let before = workspace.entropy.sequence_encoder.committed_repeats();
            let begin = frame.len();
            let end = workspace.emitted + length;
            workspace
                .emit_next(length, end == case.source.len(), &mut frame)
                .unwrap();
            let kind = usize::from((frame[begin] >> 1) & 3);
            assert_eq!(
                kind,
                usize::from(expected.kind),
                "{} call{}",
                case.name,
                expected.call
            );
            kinds[kind] += 1;
            let mut trial = before;
            if length >= 7 {
                for record in workspace.search.sequences() {
                    trial = record
                        .offset
                        .resolve(
                            record.sequence.literal_length,
                            record.sequence.distance,
                            trial,
                        )
                        .unwrap()
                        .next_repeat;
                }
            }
            if trial[..2] != before[..2] {
                changed_raw += usize::from(kind == 0);
                changed_rle += usize::from(kind == 1);
            }
            assert_eq!(
                workspace.entropy.sequence_encoder.committed_repeats(),
                if kind == 2 { trial } else { before }
            );
            assert_eq!(
                workspace.entropy.sequence_encoder.committed_repeats(),
                expected.wire_after
            );
            assert_eq!(workspace.emitted, end);
            assert!(!workspace.failed);
        }
        assert!(observed.next().is_none());
        assert!(workspace.finished);
        verify(&frame, case);
        frames += 1;
    }
    assert_eq!(frames, 274);
    assert_eq!(kinds, [181, 38, 176]);
    assert_eq!((changed_raw, changed_rle), (25, 12));
    #[cfg(feature = "std")]
    std::println!(
        "Greedy scheduled kinds={kinds:?}; changed raw={changed_raw}; changed RLE={changed_rle}; complete Rust frames={frames}"
    );
}

#[test]
fn target_search_and_short_width_equivalences_preserve_complete_frames() {
    let cases = cases().unwrap();
    for (left, right) in [
        ("chain_literal_target0", "chain_literal_target4"),
        ("chain_literal_target0", "chain_literal_target131072"),
        ("row_literal_target0", "row_literal_target4"),
        ("row_literal_target0", "row_literal_target131072"),
        ("chain_width3", "chain_width4"),
        ("row_width3", "row_width4"),
    ] {
        let a = cases.iter().find(|c| c.name == left).unwrap();
        let b = cases.iter().find(|c| c.name == right).unwrap();
        assert_eq!(a.source, b.source);
        assert_eq!(
            compress_reference_in(a.source, plan(a), Global).unwrap(),
            compress_reference_in(b.source, plan(b), Global).unwrap()
        );
    }
    let expire = cases.iter().find(|c| c.name == "row_window20").unwrap();
    let retain = cases.iter().find(|c| c.name == "row_window21").unwrap();
    assert_eq!(expire.source, retain.source);
    let a = compress_reference_in(expire.source, plan(expire), Global).unwrap();
    let b = compress_reference_in(retain.source, plan(retain), Global).unwrap();
    verify(&a, expire);
    verify(&b, retain);
    assert!(b.len() + 100_000 < a.len());
}

#[test]
fn empty_and_tiny_frames_allocate_only_output_and_selected_tables() {
    let cases = cases().unwrap();
    for name in [
        "chain_empty",
        "row_empty",
        "chain_fresh_length1",
        "chain_fresh_length6",
        "row_fresh_length1",
        "row_fresh_length6",
    ] {
        let case = cases.iter().find(|c| c.name == name).unwrap();
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let identity = owner.identity();
        let monitor = owner.monitor();
        let frame = compress_reference_in(case.source, plan(case), owner).unwrap();
        assert_eq!(frame.allocator().identity(), identity);
        assert_eq!(
            monitor.snapshot().attempts,
            if name.ends_with("_empty") { 1 } else { 3 }
        );
        assert_eq!(monitor.snapshot().live_blocks, 1);
        if name.ends_with("_empty") {
            assert_eq!(monitor.snapshot().identities_created, 1);
        }
        verify(&frame, case);
        drop(frame);
        monitor.assert_released();
        assert_eq!(
            monitor.snapshot().owners_dropped,
            monitor.snapshot().identities_created
        );
    }
}

#[test]
fn every_observed_frame_allocation_failure_releases_the_exact_owner() {
    let cases = cases().unwrap();
    let mut total = 0;
    for case in &cases {
        let name = case.name;
        let selected = plan(case);
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let frame = compress_reference_in(case.source, selected, owner).unwrap();
        verify(&frame, case);
        let attempts = monitor.snapshot().attempts;
        total += attempts;
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(frame);
        monitor.assert_released();
        for ordinal in 1..=attempts {
            let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = owner.monitor();
            monitor.set_fail_at(NonZeroU64::new(ordinal));
            assert_eq!(
                compress_reference_in(case.source, selected, owner).err(),
                Some(Error::AllocationFailed),
                "{name}: ordinal {ordinal}"
            );
            assert_eq!(monitor.snapshot().attempts, ordinal);
            assert_eq!(monitor.snapshot().failures, 1);
            assert_eq!(
                monitor.snapshot().owners_dropped,
                monitor.snapshot().identities_created
            );
            monitor.assert_released();
        }
        #[cfg(feature = "std")]
        std::println!("Greedy allocation failures: {name} ordinals=1..={attempts}");
    }
    #[cfg(feature = "std")]
    std::println!("Greedy allocation-failure campaign: 138 cases, {total} ordinals");
    assert_eq!(total, 6184);
}

#[test]
fn failed_output_append_preserves_publication_then_poisoned() {
    let cases = cases().unwrap();
    for name in ["chain_mixed_fallback", "row_mixed_fallback"] {
        let case = cases.iter().find(|c| c.name == name).unwrap();
        let selected = plan(case);
        let workspace_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let output_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let wm = workspace_owner.monitor();
        let om = output_owner.monitor();
        let mut workspace =
            ReferenceEncoderWorkspace::new(case.source, selected, workspace_owner).unwrap();
        let mut frame = prefix(selected);
        workspace.emit_next(4096, false, &mut frame).unwrap();
        let committed = workspace.entropy.sequence_encoder.committed_repeats();
        assert_ne!(committed, [1, 4, 8]);
        let mut output = Vec::new_in(output_owner);
        output.try_reserve_exact(frame.len()).unwrap();
        output.extend_from_slice(&frame);
        assert_eq!(output.len(), output.capacity());
        assert!(om.fail_next());
        let remaining = case.source.len() - 4096;
        assert_eq!(
            workspace.emit_next(remaining, true, &mut output),
            Err(Error::AllocationFailed)
        );
        assert_eq!(output.as_slice(), frame);
        assert_eq!(workspace.emitted, 4096);
        assert_eq!(
            workspace.entropy.sequence_encoder.committed_repeats(),
            committed
        );
        assert!(workspace.entropy.pending_literal_table.is_none());
        assert!(workspace.failed);
        om.clear_failure();
        let after = (wm.snapshot(), om.snapshot());
        for (length, last) in [(0, false), (remaining, true), (usize::MAX, true)] {
            assert_eq!(
                workspace.emit_next(length, last, &mut output),
                Err(Error::InvalidState)
            );
        }
        assert_eq!((wm.snapshot(), om.snapshot()), after);
        drop(output);
        drop(workspace);
        wm.assert_released();
        om.assert_released();
        verify(
            &compress_reference_in(case.source, selected, Global).unwrap(),
            case,
        );
    }
}

#[test]
fn invalid_schedules_preserve_owners_and_borrowed_results_outlive_the_source() {
    let cases = cases().unwrap();
    for name in ["chain_first_uniform", "row_first_uniform"] {
        let case = cases.iter().find(|c| c.name == name).unwrap();
        let selected = plan(case);
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        assert!(monitor.fail_next());
        let before = monitor.snapshot();
        assert_eq!(
            compress_reference_in(&case.source[..4095], selected, &owner).err(),
            Some(Error::InvalidParameter)
        );
        assert_eq!(monitor.snapshot(), before);
        monitor.clear_failure();
        let mut workspace = ReferenceEncoderWorkspace::new(case.source, selected, &owner).unwrap();
        let mut frame = prefix(selected);
        let frame_before = frame.clone();
        assert!(monitor.fail_next());
        let before = monitor.snapshot();
        for (length, last) in [
            (0, false),
            (4097, true),
            (usize::MAX, true),
            (1, true),
            (4096, false),
        ] {
            assert_eq!(
                workspace.emit_next(length, last, &mut frame),
                Err(Error::InvalidParameter)
            );
        }
        assert_eq!(monitor.snapshot(), before);
        assert_eq!(frame, frame_before);
        assert_eq!(workspace.emitted, 0);
        assert!(!workspace.failed);
        monitor.clear_failure();
        workspace.emit_next(4096, true, &mut frame).unwrap();
        verify(&frame, case);
        assert_eq!(
            monitor.snapshot().live_bytes,
            workspace.retained_capacity() as u128
        );
        drop(workspace);
        monitor.assert_released();
        let result = {
            let source = case.source.to_vec();
            compress_reference_in(&source, selected, &owner).unwrap()
        };
        assert_eq!(result.allocator().identity(), owner.identity());
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        assert_eq!(monitor.snapshot().live_bytes, result.capacity() as u128);
        verify(&result, case);
        drop(result);
        monitor.assert_released();
    }
}

/// Small explicit chain/row plans keep interpreted target checks bounded.
/// Public source-size adjustment is covered separately by the integration suite.
#[test]
fn platform_small_chain_and_row_frames_preserve_exact_owners_and_key_bounds() {
    let source: Vec<u8> = b"abcdabcd1234abcd"
        .iter()
        .copied()
        .cycle()
        .take(96)
        .collect();
    for window in [10, 15] {
        for minimum in [3, 7] {
            let parameters = RawCompressionParameters {
                window_log: window,
                chain_log: 6,
                hash_log: 6,
                search_log: 4,
                min_match: minimum,
                target_length: 0,
                strategy: 3,
            }
            .validate()
            .unwrap();
            let plan = ReferenceSearchPlan::Greedy(
                GreedyExecutionPlan::from_resolved_parameters(parameters, source.len()).unwrap(),
            );
            let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let identity = owner.identity();
            let monitor = owner.monitor();
            let frame = compress_reference_in(&source, plan, owner).unwrap();
            assert_eq!(frame.allocator().identity(), identity);
            assert_eq!(monitor.snapshot().live_blocks, 1);
            let (header, _) = FrameHeader::parse(&frame).unwrap();
            assert_eq!(header.content_size, Some(96));
            assert_eq!(header.window_size, 96);
            assert_eq!(header.dictionary_id, 0);
            assert!(!header.has_checksum);
            assert_eq!(crate::decompress(&frame, 96).unwrap(), source);
            assert!(crate::decompress(&frame, 95).is_err());
            drop(frame);
            monitor.assert_released();
            assert_eq!(
                monitor.snapshot().identities_created,
                monitor.snapshot().owners_dropped
            );
        }
    }
}
