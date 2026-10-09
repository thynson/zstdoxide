//! Parser-independent block entropy owners and transactional publication.
//!
//! This composite moves complete existing owners without cloning an allocator.
//! Memory access stays in each child's safe or independently gated backend.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::dictionary::DictionaryEntropyView;
use crate::huffman_codebook::HuffmanCodebook;
use crate::literal_encode::LiteralEncoder;
use crate::sequence_context::{LogicalSequence, SequenceEncoder};
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

#[cfg(test)]
mod tests {
    use super::*;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

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
