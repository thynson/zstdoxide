//! Tree parser/frame ownership and independently observed ordinary publication.
#![forbid(unsafe_code)]
use super::*;
use crate::greedy_plan::{GreedyExecutionPlan, GreedyTablePlan};
use crate::{FrameHeader, RawCompressionParameters, ReferenceStrategy, SourceSize};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

fn parameters(window: u32, hash: u32, chain: u32) -> RawCompressionParameters {
    RawCompressionParameters {
        window_log: window,
        hash_log: hash,
        chain_log: chain,
        search_log: 4,
        min_match: 4,
        target_length: 0,
        strategy: 6,
    }
}
#[test]
fn tree_admission_preserves_selected_owners_and_predecessor_gates() {
    for window in [10, 14, 15, 21] {
        let raw = parameters(window, 19, 19);
        let resolved = raw.validate().unwrap();
        let plan = GreedyExecutionPlan::from_tree_parameters(resolved, 4096).unwrap();
        assert_eq!(plan.depth(), crate::lazy_select::LazyDepth::Two);
        assert!(!plan.tables().is_row());
        let GreedyTablePlan::Tree { layout, attempts } = plan.tables() else {
            panic!("tree owner")
        };
        assert_eq!(layout.nodes(), 1 << 18);
        assert_eq!(attempts, 16);
        assert_eq!(
            plan.first_bytes() + plan.second_bytes(),
            layout.total_bytes()
        );
        assert_eq!(
            GreedyExecutionPlan::from_family_parameters(resolved, 4096),
            Err(Error::UnsupportedFeature)
        );
        let mut request = CompressionParameterRequest::new();
        request.set_strategy(Some(ReferenceStrategy::BinaryTreeLazy2));
        assert!(matches!(
            ReferenceSearchPlan::new(&request, 4096),
            Ok(ReferenceSearchPlan::Greedy(_))
        ));
        assert_eq!(
            request
                .resolve(SourceSize::Known(4096), 0)
                .unwrap()
                .strategy(),
            ReferenceStrategy::BinaryTreeLazy2
        );
    }
    for field in 0..4 {
        let mut raw = parameters(21, 19, 19);
        match field {
            0 => raw.window_log = 22,
            1 => raw.hash_log = 20,
            2 => raw.chain_log = 20,
            _ => raw.search_log = 9,
        };
        assert_eq!(
            GreedyExecutionPlan::from_tree_parameters(raw.validate().unwrap(), 4096),
            Err(Error::ResourceLimit)
        );
    }
    for strategy in [1, 2, 3, 4, 5, 7, 8, 9] {
        let mut raw = parameters(10, 6, 6);
        raw.strategy = strategy;
        assert_eq!(
            GreedyExecutionPlan::from_tree_parameters(raw.validate().unwrap(), 96),
            Err(Error::UnsupportedFeature)
        );
    }
}
#[test]
fn private_whole_frames_roundtrip_tiny_windows_and_retained_prefixes() {
    for window in [10, 15, 21] {
        for size in [0, 1, 6, 7, 8, 9, 16, 96, 1024, 4097, 131073, 262145] {
            let mut state = 0x91e10da5_u32;
            let mut source = Vec::new();
            for p in 0..size {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                source.push(if p % 4096 < 2048 {
                    b"abcde1234567"[p % 12]
                } else {
                    state as u8
                });
            }
            let plan = ReferenceSearchPlan::Greedy(
                GreedyExecutionPlan::from_tree_parameters(
                    parameters(window, 10, 10).validate().unwrap(),
                    size,
                )
                .unwrap(),
            );
            let frame = compress_reference_in(&source, plan, alloc::alloc::Global).unwrap();
            assert_eq!(
                crate::decompress(&frame, size).unwrap(),
                source,
                "window {window}, size {size}"
            );
        }
    }
}

/// Small explicit tuples exercise both branches and every depth under Miri.
#[test]
fn platform_tree_frames_keep_nonclone_owners_and_transactional_boundaries() {
    let source: Vec<u8> = b"abcdabcd1234abcd"
        .iter()
        .copied()
        .cycle()
        .take(96)
        .collect();
    for strategy in [6] {
        for window in [10, 15] {
            for minimum in [3, 7] {
                let parameters = RawCompressionParameters {
                    window_log: window,
                    chain_log: 6,
                    hash_log: 6,
                    search_log: 4,
                    min_match: minimum,
                    target_length: 0,
                    strategy,
                }
                .validate()
                .unwrap();
                let plan = ReferenceSearchPlan::Greedy(
                    GreedyExecutionPlan::from_tree_parameters(parameters, source.len()).unwrap(),
                );
                let owner = TrackingAllocator::new(AllocatorConfig::default())
                    .unwrap()
                    .into_non_clone();
                let monitor = owner.monitor();
                let mut workspace = ReferenceEncoderWorkspace::new(&source, plan, &owner).unwrap();
                let settings = ReferenceFrameSettings::new(source.len(), window as u8).unwrap();
                let mut frame = ReferenceFrameHeader::new(&settings).as_bytes().to_vec();
                let before = frame.clone();
                let allocation_before = monitor.snapshot();
                for (length, last) in [
                    (0, false),
                    (97, true),
                    (usize::MAX, true),
                    (96, false),
                    (1, true),
                ] {
                    assert_eq!(
                        workspace.emit_next(length, last, &mut frame),
                        Err(Error::InvalidParameter)
                    );
                }
                assert_eq!(frame, before);
                assert_eq!(monitor.snapshot(), allocation_before);
                assert_eq!(workspace.emitted, 0);
                for length in [32, 6, 58] {
                    workspace
                        .emit_next(
                            length,
                            workspace.emitted + length == source.len(),
                            &mut frame,
                        )
                        .unwrap();
                }
                assert!(workspace.finished);
                assert_eq!(
                    workspace.emit_next(0, false, &mut frame),
                    Err(Error::InvalidState)
                );
                assert_eq!(crate::decompress(&frame, 96).unwrap(), source);
                let (header, _) = FrameHeader::parse(&frame).unwrap();
                assert_eq!((header.content_size, header.window_size), (Some(96), 96));
                assert_eq!(
                    monitor.snapshot().live_bytes,
                    workspace.retained_capacity() as u128
                );
                drop(workspace);
                monitor.assert_released();
                let frame = {
                    let borrowed_source = source.clone();
                    compress_reference_in(&borrowed_source, plan, &owner).unwrap()
                };
                assert_eq!(frame.allocator().identity(), owner.identity());
                assert_eq!(monitor.snapshot().identities_created, 1);
                assert_eq!(crate::decompress(&frame, 96).unwrap(), source);
                drop(frame);
                monitor.assert_released();
                drop(owner);
                assert_eq!(monitor.snapshot().owners_dropped, 1);
            }
        }
    }
}

#[test]
fn every_tree_frame_allocation_failure_releases_exact_owners() {
    use core::num::NonZeroU64;
    let mut total = 0;
    for size in [0, 1, 7, 96, 4097] {
        let source: Vec<u8> = b"tree allocator repeated bytes"
            .iter()
            .copied()
            .cycle()
            .take(size)
            .collect();
        let plan = ReferenceSearchPlan::Greedy(
            GreedyExecutionPlan::from_tree_parameters(
                parameters(15, 6, 6).validate().unwrap(),
                size,
            )
            .unwrap(),
        );
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let frame = compress_reference_in(&source, plan, owner).unwrap();
        assert_eq!(crate::decompress(&frame, size).unwrap(), source);
        let attempts = monitor.snapshot().attempts;
        total += attempts;
        drop(frame);
        monitor.assert_released();
        for ordinal in 1..=attempts {
            let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = owner.monitor();
            monitor.set_fail_at(NonZeroU64::new(ordinal));
            assert_eq!(
                compress_reference_in(&source, plan, owner).err(),
                Some(Error::AllocationFailed),
                "size {size}, ordinal {ordinal}"
            );
            let snapshot = monitor.snapshot();
            assert_eq!(snapshot.attempts, ordinal);
            assert_eq!(snapshot.failures, 1);
            assert_eq!(snapshot.owners_dropped, snapshot.identities_created);
            monitor.assert_released();
        }
    }
    assert!(total > 20);
}

#[test]
fn late_tree_output_failure_preserves_committed_state_and_poisoned_context() {
    let source: Vec<u8> = b"abcde1234567".iter().copied().cycle().take(8192).collect();
    let plan = ReferenceSearchPlan::Greedy(
        GreedyExecutionPlan::from_tree_parameters(
            parameters(15, 10, 10).validate().unwrap(),
            source.len(),
        )
        .unwrap(),
    );
    let workspace_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let wm = workspace_owner.monitor();
    let output_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let om = output_owner.monitor();
    let mut workspace = ReferenceEncoderWorkspace::new(&source, plan, workspace_owner).unwrap();
    let settings = ReferenceFrameSettings::new(source.len(), 15).unwrap();
    let mut frame = ReferenceFrameHeader::new(&settings).as_bytes().to_vec();
    workspace.emit_next(4096, false, &mut frame).unwrap();
    let committed = workspace.entropy.sequence_encoder.committed_repeats();
    assert_ne!(committed, [1, 4, 8]);
    let mut output = Vec::new_in(output_owner);
    output.try_reserve_exact(frame.len()).unwrap();
    output.extend_from_slice(&frame);
    assert_eq!(output.len(), output.capacity());
    assert!(om.fail_next());
    assert_eq!(
        workspace.emit_next(4096, true, &mut output),
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
    let snapshots = (wm.snapshot(), om.snapshot());
    for (size, last) in [(0, false), (4096, true), (usize::MAX, true)] {
        assert_eq!(
            workspace.emit_next(size, last, &mut output),
            Err(Error::InvalidState)
        );
    }
    assert_eq!((wm.snapshot(), om.snapshot()), snapshots);
    drop(output);
    drop(workspace);
    wm.assert_released();
    om.assert_released();
    assert_eq!(
        crate::decompress(
            &compress_reference_in(&source, plan, alloc::alloc::Global).unwrap(),
            source.len()
        )
        .unwrap(),
        source
    );
}

#[test]
fn ordinary_tree_frames_match_retained_publication() {
    let counts = super::greedy_tests::check_frames(
        crate::tree_frame_oracle_fixture::cases()
            .unwrap()
            .into_iter()
            .map(|c| (2, c)),
    );
    assert_eq!(counts, (488, [274, 38, 219], 38, 32));
}
