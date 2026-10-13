//! Private complete lazy-family frames over immutable ordinary C inputs.
#![forbid(unsafe_code)]

use super::*;
use crate::greedy_frame_oracle_fixture::OracleFrameCase;
use crate::greedy_plan::GreedyExecutionPlan;
use crate::lazy_frame_oracle_fixture::cases;
use crate::{FrameHeader, RawCompressionParameters};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

fn observations() -> impl Iterator<Item = (u8, OracleFrameCase<'static>)> {
    cases()
        .unwrap()
        .into_iter()
        .map(|c| (c.depth, c.observation))
}

#[test]
fn all_depths_publish_complete_scheduled_and_whole_frames() {
    assert_eq!(
        greedy_tests::check_frames(observations()),
        (1014, [633, 114, 630], 165, 36)
    );
}

#[test]
fn every_lazy_frame_allocation_failure_releases_exact_owners() {
    let total = greedy_tests::check_allocation_failures(observations());
    // The complete campaign visits every observed allocation ordinal. Record
    // its measured count without predicting allocation behavior from C.
    assert!(total > 6184);
    #[cfg(feature = "std")]
    std::println!("Lazy-family allocation campaign: 510 cases, {total} ordinals");
}

#[test]
fn failed_lazy_output_append_does_not_publish_candidate_repeats() {
    let mut checked = 0;
    for (depth, case) in observations() {
        if case.name.ends_with("chain_mixed_fallback") || case.name.ends_with("row_mixed_fallback")
        {
            greedy_tests::check_failed_append(&case, depth);
            checked += 1;
        }
    }
    assert_eq!(checked, 6);
}

/// Small explicit tuples exercise both branches and every depth under Miri.
#[test]
fn platform_lazy_frames_keep_nonclone_owners_and_transactional_boundaries() {
    let source: Vec<u8> = b"abcdabcd1234abcd"
        .iter()
        .copied()
        .cycle()
        .take(96)
        .collect();
    for strategy in [3, 4, 5] {
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
                    GreedyExecutionPlan::from_family_parameters(parameters, source.len()).unwrap(),
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
