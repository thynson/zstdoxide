//! Whole-buffer Fast framing with persistent search and transactional entropy.
#![forbid(unsafe_code)]

use alloc::vec::Vec;
use core::alloc::Allocator;

use crate::block_entropy::BlockEntropyEncoder;
use crate::fast_plan::FastExecutionPlan;
use crate::fast_state::FastMatchState;
use crate::frame::{BlockKind, MAX_BLOCK_SIZE};
use crate::literal_encode::LiteralEncoder;
use crate::matchfinder::MatchTable;
use crate::reference_frame::{ReferenceFrameHeader, ReferenceFrameSettings};
use crate::sequence_context::SequenceEncoder;
use crate::{CompressionParameterRequest, Error, Result};

/// Encode one known-size, no-dictionary frame with an admitted Fast profile.
///
/// Admission and output-bound checks precede allocator operations. The original
/// allocator owns the result; all scratch owners retain separate fresh clones.
/// No source or request borrow escapes. This private entry point does not alter
/// the existing named policies or their checksummed frame defaults.
pub(crate) fn compress_fast_in<A: Allocator + Clone>(
    input: &[u8],
    request: &CompressionParameterRequest,
    allocator: A,
) -> Result<Vec<u8, A>> {
    let plan = FastExecutionPlan::new(request, input.len())?;
    let settings = ReferenceFrameSettings::new(plan.source_len(), plan.parameters().window_log())?;
    let bound = ReferenceFrameHeader::raw_frame_bound(&settings)?;
    let mut output = Vec::new_in(allocator);
    output
        .try_reserve_exact(bound)
        .map_err(|_| Error::AllocationFailed)?;
    for &byte in ReferenceFrameHeader::new(&settings).as_bytes() {
        output.push(byte);
    }
    if input.is_empty() {
        // The complete bound already includes this sole final empty raw block.
        for byte in [1, 0, 0] {
            output.push(byte);
        }
        return Ok(output);
    }
    let mut workspace = FastEncoderWorkspace::new(input, plan, output.allocator().clone())?;
    while workspace.emitted < input.len() {
        let length = (input.len() - workspace.emitted).min(plan.block_limit());
        let last = length == input.len() - workspace.emitted;
        workspace.emit_next(length, last, &mut output)?;
    }
    Ok(output)
}

/// One borrowed source and its existing search/entropy owners.
///
/// Search advances before emission. `emitted` advances only after an entire
/// block is appended; any intermediate failure invalidates this workspace.
/// Committed repeats live exclusively in the entropy child, never here.
struct FastEncoderWorkspace<'a, A: Allocator> {
    source: &'a [u8],
    plan: FastExecutionPlan,
    search: FastMatchState<'a, A>,
    entropy: BlockEntropyEncoder<A>,
    emitted: usize,
    failed: bool,
    finished: bool,
}

impl<'a, A: Allocator + Clone> FastEncoderWorkspace<'a, A> {
    fn new(source: &'a [u8], plan: FastExecutionPlan, factory: A) -> Result<Self> {
        if source.len() != plan.source_len() {
            return Err(Error::InvalidParameter);
        }
        let table = MatchTable::with_layout_in(plan.layout(), factory.clone())?;
        let literals = Vec::new_in(factory.clone());
        let sequences = Vec::new_in(factory.clone());
        let literal_encoder = LiteralEncoder::new_in(factory.clone());
        let sequence_encoder = SequenceEncoder::new_in(factory.clone());
        let payload = Vec::new_in(factory);
        let search = FastMatchState::from_parts(source, plan, table, literals, sequences)?;
        let mut entropy =
            BlockEntropyEncoder::from_parts(literal_encoder, sequence_encoder, payload);
        entropy.start_frame(None, None)?;
        Ok(Self {
            source,
            plan,
            search,
            entropy,
            emitted: 0,
            failed: false,
            finished: false,
        })
    }

    /// Emit a consecutive nonempty block, committing entropy only on success.
    /// Invalid scheduling leaves the workspace usable. All failures after
    /// preflight poison it, since search/scratch mutations cannot be rolled back.
    /// Corresponds to the no-dictionary block loop in `ZSTD_compress_frameChunk`.
    fn emit_next<O: Allocator>(
        &mut self,
        length: usize,
        last: bool,
        output: &mut Vec<u8, O>,
    ) -> Result<()> {
        if self.failed || self.finished {
            return Err(Error::InvalidState);
        }
        let end = self
            .emitted
            .checked_add(length)
            .filter(|&end| end <= self.source.len())
            .ok_or(Error::InvalidParameter)?;
        if length == 0 || length > self.plan.block_limit() || last != (end == self.source.len()) {
            return Err(Error::InvalidParameter);
        }
        if let Err(error) = self.emit_checked(end, last, output) {
            self.entropy.discard();
            self.failed = true;
            return Err(error);
        }
        self.emitted = end;
        self.finished = last;
        Ok(())
    }

    fn emit_checked<O: Allocator>(
        &mut self,
        end: usize,
        last: bool,
        output: &mut Vec<u8, O>,
    ) -> Result<()> {
        let block = &self.source[self.emitted..end];
        let initial_repeat = self.entropy.sequence_encoder.committed_repeats();
        let parsed = self.search.parse_next(block.len(), initial_repeat)?;
        let worthwhile = if parsed.is_some() {
            self.entropy.prepare_fast(
                self.search.literals(),
                self.search.sequences(),
                initial_repeat,
                block.len(),
                self.plan.raw_literals(),
            )?
        } else {
            // One through six bytes bypass fresh entropy, including literal RLE.
            false
        };
        // The reference gate is after entropy work, even for a uniform block.
        // Rejected gain has effective compressed size zero. Independent entropy
        // may select a different payload/mode from C while preserving the format.
        let rle = parsed.is_some()
            && self.emitted != 0
            && (!worthwhile || self.entropy.as_bytes().len() < 25)
            && block.iter().all(|&byte| byte == block[0]);
        let (kind, payload) = if rle {
            (BlockKind::Rle, &block[..1])
        } else if worthwhile {
            (BlockKind::Compressed, self.entropy.as_bytes())
        } else {
            (BlockKind::Raw, block)
        };
        append_block(output, block.len(), kind, payload, last)?;
        if kind == BlockKind::Compressed {
            self.entropy.commit();
        } else {
            self.entropy.discard();
        }
        Ok(())
    }

    #[cfg(test)]
    fn retained_capacity(&self) -> usize {
        self.search.retained_capacity() + self.entropy.retained_capacity()
    }
}

/// Reserve the complete selected block before changing any output byte.
fn append_block<O: Allocator>(
    output: &mut Vec<u8, O>,
    regenerated_size: usize,
    kind: BlockKind,
    payload: &[u8],
    last: bool,
) -> Result<()> {
    if regenerated_size > MAX_BLOCK_SIZE
        || payload.len() > regenerated_size
        || (kind == BlockKind::Raw && payload.len() != regenerated_size)
        || (kind == BlockKind::Rle && payload.len() != 1)
    {
        return Err(Error::CorruptData);
    }
    let (size, tag) = match kind {
        BlockKind::Raw => (regenerated_size, 0),
        BlockKind::Rle => (regenerated_size, 1),
        BlockKind::Compressed => (payload.len(), 2),
    };
    let value = (u32::try_from(size).map_err(|_| Error::ResourceLimit)? << 3)
        | (tag << 1)
        | u32::from(last);
    let additional = payload.len().checked_add(3).ok_or(Error::ResourceLimit)?;
    output
        .try_reserve_exact(additional)
        .map_err(|_| Error::AllocationFailed)?;
    for byte in &value.to_le_bytes()[..3] {
        output.push(*byte);
    }
    for &byte in payload {
        output.push(byte);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{alloc::Global, vec};
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    use crate::cursor::ByteCursor;
    use crate::frame::BlockHeader;
    use crate::{FrameHeader, ReferenceStrategy};

    fn request(window_log: u8) -> CompressionParameterRequest {
        let mut request = CompressionParameterRequest::with_level(1);
        request.set_window_log(window_log).unwrap();
        request.set_hash_log(10).unwrap();
        request.set_min_match(4).unwrap();
        request
    }

    fn noise(mut state: u32, length: usize) -> Vec<u8> {
        (0..length)
            .map(|_| {
                state ^= state.wrapping_shl(13);
                state ^= state >> 17;
                state ^= state.wrapping_shl(5);
                state as u8
            })
            .collect()
    }

    fn kinds(frame: &[u8]) -> Vec<(BlockKind, usize)> {
        let (_, used) = FrameHeader::parse(frame).unwrap();
        let mut cursor = ByteCursor::new(&frame[used..]);
        let mut result = Vec::new();
        loop {
            let header = BlockHeader::read(&mut cursor, MAX_BLOCK_SIZE).unwrap();
            result.push((header.kind, header.size));
            cursor
                .take(if header.kind == BlockKind::Rle {
                    1
                } else {
                    header.size
                })
                .unwrap();
            if header.last {
                break;
            }
        }
        assert_eq!(used + cursor.position(), frame.len());
        result
    }

    fn start<'a, A: Allocator + Clone>(
        input: &'a [u8],
        request: &CompressionParameterRequest,
        allocator: A,
    ) -> (FastEncoderWorkspace<'a, A>, Vec<u8>) {
        let plan = FastExecutionPlan::new(request, input.len()).unwrap();
        let settings =
            ReferenceFrameSettings::new(plan.source_len(), plan.parameters().window_log()).unwrap();
        let frame = ReferenceFrameHeader::new(&settings).as_bytes().to_vec();
        (
            FastEncoderWorkspace::new(input, plan, allocator).unwrap(),
            frame,
        )
    }

    #[test]
    fn empty_and_tiny_frames_are_raw_with_reference_header_defaults() {
        for length in 0..=6 {
            for level in [1, -7] {
                let source = vec![b'x'; length];
                let request = CompressionParameterRequest::with_level(level);
                let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
                let identity = owner.identity();
                let monitor = owner.monitor();
                let frame = compress_fast_in(&source, &request, owner).unwrap();
                assert_eq!(frame.allocator().identity(), identity);
                assert_eq!(kinds(&frame), [(BlockKind::Raw, length)]);
                assert_eq!(crate::decompress(&frame, length).unwrap(), source);
                let (header, _) = FrameHeader::parse(&frame).unwrap();
                assert_eq!(header.content_size, Some(length as u64));
                assert_eq!(header.window_size, length as u64);
                assert!(!header.has_checksum);
                assert_eq!(header.dictionary_id, 0);
                assert_eq!(monitor.snapshot().live_blocks, 1);
                if length == 0 {
                    // No workspace factory, sampled table, or entropy child.
                    assert_eq!(monitor.snapshot().identities_created, 1);
                    assert_eq!(monitor.snapshot().attempts, 1);
                    assert_eq!(
                        frame.as_slice(),
                        &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 1, 0, 0]
                    );
                }
                drop(frame);
                monitor.assert_released();
                assert_eq!(
                    monitor.snapshot().owners_dropped,
                    monitor.snapshot().identities_created
                );
            }
        }
    }

    #[test]
    fn first_uniform_block_is_not_rle_and_tiny_blocks_bypass_stale_entropy() {
        let source = [b'x'; 205];
        let (mut workspace, mut frame) = start(&source, &request(10), Global);
        let mut repeats = [1, 4, 8];
        for (index, length) in [64, 64, 6, 7, 64].into_iter().enumerate() {
            workspace.emit_next(length, index == 4, &mut frame).unwrap();
            let after = workspace.entropy.sequence_encoder.committed_repeats();
            if index == 0 {
                // Repeat1 at distance one can leave the initial triple unchanged.
                repeats = after;
            } else {
                assert_eq!(after, repeats);
            }
        }
        assert_eq!(
            kinds(&frame)
                .iter()
                .map(|entry| entry.0)
                .collect::<Vec<_>>(),
            [
                BlockKind::Compressed,
                BlockKind::Rle,
                BlockKind::Raw,
                BlockKind::Rle,
                BlockKind::Rle
            ]
        );
        assert_eq!(workspace.emitted, source.len());
        assert!(workspace.finished);
        assert_eq!(
            workspace.emit_next(1, true, &mut frame),
            Err(Error::InvalidState)
        );
        assert_eq!(crate::decompress(&frame, source.len()).unwrap(), source);
    }

    #[test]
    fn raw_then_copied_source_retains_sampling_but_only_compressed_repeats_publish() {
        let prefix = noise(0x1234_5678, 1024);
        let input = [prefix.as_slice(), prefix.as_slice()].concat();
        let (mut workspace, mut frame) = start(&input, &request(12), Global);
        workspace.emit_next(1024, false, &mut frame).unwrap();
        assert_eq!(
            workspace.entropy.sequence_encoder.committed_repeats(),
            [1, 4, 8]
        );
        workspace.emit_next(1024, true, &mut frame).unwrap();
        assert_eq!(
            workspace.entropy.sequence_encoder.committed_repeats(),
            [1024, 1, 4]
        );
        let blocks = kinds(&frame);
        assert_eq!(blocks[0], (BlockKind::Raw, 1024));
        assert_eq!(blocks[1].0, BlockKind::Compressed);
        assert!(blocks[1].1 < 64);
        assert_eq!(crate::decompress(&frame, input.len()).unwrap(), input);
    }

    #[test]
    fn mixed_fallback_blocks_preserve_last_published_state_and_decode_exactly() {
        let pattern = b"a phrase with repeated source text!".repeat(128);
        let random = noise(0x3344_5566, pattern.len());
        let uniform = vec![b'z'; pattern.len()];
        let input = [
            pattern.as_slice(),
            random.as_slice(),
            uniform.as_slice(),
            pattern.as_slice(),
        ]
        .concat();
        let (mut workspace, mut frame) = start(&input, &request(15), Global);
        workspace
            .emit_next(pattern.len(), false, &mut frame)
            .unwrap();
        let first = workspace.entropy.sequence_encoder.committed_repeats();
        assert_ne!(first, [1, 4, 8]);
        for _ in 0..2 {
            workspace
                .emit_next(pattern.len(), false, &mut frame)
                .unwrap();
            assert_eq!(
                workspace.entropy.sequence_encoder.committed_repeats(),
                first
            );
        }
        workspace
            .emit_next(pattern.len(), true, &mut frame)
            .unwrap();
        assert_eq!(
            kinds(&frame)
                .iter()
                .map(|entry| entry.0)
                .collect::<Vec<_>>(),
            [
                BlockKind::Compressed,
                BlockKind::Raw,
                BlockKind::Rle,
                BlockKind::Compressed
            ]
        );
        assert_eq!(crate::decompress(&frame, input.len()).unwrap(), input);
    }

    #[test]
    fn actual_window_controls_cross_block_matches_with_truthful_frame_metadata() {
        let first = noise(0x789a_bcde, MAX_BLOCK_SIZE);
        let other = noise(0x1020_3040, MAX_BLOCK_SIZE);
        let input = [first.as_slice(), other.as_slice(), first.as_slice()].concat();
        let mut lengths = Vec::new();
        for window_log in [17, 19] {
            let mut request = request(window_log);
            request.set_hash_log(16).unwrap();
            let frame = compress_fast_in(&input, &request, Global).unwrap();
            let (header, _) = FrameHeader::parse(&frame).unwrap();
            assert_eq!(header.window_size, input.len().min(1 << window_log) as u64);
            let blocks = kinds(&frame);
            assert_eq!(blocks.len(), 3);
            assert_eq!(blocks[0].0, BlockKind::Raw);
            assert_eq!(blocks[1].0, BlockKind::Raw);
            assert_eq!(
                blocks[2].0,
                if window_log == 17 {
                    BlockKind::Raw
                } else {
                    BlockKind::Compressed
                }
            );
            assert_eq!(crate::decompress(&frame, input.len()).unwrap(), input);
            lengths.push(frame.len());
        }
        assert!(lengths[1] + 100_000 < lengths[0]);
    }

    #[test]
    fn invalid_admission_and_schedules_preserve_armed_allocators() {
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut unsupported = request(10);
        unsupported.set_strategy(Some(ReferenceStrategy::DoubleFast));
        assert!(monitor.fail_next());
        let before = monitor.snapshot();
        assert_eq!(
            compress_fast_in(b"source", &unsupported, &owner).err(),
            Some(Error::UnsupportedFeature)
        );
        assert_eq!(monitor.snapshot(), before);
        monitor.clear_failure();
        let source = [b'a'; 96];
        let (mut workspace, mut frame) = start(&source, &request(10), &owner);
        assert!(monitor.fail_next());
        let before = monitor.snapshot();
        let frame_before = frame.clone();
        for (length, last) in [
            (0, false),
            (97, true),
            (usize::MAX, true),
            (1, true),
            (96, false),
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
        workspace.emit_next(96, true, &mut frame).unwrap();
        assert_eq!(crate::decompress(&frame, source.len()).unwrap(), source);
        drop(workspace);
        monitor.assert_released();
    }

    #[test]
    fn failed_output_reservation_preserves_bytes_and_committed_repeats_then_poisoned() {
        let source = b"a repeated phrase with source history.".repeat(128);
        let midpoint = source.len() / 2;
        let workspace_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let output_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let wm = workspace_owner.monitor();
        let om = output_owner.monitor();
        let (mut workspace, mut frame) = start(&source, &request(14), workspace_owner);
        workspace.emit_next(midpoint, false, &mut frame).unwrap();
        let first_repeat = workspace.entropy.sequence_encoder.committed_repeats();
        assert_ne!(first_repeat, [1, 4, 8]);
        // A separate output owner has exactly the existing prefix capacity.
        let mut output = Vec::new_in(output_owner);
        output.try_reserve_exact(frame.len()).unwrap();
        for &byte in &frame {
            output.push(byte);
        }
        assert_eq!(output.len(), output.capacity());
        assert!(om.fail_next());
        assert_eq!(
            workspace.emit_next(midpoint, true, &mut output),
            Err(Error::AllocationFailed)
        );
        assert_eq!(output.as_slice(), frame);
        assert_eq!(workspace.emitted, midpoint);
        assert_eq!(
            workspace.entropy.sequence_encoder.committed_repeats(),
            first_repeat
        );
        assert!(workspace.entropy.pending_literal_table.is_none());
        assert!(workspace.failed);
        om.clear_failure();
        let after = (wm.snapshot(), om.snapshot());
        for (length, last) in [(0, false), (midpoint, true), (usize::MAX, true)] {
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
        let recovered = compress_fast_in(&source, &request(14), Global).unwrap();
        assert_eq!(crate::decompress(&recovered, source.len()).unwrap(), source);
    }

    #[test]
    fn exact_workspace_owners_and_borrowed_nonclone_result_release_independently() {
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let input = b"a source with useful repetitions".repeat(64);
        let (mut workspace, mut frame) = start(&input, &request(12), owner);
        workspace.emit_next(input.len(), true, &mut frame).unwrap();
        assert_eq!(
            monitor.snapshot().live_bytes,
            workspace.retained_capacity() as u128
        );
        assert!(monitor.snapshot().identities_created >= 6);
        drop(workspace);
        monitor.assert_released();
        assert_eq!(
            monitor.snapshot().owners_dropped,
            monitor.snapshot().identities_created
        );

        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let frame = {
            let source = input.clone();
            let request = request(12);
            compress_fast_in(&source, &request, &owner).unwrap()
        };
        assert_eq!(frame.allocator().identity(), owner.identity());
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        assert_eq!(monitor.snapshot().live_bytes, frame.capacity() as u128);
        assert_eq!(crate::decompress(&frame, input.len()).unwrap(), input);
        drop(frame);
        monitor.assert_released();
    }

    #[test]
    fn each_small_complete_frame_allocation_failure_releases_every_owner() {
        for input in [&b""[..], &b"a useful phrase repeated many times!".repeat(4)] {
            for level in [1, -7] {
                let request = CompressionParameterRequest::with_level(level);
                let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
                let monitor = owner.monitor();
                let frame = compress_fast_in(input, &request, owner).unwrap();
                let attempts = monitor.snapshot().attempts;
                assert_eq!(monitor.snapshot().live_blocks, 1);
                assert_eq!(crate::decompress(&frame, input.len()).unwrap(), input);
                drop(frame);
                monitor.assert_released();
                for ordinal in 1..=attempts {
                    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
                    let monitor = owner.monitor();
                    monitor.set_fail_at(NonZeroU64::new(ordinal));
                    assert_eq!(
                        compress_fast_in(input, &request, owner).err(),
                        Some(Error::AllocationFailed)
                    );
                    assert_eq!(monitor.snapshot().attempts, ordinal);
                    assert_eq!(monitor.snapshot().failures, 1);
                    assert_eq!(
                        monitor.snapshot().owners_dropped,
                        monitor.snapshot().identities_created
                    );
                    monitor.assert_released();
                }
            }
        }
    }
}
