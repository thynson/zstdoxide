//! Parser-independent block entropy owners and transactional publication.
//!
//! This composite moves complete existing owners without cloning an allocator.
//! Memory access stays in each child's safe or independently gated backend.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::dictionary::DictionaryEntropyView;
use crate::frame::MAX_BLOCK_SIZE;
use crate::huffman_codebook::HuffmanCodebook;
use crate::literal_encode::LiteralEncoder;
use crate::reference_policy::minimum_gain;
use crate::sequence_context::{LogicalSequence, SequenceEncoder, SequenceWithOffset};
use crate::sequence_plan::PlannedSequence;
use crate::{Error, Result};

/// Reusable entropy state independent of named or reference match search.
///
/// Every allocation retains the allocator that created it. No parser, source
/// borrow, dictionary handle or extra allocator factory is stored here. Candidate
/// trees/tables/repeats become committed state only after their bytes are emitted.
/// Field aggregation does not promise the old workspace's interleaved drop order;
/// every independent owner is still released through its exact allocator.
pub(crate) struct BlockEntropyEncoder<A: Allocator = Global> {
    literal_encoder: LiteralEncoder<A>,
    pub(crate) literal_table: Option<HuffmanCodebook>,
    pub(crate) pending_literal_table: Option<HuffmanCodebook>,
    pub(crate) sequence_encoder: SequenceEncoder<A>,
    payload: Vec<u8, A>,
}

impl<A: Allocator> BlockEntropyEncoder<A> {
    /// Move preconstructed owners in without making any allocation or clone.
    /// The named workspace constructs these children in its established order,
    /// interleaved with parser vectors, before assembling this composite.
    pub(crate) const fn from_parts(
        literal_encoder: LiteralEncoder<A>,
        sequence_encoder: SequenceEncoder<A>,
        payload: Vec<u8, A>,
    ) -> Self {
        Self {
            literal_encoder,
            literal_table: None,
            pending_literal_table: None,
            sequence_encoder,
            payload,
        }
    }

    /// Discard candidate publication while retaining all reusable storage.
    pub(crate) fn discard(&mut self) {
        self.pending_literal_table = None;
        self.sequence_encoder.discard();
    }

    /// Publish only after output capacity is reserved and every byte is appended.
    /// No allocation or user-defined allocator operation occurs here.
    pub(crate) fn commit(&mut self) {
        if let Some(table) = self.pending_literal_table.take() {
            self.literal_table = Some(table);
        }
        self.sequence_encoder.commit();
    }

    /// Reset frame state while retaining children and payload capacities.
    pub(crate) fn reset(&mut self) {
        self.literal_table = None;
        self.pending_literal_table = None;
        self.sequence_encoder.reset();
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.payload
    }

    /// Owned payload capacity, excluding this inline composite and child handles.
    pub(crate) fn retained_capacity(&self) -> usize {
        self.literal_encoder.retained_capacity()
            + self.sequence_encoder.retained_capacity()
            + self.payload.capacity()
    }
}

impl<A: Allocator + Clone> BlockEntropyEncoder<A> {
    /// Prepare a complete reference candidate without publishing frame state.
    ///
    /// Snapshot and complete reconstruction checks precede allocation. Offset
    /// intent is checked in source order, but source/window eligibility remains
    /// the parser's responsibility. Failure or insufficient gain clears exposed
    /// payload and candidate publication; committed state and owners survive.
    pub(crate) fn prepare_reference(
        &mut self,
        literals: &[u8],
        sequences: &[SequenceWithOffset],
        initial_repeat: [u32; 3],
        regenerated_size: usize,
        raw_literals: bool,
    ) -> Result<bool> {
        self.discard();
        self.payload.clear();
        let result = self.prepare_reference_inner(
            literals,
            sequences,
            initial_repeat,
            regenerated_size,
            raw_literals,
        );
        if !matches!(result, Ok(true)) {
            self.discard();
            self.payload.clear();
        }
        result
    }

    fn prepare_reference_inner(
        &mut self,
        literals: &[u8],
        sequences: &[SequenceWithOffset],
        initial_repeat: [u32; 3],
        regenerated_size: usize,
        raw_literals: bool,
    ) -> Result<bool> {
        if initial_repeat != self.sequence_encoder.committed_repeats() {
            return Err(Error::InvalidState);
        }
        validate_reference_reconstruction(
            literals.len(),
            sequences,
            initial_repeat,
            regenerated_size,
        )?;
        // The normal C block builder bypasses fresh sequence construction below
        // seven bytes. Avoid entropy work for a candidate that cannot be emitted.
        if regenerated_size < 7 {
            return Ok(false);
        }
        self.pending_literal_table = self.literal_encoder.select_reference(
            literals,
            self.literal_table.as_ref(),
            raw_literals,
        )?;
        self.sequence_encoder
            .prepare_with_offsets(sequences, initial_repeat)?;
        self.join_sections()?;
        // Includes both section headers; excludes the outer three-byte block
        // header, as in the shared reference whole-block gain comparison.
        Ok(reference_payload_is_worthwhile(
            self.payload.len(),
            regenerated_size,
        ))
    }

    /// Preserve dictionary preparation ordering: the caller constructs the
    /// plain literal tree and search seeds before entering this fallible step.
    /// A failed setup requires frame reset, just as in the enclosing workspace.
    pub(crate) fn start_frame(
        &mut self,
        entropy: Option<DictionaryEntropyView<'_>>,
        literal_table: Option<HuffmanCodebook>,
    ) -> Result<()> {
        self.sequence_encoder.start_frame_with_entropy(entropy)?;
        self.literal_table = literal_table;
        self.pending_literal_table = None;
        Ok(())
    }

    /// Prepare the existing named policy's literal/sequence representation.
    /// Selection and reserve order remain unchanged; the caller decides whether
    /// the complete payload is worthwhile and owns publication after output.
    pub(crate) fn prepare_named(
        &mut self,
        literals: &[u8],
        sequences: &[LogicalSequence],
    ) -> Result<()> {
        self.pending_literal_table = self
            .literal_encoder
            .select(literals, self.literal_table.as_ref())?;
        self.sequence_encoder.prepare(sequences)?;
        self.join_sections()
    }

    fn join_sections(&mut self) -> Result<()> {
        let stream = self.sequence_encoder.as_bytes();
        let section = self.literal_encoder.as_bytes();
        let capacity = section
            .len()
            .checked_add(stream.len())
            .ok_or(Error::ResourceLimit)?;
        self.payload.clear();
        self.payload
            .try_reserve_exact(capacity)
            .map_err(|_| Error::AllocationFailed)?;
        for &byte in section {
            self.payload.push(byte);
        }
        for &byte in stream {
            self.payload.push(byte);
        }
        Ok(())
    }
}

/// Validate lengths and repeat intent without allocating or reading source data.
/// Literals includes both each sequence's leading literals and the final tail.
fn validate_reference_reconstruction(
    literal_count: usize,
    sequences: &[SequenceWithOffset],
    initial_repeat: [u32; 3],
    regenerated_size: usize,
) -> Result<()> {
    if regenerated_size > MAX_BLOCK_SIZE || literal_count > regenerated_size {
        return Err(Error::CorruptData);
    }
    if sequences.len() > MAX_BLOCK_SIZE / 4 {
        return Err(Error::ResourceLimit);
    }
    let mut used_literals = 0_usize;
    let mut matched_bytes = 0_usize;
    let mut repeats = initial_repeat;
    for record in sequences {
        let sequence = record.sequence;
        if sequence.match_length < 4 {
            return Err(Error::CorruptData);
        }
        used_literals = used_literals
            .checked_add(usize::try_from(sequence.literal_length).map_err(|_| Error::CorruptData)?)
            .filter(|&count| count <= literal_count)
            .ok_or(Error::CorruptData)?;
        matched_bytes = matched_bytes
            .checked_add(usize::try_from(sequence.match_length).map_err(|_| Error::CorruptData)?)
            .filter(|&count| count <= regenerated_size)
            .ok_or(Error::CorruptData)?;
        repeats = PlannedSequence::with_offset(
            sequence.literal_length,
            sequence.match_length,
            sequence.distance,
            record.offset,
            repeats,
        )?
        .next_repeat;
    }
    if literal_count.checked_add(matched_bytes) != Some(regenerated_size) {
        return Err(Error::CorruptData);
    }
    Ok(())
}

fn reference_payload_is_worthwhile(payload_bytes: usize, source_bytes: usize) -> bool {
    source_bytes
        .checked_sub(minimum_gain(source_bytes))
        .is_some_and(|limit| payload_bytes < limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    use crate::sequence_plan::OffsetIntent;
    use core::num::NonZeroU64;

    fn context<A: Allocator + Clone>(owner: A) -> BlockEntropyEncoder<A> {
        let literal = LiteralEncoder::new_in(owner.clone());
        let sequence = SequenceEncoder::new_in(owner.clone());
        BlockEntropyEncoder::from_parts(literal, sequence, Vec::new_in(owner))
    }

    fn record(ll: u32, ml: u32, distance: u32, offset: OffsetIntent) -> SequenceWithOffset {
        SequenceWithOffset {
            sequence: LogicalSequence {
                literal_length: ll,
                match_length: ml,
                distance,
            },
            offset,
        }
    }

    fn append_compressed(frame: &mut Vec<u8>, payload: &[u8], last: bool) {
        let header = ((payload.len() as u32) << 3) | 4 | u32::from(last);
        frame.extend_from_slice(&header.to_le_bytes()[..3]);
        frame.extend_from_slice(payload);
    }

    #[test]
    fn fast_candidate_emits_explicit_state_only_after_complete_block_publication() {
        let mut entropy = context(Global);
        let first = [record(10, 90, 10, OffsetIntent::Explicit)];
        assert!(
            entropy
                .prepare_reference(b"abcdefghij", &first, [1, 4, 8], 100, true)
                .unwrap()
        );
        assert_eq!(entropy.sequence_encoder.committed_repeats(), [1, 4, 8]);
        let mut frame = Vec::from(&[0x28, 0xb5, 0x2f, 0xfd, 0x20, 132][..]);
        append_compressed(&mut frame, entropy.as_bytes(), false);
        entropy.commit();
        assert_eq!(entropy.sequence_encoder.committed_repeats(), [10, 1, 4]);
        let second = [record(0, 32, 1, OffsetIntent::Repeat1)];
        assert!(
            entropy
                .prepare_reference(&[], &second, [10, 1, 4], 32, true)
                .unwrap()
        );
        assert_eq!(entropy.sequence_encoder.committed_repeats(), [10, 1, 4]);
        append_compressed(&mut frame, entropy.as_bytes(), true);
        entropy.commit();
        assert_eq!(entropy.sequence_encoder.committed_repeats(), [1, 10, 4]);
        let mut expected = Vec::new();
        for _ in 0..10 {
            expected.extend_from_slice(b"abcdefghij");
        }
        expected.resize(132, b'j');
        assert_eq!(crate::decompress(&frame, 132).unwrap(), expected);
    }

    #[test]
    fn fast_preflight_rejects_snapshot_and_reconstruction_before_any_owner_request() {
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut entropy = context(owner);
        let valid = [record(10, 90, 10, OffsetIntent::Explicit)];
        assert!(
            entropy
                .prepare_reference(b"abcdefghij", &valid, [1, 4, 8], 100, true)
                .unwrap()
        );
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        assert_eq!(
            entropy.prepare_reference(&[], &[], [9, 4, 8], usize::MAX, false),
            Err(Error::InvalidState)
        );
        assert_eq!(monitor.snapshot(), before);
        assert!(entropy.as_bytes().is_empty());
        assert!(entropy.sequence_encoder.as_bytes().is_empty());
        for (literals, sequences, size) in [
            (&b"abcdefghij"[..], valid.as_slice(), 99),
            (&b"abcdefghi"[..], valid.as_slice(), 99),
            (&b"abcdefghij"[..], valid.as_slice(), MAX_BLOCK_SIZE + 1),
            (&b"a"[..], &[record(1, 3, 1, OffsetIntent::Explicit)][..], 4),
            (
                &b"abcdefghij"[..],
                &[record(10, 90, 10, OffsetIntent::Repeat1)][..],
                100,
            ),
            (
                &b"abcdefghij"[..],
                &[record(u32::MAX, 90, 10, OffsetIntent::Explicit)][..],
                100,
            ),
        ] {
            assert_eq!(
                entropy.prepare_reference(literals, sequences, [1, 4, 8], size, false),
                Err(Error::CorruptData)
            );
            assert_eq!(monitor.snapshot(), before);
            assert!(entropy.as_bytes().is_empty());
            assert!(entropy.pending_literal_table.is_none());
            entropy.commit();
            assert_eq!(entropy.sequence_encoder.committed_repeats(), [1, 4, 8]);
        }
        drop(entropy);
        monitor.assert_released();
    }

    #[test]
    fn fast_block_gain_is_strict_and_rejected_candidates_preserve_committed_state() {
        for (source, gain) in [
            (0, 2),
            (63, 2),
            (64, 3),
            (127, 3),
            (128, 4),
            (131_072, 2050),
        ] {
            assert_eq!(minimum_gain(source), gain);
            if let Some(boundary) = source.checked_sub(gain) {
                assert!(!reference_payload_is_worthwhile(boundary, source));
                assert!(reference_payload_is_worthwhile(boundary - 1, source));
            } else {
                assert!(!reference_payload_is_worthwhile(0, source));
            }
        }
        let mut entropy = context(Global);
        let first = [record(10, 90, 10, OffsetIntent::Explicit)];
        assert!(
            entropy
                .prepare_reference(b"abcdefghij", &first, [1, 4, 8], 100, true)
                .unwrap()
        );
        entropy.commit();
        let committed = entropy.sequence_encoder.committed_repeats();
        for input in [&b"abcdef"[..], &b"abcdefg"[..], &b"abcdefghijklmno"[..]] {
            assert!(
                !entropy
                    .prepare_reference(input, &[], committed, input.len(), true)
                    .unwrap()
            );
            assert!(entropy.as_bytes().is_empty());
            assert!(entropy.sequence_encoder.as_bytes().is_empty());
            assert!(entropy.pending_literal_table.is_none());
            entropy.commit();
            assert_eq!(entropy.sequence_encoder.committed_repeats(), committed);
        }
    }

    #[test]
    fn fast_reconstruction_counts_final_literals_and_checks_complete_total() {
        let sequence = [record(10, 90, 10, OffsetIntent::Explicit)];
        assert_eq!(
            validate_reference_reconstruction(13, &sequence, [1, 4, 8], 103),
            Ok(())
        );
        assert_eq!(
            validate_reference_reconstruction(13, &sequence, [1, 4, 8], 100),
            Err(Error::CorruptData)
        );
        let mut entropy = context(Global);
        assert!(
            entropy
                .prepare_reference(b"abcdefghijXYZ", &sequence, [1, 4, 8], 103, true)
                .unwrap()
        );
        let mut frame = Vec::from(&[0x28, 0xb5, 0x2f, 0xfd, 0x20, 103][..]);
        append_compressed(&mut frame, entropy.as_bytes(), true);
        let decoded = crate::decompress(&frame, 103).unwrap();
        assert_eq!(&decoded[100..], b"XYZ");
        for chunk in decoded[..100].chunks(10) {
            assert_eq!(chunk, b"abcdefghij");
        }
    }

    fn committed_tree_and_repeats<A: Allocator + Clone>(
        entropy: &mut BlockEntropyEncoder<A>,
    ) -> Vec<u8> {
        let literals: Vec<_> = (0..64).map(|i| (i % 16) as u8).collect();
        let first = [record(64, 128, 16, OffsetIntent::Explicit)];
        assert!(
            entropy
                .prepare_reference(&literals, &first, [1, 4, 8], 192, false)
                .unwrap()
        );
        assert!(entropy.pending_literal_table.is_some());
        let payload = entropy.as_bytes().to_vec();
        entropy.commit();
        assert_eq!(entropy.sequence_encoder.committed_repeats(), [16, 1, 4]);
        assert!(entropy.literal_table.as_ref().unwrap().covers(&literals));
        payload
    }

    fn replacement_candidate() -> (Vec<u8>, [SequenceWithOffset; 2]) {
        let literals = (0..32).map(|i| (i % 2) as u8).collect();
        let sequences = [
            record(32, 48, 32, OffsetIntent::Explicit),
            record(0, 48, 8, OffsetIntent::Explicit),
        ];
        (literals, sequences)
    }

    fn committed_codes<A: Allocator>(entropy: &BlockEntropyEncoder<A>) -> [Result<(u16, u8)>; 256] {
        core::array::from_fn(|i| {
            entropy
                .literal_table
                .as_ref()
                .unwrap()
                .code(i as u8)
                .map(|code| (code.value, code.bits))
        })
    }

    #[test]
    fn fast_invalid_replacement_discards_pending_tree_but_preserves_committed_entropy() {
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut entropy = context(owner);
        committed_tree_and_repeats(&mut entropy);
        let original = committed_codes(&entropy);
        let (literals, sequences) = replacement_candidate();
        for stale in [true, false] {
            assert!(
                entropy
                    .prepare_reference(&literals, &sequences, [16, 1, 4], 128, false)
                    .unwrap()
            );
            // The candidate has a different alphabet and repeat triple from
            // committed state. Replacement must discard both publication paths.
            assert!(
                entropy
                    .pending_literal_table
                    .as_ref()
                    .unwrap()
                    .code(2)
                    .is_err()
            );
            assert_eq!(committed_codes(&entropy), original);
            let before = monitor.snapshot();
            assert!(monitor.fail_next());
            let (snapshot, size, error) = if stale {
                ([8, 32, 16], 128, Error::InvalidState)
            } else {
                ([16, 1, 4], 127, Error::CorruptData)
            };
            assert_eq!(
                entropy.prepare_reference(&literals, &sequences, snapshot, size, false),
                Err(error)
            );
            assert_eq!(monitor.snapshot(), before);
            assert!(entropy.pending_literal_table.is_none());
            assert!(entropy.as_bytes().is_empty());
            assert!(entropy.sequence_encoder.as_bytes().is_empty());
            entropy.commit();
            assert_eq!(committed_codes(&entropy), original);
            assert_eq!(entropy.sequence_encoder.committed_repeats(), [16, 1, 4]);
            monitor.clear_failure();
        }
        drop(entropy);
        monitor.assert_released();
    }

    #[test]
    fn fast_late_warmed_failure_preserves_committed_tree_repeats_and_decodable_recovery() {
        let (literals, sequences) = replacement_candidate();
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut probe = context(owner);
        committed_tree_and_repeats(&mut probe);
        let before = monitor.snapshot().attempts;
        assert!(
            probe
                .prepare_reference(&literals, &sequences, [16, 1, 4], 128, false)
                .unwrap()
        );
        let ordinal = monitor.snapshot().attempts - before;
        let expected = probe.as_bytes().to_vec();
        assert!(ordinal > 1);
        assert!(probe.pending_literal_table.is_some());
        drop(probe);
        monitor.assert_released();

        // Select just the last observed request rather than repeat the full
        // fresh-context ordinal campaign. This exercises warmed publication.
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut entropy = context(owner);
        let first = committed_tree_and_repeats(&mut entropy);
        let original = committed_codes(&entropy);
        monitor.set_fail_at(NonZeroU64::new(monitor.snapshot().attempts + ordinal));
        assert_eq!(
            entropy.prepare_reference(&literals, &sequences, [16, 1, 4], 128, false),
            Err(Error::AllocationFailed)
        );
        assert_eq!(monitor.snapshot().failures, 1);
        assert!(entropy.as_bytes().is_empty());
        assert!(entropy.sequence_encoder.as_bytes().is_empty());
        assert!(entropy.pending_literal_table.is_none());
        entropy.commit();
        assert_eq!(committed_codes(&entropy), original);
        assert_eq!(entropy.sequence_encoder.committed_repeats(), [16, 1, 4]);
        monitor.clear_failure();
        assert!(
            entropy
                .prepare_reference(&literals, &sequences, [16, 1, 4], 128, false)
                .unwrap()
        );
        assert_eq!(entropy.as_bytes(), expected);
        let mut frame = Vec::from(&[0x28, 0xb5, 0x2f, 0xfd, 0x60, 64, 0][..]);
        append_compressed(&mut frame, &first, false);
        append_compressed(&mut frame, entropy.as_bytes(), true);
        entropy.commit();
        assert_eq!(entropy.sequence_encoder.committed_repeats(), [8, 32, 16]);
        assert!(entropy.literal_table.as_ref().unwrap().code(2).is_err());
        let mut expected_source: Vec<_> = (0..192).map(|i| (i % 16) as u8).collect();
        expected_source.extend((0..128).map(|i| (i % 2) as u8));
        assert_eq!(crate::decompress(&frame, 320).unwrap(), expected_source);
        drop(entropy);
        monitor.assert_released();
        assert_eq!(
            monitor.snapshot().owners_dropped,
            monitor.snapshot().identities_created
        );
    }

    #[test]
    fn fast_every_fresh_allocation_failure_hides_candidate_and_recovers_exact_owners() {
        let sequences = [record(10, 90, 10, OffsetIntent::Explicit)];
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut probe = context(owner);
        assert!(
            probe
                .prepare_reference(b"abcdefghij", &sequences, [1, 4, 8], 100, false)
                .unwrap()
        );
        let expected = probe.as_bytes().to_vec();
        let attempts = monitor.snapshot().attempts;
        assert!(attempts >= 3);
        drop(probe);
        monitor.assert_released();
        for ordinal in 1..=attempts {
            let owner = TrackingAllocator::new(AllocatorConfig {
                fail_at: NonZeroU64::new(ordinal),
                ..AllocatorConfig::default()
            })
            .unwrap();
            let monitor = owner.monitor();
            let mut entropy = context(owner);
            assert_eq!(
                entropy.prepare_reference(b"abcdefghij", &sequences, [1, 4, 8], 100, false),
                Err(Error::AllocationFailed),
                "ordinal {ordinal}"
            );
            assert_eq!(monitor.snapshot().failures, 1);
            assert!(entropy.as_bytes().is_empty());
            assert!(entropy.sequence_encoder.as_bytes().is_empty());
            assert!(entropy.pending_literal_table.is_none());
            entropy.commit();
            assert_eq!(entropy.sequence_encoder.committed_repeats(), [1, 4, 8]);
            assert!(
                entropy
                    .prepare_reference(b"abcdefghij", &sequences, [1, 4, 8], 100, false)
                    .unwrap()
            );
            assert_eq!(entropy.as_bytes(), expected);
            entropy.commit();
            assert_eq!(entropy.sequence_encoder.committed_repeats(), [10, 1, 4]);
            drop(entropy);
            monitor.assert_released();
            assert_eq!(
                monitor.snapshot().owners_dropped,
                monitor.snapshot().identities_created
            );
        }
    }

    #[test]
    fn fast_borrowed_nonclone_owner_recovers_without_transferring_allocator_identity() {
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let mut entropy = context(&owner);
        let sequences = [record(10, 90, 10, OffsetIntent::Explicit)];
        assert!(monitor.fail_next());
        assert_eq!(
            entropy.prepare_reference(b"abcdefghij", &sequences, [1, 4, 8], 100, true),
            Err(Error::AllocationFailed)
        );
        assert!(entropy.as_bytes().is_empty());
        assert!(
            entropy
                .prepare_reference(b"abcdefghij", &sequences, [1, 4, 8], 100, true)
                .unwrap()
        );
        let expected = entropy.as_bytes().to_vec();
        entropy.commit();
        entropy.reset();
        assert!(
            entropy
                .prepare_reference(b"abcdefghij", &sequences, [1, 4, 8], 100, true)
                .unwrap()
        );
        assert_eq!(entropy.as_bytes(), expected);
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(entropy);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        drop(owner);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }

    #[test]
    fn assembling_independent_domains_moves_complete_owners_without_requests() {
        let literal_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let literal_monitor = literal_owner.monitor();
        let sequence_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let sequence_monitor = sequence_owner.monitor();
        let payload_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let payload_monitor = payload_owner.monitor();
        let literal = LiteralEncoder::new_in(literal_owner);
        let sequence = SequenceEncoder::new_in(sequence_owner);
        let mut payload = Vec::new_in(payload_owner);
        payload.try_reserve_exact(3).unwrap();
        payload.extend_from_slice(b"abc");
        let pointer = payload.as_ptr();
        let monitors = [&literal_monitor, &sequence_monitor, &payload_monitor];
        let before = monitors.map(|monitor| monitor.snapshot());
        for monitor in monitors {
            assert!(monitor.fail_next());
        }
        let mut entropy = BlockEntropyEncoder::from_parts(literal, sequence, payload);
        assert_eq!(entropy.as_bytes(), b"abc");
        assert_eq!(entropy.as_bytes().as_ptr(), pointer);
        entropy.discard();
        entropy.reset();
        for (monitor, snapshot) in monitors.into_iter().zip(before) {
            assert_eq!(monitor.snapshot(), snapshot);
        }
        drop(entropy);
        for monitor in monitors {
            monitor.assert_released();
            assert_eq!(
                monitor.snapshot().owners_dropped,
                monitor.snapshot().identities_created
            );
        }
    }
}
