//! Transactional sequence tables and repeat offsets for one emitted frame.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::bitwriter::BitWriter;
use crate::constants::{LL_DEFAULT, ML_DEFAULT, OF_DEFAULT};
use crate::dictionary::{Dictionary, DictionaryEntropyView};
use crate::frame::MAX_BLOCK_SIZE;
use crate::fse_encode::FseEncodingTable;
use crate::fse_select::{self, Mode};
use crate::sequence_encode::encode_sequences;
use crate::sequence_plan::PlannedSequence;
use crate::{Error, Result};

/// A checked match parser's logical output, before entropy or repeat planning.
/// These initialized scalar fields need no separate unsafe representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LogicalSequence {
    pub(crate) literal_length: u32,
    pub(crate) match_length: u32,
    pub(crate) distance: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Source {
    Unavailable,
    Predefined,
    Owned,
}

/// The three channels always appear in wire order: LL, OF, ML. Slots own
/// immutable tables; tags determine which table is available in the frame.
/// Inactive tables remain allocated across discard/reset. New selection or
/// dictionary setup may replace a slot; immutable table construction itself
/// allocates fresh storage, rather than rebuilding an existing allocation.
struct Channel<A: Allocator> {
    source: Source,
    current: Option<FseEncodingTable<A>>,
    spare: Option<FseEncodingTable<A>>,
}

impl<A: Allocator> Channel<A> {
    const fn new() -> Self {
        Self {
            source: Source::Unavailable,
            current: None,
            spare: None,
        }
    }

    fn available<'a>(
        &'a self,
        predefined: &'a FseEncodingTable<A>,
    ) -> Option<&'a FseEncodingTable<A>> {
        match self.source {
            Source::Unavailable => None,
            Source::Predefined => Some(predefined),
            Source::Owned => self.current.as_ref(),
        }
    }

    fn retained_capacity(&self) -> usize {
        [&self.current, &self.spare]
            .into_iter()
            .filter_map(Option::as_ref)
            .map(FseEncodingTable::retained_capacity)
            .sum()
    }
}

/// Own the roles of `ZSTD_compressedBlockState_t` and its candidate snapshot.
///
/// No self-reference or input/dictionary borrow survives a call. A successful
/// prepare owns the complete section and records pending value tags; only
/// commit publishes those tags and the candidate repeat triple. Raw/RLE block
/// fallback calls discard. Empty sequence sections preserve all frame state.
///
/// Three cached defaults, three current slots, and three spare slots bound
/// table retention independently of frame length. Plans, symbol vectors,
/// serialized output and the bit writer have block-bounded reusable capacity.
/// Storage access composes the independently gated FSE and bit-writer backends;
/// scalar availability/pending tags need no artificial unsafe implementation.
pub(crate) struct SequenceEncoder<A: Allocator = Global> {
    defaults: Option<[FseEncodingTable<A>; 3]>,
    channels: [Channel<A>; 3],
    repeat: [u32; 3],
    pending_repeat: [u32; 3],
    pending_modes: Option<[Mode; 3]>,
    prepared: bool,
    plans: Vec<PlannedSequence, A>,
    symbols: [Vec<u8, A>; 3],
    section: Vec<u8, A>,
    writer: BitWriter<A>,
    allocator: A,
}

impl SequenceEncoder<Global> {
    pub(crate) const fn new() -> Self {
        Self {
            defaults: None,
            channels: [Channel::new(), Channel::new(), Channel::new()],
            repeat: [1, 4, 8],
            pending_repeat: [1, 4, 8],
            pending_modes: None,
            prepared: false,
            plans: Vec::new(),
            symbols: [Vec::new(), Vec::new(), Vec::new()],
            section: Vec::new(),
            writer: BitWriter::new(),
            allocator: Global,
        }
    }
}

impl<A: Allocator + Clone> SequenceEncoder<A> {
    /// Keep a factory allocator and give every fresh owner its own clone.
    /// Clones need not be equivalent: storage moves only with its retained
    /// allocator. Ordinary Clone may itself allocate or panic; only fallible
    /// collection allocation errors map to AllocationFailed.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "allocator-aware block contexts are migrated in the next checkpoint"
        )
    )]
    pub(crate) fn new_in(allocator: A) -> Self {
        Self {
            defaults: None,
            channels: [Channel::new(), Channel::new(), Channel::new()],
            repeat: [1, 4, 8],
            pending_repeat: [1, 4, 8],
            pending_modes: None,
            prepared: false,
            plans: Vec::new_in(allocator.clone()),
            symbols: [
                Vec::new_in(allocator.clone()),
                Vec::new_in(allocator.clone()),
                Vec::new_in(allocator.clone()),
            ],
            section: Vec::new_in(allocator.clone()),
            writer: BitWriter::new_in(allocator.clone()),
            allocator,
        }
    }

    /// Preserve the unambiguous Global dictionary convenience entry point.
    /// Dictionaries with another allocator enter through their borrowed view.
    pub(crate) fn start_frame(&mut self, dictionary: Option<&Dictionary>) -> Result<()> {
        self.start_frame_with_entropy(
            dictionary
                .and_then(Dictionary::entropy)
                .map(|entropy| entropy.view()),
        )
    }

    /// Copy exact prepared dictionary tables and repeats, including duplicate
    /// repeats and normalized singleton tables with nonzero state widths.
    /// Construction is completed before changing this context. Raw/no
    /// dictionary starts with no reusable table and default repeats.
    pub(crate) fn start_frame_with_entropy(
        &mut self,
        entropy: Option<DictionaryEntropyView<'_>>,
    ) -> Result<()> {
        let prepared = if let Some(entropy) = entropy {
            let mut repeat = [0; 3];
            for (destination, &source) in repeat.iter_mut().zip(&entropy.repeat) {
                *destination = u32::try_from(source).map_err(|_| Error::InvalidDictionary)?;
            }
            let tables = [
                FseEncodingTable::from_decoding_view_in(
                    entropy.literal_lengths,
                    self.allocator.clone(),
                )?,
                FseEncodingTable::from_decoding_view_in(entropy.offsets, self.allocator.clone())?,
                FseEncodingTable::from_decoding_view_in(
                    entropy.match_lengths,
                    self.allocator.clone(),
                )?,
            ];
            Some((tables, repeat))
        } else {
            None
        };
        self.reset();
        if let Some((tables, repeat)) = prepared {
            for (channel, table) in self.channels.iter_mut().zip(tables) {
                channel.current = Some(table);
                channel.source = Source::Owned;
            }
            self.repeat = repeat;
            self.pending_repeat = repeat;
        }
        Ok(())
    }

    /// Replace any older candidate. Validation or allocation failure leaves
    /// committed tables/repeats unchanged and exposes no partial section.
    /// Logical lengths are block-bounded; source-distance eligibility remains
    /// the match parser's responsibility, independently of wire representability.
    pub(crate) fn prepare(&mut self, sequences: &[LogicalSequence]) -> Result<()> {
        self.discard();
        let result = self.prepare_inner(sequences);
        if result.is_err() {
            self.discard();
        }
        result
    }

    fn prepare_inner(&mut self, sequences: &[LogicalSequence]) -> Result<()> {
        if sequences.len() > MAX_BLOCK_SIZE / 3 {
            return Err(Error::ResourceLimit);
        }
        if sequences.is_empty() {
            self.section
                .try_reserve_exact(1)
                .map_err(|_| Error::AllocationFailed)?;
            self.section.push(0);
            self.prepared = true;
            return Ok(());
        }
        self.plans
            .try_reserve_exact(sequences.len())
            .map_err(|_| Error::AllocationFailed)?;
        for symbols in &mut self.symbols {
            symbols
                .try_reserve_exact(sequences.len())
                .map_err(|_| Error::AllocationFailed)?;
        }
        let mut repeat = self.repeat;
        let mut generated = 0_u32;
        for sequence in sequences {
            let plan = PlannedSequence::new(
                sequence.literal_length,
                sequence.match_length,
                sequence.distance,
                repeat,
            )?;
            generated = generated
                .checked_add(plan.literal_length)
                .and_then(|size| size.checked_add(plan.match_length))
                .filter(|&size| size <= MAX_BLOCK_SIZE as u32)
                .ok_or(Error::CorruptData)?;
            repeat = plan.next_repeat;
            for (symbols, code) in
                self.symbols
                    .iter_mut()
                    .zip([plan.literal_code, plan.offset_code, plan.match_code])
            {
                symbols.push(code);
            }
            self.plans.push(plan);
        }
        if self.defaults.is_none() {
            self.defaults = Some([
                FseEncodingTable::new_in(&LL_DEFAULT, 6, self.allocator.clone())?,
                FseEncodingTable::new_in(&OF_DEFAULT, 5, self.allocator.clone())?,
                FseEncodingTable::new_in(&ML_DEFAULT, 6, self.allocator.clone())?,
            ]);
        }
        let defaults = self.defaults.as_ref().ok_or(Error::CorruptData)?;
        let choices = [
            fse_select::select_in(
                &self.symbols[0],
                &defaults[0],
                self.channels[0].available(&defaults[0]),
                35,
                9,
                self.allocator.clone(),
            )?,
            fse_select::select_in(
                &self.symbols[1],
                &defaults[1],
                self.channels[1].available(&defaults[1]),
                31,
                8,
                self.allocator.clone(),
            )?,
            fse_select::select_in(
                &self.symbols[2],
                &defaults[2],
                self.channels[2].available(&defaults[2]),
                52,
                9,
                self.allocator.clone(),
            )?,
        ];
        encode_sequences(
            &self.plans,
            choices[0].table(),
            choices[1].table(),
            choices[2].table(),
            &mut self.writer,
        )?;
        // Reserve for the longest possible count header before any infallible
        // section append or pending ownership transfer. A block has at most
        // 43,690 sequences, which fits all three count encodings below.
        let mut required = self
            .writer
            .as_bytes()
            .len()
            .checked_add(4)
            .ok_or(Error::ResourceLimit)?;
        for choice in &choices {
            required = required
                .checked_add(choice.description().len())
                .ok_or(Error::ResourceLimit)?;
        }
        self.section
            .try_reserve_exact(required)
            .map_err(|_| Error::AllocationFailed)?;
        let count = sequences.len();
        if count < 128 {
            self.section.push(count as u8);
        } else if count < 0x7f00 {
            self.section.push(((count >> 8) + 128) as u8);
            self.section.push(count as u8);
        } else {
            self.section.push(255);
            for byte in ((count - 0x7f00) as u16).to_le_bytes() {
                self.section.push(byte);
            }
        }
        let modes = choices.each_ref().map(|choice| choice.mode());
        self.section
            .push(((modes[0] as u8) << 6) | ((modes[1] as u8) << 4) | ((modes[2] as u8) << 2));
        for choice in &choices {
            for &byte in choice.description() {
                self.section.push(byte);
            }
        }
        for &byte in self.writer.as_bytes() {
            self.section.push(byte);
        }
        // Consume every borrowed choice before mutating the owner slots. Each
        // selected replacement is now complete; no fallible operation follows.
        let replacements = choices.map(|choice| choice.into_replacement());
        for (channel, replacement) in self.channels.iter_mut().zip(replacements) {
            if let Some(table) = replacement {
                channel.spare = Some(table);
            }
        }
        self.pending_modes = Some(modes);
        self.pending_repeat = repeat;
        self.prepared = true;
        Ok(())
    }
}

impl<A: Allocator> SequenceEncoder<A> {
    /// Invalidate frame state without freeing cached/default/spare allocations.
    /// A subsequent start_frame may install a dictionary for the next frame.
    pub(crate) fn reset(&mut self) {
        self.discard();
        for channel in &mut self.channels {
            channel.source = Source::Unavailable;
        }
        self.repeat = [1, 4, 8];
        self.pending_repeat = self.repeat;
    }

    /// Complete candidate sequence section, retained after commit until the
    /// next prepare, discard or reset. Without a complete section this is empty.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.section
    }

    /// Publish only after the caller accepted the section into its output.
    /// Transfer is infallible and performs no allocation. Without a prepared
    /// candidate it is an idempotent no-op, as is a repeated commit.
    pub(crate) fn commit(&mut self) {
        if !self.prepared {
            return;
        }
        if let Some(modes) = self.pending_modes.take() {
            for (channel, mode) in self.channels.iter_mut().zip(modes) {
                match mode {
                    Mode::Predefined => channel.source = Source::Predefined,
                    Mode::Repeat => {}
                    Mode::Rle | Mode::Compressed => {
                        core::mem::swap(&mut channel.current, &mut channel.spare);
                        channel.source = Source::Owned;
                    }
                }
            }
            self.repeat = self.pending_repeat;
        }
        self.prepared = false;
    }

    /// Drop candidate availability and bytes, retaining every owned slot and
    /// buffer allocation. Committed entropy and repeat offsets are unchanged.
    pub(crate) fn discard(&mut self) {
        self.prepared = false;
        self.pending_modes = None;
        self.pending_repeat = self.repeat;
        self.plans.clear();
        for symbols in &mut self.symbols {
            symbols.clear();
        }
        self.section.clear();
        self.writer.reset();
    }

    /// Codec-owned payload capacity, including inactive current/spare tables
    /// and cached defaults. Excludes inline records, caller-owned input, and
    /// allocator-internal bookkeeping whose storage is opaque to this context.
    pub(crate) fn retained_capacity(&self) -> usize {
        self.defaults.as_ref().map_or(0, |tables| {
            tables.iter().map(FseEncodingTable::retained_capacity).sum()
        }) + self
            .channels
            .iter()
            .map(Channel::retained_capacity)
            .sum::<usize>()
            + self.plans.capacity() * core::mem::size_of::<PlannedSequence>()
            + self.symbols.iter().map(Vec::capacity).sum::<usize>()
            + self.section.capacity()
            + self.writer.retained_capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entropy_encode::write_normalized_counts;
    use crate::history::History;
    use crate::sequences::SequenceState;
    use alloc::vec;
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn allocator() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 64,
            fail_at: None,
        })
        .unwrap()
    }

    #[test]
    fn allocator_context_retains_every_slot_across_move_reset_and_discard() {
        let owner = allocator();
        let monitor = owner.monitor();
        let mut custom = SequenceEncoder::new_in(owner);
        let mut global = SequenceEncoder::new();
        assert_eq!(monitor.snapshot().attempts, 0);
        assert_ne!(
            custom.plans.allocator().identity(),
            custom.section.allocator().identity()
        );
        for symbols in &custom.symbols {
            assert_ne!(symbols.allocator().identity(), custom.allocator.identity());
        }
        for phase in [0, 1] {
            let workload = dense_workload(phase);
            custom.prepare(&workload).unwrap();
            global.prepare(&workload).unwrap();
            assert_eq!(custom.as_bytes(), global.as_bytes());
            assert_eq!(custom.pending_modes, Some([Mode::Compressed; 3]));
            custom.commit();
            global.commit();
        }
        assert!(custom.channels.iter().all(|channel| {
            channel.current.as_ref().unwrap().retained_capacity() != 0
                && channel.spare.as_ref().unwrap().retained_capacity() != 0
        }));
        // Nine dense tables have two allocations each. Plans, three symbol
        // vectors, the complete section, and the writer account for six more.
        assert_eq!(monitor.snapshot().live_blocks, 24);
        assert_eq!(
            monitor.snapshot().live_bytes,
            custom.retained_capacity() as u128
        );
        let before = monitor.snapshot();
        let capacity = custom.retained_capacity();
        let mut moved = custom;
        moved.discard();
        moved.reset();
        moved.commit();
        assert_eq!(monitor.snapshot(), before);
        assert_eq!(moved.retained_capacity(), capacity);
        assert_eq!(moved.repeat, [1, 4, 8]);
        assert!(moved.as_bytes().is_empty());
        assert!(
            moved
                .channels
                .iter()
                .all(|channel| channel.source == Source::Unavailable)
        );
        drop(moved);
        monitor.assert_released();
    }

    #[test]
    fn allocator_dictionary_view_is_copied_into_borrowed_nonclone_context_storage() {
        let serialized = singleton_dictionary([10; 3]).as_bytes().to_vec();
        let source_owner = allocator();
        let source_monitor = source_owner.monitor();
        let dictionary = Dictionary::from_bytes_in(&serialized, source_owner).unwrap();
        let source_before = source_monitor.snapshot();
        let destination = allocator().into_non_clone();
        let monitor = destination.monitor();
        let mut context = SequenceEncoder::new_in(&destination);
        context
            .start_frame_with_entropy(dictionary.entropy().map(|entropy| entropy.view()))
            .unwrap();
        assert_eq!(source_monitor.snapshot(), source_before);
        assert_eq!(monitor.snapshot().live_blocks, 6);
        assert_eq!(context.repeat, [10; 3]);
        for channel in &context.channels {
            assert_eq!(channel.current.as_ref().unwrap().table_log(), 5);
        }
        drop(dictionary);
        source_monitor.assert_released();
        context.prepare(&[logical(1, 3, 10)]).unwrap();
        assert_eq!(context.as_bytes(), [1, 0xfc, 0, 0x80]);
        context.commit();
        assert_eq!(context.repeat, [10; 3]);
        assert_eq!(
            monitor.snapshot().live_bytes,
            context.retained_capacity() as u128
        );
        let before = monitor.snapshot();
        context.start_frame_with_entropy(None).unwrap();
        assert_eq!(context.repeat, [1, 4, 8]);
        assert_eq!(monitor.snapshot(), before);
        drop(context);
        monitor.assert_released();
    }

    #[test]
    fn allocator_dictionary_copy_failures_preserve_the_complete_pending_context() {
        let dictionary = singleton_dictionary([10; 3]);
        let entropy = dictionary.entropy().unwrap().view();
        let baseline = allocator();
        let baseline_monitor = baseline.monitor();
        let mut probe = SequenceEncoder::new_in(baseline);
        probe.start_frame_with_entropy(Some(entropy)).unwrap();
        let attempts = baseline_monitor.snapshot().attempts;
        assert_eq!(attempts, 6);
        drop(probe);
        baseline_monitor.assert_released();

        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        let mut context = SequenceEncoder::new_in(&owner);
        context.prepare(&[logical(7, 3, 7)]).unwrap();
        let first = context.as_bytes().to_vec();
        context.commit();
        context.prepare(&[logical(1, 3, 7)]).unwrap();
        let pending = context.as_bytes().to_vec();
        let committed_repeat = context.repeat;
        let pending_repeat = context.pending_repeat;
        let pending_modes = context.pending_modes;
        let sources = context.channels.each_ref().map(|channel| channel.source);
        let capacity = context.retained_capacity();
        for ordinal in 1..=attempts {
            let before = monitor.snapshot();
            monitor.set_fail_at(NonZeroU64::new(before.attempts + ordinal));
            assert_eq!(
                context.start_frame_with_entropy(Some(entropy)),
                Err(Error::AllocationFailed)
            );
            assert_eq!(monitor.snapshot().attempts, before.attempts + ordinal);
            assert_eq!(monitor.snapshot().failures, before.failures + 1);
            assert_eq!(monitor.snapshot().live_bytes, before.live_bytes);
            assert_eq!(monitor.snapshot().live_blocks, before.live_blocks);
            assert_eq!(context.retained_capacity(), capacity);
            assert_eq!(context.as_bytes(), pending);
            assert_eq!(context.repeat, committed_repeat);
            assert_eq!(context.pending_repeat, pending_repeat);
            assert_eq!(context.pending_modes, pending_modes);
            assert_eq!(
                context.channels.each_ref().map(|channel| channel.source),
                sources
            );
            assert!(context.prepared);
            monitor.clear_failure();
        }
        context.commit();
        let mut state = SequenceState::new();
        let mut history = History::new(14);
        history.start_frame(1024);
        state
            .decode(&first, b"abcdefg", &mut history, MAX_BLOCK_SIZE)
            .unwrap();
        state
            .decode(context.as_bytes(), b"X", &mut history, MAX_BLOCK_SIZE)
            .unwrap();
        assert_eq!(history.into_bytes(), b"abcdefgabcXefg");
        drop(context);
        monitor.assert_released();
    }

    #[test]
    fn allocator_prepare_failure_sweeps_preserve_committed_state_and_release_candidates() {
        let first: Vec<_> = (0..64).map(|index| logical(1 + index % 2, 3, 7)).collect();
        let next = [logical(2, 3, 5), logical(3, 4, 5)];
        for initialized in [false, true] {
            let baseline = allocator();
            let baseline_monitor = baseline.monitor();
            let mut probe = SequenceEncoder::new_in(baseline);
            if initialized {
                probe.prepare(&first).unwrap();
                probe.commit();
                assert!(
                    probe
                        .channels
                        .iter()
                        .any(|channel| channel.source == Source::Owned)
                );
            }
            let start = baseline_monitor.snapshot().attempts;
            probe.prepare(&next).unwrap();
            let attempts = baseline_monitor.snapshot().attempts - start;
            let expected = probe.as_bytes().to_vec();
            assert!(attempts > 1);
            drop(probe);
            baseline_monitor.assert_released();
            for ordinal in 1..=attempts {
                let owner = allocator().into_non_clone();
                let monitor = owner.monitor();
                let mut context = SequenceEncoder::new_in(&owner);
                if initialized {
                    context.prepare(&first).unwrap();
                    context.commit();
                }
                let repeat = context.repeat;
                let sources = context.channels.each_ref().map(|channel| channel.source);
                let start = monitor.snapshot().attempts;
                monitor.set_fail_at(NonZeroU64::new(start + ordinal));
                assert_eq!(context.prepare(&next), Err(Error::AllocationFailed));
                assert_eq!(monitor.snapshot().attempts, start + ordinal);
                assert_eq!(monitor.snapshot().failures, 1);
                assert!(context.as_bytes().is_empty());
                assert!(!context.prepared);
                assert!(context.pending_modes.is_none());
                assert_eq!(context.repeat, repeat);
                assert_eq!(context.pending_repeat, repeat);
                assert_eq!(
                    context.channels.each_ref().map(|channel| channel.source),
                    sources
                );
                assert_eq!(
                    monitor.snapshot().live_bytes,
                    context.retained_capacity() as u128
                );
                // Defaults are published as a complete array, never individual
                // partially constructed owners. Retained scratch may grow.
                if let Some(defaults) = &context.defaults {
                    assert_eq!(
                        defaults.each_ref().map(|table| table.table_log()),
                        [6, 5, 6]
                    );
                }
                context.commit();
                assert_eq!(context.repeat, repeat);
                monitor.clear_failure();
                context.prepare(&next).unwrap();
                assert_eq!(context.as_bytes(), expected);
                drop(context);
                monitor.assert_released();
            }
        }
    }

    fn logical(literal_length: u32, match_length: u32, distance: u32) -> LogicalSequence {
        LogicalSequence {
            literal_length,
            match_length,
            distance,
        }
    }

    fn dense_workload(phase: u32) -> Vec<LogicalSequence> {
        (0..512)
            .map(|index| {
                logical(
                    1 + phase * 7 + index % 3,
                    3 + phase * 8 + index % 7,
                    (4 << (phase * 4)) << (index % 4),
                )
            })
            .collect()
    }

    fn modes(section: &[u8]) -> [u8; 3] {
        let width = match section[0] {
            0..=127 => 1,
            128..=254 => 2,
            _ => 3,
        };
        let modes = section[width];
        [modes >> 6, (modes >> 4) & 3, (modes >> 2) & 3]
    }

    fn singleton_dictionary(repeat: [u32; 3]) -> Dictionary {
        let mut bytes = 0xec30_a437_u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&12345_u32.to_le_bytes());
        bytes.extend_from_slice(&[128, 0x10]); // Two-symbol Huffman table.
        for counts in [&[32][..], &[32][..], &[0, 32][..]] {
            bytes.extend_from_slice(&write_normalized_counts(counts, 5).unwrap());
        }
        for offset in repeat {
            bytes.extend_from_slice(&offset.to_le_bytes());
        }
        bytes.extend_from_slice(b"0123456789");
        Dictionary::from_bytes(&bytes).unwrap()
    }

    #[test]
    fn discard_and_failed_prepare_preserve_committed_repeats_and_tables() {
        let mut encoder = SequenceEncoder::new();
        let first = [logical(7, 3, 7)];
        encoder.prepare(&first).unwrap();
        let original = encoder.as_bytes().to_vec();
        assert_eq!(encoder.repeat, [1, 4, 8]);
        assert_eq!(encoder.pending_repeat, [7, 1, 4]);
        encoder.discard();
        assert!(encoder.as_bytes().is_empty());
        encoder.commit();
        assert_eq!(encoder.repeat, [1, 4, 8]);
        encoder.prepare(&first).unwrap();
        assert_eq!(encoder.as_bytes(), original);
        encoder.commit();
        assert_eq!(encoder.repeat, [7, 1, 4]);
        let sources = encoder.channels.each_ref().map(|channel| channel.source);
        encoder.commit();
        assert_eq!(encoder.repeat, [7, 1, 4]);

        encoder.prepare(&[logical(1, 3, 5)]).unwrap();
        assert_eq!(encoder.pending_repeat, [5, 7, 1]);
        assert_eq!(encoder.repeat, [7, 1, 4]);
        assert_eq!(
            encoder.prepare(&[logical(1, 3, 0)]),
            Err(Error::CorruptData)
        );
        assert!(encoder.as_bytes().is_empty());
        encoder.commit();
        assert_eq!(encoder.repeat, [7, 1, 4]);
        assert_eq!(
            encoder.channels.each_ref().map(|channel| channel.source),
            sources
        );
        encoder.prepare(&[logical(1, 3, 7)]).unwrap();
        assert_eq!(encoder.plans[0].offset_value, 1);

        let mut decoder = SequenceState::new();
        let mut history = History::new(14);
        history.start_frame(1024);
        decoder
            .decode(&original, b"abcdefg", &mut history, MAX_BLOCK_SIZE)
            .unwrap();
        decoder
            .decode(encoder.as_bytes(), b"X", &mut history, MAX_BLOCK_SIZE)
            .unwrap();
        assert_eq!(history.into_bytes(), b"abcdefgabcXefg");
    }

    #[test]
    fn empty_sections_and_repeated_commit_leave_all_frame_state_unchanged() {
        let mut encoder = SequenceEncoder::new();
        assert_eq!(encoder.retained_capacity(), 0);
        encoder.commit();
        encoder.prepare(&[]).unwrap();
        assert_eq!(encoder.as_bytes(), &[0]);
        assert!(encoder.defaults.is_none());
        encoder.commit();
        assert!(
            encoder
                .channels
                .iter()
                .all(|channel| channel.source == Source::Unavailable)
        );

        let workload = dense_workload(0);
        encoder.prepare(&workload).unwrap();
        assert_eq!(encoder.pending_modes, Some([Mode::Compressed; 3]));
        encoder.commit();
        let sources = encoder.channels.each_ref().map(|channel| channel.source);
        let repeat = encoder.repeat;
        let current_capacity = encoder
            .channels
            .each_ref()
            .map(|channel| channel.current.as_ref().unwrap().retained_capacity());
        encoder.prepare(&[]).unwrap();
        assert_eq!(encoder.as_bytes(), &[0]);
        encoder.commit();
        encoder.commit();
        assert_eq!(
            encoder.channels.each_ref().map(|channel| channel.source),
            sources
        );
        assert_eq!(encoder.repeat, repeat);
        assert_eq!(
            encoder
                .channels
                .each_ref()
                .map(|channel| { channel.current.as_ref().unwrap().retained_capacity() }),
            current_capacity
        );
        encoder.prepare(&workload).unwrap();
        assert_eq!(modes(encoder.as_bytes()), [3; 3]);
    }

    #[test]
    fn reset_and_discard_retain_all_six_owned_slots_and_every_buffer() {
        let mut encoder = SequenceEncoder::new();
        for phase in [0, 1] {
            encoder.prepare(&dense_workload(phase)).unwrap();
            assert_eq!(encoder.pending_modes, Some([Mode::Compressed; 3]));
            encoder.commit();
        }
        for channel in &encoder.channels {
            assert!(channel.current.as_ref().unwrap().retained_capacity() > 0);
            assert!(channel.spare.as_ref().unwrap().retained_capacity() > 0);
        }
        let tables = encoder
            .defaults
            .as_ref()
            .unwrap()
            .iter()
            .map(FseEncodingTable::retained_capacity)
            .sum::<usize>()
            + encoder
                .channels
                .iter()
                .map(Channel::retained_capacity)
                .sum::<usize>();
        let buffers = encoder.plans.capacity() * core::mem::size_of::<PlannedSequence>()
            + encoder.symbols.iter().map(Vec::capacity).sum::<usize>()
            + encoder.section.capacity()
            + encoder.writer.retained_capacity();
        assert!(tables > 0 && buffers > 0);
        assert_eq!(encoder.retained_capacity(), tables + buffers);
        encoder.discard();
        assert_eq!(encoder.retained_capacity(), tables + buffers);
        encoder.reset();
        assert_eq!(encoder.retained_capacity(), tables + buffers);
        assert_eq!(encoder.repeat, [1, 4, 8]);
        assert!(
            encoder
                .channels
                .iter()
                .all(|channel| channel.source == Source::Unavailable)
        );
        assert!(encoder.as_bytes().is_empty());
        encoder.prepare(&dense_workload(1)).unwrap();
        assert_ne!(modes(encoder.as_bytes()), [3; 3]);
    }

    #[test]
    fn dictionary_state_is_owned_and_keeps_duplicates_and_nonzero_singleton_logs() {
        let dictionary = singleton_dictionary([10; 3]);
        let serialized = dictionary.as_bytes().to_vec();
        let mut encoder = SequenceEncoder::new();
        encoder.start_frame(Some(&dictionary)).unwrap();
        drop(dictionary);
        assert_eq!(encoder.repeat, [10; 3]);
        assert!(encoder.defaults.is_none());
        for channel in &encoder.channels {
            let table = channel.current.as_ref().unwrap();
            assert_eq!(table.table_log(), 5);
            assert!(table.retained_capacity() > 0);
        }
        encoder.prepare(&[logical(1, 3, 10)]).unwrap();
        assert_eq!(encoder.plans[0].offset_value, 1);
        assert_eq!(encoder.as_bytes(), &[1, 0xfc, 0, 0x80]);
        let dictionary = Dictionary::from_bytes(&serialized).unwrap();
        let mut decoder = SequenceState::with_repeat([10; 3]);
        let mut history = History::new(14);
        history.start_frame(1024);
        history.append(dictionary.content()).unwrap();
        decoder
            .decode_with_dictionary(
                encoder.as_bytes(),
                b"a",
                &mut history,
                MAX_BLOCK_SIZE,
                dictionary.entropy(),
            )
            .unwrap();
        assert_eq!(history.into_bytes(), b"0123456789a123");
        encoder.commit();
        assert_eq!(encoder.repeat, [10; 3]);

        let capacity = encoder.retained_capacity();
        let raw = Dictionary::raw(b"raw content").unwrap();
        encoder.start_frame(Some(&raw)).unwrap();
        assert_eq!(encoder.retained_capacity(), capacity);
        assert_eq!(encoder.repeat, [1, 4, 8]);
        encoder.prepare(&[logical(1, 3, 1)]).unwrap();
        assert_eq!(modes(encoder.as_bytes()), [0; 3]);
        encoder.commit();
        assert!(
            encoder
                .channels
                .iter()
                .all(|channel| channel.source == Source::Predefined)
        );
        encoder.start_frame(None).unwrap();
        encoder.prepare(&[logical(1, 3, 1)]).unwrap();
        assert_eq!(modes(encoder.as_bytes()), [0; 3]);
    }

    #[test]
    fn a_replaced_dictionary_channel_never_reappears_as_the_repeat_source() {
        let dictionary = singleton_dictionary([1, 4, 8]);
        let mut encoder = SequenceEncoder::new();
        encoder.start_frame(Some(&dictionary)).unwrap();
        let workload = vec![logical(2, 3, 1); 64];
        encoder.prepare(&workload).unwrap();
        assert_eq!(modes(encoder.as_bytes()), [1, 3, 3]);
        encoder.commit();
        assert_eq!(encoder.channels[0].current.as_ref().unwrap().table_log(), 0);
        assert_eq!(encoder.channels[0].spare.as_ref().unwrap().table_log(), 5);
        encoder.prepare(&[logical(1, 3, 1)]).unwrap();
        assert_eq!(modes(encoder.as_bytes()), [0, 3, 3]);
        encoder.commit();
        assert_eq!(encoder.channels[0].source, Source::Predefined);
        encoder.prepare(&[logical(1, 3, 1)]).unwrap();
        assert_eq!(modes(encoder.as_bytes()), [3; 3]);
        assert_eq!(
            encoder.channels[0]
                .available(&encoder.defaults.as_ref().unwrap()[0])
                .unwrap()
                .table_log(),
            6
        );
    }

    #[test]
    fn invalid_counts_lengths_and_distances_never_publish_partial_sections() {
        let mut encoder = SequenceEncoder::new();
        for sequences in [
            vec![logical(0, 2, 1)],
            vec![logical(u32::MAX, 3, 1)],
            vec![logical(0, 3, 0)],
            vec![logical(0, 3, u32::MAX)],
            vec![logical(0, MAX_BLOCK_SIZE as u32, 1), logical(0, 3, 1)],
        ] {
            assert_eq!(encoder.prepare(&sequences), Err(Error::CorruptData));
            assert!(encoder.as_bytes().is_empty());
            encoder.commit();
            assert_eq!(encoder.repeat, [1, 4, 8]);
        }
        assert_eq!(
            encoder.prepare(&vec![logical(0, 3, 1); MAX_BLOCK_SIZE / 3 + 1]),
            Err(Error::ResourceLimit)
        );
        assert!(encoder.as_bytes().is_empty());
        assert!(
            encoder
                .channels
                .iter()
                .all(|channel| channel.source == Source::Unavailable)
        );
    }

    #[test]
    fn complete_sections_decode_all_count_header_boundaries_and_overlap() {
        for count in [1, 127, 128, 255, 256, 0x7eff, 0x7f00, MAX_BLOCK_SIZE / 3] {
            let mut sequences = vec![logical(0, 3, 1); count];
            sequences[0].literal_length = 1;
            let mut encoder = SequenceEncoder::new();
            encoder.prepare(&sequences).unwrap();
            let expected_header = if count < 128 {
                vec![count as u8]
            } else if count < 0x7f00 {
                vec![((count >> 8) + 128) as u8, count as u8]
            } else {
                let encoded = ((count - 0x7f00) as u16).to_le_bytes();
                vec![255, encoded[0], encoded[1]]
            };
            assert!(encoder.as_bytes().starts_with(&expected_header));
            let mut history = History::new(1 + count * 3);
            history.start_frame(MAX_BLOCK_SIZE);
            SequenceState::new()
                .decode(encoder.as_bytes(), b"x", &mut history, MAX_BLOCK_SIZE)
                .unwrap();
            assert_eq!(history.into_bytes(), vec![b'x'; 1 + count * 3]);
        }
    }

    fn append_block(frame: &mut Vec<u8>, payload: &[u8], kind: u32, last: bool) {
        let header = ((payload.len() as u32) << 3) | (kind << 1) | u32::from(last);
        frame.extend_from_slice(&header.to_le_bytes()[..3]);
        frame.extend_from_slice(payload);
    }

    fn transactional_frame() -> (Vec<u8>, Vec<u8>) {
        let mut encoder = SequenceEncoder::new();
        let workload: Vec<_> = (0..64).map(|index| logical(1 + index % 2, 3, 1)).collect();
        let mut expected = Vec::new();
        let mut blocks = Vec::new();
        for phase in 0..3 {
            // A rejected candidate must not alter either reused table support
            // or the initial repeat triple of the subsequently emitted block.
            encoder.prepare(&dense_workload(1)).unwrap();
            encoder.discard();
            encoder.prepare(&workload).unwrap();
            assert_eq!(
                modes(encoder.as_bytes()),
                if phase == 0 { [2, 1, 1] } else { [3; 3] }
            );
            let literals: Vec<_> = (0..96)
                .map(|index| (index * 29 + phase * 7) as u8)
                .collect();
            let mut position = 0;
            for sequence in &workload {
                let end = position + sequence.literal_length as usize;
                expected.extend_from_slice(&literals[position..end]);
                expected.extend_from_slice(&[*expected.last().unwrap(); 3]);
                position = end;
            }
            let mut block = (((literals.len() as u16) << 4) | 4).to_le_bytes().to_vec();
            block.extend_from_slice(&literals);
            block.extend_from_slice(encoder.as_bytes());
            append_block(&mut blocks, &block, 2, phase == 2);
            encoder.commit();
            if phase != 2 {
                // Whole-block raw fallback preserves entropy state.
                append_block(&mut blocks, b"RAW", 0, false);
                expected.extend_from_slice(b"RAW");
                // A compressed block with no sequences also preserves it.
                encoder.prepare(&[]).unwrap();
                let mut literal_only = vec![3 << 3];
                literal_only.extend_from_slice(b"lit");
                literal_only.extend_from_slice(encoder.as_bytes());
                append_block(&mut blocks, &literal_only, 2, false);
                expected.extend_from_slice(b"lit");
                encoder.commit();
            }
        }
        let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0xa0];
        frame.extend_from_slice(&(expected.len() as u32).to_le_bytes());
        frame.extend_from_slice(&blocks);
        assert_eq!(crate::decompress(&frame, expected.len()).unwrap(), expected);
        (frame, expected)
    }

    #[test]
    fn new_repeat_raw_and_empty_blocks_share_only_committed_frame_state() {
        transactional_frame();
    }

    fn dictionary_frame() -> (Vec<u8>, Dictionary) {
        let dictionary = singleton_dictionary([10; 3]);
        let mut encoder = SequenceEncoder::new();
        encoder.start_frame(Some(&dictionary)).unwrap();
        encoder.prepare(&[logical(1, 3, 10)]).unwrap();
        assert_eq!(encoder.as_bytes(), &[1, 0xfc, 0, 0x80]);
        // A 1 KiB window permits this six-byte compressed block. A tiny
        // single-segment window of four bytes would make its payload illegal.
        // Both the dictionary ID and four-byte content size use full fields.
        let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0x83, 0];
        frame.extend_from_slice(&dictionary.id().to_le_bytes());
        frame.extend_from_slice(&4_u32.to_le_bytes());
        let mut block = vec![1 << 3, b'a'];
        block.extend_from_slice(encoder.as_bytes());
        append_block(&mut frame, &block, 2, true);
        encoder.commit();
        (frame, dictionary)
    }

    #[test]
    fn dictionary_frame_uses_exact_nonzero_initial_state_widths() {
        let (frame, dictionary) = dictionary_frame();
        assert_eq!(
            crate::decompress_with_dictionary(
                &frame,
                &dictionary.into_shared(),
                crate::DecodeOptions::new(4),
            )
            .unwrap(),
            b"a123"
        );
    }

    #[cfg(feature = "std")]
    #[test]
    #[ignore = "requires official zstd v1.5.7 CLI; set ZSTD_BIN"]
    fn official_codec_decodes_transactional_and_dictionary_frames() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let executable = std::env::var_os("ZSTD_BIN").unwrap_or_else(|| "zstd".into());
        let version = Command::new(&executable).arg("--version").output().unwrap();
        assert!(version.status.success());
        assert!(std::string::String::from_utf8_lossy(&version.stdout).contains("1.5.7"));
        let (frame, expected) = transactional_frame();
        let mut child = Command::new(&executable)
            .args(["-q", "-d", "-c"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&frame).unwrap();
        let decoded = child.wait_with_output().unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            std::string::String::from_utf8_lossy(&decoded.stderr)
        );
        assert_eq!(decoded.stdout, expected);
        std::eprintln!(
            "Official v1.5.7 decoded transactional 7-block frame: {} output bytes",
            expected.len()
        );

        let (frame, dictionary) = dictionary_frame();
        let path = std::env::temp_dir().join(std::format!(
            "zstdoxide-sequence-context-{}-{}.dict",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        struct DictionaryFile(std::path::PathBuf);
        impl Drop for DictionaryFile {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let file = DictionaryFile(path);
        std::fs::write(&file.0, dictionary.as_bytes()).unwrap();
        let mut child = Command::new(&executable)
            .args(["-q", "-d", "-c", "-D"])
            .arg(&file.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&frame).unwrap();
        let decoded = child.wait_with_output().unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            std::string::String::from_utf8_lossy(&decoded.stderr)
        );
        assert_eq!(decoded.stdout, b"a123");
        std::eprintln!("Official v1.5.7 decoded duplicate-repeat dictionary frame: 4 output bytes");
    }
}
