//! Shared reference framing with selected search and one transactional entropy owner.
#![forbid(unsafe_code)]

use alloc::vec::Vec;
use core::alloc::Allocator;

#[cfg(test)]
use crate::CompressionParameterRequest;
use crate::block_entropy::BlockEntropyEncoder;
use crate::compiled_dictionary::CompiledDictionary;
use crate::dictionary::{DictionaryEntropyView, DictionaryView};
#[cfg(test)]
use crate::fast_plan::FastExecutionPlan;
use crate::frame::{BlockKind, MAX_BLOCK_SIZE};
use crate::frame_sink::FrameSink;
use crate::huffman_codebook::HuffmanCodebook;
use crate::literal_encode::LiteralEncoder;
use crate::optimal_retained::Publication;
use crate::reference_frame::{ReferenceFrameHeader, ReferenceFrameSettings};
use crate::reference_search::{CompiledCandidates, ReferenceSearchPlan, ReferenceSearchState};
use crate::sequence_context::SequenceEncoder;
use crate::{Error, Result};

/// Append-only destination for one encoded reference frame.
///
/// The owned-vector implementation reports allocation failure through
/// [`Error::AllocationFailed`]; the caller-owned sink reports a short
/// destination through [`Error::ResourceLimit`]. Every append either publishes
/// all bytes or leaves the destination unchanged, so a rejected block never
/// exposes a partial header.
pub(crate) trait ReferenceOutput {
    fn append(&mut self, bytes: &[u8]) -> Result<()>;
    fn append_block(&mut self, header: [u8; 3], payload: &[u8]) -> Result<()>;
}

impl<A: Allocator> ReferenceOutput for Vec<u8, A> {
    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        self.try_reserve_exact(bytes.len())
            .map_err(|_| Error::AllocationFailed)?;
        self.extend_from_slice(bytes);
        Ok(())
    }

    fn append_block(&mut self, header: [u8; 3], payload: &[u8]) -> Result<()> {
        let additional = header
            .len()
            .checked_add(payload.len())
            .ok_or(Error::ResourceLimit)?;
        self.try_reserve_exact(additional)
            .map_err(|_| Error::AllocationFailed)?;
        self.extend_from_slice(&header);
        self.extend_from_slice(payload);
        Ok(())
    }
}

impl ReferenceOutput for FrameSink<'_> {
    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        FrameSink::append(self, bytes)
    }

    fn append_block(&mut self, header: [u8; 3], payload: &[u8]) -> Result<()> {
        // Check the complete extent before the first write so a short
        // destination never exposes a partial block header.
        let total = header
            .len()
            .checked_add(payload.len())
            .ok_or(Error::ResourceLimit)?;
        if total > self.remaining() {
            return Err(Error::ResourceLimit);
        }
        self.append(&header)?;
        self.append(payload)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/support/double_fast_frame_encode.rs"]
mod double_fast_tests;

#[cfg(test)]
#[path = "../tests/support/greedy_frame_encode.rs"]
mod greedy_tests;

#[cfg(test)]
#[path = "../tests/support/lazy_frame_encode.rs"]
mod lazy_tests;

#[cfg(test)]
#[path = "../tests/support/tree_frame_encode.rs"]
mod tree_tests;

#[cfg(test)]
#[path = "../tests/support/optimal_retained_frame_encode.rs"]
mod retained_tests;

/// Encode one known-size, no-dictionary frame with an admitted reference profile.
///
/// Admission and output-bound checks precede allocator operations. The original
/// allocator owns the result; all scratch owners retain separate fresh clones.
/// No source or request borrow escapes. This private entry point does not alter
/// the existing named policies or their checksummed frame defaults.
#[cfg(test)]
fn compress_fast_in<A: Allocator + Clone>(
    input: &[u8],
    request: &CompressionParameterRequest,
    allocator: A,
) -> Result<Vec<u8, A>> {
    let plan = ReferenceSearchPlan::Fast(FastExecutionPlan::new(request, input.len())?);
    compress_reference_in(input, plan, allocator)
}

/// Encode using a pre-admitted static search plan without resolving it again.
/// Source/metadata/bound errors precede output allocation; empty frames allocate
/// no workspace. Both public strategies use this same publication path.
pub(crate) fn compress_reference_in<A: Allocator + Clone>(
    input: &[u8],
    plan: ReferenceSearchPlan,
    allocator: A,
) -> Result<Vec<u8, A>> {
    if input.len() != plan.source_len() {
        return Err(Error::InvalidParameter);
    }
    let settings = ReferenceFrameSettings::new(plan.source_len(), plan.parameters().window_log())?;
    let bound = plan.frame_bound()?;
    let mut output = Vec::new_in(allocator);
    output
        .try_reserve_exact(bound)
        .map_err(|_| Error::AllocationFailed)?;
    if input.is_empty() {
        // The complete bound already includes this sole final empty raw block.
        ReferenceOutput::append(&mut output, ReferenceFrameHeader::new(&settings).as_bytes())?;
        ReferenceOutput::append(&mut output, &[1, 0, 0])?;
        return Ok(output);
    }
    let scratch = output.allocator().clone();
    compress_reference_to(input, plan, &mut output, scratch)?;
    Ok(output)
}

/// Encode one admitted frame into caller-owned storage.
///
/// Header and complete blocks are appended atomically. A short destination
/// returns [`Error::ResourceLimit`] after earlier whole appends, and bytes
/// outside the supplied slice are never touched. Scratch uses the supplied
/// allocator and is released before return.
pub(crate) fn compress_reference_into_in<A: Allocator + Clone>(
    input: &[u8],
    output: &mut [u8],
    plan: ReferenceSearchPlan,
    allocator: A,
) -> Result<usize> {
    let mut sink = FrameSink::new(output);
    compress_reference_to(input, plan, &mut sink, allocator)?;
    Ok(sink.len())
}

fn compress_reference_to<A: Allocator + Clone, O: ReferenceOutput>(
    input: &[u8],
    plan: ReferenceSearchPlan,
    output: &mut O,
    allocator: A,
) -> Result<()> {
    if input.len() != plan.source_len() {
        return Err(Error::InvalidParameter);
    }
    let settings = ReferenceFrameSettings::new(plan.source_len(), plan.parameters().window_log())?;
    output.append(ReferenceFrameHeader::new(&settings).as_bytes())?;
    if input.is_empty() {
        output.append(&[1, 0, 0])?;
        return Ok(());
    }
    let mut workspace = ReferenceEncoderWorkspace::new(input, plan, allocator)?;
    while workspace.emitted < input.len() {
        let length = plan.next_block_len(workspace.emitted)?;
        let last = length == input.len() - workspace.emitted;
        workspace.emit_next(length, last, output)?;
    }
    Ok(())
}

/// Encode using a pre-admitted plan with an immutable dictionary view.
///
/// The in-window suffix of dictionary content is staged contiguously before
/// the input in per-call scratch, and the existing profile parsers run over
/// that combined borrow unchanged: match extension crosses the staged boundary
/// contiguously, the existing candidate floor bounds every emitted offset by
/// the resolved window (`ZSTD_checkDictValidity` semantics), and combined-space
/// positions produce decoder-correct offsets by construction. Frame flags
/// follow the parameterized family (known size, no checksum) plus the
/// dictionary ID field. A formatted dictionary's validated entropy also seeds
/// the frame: its literal codebook and sequence tables/repeats become the first
/// block's previous state, exactly as `ZSTD_loadCEntropy` fills
/// `blockState.prevCBlock` before the first block, so covered literals may be
/// treeless and covered sequences may repeat the prepared tables. The staging
/// copy is released before return; no workspace survives the call.
/// Dictionary-aware profile admission is staged: a nonzero prefix currently
/// admits the Fast and DoubleFast profiles and the complete Greedy family
/// (chain, row and tree), and every optimal-family profile is explicitly
/// rejected with [`crate::Error::UnsupportedFeature`] under the
/// supported-subset clause instead of silently dropping the dictionary.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "public parameterized dictionary exposure is the next module36 item"
    )
)]
pub(crate) fn compress_reference_with_dictionary_in<A: Allocator + Clone>(
    input: &[u8],
    dictionary: DictionaryView<'_>,
    plan: ReferenceSearchPlan,
    allocator: A,
) -> Result<Vec<u8, A>> {
    if input.len() != plan.source_len() {
        return Err(Error::InvalidParameter);
    }
    let settings = ReferenceFrameSettings::new(plan.source_len(), plan.parameters().window_log())?;
    let staged = dictionary.content().len().min(settings.window_size());
    if staged > 0 && !plan.admits_dictionary_staging() {
        return Err(Error::UnsupportedFeature);
    }
    let (_, dictionary_bytes) = crate::encode::dictionary_field(dictionary.id());
    let bound = plan
        .frame_bound()?
        .checked_add(dictionary_bytes)
        .ok_or(Error::ResourceLimit)?;
    let mut output = Vec::new_in(allocator);
    output
        .try_reserve_exact(bound)
        .map_err(|_| Error::AllocationFailed)?;
    if input.is_empty() {
        // The complete bound already includes this sole final empty raw block.
        let header = ReferenceFrameHeader::with_dictionary_id(&settings, dictionary.id());
        ReferenceOutput::append(&mut output, header.as_bytes())?;
        ReferenceOutput::append(&mut output, &[1, 0, 0])?;
        return Ok(output);
    }
    let scratch = output.allocator().clone();
    compress_reference_with_dictionary_to(input, dictionary, plan, &mut output, scratch)?;
    Ok(output)
}

/// Encode one admitted dictionary frame into caller-owned storage.
///
/// Admission and prepared-table errors precede the first output byte. Complete
/// header/block appends are atomic; a short destination returns
/// [`Error::ResourceLimit`] after earlier whole appends and never mutates bytes
/// outside the supplied slice.
pub(crate) fn compress_reference_with_dictionary_into_in<A: Allocator + Clone>(
    input: &[u8],
    output: &mut [u8],
    dictionary: DictionaryView<'_>,
    plan: ReferenceSearchPlan,
    allocator: A,
) -> Result<usize> {
    let mut sink = FrameSink::new(output);
    compress_reference_with_dictionary_to(input, dictionary, plan, &mut sink, allocator)?;
    Ok(sink.len())
}

fn compress_reference_with_dictionary_to<A: Allocator + Clone, O: ReferenceOutput>(
    input: &[u8],
    dictionary: DictionaryView<'_>,
    plan: ReferenceSearchPlan,
    output: &mut O,
    allocator: A,
) -> Result<()> {
    if input.len() != plan.source_len() {
        return Err(Error::InvalidParameter);
    }
    let settings = ReferenceFrameSettings::new(plan.source_len(), plan.parameters().window_log())?;
    let content = dictionary.content();
    let staged = content.len().min(settings.window_size());
    if staged > 0 && !plan.admits_dictionary_staging() {
        // Supported dictionary subset: reject unsupported profiles before the
        // first output byte instead of silently dropping the dictionary.
        return Err(Error::UnsupportedFeature);
    }
    let header = ReferenceFrameHeader::with_dictionary_id(&settings, dictionary.id());
    let entropy = dictionary.entropy();
    // Prepared dictionaries allow twelve-bit codes. The codebook copies plain
    // values, retaining no source-table borrow, and construction allocates no
    // heap storage; an invalid prepared tree fails before output admission.
    let literal_table = entropy
        .map(|entropy| HuffmanCodebook::from_lengths(&entropy.huffman.code_lengths(), 12))
        .transpose()?;
    let (_, dictionary_bytes) = crate::encode::dictionary_field(dictionary.id());
    plan.frame_bound()?
        .checked_add(dictionary_bytes)
        .ok_or(Error::ResourceLimit)?;
    output.append(header.as_bytes())?;
    if input.is_empty() {
        output.append(&[1, 0, 0])?;
        return Ok(());
    }
    let mut combined = Vec::new_in(allocator.clone());
    combined
        .try_reserve_exact(
            staged
                .checked_add(input.len())
                .ok_or(Error::ResourceLimit)?,
        )
        .map_err(|_| Error::AllocationFailed)?;
    combined.extend_from_slice(&content[content.len() - staged..]);
    combined.extend_from_slice(input);
    let mut workspace = ReferenceEncoderWorkspace::new_with_prefix_and_entropy(
        &combined,
        staged,
        plan,
        allocator,
        entropy,
        literal_table,
    )?;
    while workspace.emitted < input.len() {
        let length = plan.next_block_len(workspace.emitted)?;
        let last = length == input.len() - workspace.emitted;
        workspace.emit_next(length, last, output)?;
    }
    Ok(())
}

/// Encode using a compiled dictionary's immutable tables and prepared entropy.
///
/// The call stages exactly the compiled loaded suffix before its input, then
/// binds the per-call search owners to the compiled candidate views instead of
/// seeding them. The frame header carries the compiled identifier and the
/// first block receives the compiled entropy seed. The compiled profile fixes
/// window/layout and must match the candidate family; no compiled table is
/// cloned, copied or mutated. The staging copy and per-call owners are released
/// before return.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "public compiled-dictionary emission follows")
)]
pub(crate) fn compress_reference_with_compiled_in<A: Allocator + Clone, B: Allocator + Clone>(
    input: &[u8],
    compiled: &CompiledDictionary<'_, B>,
    plan: ReferenceSearchPlan,
    allocator: A,
) -> Result<Vec<u8, A>> {
    if input.len() != plan.source_len() {
        return Err(Error::InvalidParameter);
    }
    let settings = ReferenceFrameSettings::new(plan.source_len(), plan.parameters().window_log())?;
    let content = compiled.content();
    let staged = compiled.loaded_len();
    if staged == 0 || staged > content.len() || staged > settings.window_size() {
        return Err(Error::InvalidParameter);
    }
    if !plan.admits_dictionary_staging() {
        return Err(Error::UnsupportedFeature);
    }
    let candidates = compiled.search_candidates()?;
    if !candidates.matches(plan) {
        return Err(Error::InvalidParameter);
    }
    let header = ReferenceFrameHeader::with_dictionary_id(&settings, compiled.dictionary_id());
    let seed = compiled.first_block_seed();
    let (_, dictionary_bytes) = crate::encode::dictionary_field(compiled.dictionary_id());
    let bound = plan
        .frame_bound()?
        .checked_add(dictionary_bytes)
        .ok_or(Error::ResourceLimit)?;
    let mut output = Vec::new_in(allocator);
    output
        .try_reserve_exact(bound)
        .map_err(|_| Error::AllocationFailed)?;
    if input.is_empty() {
        ReferenceOutput::append(&mut output, header.as_bytes())?;
        ReferenceOutput::append(&mut output, &[1, 0, 0])?;
        return Ok(output);
    }
    let scratch = output.allocator().clone();
    let mut combined = Vec::new_in(scratch.clone());
    combined
        .try_reserve_exact(
            staged
                .checked_add(input.len())
                .ok_or(Error::ResourceLimit)?,
        )
        .map_err(|_| Error::AllocationFailed)?;
    combined.extend_from_slice(&content[content.len() - staged..]);
    combined.extend_from_slice(input);
    let mut workspace = ReferenceEncoderWorkspace::new_with_prefix_compiled(
        &combined, staged, plan, scratch, seed, candidates,
    )?;
    let mut sink = output;
    ReferenceOutput::append(&mut sink, header.as_bytes())?;
    while workspace.emitted < input.len() {
        let length = plan.next_block_len(workspace.emitted)?;
        let last = length == input.len() - workspace.emitted;
        workspace.emit_next(length, last, &mut sink)?;
    }
    Ok(sink)
}

/// One borrowed source and its existing search/entropy owners.
///
/// Search advances before emission. `emitted` advances only after an entire
/// block is appended; any intermediate failure invalidates this workspace.
/// Entropy owns committed repeats; retained search checks a copied snapshot.
struct ReferenceEncoderWorkspace<'a, A: Allocator> {
    source: &'a [u8],
    prefix_len: usize,
    plan: ReferenceSearchPlan,
    search: ReferenceSearchState<'a, A>,
    entropy: BlockEntropyEncoder<A>,
    emitted: usize,
    failed: bool,
    finished: bool,
}

impl<'a, A: Allocator + Clone> ReferenceEncoderWorkspace<'a, A> {
    fn new(source: &'a [u8], plan: ReferenceSearchPlan, factory: A) -> Result<Self> {
        Self::new_with_prefix_and_entropy(source, 0, plan, factory, None, None)
    }

    /// Bind a combined borrow and an optional prepared entropy seed.
    ///
    /// A formatted dictionary supplies the first block's previous literal tree
    /// and the sequence tables/repeat triple, mirroring `ZSTD_loadCEntropy`.
    /// The seed is borrowed only for setup and copies no dictionary ownership
    /// into the workspace; `emitted` and all block schedules remain
    /// frame-relative.
    fn new_with_prefix_and_entropy(
        source: &'a [u8],
        prefix_len: usize,
        plan: ReferenceSearchPlan,
        factory: A,
        dictionary_entropy: Option<DictionaryEntropyView<'_>>,
        literal_table: Option<HuffmanCodebook>,
    ) -> Result<Self> {
        if source
            .len()
            .checked_sub(prefix_len)
            .is_none_or(|frame_len| frame_len != plan.source_len())
        {
            return Err(Error::InvalidParameter);
        }
        let search = ReferenceSearchState::new_with_prefix(source, prefix_len, plan, &factory)?;
        let literal_encoder = LiteralEncoder::new_in(factory.clone());
        let sequence_encoder = SequenceEncoder::new_in(factory.clone());
        let payload = Vec::new_in(factory);
        let mut entropy =
            BlockEntropyEncoder::from_parts(literal_encoder, sequence_encoder, payload);
        entropy.start_frame(dictionary_entropy, literal_table)?;
        Ok(Self {
            source,
            prefix_len,
            plan,
            search,
            entropy,
            emitted: 0,
            failed: false,
            finished: false,
        })
    }

    /// Bind per-call owners to compiled candidate views without a seeding pass.
    ///
    /// The compiled candidate length must equal the staged prefix; the search
    /// constructor validates family, layout and staged geometry before any
    /// owner is reset. The compiled entropy seed is installed through the same
    /// transactional start_frame path as combined staging.
    fn new_with_prefix_compiled(
        source: &'a [u8],
        prefix_len: usize,
        plan: ReferenceSearchPlan,
        factory: A,
        seed: Option<(DictionaryEntropyView<'_>, HuffmanCodebook)>,
        candidates: CompiledCandidates<'a>,
    ) -> Result<Self> {
        let search = ReferenceSearchState::new_with_compiled(
            source, prefix_len, plan, &factory, candidates,
        )?;
        let literal_encoder = LiteralEncoder::new_in(factory.clone());
        let sequence_encoder = SequenceEncoder::new_in(factory.clone());
        let payload = Vec::new_in(factory);
        let mut entropy =
            BlockEntropyEncoder::from_parts(literal_encoder, sequence_encoder, payload);
        let (dictionary_entropy, literal_table) = match seed {
            Some((entropy, table)) => (Some(entropy), Some(table)),
            None => (None, None),
        };
        entropy.start_frame(dictionary_entropy, literal_table)?;
        Ok(Self {
            source,
            prefix_len,
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
    fn emit_next<O: ReferenceOutput>(
        &mut self,
        length: usize,
        last: bool,
        output: &mut O,
    ) -> Result<()> {
        if self.failed || self.finished {
            return Err(Error::InvalidState);
        }
        let end = self
            .emitted
            .checked_add(length)
            .filter(|&end| end <= self.plan.source_len())
            .ok_or(Error::InvalidParameter)?;
        if length == 0
            || length > self.plan.block_limit()
            || last != (end == self.plan.source_len())
            || (self.plan.is_retained() && length != self.plan.next_block_len(self.emitted)?)
            || (matches!(
                self.plan,
                ReferenceSearchPlan::Optimal(_)
                    | ReferenceSearchPlan::Ultra(_)
                    | ReferenceSearchPlan::Ultra2(_)
            ) && (self.emitted != 0 || end != self.source.len()))
        {
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

    fn emit_checked<O: ReferenceOutput>(
        &mut self,
        end: usize,
        last: bool,
        output: &mut O,
    ) -> Result<()> {
        let length = end - self.emitted;
        let start = self
            .prefix_len
            .checked_add(self.emitted)
            .filter(|&start| start <= self.source.len())
            .ok_or(Error::InvalidParameter)?;
        let stop = start
            .checked_add(length)
            .filter(|&stop| stop <= self.source.len())
            .ok_or(Error::InvalidParameter)?;
        let block = &self.source[start..stop];
        let initial_repeat = self.entropy.sequence_encoder.committed_repeats();
        let parsed = self.search.parse_next(block.len(), initial_repeat)?;
        let worthwhile = if parsed {
            if matches!(
                self.plan,
                ReferenceSearchPlan::Optimal(_)
                    | ReferenceSearchPlan::Ultra(_)
                    | ReferenceSearchPlan::Ultra2(_)
                    | ReferenceSearchPlan::RetainedOptimal(_)
                    | ReferenceSearchPlan::RetainedUltra(_)
                    | ReferenceSearchPlan::RetainedUltra2(_)
            ) {
                self.entropy.prepare_optimal(
                    self.search.literals(),
                    self.search.sequences(),
                    initial_repeat,
                    block.len(),
                )?
            } else {
                self.entropy.prepare_reference(
                    self.search.literals(),
                    self.search.sequences(),
                    initial_repeat,
                    block.len(),
                    crate::reference_policy::raw_literals(self.plan.parameters()),
                )?
            }
        } else {
            // One through six bytes bypass fresh entropy, including literal RLE.
            false
        };
        // The reference gate is after entropy work, even for a uniform block.
        // Rejected gain has effective compressed size zero. Independent entropy
        // may select a different payload/mode from C while preserving the format.
        let rle = parsed
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
        let publication = match kind {
            BlockKind::Raw => Publication::Raw,
            BlockKind::Rle => Publication::Rle,
            BlockKind::Compressed => Publication::Compressed,
        };
        let retained = self.plan.is_retained();
        let after = if retained && kind == BlockKind::Compressed {
            self.entropy.sequence_encoder.pending_repeats()?
        } else {
            initial_repeat
        };
        self.search.validate_publication(publication, after)?;
        append_block(output, block.len(), kind, payload, last)?;
        if kind == BlockKind::Compressed {
            self.entropy.commit();
        } else {
            self.entropy.discard();
        }
        self.search.publish(publication, after)?;
        if retained && self.entropy.sequence_encoder.committed_repeats() != after {
            return Err(Error::CorruptData);
        }
        Ok(())
    }

    #[cfg(test)]
    fn reset_retained(&mut self) -> Result<()> {
        if !self.plan.is_retained() {
            return Err(Error::InvalidState);
        }
        self.failed = true;
        self.search.reset_retained()?;
        self.entropy.reset();
        self.entropy.start_frame(None, None)?;
        self.emitted = 0;
        self.finished = false;
        self.failed = false;
        Ok(())
    }

    #[cfg(test)]
    fn retained_capacity(&self) -> usize {
        self.search.retained_capacity() + self.entropy.retained_capacity()
    }
}

/// Reserve the complete selected block before changing any output byte.
fn append_block<O: ReferenceOutput>(
    output: &mut O,
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
    let mut header = [0_u8; 3];
    header.copy_from_slice(&value.to_le_bytes()[..3]);
    output.append_block(header, payload)
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

    fn combined_pattern(length: usize) -> Vec<u8> {
        (0..length)
            .map(|i| b"combined dictionary staging reaches across the boundary"[i % 55])
            .collect()
    }

    #[test]
    fn combined_fast_dictionary_roundtrip_reaches_and_shrinks() {
        // Deterministic noise cannot match itself, so every improvement must
        // come from reaching into the staged dictionary region.
        let pattern = noise(0xfeed_beef, 1024);
        let input = [
            noise(0x1a2b_3c4d, 256),
            pattern.clone(),
            noise(0x9876_5432, 256),
        ]
        .concat();
        let dictionary = crate::Dictionary::raw(&pattern).unwrap();
        let view = dictionary.view();
        let request = request(11);
        let plan =
            ReferenceSearchPlan::new_with_dictionary(&request, input.len(), pattern.len()).unwrap();
        let frame = compress_reference_with_dictionary_in(&input, view, plan, Global).unwrap();
        let plain = crate::compress_with_parameters(&input, &request).unwrap();
        assert_ne!(frame, plain);
        assert!(frame.len() < plain.len());
        let decoded = crate::decompress_with_dictionary_in(
            &frame,
            &view,
            crate::DecodeOptions::new(input.len()),
            Global,
        )
        .unwrap();
        assert_eq!(decoded, input);
        assert_eq!(crate::decompress(&plain, input.len()).unwrap(), input);
    }

    #[test]
    fn combined_fast_empty_edges_match_dictionary_free_bytes() {
        let request = request(11);
        let dictionary = crate::Dictionary::raw(&combined_pattern(64)).unwrap();
        let view = dictionary.view();
        let plan = ReferenceSearchPlan::new_with_dictionary(&request, 0, 64).unwrap();
        let empty = compress_reference_with_dictionary_in(b"", view, plan, Global).unwrap();
        assert_eq!(
            empty,
            crate::compress_with_parameters(b"", &request).unwrap()
        );
        assert_eq!(
            crate::decompress_with_dictionary_in(
                &empty,
                &view,
                crate::DecodeOptions::new(0),
                Global,
            )
            .unwrap(),
            b""
        );
        // An empty dictionary stages no prefix and emits the exact plain bytes.
        let empty_dictionary = crate::Dictionary::raw(b"").unwrap();
        let input = noise(0x1111_2222, 700);
        let plan = ReferenceSearchPlan::new_with_dictionary(&request, input.len(), 0).unwrap();
        let frame =
            compress_reference_with_dictionary_in(&input, empty_dictionary.view(), plan, Global)
                .unwrap();
        assert_eq!(
            frame,
            crate::compress_with_parameters(&input, &request).unwrap()
        );
        // A dictionary smaller than the key width can stage no key and still
        // decodes through the ordinary dictionary path.
        let tiny = crate::Dictionary::raw(b"abc").unwrap();
        let plan = ReferenceSearchPlan::new_with_dictionary(&request, input.len(), 3).unwrap();
        let staged =
            compress_reference_with_dictionary_in(&input, tiny.view(), plan, Global).unwrap();
        assert_eq!(
            crate::decompress_with_dictionary_in(
                &staged,
                &tiny.view(),
                crate::DecodeOptions::new(input.len()),
                Global,
            )
            .unwrap(),
            input
        );
    }

    #[test]
    fn combined_fast_stages_only_the_in_window_dictionary_suffix() {
        // Window 2^11 stages only the dictionary's last 2048 bytes: a far
        // pattern outside that suffix is unreachable, and a dictionary
        // truncated to exactly the suffix emits identical frame bytes under
        // the same pre-admitted plan.
        let far = combined_pattern(1024);
        let tail = noise(0xabcd_ef01, 3072);
        let full = crate::Dictionary::raw(&[far, tail].concat()).unwrap();
        let suffix = crate::Dictionary::raw(&full.as_bytes()[2048..]).unwrap();
        let input = noise(0x1234_5678, 512);
        let request = request(11);
        let plan_a = ReferenceSearchPlan::new_with_dictionary(&request, input.len(), 4096).unwrap();
        let frame_a =
            compress_reference_with_dictionary_in(&input, full.view(), plan_a, Global).unwrap();
        let plan_b = ReferenceSearchPlan::new_with_dictionary(&request, input.len(), 4096).unwrap();
        let frame_b =
            compress_reference_with_dictionary_in(&input, suffix.view(), plan_b, Global).unwrap();
        assert_eq!(frame_a, frame_b);
        assert_eq!(
            crate::decompress_with_dictionary_in(
                &frame_a,
                &full.view(),
                crate::DecodeOptions::new(input.len()),
                Global,
            )
            .unwrap(),
            input
        );
        // The far pattern as input cannot reach its out-of-window copy.
        let pattern_input = combined_pattern(1024);
        let plan_c =
            ReferenceSearchPlan::new_with_dictionary(&request, pattern_input.len(), 4096).unwrap();
        let frame_c =
            compress_reference_with_dictionary_in(&pattern_input, full.view(), plan_c, Global)
                .unwrap();
        let plan_d =
            ReferenceSearchPlan::new_with_dictionary(&request, pattern_input.len(), 4096).unwrap();
        let frame_d =
            compress_reference_with_dictionary_in(&pattern_input, suffix.view(), plan_d, Global)
                .unwrap();
        assert_eq!(frame_c, frame_d);
    }

    #[test]
    fn combined_staging_rejects_unstaged_profiles_without_dropping_dictionaries() {
        let input = noise(0x0f0f_0f0f, 600);
        let dictionary = crate::Dictionary::raw(&combined_pattern(512)).unwrap();
        // Levels 12/14/17 select the optimal/ultra/ultra2 families; the
        // supported-subset clause rejects their dictionary staging
        // explicitly instead of dropping the dictionary.
        for level in [12, 14, 17] {
            let request = CompressionParameterRequest::with_level(level);
            let plan =
                ReferenceSearchPlan::new_with_dictionary(&request, input.len(), 512).unwrap();
            let result =
                compress_reference_with_dictionary_in(&input, dictionary.view(), plan, Global);
            assert_eq!(
                result.unwrap_err(),
                crate::Error::UnsupportedFeature,
                "level {level}"
            );
        }
    }

    #[test]
    fn combined_staging_rejects_retained_optimal_families_under_supported_subset() {
        // Inputs past the retained threshold select the retained
        // Optimal/Ultra/Ultra2 plans; the supported-subset clause rejects
        // their dictionary staging exactly like the fresh variants. The
        // rejection precedes any parsing, so oversized inputs stay cheap.
        let input = noise(0x1e1e_1e1e, 20_000);
        let dictionary = crate::Dictionary::raw(&combined_pattern(512)).unwrap();
        // The retained plans admit only capped tables, so the requests pin
        // the same logs the retained optimal tests use; the source size
        // selects the retained variants of all three strategies.
        for strategy in [
            ReferenceStrategy::Optimal,
            ReferenceStrategy::Ultra,
            ReferenceStrategy::Ultra2,
        ] {
            let mut request = CompressionParameterRequest::with_level(11);
            request.set_strategy(Some(strategy));
            request.set_window_log(16).unwrap();
            request.set_hash_log(12).unwrap();
            request.set_chain_log(12).unwrap();
            request.set_search_log(6).unwrap();
            request.set_min_match(3).unwrap();
            request.set_target_length(1).unwrap();
            let plan =
                ReferenceSearchPlan::new_with_dictionary(&request, input.len(), 512).unwrap();
            let result =
                compress_reference_with_dictionary_in(&input, dictionary.view(), plan, Global);
            assert_eq!(
                result.unwrap_err(),
                crate::Error::UnsupportedFeature,
                "strategy {strategy:?}"
            );
        }
    }

    #[test]
    fn combined_double_fast_dictionary_roundtrip_reaches_and_shrinks() {
        // Level 0 selects the DoubleFast profile; deterministic noise content
        // cannot match itself, so every improvement must come from the staged
        // dictionary region through either seeded table.
        let pattern = noise(0xc0de_d00d, 1024);
        let input = [
            noise(0x2143_6587, 256),
            pattern.clone(),
            noise(0x89ab_cdef, 256),
        ]
        .concat();
        let dictionary = crate::Dictionary::raw(&pattern).unwrap();
        let view = dictionary.view();
        let request = CompressionParameterRequest::with_level(0);
        let plan =
            ReferenceSearchPlan::new_with_dictionary(&request, input.len(), pattern.len()).unwrap();
        let frame = compress_reference_with_dictionary_in(&input, view, plan, Global).unwrap();
        let plain = crate::compress_with_parameters(&input, &request).unwrap();
        assert_ne!(frame, plain);
        assert!(frame.len() < plain.len());
        let decoded = crate::decompress_with_dictionary_in(
            &frame,
            &view,
            crate::DecodeOptions::new(input.len()),
            Global,
        )
        .unwrap();
        assert_eq!(decoded, input);
        assert_eq!(crate::decompress(&plain, input.len()).unwrap(), input);
    }

    #[test]
    fn combined_greedy_chain_family_dictionary_roundtrip_reaches_and_shrinks() {
        // Levels 4, 5 and 6 select the Greedy, Lazy and Lazy2 strategies on
        // this tier, all under the chain family. Deterministic noise content
        // cannot match itself, so every improvement must come from the staged
        // dictionary region through the seeded chain.
        let pattern = noise(0x5eed_1eaf, 1024);
        let input = [
            noise(0x2468_ace0, 256),
            pattern.clone(),
            noise(0x1357_9bdf, 256),
        ]
        .concat();
        let dictionary = crate::Dictionary::raw(&pattern).unwrap();
        let view = dictionary.view();
        for level in [4, 5, 6] {
            let request = CompressionParameterRequest::with_level(level);
            let plan =
                ReferenceSearchPlan::new_with_dictionary(&request, input.len(), pattern.len())
                    .unwrap();
            let frame = compress_reference_with_dictionary_in(&input, view, plan, Global).unwrap();
            let plain = crate::compress_with_parameters(&input, &request).unwrap();
            assert_ne!(frame, plain);
            assert!(frame.len() < plain.len());
            let decoded = crate::decompress_with_dictionary_in(
                &frame,
                &view,
                crate::DecodeOptions::new(input.len()),
                Global,
            )
            .unwrap();
            assert_eq!(decoded, input);
            assert_eq!(crate::decompress(&plain, input.len()).unwrap(), input);
        }
    }

    #[test]
    fn combined_tree_family_dictionary_roundtrip_reaches_and_shrinks() {
        // Level 9 selects BinaryTreeLazy2. The tree needs no seeding: its
        // first query back-fills the staged region through the
        // unsorted-insert frontier, and deterministic noise content cannot
        // match itself, so every improvement must come from that region.
        let pattern = noise(0x7ee_5eed, 1024);
        let input = [
            noise(0x0bad_c0de, 256),
            pattern.clone(),
            noise(0x600d_f00d, 256),
        ]
        .concat();
        let dictionary = crate::Dictionary::raw(&pattern).unwrap();
        let view = dictionary.view();
        let request = CompressionParameterRequest::with_level(9);
        let plan =
            ReferenceSearchPlan::new_with_dictionary(&request, input.len(), pattern.len()).unwrap();
        let frame = compress_reference_with_dictionary_in(&input, view, plan, Global).unwrap();
        let plain = crate::compress_with_parameters(&input, &request).unwrap();
        assert_ne!(frame, plain);
        assert!(frame.len() < plain.len());
        let decoded = crate::decompress_with_dictionary_in(
            &frame,
            &view,
            crate::DecodeOptions::new(input.len()),
            Global,
        )
        .unwrap();
        assert_eq!(decoded, input);
        assert_eq!(crate::decompress(&plain, input.len()).unwrap(), input);
    }

    #[test]
    fn combined_lazy_row_family_dictionary_roundtrip_reaches_and_shrinks() {
        // A window override past the chain cutoff selects the row family;
        // seeded rows under the fresh salt must offer the same reach.
        let pattern = noise(0xd15e_a5e5, 1024);
        let input = [
            noise(0x0fed_cba9, 256),
            pattern.clone(),
            noise(0x8642_0000, 256),
        ]
        .concat();
        let dictionary = crate::Dictionary::raw(&pattern).unwrap();
        let view = dictionary.view();
        let mut request = CompressionParameterRequest::with_level(5);
        request.set_window_log(15).unwrap();
        let plan =
            ReferenceSearchPlan::new_with_dictionary(&request, input.len(), pattern.len()).unwrap();
        let frame = compress_reference_with_dictionary_in(&input, view, plan, Global).unwrap();
        let plain = crate::compress_with_parameters(&input, &request).unwrap();
        assert_ne!(frame, plain);
        assert!(frame.len() < plain.len());
        let decoded = crate::decompress_with_dictionary_in(
            &frame,
            &view,
            crate::DecodeOptions::new(input.len()),
            Global,
        )
        .unwrap();
        assert_eq!(decoded, input);
        assert_eq!(crate::decompress(&plain, input.len()).unwrap(), input);
    }

    #[test]
    fn combined_fast_emits_prepared_dictionary_ids_and_roundtrips() {
        const PREPARED: &[u8] = include_bytes!("../tests/upstream/dictionaries/formatted.dict");
        let dictionary = crate::Dictionary::from_bytes(PREPARED).unwrap();
        let view = dictionary.view();
        assert_ne!(view.id(), 0);
        let shared: Vec<u8> = view.content()[..1024].to_vec();
        let input = [noise(0x7777_8888, 128), shared].concat();
        let request = request(11);
        let plan =
            ReferenceSearchPlan::new_with_dictionary(&request, input.len(), view.content().len())
                .unwrap();
        let frame = compress_reference_with_dictionary_in(&input, view, plan, Global).unwrap();
        let decoded = crate::decompress_with_dictionary_in(
            &frame,
            &view,
            crate::DecodeOptions::new(input.len()),
            Global,
        )
        .unwrap();
        assert_eq!(decoded, input);
        // The decoder matches the emitted identifier against this dictionary,
        // so a different content owner with the same identifier mismatch is
        // out of scope here; byte-level header checks live in frame tests.
        assert!(frame.len() > 4);
    }

    /// Borrow the first compressed block payload after its three-byte header.
    fn first_compressed_block(frame: &[u8]) -> &[u8] {
        let (_, used) = FrameHeader::parse(frame).unwrap();
        let mut cursor = ByteCursor::new(&frame[used..]);
        let header = BlockHeader::read(&mut cursor, MAX_BLOCK_SIZE).unwrap();
        assert_eq!(header.kind, BlockKind::Compressed);
        let start = used + cursor.position();
        &frame[start..start + header.size]
    }

    /// Read the first literal-section kind and, for a nonempty sequence
    /// section, its two-bit mode byte in LL/OF/ML channel order.
    fn first_literal_kind_and_sequence_modes(payload: &[u8]) -> (u32, Option<u8>) {
        let value = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
        let kind = value & 3;
        let literal_bytes = match kind {
            0 | 1 => {
                let header = if value & 4 == 0 {
                    1
                } else if value & 8 == 0 {
                    2
                } else {
                    3
                };
                let regenerated = if header == 1 {
                    (value >> 3) as usize
                } else {
                    (value >> 4) as usize
                };
                header + if kind == 1 { 1 } else { regenerated }
            }
            2 | 3 => {
                let format = ((value >> 2) & 3) as usize;
                let bits = [10, 14, 18][format];
                let header = [3, 4, 5][format];
                let compressed = ((value >> (4 + bits)) & ((1 << bits) - 1)) as usize;
                header + compressed
            }
            _ => unreachable!(),
        };
        let count = payload[literal_bytes];
        let count_bytes = match count {
            0..=127 => 1,
            128..=254 => 2,
            _ => 3,
        };
        let modes = payload[literal_bytes + count_bytes];
        (kind, (count != 0).then_some(modes))
    }

    fn mode_repeats(mode: u8) -> bool {
        mode == 3
    }

    #[test]
    fn formatted_dictionary_entropy_seeds_the_first_block_across_profiles() {
        const PREPARED: &[u8] = include_bytes!("../tests/upstream/dictionaries/formatted.dict");
        const INPUT: &[u8] =
            include_bytes!("../tests/upstream/dictionaries/dictionary-treeless-literals.expected");
        let dictionary = crate::Dictionary::from_bytes(PREPARED).unwrap();
        let view = dictionary.view();
        let raw = crate::Dictionary::raw(view.content()).unwrap();
        // Levels 1, 4 and 9 select Fast, Greedy and BinaryTreeLazy2 on this
        // tier, so the exact prepared entropy seed reaches all admitted
        // combined-staging families.
        for level in [1, 4, 9] {
            let request = CompressionParameterRequest::with_level(level);
            let plan =
                ReferenceSearchPlan::new_with_dictionary(&request, INPUT.len(), PREPARED.len())
                    .unwrap();
            let frame = compress_reference_with_dictionary_in(INPUT, view, plan, Global).unwrap();
            let payload = first_compressed_block(&frame);
            let (literal_kind, modes) = first_literal_kind_and_sequence_modes(payload);
            // Only a seeded previous tree can produce a treeless header (3) in
            // the first block; a fresh frame requires kind 2 or raw literals.
            assert_eq!(literal_kind, 3, "level {level}");
            let modes = modes.expect("first block carries sequences");
            assert!(
                mode_repeats((modes >> 6) & 3)
                    || mode_repeats((modes >> 4) & 3)
                    || mode_repeats((modes >> 2) & 3),
                "level {level}: modes {modes:02x}"
            );
            assert_eq!(
                crate::decompress_with_dictionary_in(
                    &frame,
                    &view,
                    crate::DecodeOptions::new(INPUT.len()),
                    Global,
                )
                .unwrap(),
                INPUT
            );
            assert_eq!(
                compress_reference_with_dictionary_in(INPUT, view, plan, Global).unwrap(),
                frame
            );
            // The same content as a raw dictionary owns no prepared entropy:
            // no treeless literals, no repeat modes, and a larger frame.
            let raw_plan =
                ReferenceSearchPlan::new_with_dictionary(&request, INPUT.len(), PREPARED.len())
                    .unwrap();
            let raw_frame =
                compress_reference_with_dictionary_in(INPUT, raw.view(), raw_plan, Global).unwrap();
            let raw_payload = first_compressed_block(&raw_frame);
            let (raw_kind, raw_modes) = first_literal_kind_and_sequence_modes(raw_payload);
            assert_ne!(raw_kind, 3, "level {level}");
            let raw_modes = raw_modes.expect("first block carries sequences");
            assert!(
                !mode_repeats((raw_modes >> 6) & 3)
                    && !mode_repeats((raw_modes >> 4) & 3)
                    && !mode_repeats((raw_modes >> 2) & 3),
                "level {level}: modes {raw_modes:02x}"
            );
            assert!(frame.len() < raw_frame.len(), "level {level}");
            assert_eq!(
                crate::decompress_with_dictionary_in(
                    &raw_frame,
                    &raw.view(),
                    crate::DecodeOptions::new(INPUT.len()),
                    Global,
                )
                .unwrap(),
                INPUT
            );
        }
    }

    #[test]
    fn prepared_entropy_never_forces_uncovered_treeless_literals() {
        // This prepared Huffman tree lacks fourteen symbols; a literal block
        // containing one must build a new tree instead of repeating the
        // dictionary's incomplete table.
        const PREPARED: &[u8] =
            include_bytes!("../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols");
        const TEXT: &[u8] =
            include_bytes!("../tests/upstream/dictionaries/dictionary-treeless-literals.expected");
        let dictionary = crate::Dictionary::from_bytes(PREPARED).unwrap();
        let view = dictionary.view();
        assert_eq!(view.entropy().unwrap().huffman.code_lengths()[0xc0], 0);
        let mut input = [TEXT; 4].concat();
        for byte in input.iter_mut().step_by(37) {
            *byte = 0xc0;
        }
        for level in [1, 4] {
            let request = CompressionParameterRequest::with_level(level);
            let plan =
                ReferenceSearchPlan::new_with_dictionary(&request, input.len(), PREPARED.len())
                    .unwrap();
            let frame = compress_reference_with_dictionary_in(&input, view, plan, Global).unwrap();
            let payload = first_compressed_block(&frame);
            let (literal_kind, _) = first_literal_kind_and_sequence_modes(payload);
            assert_ne!(literal_kind, 3, "level {level}");
            assert_eq!(
                crate::decompress_with_dictionary_in(
                    &frame,
                    &view,
                    crate::DecodeOptions::new(input.len()),
                    Global,
                )
                .unwrap(),
                input
            );
        }
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
    ) -> (ReferenceEncoderWorkspace<'a, A>, Vec<u8>) {
        let plan = FastExecutionPlan::new(request, input.len()).unwrap();
        let settings =
            ReferenceFrameSettings::new(plan.source_len(), plan.parameters().window_log()).unwrap();
        let frame = ReferenceFrameHeader::new(&settings).as_bytes().to_vec();
        (
            ReferenceEncoderWorkspace::new(input, ReferenceSearchPlan::Fast(plan), allocator)
                .unwrap(),
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

#[cfg(test)]
mod optimal_tests {
    use super::*;
    use crate::RawCompressionParameters;
    use crate::optimal_fresh_wide_fixture as fixture;
    use crate::optimal_plan::OptimalExecutionPlan;
    use alloc::alloc::Global;
    #[test]
    fn every_complete_observed_source_uses_shared_frame_entropy() {
        let mut cases = 0;
        for case in fixture::read(fixture::PACK)
            .unwrap()
            .into_iter()
            .filter(|c| c.literal_mode != 1)
        {
            let parameters = RawCompressionParameters {
                window_log: u32::from(case.window),
                hash_log: u32::from(case.hash),
                chain_log: u32::from(case.chain),
                search_log: u32::from(case.search),
                min_match: case.minimum,
                target_length: case.target,
                strategy: 7,
            }
            .validate()
            .unwrap();
            let plan = ReferenceSearchPlan::Optimal(
                OptimalExecutionPlan::from_resolved_parameters(parameters, case.source.len())
                    .unwrap(),
            );
            let frame = compress_reference_in(case.source, plan, Global).unwrap();
            assert_eq!(
                crate::decompress(&frame, 16384).unwrap(),
                case.source,
                "{}",
                case.name
            );
            assert_eq!(
                compress_reference_in(case.source, plan, Global).unwrap(),
                frame
            );
            let mut workspace = ReferenceEncoderWorkspace::new(case.source, plan, Global).unwrap();
            let mut output = Vec::new();
            assert_eq!(
                workspace.emit_next(case.source.len() / 2, false, &mut output),
                Err(Error::InvalidParameter)
            );
            assert!(!workspace.failed);
            assert!(output.is_empty());
            workspace
                .emit_next(case.source.len(), true, &mut output)
                .unwrap();
            assert!(workspace.finished);
            cases += 1;
        }
        assert_eq!(cases, 17);
    }
    #[test]
    fn short_periodic_three_byte_matches_reach_shared_entropy() {
        let input: Vec<_> = (0..64).map(|i| b"abcXabcYabcZabcd"[i % 15]).collect();
        let mut request = crate::CompressionParameterRequest::with_level(11);
        request.set_strategy(Some(crate::ReferenceStrategy::Optimal));
        request.set_window_log(16).unwrap();
        request.set_hash_log(12).unwrap();
        request.set_chain_log(12).unwrap();
        request.set_search_log(6).unwrap();
        request.set_min_match(3).unwrap();
        request.set_target_length(1).unwrap();
        let parameters = request.resolve(crate::SourceSize::Known(64), 0).unwrap();
        let plan = ReferenceSearchPlan::Optimal(
            OptimalExecutionPlan::from_resolved_parameters(parameters, 64).unwrap(),
        );
        let mut workspace = ReferenceEncoderWorkspace::new(&input, plan, Global).unwrap();
        workspace
            .search
            .parse_next(64, [1, 4, 8])
            .expect("parse complete periodic source");
        workspace
            .entropy
            .prepare_optimal(
                workspace.search.literals(),
                workspace.search.sequences(),
                [1, 4, 8],
                64,
            )
            .expect("prepare three-byte sequence entropy");
        let frame = compress_reference_in(&input, plan, Global).unwrap();
        assert_eq!(crate::decompress(&frame, 64).unwrap(), input);
    }
}

#[cfg(test)]
mod ultra_frame_tests {
    use super::*;
    use crate::RawCompressionParameters;
    use crate::optimal_plan::OptimalExecutionPlan;
    use crate::ultra_fresh_fixture as fixture;
    use alloc::alloc::Global;
    #[test]
    fn every_complete_observed_source_uses_shared_frame_entropy() {
        let mut cases = 0;
        for case in fixture::read(fixture::PACK)
            .unwrap()
            .into_iter()
            .filter(|c| c.policy == 2 && c.literal_mode != 1)
        {
            let parameters = RawCompressionParameters {
                window_log: u32::from(case.window),
                hash_log: u32::from(case.hash),
                chain_log: u32::from(case.chain),
                search_log: u32::from(case.search),
                min_match: case.minimum,
                target_length: case.target,
                strategy: 8,
            }
            .validate()
            .unwrap();
            let plan = ReferenceSearchPlan::Ultra(
                OptimalExecutionPlan::from_ultra_parameters(parameters, case.source.len()).unwrap(),
            );
            let frame = compress_reference_in(case.source, plan, Global).unwrap();
            assert_eq!(
                crate::decompress(&frame, 16384).unwrap(),
                case.source,
                "{}",
                case.name
            );
            assert_eq!(
                compress_reference_in(case.source, plan, Global).unwrap(),
                frame
            );
            let mut workspace = ReferenceEncoderWorkspace::new(case.source, plan, Global).unwrap();
            let mut output = Vec::new();
            assert_eq!(
                workspace.emit_next(case.source.len() / 2, false, &mut output),
                Err(Error::InvalidParameter)
            );
            assert!(!workspace.failed);
            assert!(output.is_empty());
            workspace
                .emit_next(case.source.len(), true, &mut output)
                .unwrap();
            assert!(workspace.finished);
            cases += 1;
        }
        assert_eq!(cases, 19);
    }
    #[test]
    fn short_periodic_three_byte_matches_reach_shared_entropy() {
        let input: Vec<_> = (0..64).map(|i| b"abcXabcYabcZabcd"[i % 15]).collect();
        let mut request = crate::CompressionParameterRequest::with_level(11);
        request.set_strategy(Some(crate::ReferenceStrategy::Ultra));
        request.set_window_log(16).unwrap();
        request.set_hash_log(12).unwrap();
        request.set_chain_log(12).unwrap();
        request.set_search_log(6).unwrap();
        request.set_min_match(3).unwrap();
        request.set_target_length(1).unwrap();
        let parameters = request.resolve(crate::SourceSize::Known(64), 0).unwrap();
        let plan = ReferenceSearchPlan::Ultra(
            OptimalExecutionPlan::from_ultra_parameters(parameters, 64).unwrap(),
        );
        let mut workspace = ReferenceEncoderWorkspace::new(&input, plan, Global).unwrap();
        workspace
            .search
            .parse_next(64, [1, 4, 8])
            .expect("parse complete periodic source");
        workspace
            .entropy
            .prepare_optimal(
                workspace.search.literals(),
                workspace.search.sequences(),
                [1, 4, 8],
                64,
            )
            .expect("prepare three-byte sequence entropy");
        let frame = compress_reference_in(&input, plan, Global).unwrap();
        assert_eq!(crate::decompress(&frame, 64).unwrap(), input);
    }
    #[test]
    fn every_private_ultra_frame_allocation_failure_releases_exact_owners() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let source: Vec<_> = (0..64).map(|i| b"abcXabcYabcZabcd"[i % 15]).collect();
        let parameters = RawCompressionParameters {
            window_log: 10,
            hash_log: 6,
            chain_log: 6,
            search_log: 1,
            min_match: 3,
            target_length: 1,
            strategy: 8,
        }
        .validate()
        .unwrap();
        let plan = ReferenceSearchPlan::Ultra(
            OptimalExecutionPlan::from_ultra_parameters(parameters, 64).unwrap(),
        );
        let factory = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = factory.monitor();
        let frame = compress_reference_in(&source, plan, factory).unwrap();
        let attempts = monitor.snapshot().attempts;
        assert_eq!(crate::decompress(&frame, 64).unwrap(), source);
        drop(frame);
        monitor.assert_released();
        assert!(attempts >= 10);
        for failed in 1..=attempts {
            let factory = TrackingAllocator::new(AllocatorConfig {
                fail_at: core::num::NonZeroU64::new(failed),
                ..AllocatorConfig::default()
            })
            .unwrap();
            let monitor = factory.monitor();
            assert!(
                matches!(
                    compress_reference_in(&source, plan, factory),
                    Err(Error::AllocationFailed)
                ),
                "failure {failed}"
            );
            monitor.assert_released();
            let snapshot = monitor.snapshot();
            assert_eq!(snapshot.owners_dropped, snapshot.identities_created);
        }
    }
    #[test]
    fn failed_output_publication_keeps_output_and_poisons_workspace() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let source = [b'a'; 64];
        let parameters = RawCompressionParameters {
            window_log: 10,
            hash_log: 6,
            chain_log: 6,
            search_log: 1,
            min_match: 3,
            target_length: 1,
            strategy: 8,
        }
        .validate()
        .unwrap();
        let plan = ReferenceSearchPlan::Ultra(
            OptimalExecutionPlan::from_ultra_parameters(parameters, 64).unwrap(),
        );
        let mut workspace = ReferenceEncoderWorkspace::new(&source, plan, Global).unwrap();
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut output = Vec::new_in(owner);
        assert!(monitor.fail_next());
        assert_eq!(
            workspace.emit_next(64, true, &mut output),
            Err(Error::AllocationFailed)
        );
        assert!(output.is_empty());
        assert!(workspace.failed);
        assert!(!workspace.finished);
        assert_eq!(workspace.emitted, 0);
        assert_eq!(
            workspace.emit_next(64, true, &mut output),
            Err(Error::InvalidState)
        );
        drop(output);
        monitor.assert_released();
    }
}

#[cfg(test)]
mod ultra2_frame_tests {
    use super::*;
    use crate::{RawCompressionParameters, ultra2_frame_plan::Ultra2FramePlan};
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    fn plan() -> ReferenceSearchPlan {
        ReferenceSearchPlan::Ultra2(
            Ultra2FramePlan::new(
                RawCompressionParameters {
                    window_log: 10,
                    hash_log: 6,
                    chain_log: 6,
                    search_log: 1,
                    min_match: 3,
                    target_length: 1,
                    strategy: 9,
                }
                .validate()
                .unwrap(),
                64,
            )
            .unwrap(),
        )
    }
    #[test]
    fn ultra2_rejects_partial_schedule_without_mutation_then_emits_one_frame() {
        let source = [b'a'; 64];
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let mut w = ReferenceEncoderWorkspace::new(&source, plan(), &owner).unwrap();
        let settings = ReferenceFrameSettings::new(64, 10).unwrap();
        let mut output = ReferenceFrameHeader::new(&settings).as_bytes().to_vec();
        let before = output.clone();
        let allocations = monitor.snapshot();
        for (n, last) in [
            (0, false),
            (32, false),
            (32, true),
            (64, false),
            (65, true),
            (usize::MAX, true),
        ] {
            assert_eq!(
                w.emit_next(n, last, &mut output),
                Err(Error::InvalidParameter)
            );
            assert_eq!(output, before);
            assert_eq!(monitor.snapshot(), allocations);
            assert_eq!(w.emitted, 0);
            assert!(!w.failed);
        }
        w.emit_next(64, true, &mut output).unwrap();
        assert_eq!(crate::decompress(&output, 64).unwrap(), source);
        let completed = output.clone();
        assert_eq!(w.emit_next(64, true, &mut output), Err(Error::InvalidState));
        assert_eq!(output, completed);
        drop(w);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        drop(owner);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }
    #[test]
    fn ultra2_seed_reservation_failure_cannot_publish_entropy_or_output() {
        let source = [b'a'; 64];
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut w = ReferenceEncoderWorkspace::new(&source, plan(), &owner).unwrap();
        let mut output = Vec::new();
        let repeats = w.entropy.sequence_encoder.committed_repeats();
        assert!(monitor.fail_next());
        assert_eq!(
            w.emit_next(64, true, &mut output),
            Err(Error::AllocationFailed)
        );
        assert!(w.failed);
        assert_eq!(w.emitted, 0);
        assert!(output.is_empty());
        assert_eq!(w.entropy.sequence_encoder.committed_repeats(), repeats);
        assert!(w.search.literals().is_empty());
        assert!(w.search.sequences().is_empty());
        assert_eq!(w.emit_next(64, true, &mut output), Err(Error::InvalidState));
        drop(w);
        monitor.assert_released();
    }
}

#[cfg(test)]
#[path = "../tests/support/ultra_retained_frame_encode.rs"]
mod retained_ultra_tests;

#[cfg(test)]
#[path = "../tests/support/ultra2_retained_frame_encode.rs"]
mod retained_ultra2_integration_frames;
