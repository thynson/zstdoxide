//! Private complete retained frames; public request admission remains closed.
#![forbid(unsafe_code)]
use super::*;
use crate::optimal_retained_frames_fixture::{self as fixture, Case};
use crate::optimal_retained_plan::RetainedOptimalPlan;
use crate::{FrameHeader, RawCompressionParameters};
use alloc::alloc::Global;
use core::num::NonZeroU64;
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

fn plan(case: &Case<'_>) -> ReferenceSearchPlan {
    let parameters = RawCompressionParameters {
        window_log: case.window.into(),
        hash_log: case.hash.into(),
        chain_log: case.chain.into(),
        search_log: case.search.into(),
        min_match: case.minimum,
        target_length: case.target,
        strategy: 7,
    }
    .validate()
    .unwrap();
    ReferenceSearchPlan::RetainedOptimal(
        RetainedOptimalPlan::new(
            parameters,
            case.source.len(),
            &case.blocks.iter().map(|b| b.length).collect::<Vec<_>>(),
            case.literal_mode != 0,
        )
        .unwrap(),
    )
}
fn prefix(plan: ReferenceSearchPlan) -> Vec<u8> {
    ReferenceFrameHeader::new(
        &ReferenceFrameSettings::new(plan.source_len(), plan.parameters().window_log()).unwrap(),
    )
    .as_bytes()
    .to_vec()
}
fn verify(frame: &[u8], case: &Case<'_>) {
    assert_eq!(
        crate::decompress(frame, case.source.len()).unwrap(),
        case.source,
        "{}",
        case.name
    );
    let (header, _) = FrameHeader::parse(frame).unwrap();
    assert_eq!(header.content_size, Some(case.source.len() as u64));
    assert!(!header.has_checksum);
    assert_eq!(header.dictionary_id, 0);
}
fn kind_at(bytes: &[u8]) -> BlockKind {
    match (bytes[0] >> 1) & 3 {
        0 => BlockKind::Raw,
        1 => BlockKind::Rle,
        2 => BlockKind::Compressed,
        _ => panic!("reserved block"),
    }
}

#[test]
fn retained_frame_complete_mixed_publications_and_reset() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let mut kinds = [0; 3];
    for case in cases.iter().filter(|c| c.literal_mode == 0) {
        let selected = plan(case);
        let frame = compress_reference_in(case.source, selected, Global).unwrap();
        verify(&frame, case);
        assert!(frame.len() <= selected.frame_bound().unwrap());
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let mut workspace = ReferenceEncoderWorkspace::new(case.source, selected, &owner).unwrap();
        for pass in 0..2 {
            let mut output = prefix(selected);
            for (index, block) in case.blocks.iter().enumerate() {
                let before = workspace.entropy.sequence_encoder.committed_repeats();
                let offset = output.len();
                workspace
                    .emit_next(block.length, index + 1 == case.blocks.len(), &mut output)
                    .unwrap();
                let kind = kind_at(&output[offset..]);
                if pass == 0 {
                    kinds[match kind {
                        BlockKind::Raw => 0,
                        BlockKind::Rle => 1,
                        BlockKind::Compressed => 2,
                    }] += 1;
                }
                if kind != BlockKind::Compressed {
                    assert_eq!(
                        workspace.entropy.sequence_encoder.committed_repeats(),
                        before
                    );
                }
                assert!(workspace.search.literals().is_empty());
                assert!(workspace.search.sequences().is_empty());
            }
            assert_eq!(output, frame, "{} pass {pass}", case.name);
            assert!(workspace.finished);
            assert_eq!(
                monitor.snapshot().live_bytes,
                workspace.retained_capacity() as u128
            );
            let before = monitor.snapshot();
            assert!(monitor.fail_next());
            workspace.reset_retained().unwrap();
            assert_eq!(monitor.snapshot(), before);
            monitor.clear_failure();
            assert_eq!(
                workspace.entropy.sequence_encoder.committed_repeats(),
                [1, 4, 8]
            );
        }
        drop(workspace);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().identities_created, 1);
    }
    assert!(kinds.iter().all(|&count| count != 0), "{kinds:?}");
}

#[test]
fn retained_frame_invalid_boundaries_precede_every_mutation() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let case = cases.iter().find(|c| c.source.len() == 16385).unwrap();
    let selected = plan(case);
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = owner.monitor();
    assert!(monitor.fail_next());
    let before = monitor.snapshot();
    assert_eq!(
        compress_reference_in(&case.source[..16384], selected, &owner).err(),
        Some(Error::InvalidParameter)
    );
    assert_eq!(monitor.snapshot(), before);
    monitor.clear_failure();
    let mut workspace = ReferenceEncoderWorkspace::new(case.source, selected, &owner).unwrap();
    let mut output = prefix(selected);
    let initial = output.clone();
    assert!(monitor.fail_next());
    let before = monitor.snapshot();
    for (length, last) in [
        (0, false),
        (1, false),
        (case.blocks[0].length - 1, false),
        (case.blocks[0].length, true),
        (usize::MAX, true),
    ] {
        assert_eq!(
            workspace.emit_next(length, last, &mut output),
            Err(Error::InvalidParameter)
        );
    }
    assert_eq!(monitor.snapshot(), before);
    assert_eq!(output, initial);
    assert!(!workspace.failed);
    assert_eq!(workspace.emitted, 0);
    monitor.clear_failure();
    for (index, block) in case.blocks.iter().enumerate() {
        workspace
            .emit_next(block.length, index + 1 == case.blocks.len(), &mut output)
            .unwrap();
    }
    verify(&output, case);
    drop(workspace);
    monitor.assert_released();
}

#[test]
fn retained_frame_every_allocation_failure_releases_exact_owners() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let case = cases
        .iter()
        .find(|c| c.name == "wide_frames_mixed")
        .unwrap();
    let selected = plan(case);
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = owner.monitor();
    let frame = compress_reference_in(case.source, selected, owner).unwrap();
    verify(&frame, case);
    let attempts = monitor.snapshot().attempts;
    assert!(attempts > 9);
    assert_eq!(monitor.snapshot().live_blocks, 1);
    drop(frame);
    monitor.assert_released();
    for ordinal in 1..=attempts {
        let owner = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(ordinal),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let monitor = owner.monitor();
        assert_eq!(
            compress_reference_in(case.source, selected, owner).err(),
            Some(Error::AllocationFailed),
            "ordinal {ordinal}"
        );
        monitor.assert_released();
        assert_eq!(
            monitor.snapshot().identities_created,
            monitor.snapshot().owners_dropped
        );
    }
}

#[test]
fn retained_frame_append_failure_preserves_committed_history_until_reset() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let case = cases
        .iter()
        .find(|c| c.name == "wide_frames_mixed")
        .unwrap();
    let selected = plan(case);
    let owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let monitor = owner.monitor();
    let output_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let om = output_owner.monitor();
    let mut workspace = ReferenceEncoderWorkspace::new(case.source, selected, &owner).unwrap();
    let mut output = Vec::new_in(output_owner);
    output.extend_from_slice(&prefix(selected));
    workspace
        .emit_next(case.blocks[0].length, false, &mut output)
        .unwrap();
    output.shrink_to_fit();
    let saved = output.to_vec();
    let repeats = workspace.entropy.sequence_encoder.committed_repeats();
    assert!(om.fail_next());
    assert_eq!(
        workspace.emit_next(case.blocks[1].length, false, &mut output),
        Err(Error::AllocationFailed)
    );
    assert_eq!(&*output, &*saved);
    assert_eq!(workspace.emitted, case.blocks[0].length);
    assert_eq!(
        workspace.entropy.sequence_encoder.committed_repeats(),
        repeats
    );
    assert!(workspace.failed);
    assert!(workspace.entropy.pending_literal_table.is_none());
    assert_eq!(
        workspace.emit_next(case.blocks[1].length, false, &mut output),
        Err(Error::InvalidState)
    );
    om.clear_failure();
    let before = monitor.snapshot();
    assert!(monitor.fail_next());
    workspace.reset_retained().unwrap();
    assert_eq!(monitor.snapshot(), before);
    monitor.clear_failure();
    output.clear();
    output.extend_from_slice(&prefix(selected));
    for (index, block) in case.blocks.iter().enumerate() {
        workspace
            .emit_next(block.length, index + 1 == case.blocks.len(), &mut output)
            .unwrap();
    }
    verify(&output, case);
    drop(workspace);
    drop(output);
    monitor.assert_released();
    om.assert_released();
}

#[cfg(feature = "std")]
#[test]
fn retained_frame_actual_wire_history_capture() {
    use core::fmt::Write;
    use std::io::Write as IoWrite;
    let destination =
        std::env::var_os("ZSTDOXIDE_RETAINED_FRAME_CAPTURE").map(std::path::PathBuf::from);
    let cases = fixture::read(fixture::PACK).unwrap();
    let mut records = alloc::string::String::from("[\n");
    let mut count = 0;
    for case in cases.iter().filter(|c| c.literal_mode == 0) {
        let selected = plan(case);
        let mut workspace = ReferenceEncoderWorkspace::new(case.source, selected, Global).unwrap();
        let mut output = prefix(selected);
        if count != 0 {
            records.push_str(",\n");
        }
        write!(records, "{{\"name\":\"{}\",\"blocks\":[", case.name).unwrap();
        let mut start = 0;
        for (index, block) in case.blocks.iter().enumerate() {
            let incoming = workspace.entropy.sequence_encoder.committed_repeats();
            let offset = output.len();
            workspace
                .emit_next(block.length, index + 1 == case.blocks.len(), &mut output)
                .unwrap();
            let kind = match kind_at(&output[offset..]) {
                BlockKind::Raw => 0,
                BlockKind::Rle => 1,
                BlockKind::Compressed => 2,
            };
            let ReferenceSearchState::RetainedOptimal(state) = &workspace.search else {
                panic!("retained state")
            };
            let (literals, sequences) = state.audit_tokens().unwrap();
            let mut rows = Vec::new();
            let mut anchor = start;
            let mut candidate = incoming;
            for sequence in sequences {
                let s = sequence.sequence;
                let resolved = sequence
                    .offset
                    .resolve(s.literal_length, s.distance, candidate)
                    .unwrap();
                rows.push([
                    anchor as u32,
                    s.literal_length,
                    s.match_length,
                    resolved.offset_value,
                ]);
                anchor += s.literal_length as usize + s.match_length as usize;
                candidate = resolved.next_repeat;
            }
            let tail = start + block.length - anchor;
            let committed = workspace.entropy.sequence_encoder.committed_repeats();
            assert_eq!(committed, if kind == 2 { candidate } else { incoming });
            if index != 0 {
                records.push(',');
            }
            write!(records,"{{\"start\":{start},\"length\":{},\"kind\":{kind},\"incoming\":{incoming:?},\"candidate\":{candidate:?},\"committed\":{committed:?},\"tail\":{tail},\"sequences\":{rows:?},\"literals\":{literals:?}}}",block.length).unwrap();
            start += block.length;
        }
        records.push_str("]}");
        verify(&output, case);
        if let Some(path) = &destination {
            for (suffix, bytes) in [("zst", &output[..]), ("raw", case.source)] {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path.join(alloc::format!("{}.{suffix}", case.name)))
                    .unwrap();
                file.write_all(bytes).unwrap();
            }
        }
        count += 1;
    }
    records.push_str("\n]\n");
    assert_eq!(count, 16);
    if let Some(path) = destination {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path.join("observations.json"))
            .unwrap();
        file.write_all(records.as_bytes()).unwrap();
    }
}

/// A small explicit private schedule keeps foreign interpretation bounded while
/// exercising the same factory, shared entropy and retained publication path.
#[test]
fn retained_frame_small_platform_owners_publication_and_reset() {
    let mut source = [b'a'; 192];
    let mut value = 0x9e3779b9_u32;
    for byte in &mut source[64..128] {
        value ^= value << 13;
        value ^= value >> 17;
        value ^= value << 5;
        *byte = value as u8;
    }
    let parameters = RawCompressionParameters {
        window_log: 10,
        hash_log: 6,
        chain_log: 6,
        search_log: 1,
        min_match: 3,
        target_length: 0,
        strategy: 7,
    }
    .validate()
    .unwrap();
    let selected = ReferenceSearchPlan::RetainedOptimal(
        RetainedOptimalPlan::new(parameters, source.len(), &[64, 64, 64], false).unwrap(),
    );
    let owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let monitor = owner.monitor();
    let mut workspace = ReferenceEncoderWorkspace::new(&source, selected, &owner).unwrap();
    let mut baseline = Vec::new();
    for pass in 0..2 {
        let mut output = prefix(selected);
        let mut observed_kinds = Vec::new();
        for index in 0..3 {
            let before = workspace.entropy.sequence_encoder.committed_repeats();
            let position = output.len();
            workspace.emit_next(64, index == 2, &mut output).unwrap();
            let kind = kind_at(&output[position..]);
            observed_kinds.push(kind);
            if kind != BlockKind::Compressed {
                assert_eq!(
                    workspace.entropy.sequence_encoder.committed_repeats(),
                    before
                );
            }
        }
        assert_eq!(crate::decompress(&output, source.len()).unwrap(), source);
        assert_eq!(
            observed_kinds,
            [BlockKind::Compressed, BlockKind::Raw, BlockKind::Rle]
        );
        if pass == 0 {
            baseline = output;
        } else {
            assert_eq!(output, baseline);
        }
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        workspace.reset_retained().unwrap();
        assert_eq!(monitor.snapshot(), before);
        monitor.clear_failure();
    }
    assert_eq!(monitor.snapshot().identities_created, 1);
    drop(workspace);
    monitor.assert_released();
}
