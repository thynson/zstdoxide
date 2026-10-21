//! Private Optimal and bounded scripted/ordinary Ultra paths with checked providers.
//! Upstream counterpart: ZSTD_compressBlock_opt_generic, zstd v1.5.7.
#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "Private Ultra parsing and internal diagnostics are not exposed publicly"
)]
use crate::optimal_frequencies::{FrequencySnapshot, OptimalFrequencies};
use crate::optimal_matches::OptimalMatches;
use crate::optimal_path_layout::{
    BLOCK_LIMIT, PATH_HORIZON, PathCell, PathIndex, PathPhase, PathPrice, ReverseCursor,
};
use crate::optimal_paths::OptimalPaths;
use crate::optimal_price::PricePolicy;
use crate::optimal_search::OptimalSearch;
use crate::optimal_search_layout::{OptimalMatch, OptimalQuery};
use crate::{Error, Result};
use core::alloc::Allocator;

pub(crate) trait PathProvider {
    fn query<A: Allocator>(
        &mut self,
        query: OptimalQuery,
        matches: &mut OptimalMatches<A>,
    ) -> Result<()>;
}
/// The ordinary provider can only be constructed by the paired fresh entry.
struct FreshProvider<'s, 'a, H: Allocator, N: Allocator, S: Allocator, B: Allocator>(
    &'s mut OptimalSearch<'a, H, N, S, B>,
);
impl<H: Allocator, N: Allocator, S: Allocator, B: Allocator> PathProvider
    for FreshProvider<'_, '_, H, N, S, B>
{
    fn query<A: Allocator>(
        &mut self,
        query: OptimalQuery,
        matches: &mut OptimalMatches<A>,
    ) -> Result<()> {
        for &candidate in self.0.query_fresh(query)? {
            matches.push(candidate)?;
        }
        Ok(())
    }
}
struct Ultra2Provider<'s, 'a, H: Allocator, N: Allocator, S: Allocator, B: Allocator>(
    &'s mut OptimalSearch<'a, H, N, S, B>,
);
impl<H: Allocator, N: Allocator, S: Allocator, B: Allocator> PathProvider
    for Ultra2Provider<'_, '_, H, N, S, B>
{
    fn query<A: Allocator>(
        &mut self,
        query: OptimalQuery,
        matches: &mut OptimalMatches<A>,
    ) -> Result<()> {
        for &candidate in self.0.query_ultra2(query)? {
            matches.push(candidate)?;
        }
        Ok(())
    }
}
struct RetainedProvider<'s, 'a, H: Allocator, N: Allocator, S: Allocator, B: Allocator>(
    &'s mut OptimalSearch<'a, H, N, S, B>,
);
impl<H: Allocator, N: Allocator, S: Allocator, B: Allocator> PathProvider
    for RetainedProvider<'_, '_, H, N, S, B>
{
    fn query<A: Allocator>(
        &mut self,
        query: OptimalQuery,
        matches: &mut OptimalMatches<A>,
    ) -> Result<()> {
        for &candidate in self.0.query_retained(query)? {
            matches.push(candidate)?;
        }
        Ok(())
    }
}
pub(crate) trait PathObserver {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        event: PathEvent,
        paths: &OptimalPaths<P>,
        frequencies: &OptimalFrequencies<F>,
        matches: &[OptimalMatch],
    ) -> Result<()>;
}
/// Scalar event tags follow the independently captured transition protocol.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PathEvent {
    pub(crate) stage: u8,
    pub(crate) reverse_phase: bool,
    pub(crate) ip: usize,
    pub(crate) anchor: usize,
    pub(crate) cur: Option<usize>,
    pub(crate) last: Option<usize>,
    pub(crate) tail: Option<usize>,
    pub(crate) repeats: [u32; 3],
    pub(crate) stretch: Option<PathCell>,
    pub(crate) reverse: Option<[usize; 3]>,
    pub(crate) query: Option<OptimalQuery>,
    pub(crate) appended: Option<[u32; 4]>,
    pub(crate) alternative: Option<[i64; 12]>,
    pub(crate) sequences: usize,
}
impl PathEvent {
    fn at(stage: u8, cur: usize, last: usize) -> Self {
        Self {
            stage,
            cur: Some(cur),
            last: Some(last),
            ..Self::default()
        }
    }
    fn selected(stage: u8, cur: usize, last: usize, stretch: PathCell) -> Self {
        Self {
            stretch: Some(stretch),
            ..Self::at(stage, cur, last)
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PathResult {
    pub(crate) tail: usize,
    pub(crate) repeats: [u32; 3],
    pub(crate) sequences: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Status {
    Ready,
    Finished,
    Poisoned,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Scope {
    Scripted,
    UltraFresh,
    UltraRetained,
    Ultra2,
}
pub(crate) struct OptimalParser<'a, F: Allocator, P: Allocator, M: Allocator> {
    source: &'a [u8],
    block_start: usize,
    block_end: usize,
    retained_end: usize,
    minimum: usize,
    target: usize,
    frequencies: OptimalFrequencies<F>,
    paths: OptimalPaths<P>,
    matches: OptimalMatches<M>,
    ip: usize,
    anchor: usize,
    repeats: [u32; 3],
    wire_repeats: [u32; 3],
    sequences: usize,
    last_stretch: PathCell,
    status: Status,
    scope: Scope,
    ultra2: Option<bool>,
}
fn resolved(off: u32, repeats: [u32; 3], zero: bool) -> Result<(u32, [u32; 3])> {
    if off == 0 || repeats.contains(&0) {
        return Err(Error::CorruptData);
    }
    let (distance, next) = if off > 3 {
        let d = off - 3;
        (d, [d, repeats[0], repeats[1]])
    } else {
        match off - 1 + u32::from(zero) {
            0 => (repeats[0], repeats),
            1 => (repeats[1], [repeats[1], repeats[0], repeats[2]]),
            2 => (repeats[2], [repeats[2], repeats[0], repeats[1]]),
            3 => {
                let d = repeats[0]
                    .checked_sub(1)
                    .filter(|&v| v > 0)
                    .ok_or(Error::CorruptData)?;
                (d, [d, repeats[0], repeats[1]])
            }
            _ => return Err(Error::CorruptData),
        }
    };
    if distance == 0 {
        return Err(Error::CorruptData);
    }
    Ok((distance, next))
}
impl<'a, F: Allocator, P: Allocator, M: Allocator> OptimalParser<'a, F, P, M> {
    pub(crate) fn from_parts(
        source: &'a [u8],
        minimum: u8,
        target: usize,
        frequencies: OptimalFrequencies<F>,
        paths: OptimalPaths<P>,
        matches: OptimalMatches<M>,
    ) -> Result<Self> {
        Self::from_policy_parts(
            PricePolicy::Optimal,
            source,
            minimum,
            target,
            frequencies,
            paths,
            matches,
        )
    }
    /// Explicit private scripted composition, retaining the 8192-byte Ultra bound.
    pub(crate) fn from_policy_parts(
        policy: PricePolicy,
        source: &'a [u8],
        minimum: u8,
        target: usize,
        frequencies: OptimalFrequencies<F>,
        paths: OptimalPaths<P>,
        matches: OptimalMatches<M>,
    ) -> Result<Self> {
        if frequencies.policy() != policy || (policy == PricePolicy::Ultra && source.len() > 8192) {
            return Err(Error::InvalidParameter);
        }
        Self::initialize_parts(
            source,
            minimum,
            target,
            frequencies,
            paths,
            matches,
            Scope::Scripted,
        )
    }
    /// Independently qualified ordinary Ultra envelope. The larger source owner
    /// cannot be used with an arbitrary scripted provider or retained planning.
    pub(crate) fn from_ultra_fresh_parts(
        source: &'a [u8],
        minimum: u8,
        target: usize,
        frequencies: OptimalFrequencies<F>,
        paths: OptimalPaths<P>,
        matches: OptimalMatches<M>,
    ) -> Result<Self> {
        if frequencies.policy() != PricePolicy::Ultra || !(64..=16384).contains(&source.len()) {
            return Err(Error::InvalidParameter);
        }
        Self::initialize_parts(
            source,
            minimum,
            target,
            frequencies,
            paths,
            matches,
            Scope::UltraFresh,
        )
    }
    /// Own a bounded retained-Ultra workspace without admitting any provider.
    /// A later checked retained plan binds the source window and block schedule.
    /// Existing fresh, scripted, Ultra2 and Optimal retained entries reject it.
    pub(crate) fn from_ultra_retained_parts(
        source: &'a [u8],
        minimum: u8,
        target: usize,
        frequencies: OptimalFrequencies<F>,
        paths: OptimalPaths<P>,
        matches: OptimalMatches<M>,
    ) -> Result<Self> {
        if frequencies.policy() != PricePolicy::Ultra || !(128..=32768).contains(&source.len()) {
            return Err(Error::InvalidParameter);
        }
        Self::initialize_parts(
            source,
            minimum,
            target,
            frequencies,
            paths,
            matches,
            Scope::UltraRetained,
        )
    }
    pub(crate) fn from_ultra2_parts(
        source: &'a [u8],
        minimum: u8,
        target: usize,
        frequencies: OptimalFrequencies<F>,
        paths: OptimalPaths<P>,
        matches: OptimalMatches<M>,
    ) -> Result<Self> {
        if frequencies.policy() != PricePolicy::Ultra || !(7..=16384).contains(&source.len()) {
            return Err(Error::InvalidParameter);
        }
        Self::initialize_parts(
            source,
            minimum,
            target,
            frequencies,
            paths,
            matches,
            Scope::Ultra2,
        )
    }
    fn initialize_parts(
        source: &'a [u8],
        minimum: u8,
        target: usize,
        mut frequencies: OptimalFrequencies<F>,
        mut paths: OptimalPaths<P>,
        mut matches: OptimalMatches<M>,
        scope: Scope,
    ) -> Result<Self> {
        let valid_source = if scope == Scope::Ultra2 {
            (7..=16384).contains(&source.len())
        } else {
            (9..=BLOCK_LIMIT as usize).contains(&source.len())
        };
        if !valid_source || !(3..=7).contains(&minimum) || target >= PATH_HORIZON {
            return Err(Error::InvalidParameter);
        }
        frequencies.reset()?;
        paths.reset()?;
        matches.clear();
        Ok(Self {
            source,
            block_start: 0,
            block_end: source.len(),
            retained_end: 0,
            minimum: if minimum == 3 { 3 } else { 4 },
            target,
            frequencies,
            paths,
            matches,
            ip: 0,
            anchor: 0,
            repeats: [1, 4, 8],
            wire_repeats: [1, 4, 8],
            sequences: 0,
            last_stretch: PathCell::ZERO,
            status: Status::Ready,
            scope,
            ultra2: (scope == Scope::Ultra2).then_some(false),
        })
    }
    pub(crate) fn reset(&mut self) -> Result<()> {
        self.status = Status::Poisoned;
        self.frequencies.reset()?;
        self.paths.reset()?;
        self.matches.clear();
        self.block_start = 0;
        self.block_end = self.source.len();
        self.retained_end = 0;
        self.ip = 0;
        self.anchor = 0;
        self.repeats = [1, 4, 8];
        self.wire_repeats = self.repeats;
        self.sequences = 0;
        self.last_stretch = PathCell::ZERO;
        self.status = Status::Ready;
        self.ultra2 = self.ultra2.map(|_| false);
        Ok(())
    }
    /// Parse one bounded ordinary fresh block with independently owned search.
    /// A retry requires resetting both this parser and the search. Observer
    /// effects are caller-owned and cannot be rolled back after failure.
    pub(crate) fn parse_fresh<
        H: Allocator,
        N: Allocator,
        S: Allocator,
        B: Allocator,
        O: PathObserver,
    >(
        &mut self,
        search: &mut OptimalSearch<'a, H, N, S, B>,
        observer: &mut O,
    ) -> Result<PathResult> {
        self.require_ordinary_policy()?;
        search.validate_fresh_start(self.source, self.minimum, self.target)?;
        self.parse(&mut FreshProvider(search), observer)
    }
    /// Parse one qualified ordinary Ultra source; this does not publish tokens.
    pub(crate) fn parse_ultra_fresh<
        H: Allocator,
        N: Allocator,
        S: Allocator,
        B: Allocator,
        O: PathObserver,
    >(
        &mut self,
        search: &mut OptimalSearch<'a, H, N, S, B>,
        observer: &mut O,
    ) -> Result<PathResult> {
        if self.ultra2.is_some()
            || self.scope == Scope::UltraRetained
            || self.frequencies.policy() != PricePolicy::Ultra
        {
            return Err(Error::UnsupportedFeature);
        }
        if !(64..=16384).contains(&self.source.len()) {
            return Err(Error::InvalidParameter);
        }
        search.validate_fresh_start(self.source, self.minimum, self.target)?;
        self.parse_with_provider(&mut FreshProvider(search), observer)
    }
    pub(crate) fn ultra2_configuration(&self) -> Result<(&'a [u8], usize, usize, bool)> {
        if self.ultra2 != Some(false) || self.frequencies.policy() != PricePolicy::Ultra {
            return Err(Error::UnsupportedFeature);
        }
        if self.status != Status::Ready
            || self.retained_end != 0
            || self.frequencies.snapshot()?.initialized
        {
            return Err(Error::InvalidState);
        }
        Ok((
            self.source,
            self.minimum,
            self.target,
            self.frequencies.disabled_literals(),
        ))
    }
    pub(crate) fn ultra2_frequencies(&self) -> Result<FrequencySnapshot> {
        if self.ultra2.is_none() {
            return Err(Error::UnsupportedFeature);
        }
        self.frequencies.snapshot()
    }
    pub(crate) fn parse_ultra2<
        H: Allocator,
        N: Allocator,
        S: Allocator,
        B: Allocator,
        O: PathObserver,
    >(
        &mut self,
        search: &mut OptimalSearch<'a, H, N, S, B>,
        observer: &mut O,
    ) -> Result<PathResult> {
        let rebased = self.ultra2.ok_or(Error::UnsupportedFeature)?;
        if self.frequencies.policy() != PricePolicy::Ultra {
            return Err(Error::UnsupportedFeature);
        }
        search.validate_ultra2_phase(self.source, self.minimum, self.target, rebased)?;
        self.parse_with_provider(&mut Ultra2Provider(search), observer)
    }
    /// Preserve exact frequency/path owners; reset only invocation-local scratch.
    pub(crate) fn rebase_ultra2<H: Allocator, N: Allocator, S: Allocator, B: Allocator>(
        &mut self,
        search: &mut OptimalSearch<'a, H, N, S, B>,
    ) -> Result<()> {
        if self.ultra2 != Some(false) || self.status != Status::Finished || self.retained_end != 0 {
            return Err(Error::InvalidState);
        }
        search.validate_ultra2_binding(self.source, self.minimum, self.target, false)?;
        search.rebase_ultra2()?;
        self.ip = 0;
        self.anchor = 0;
        self.repeats = [1, 4, 8];
        self.wire_repeats = self.repeats;
        self.sequences = 0;
        self.last_stretch = PathCell::ZERO;
        self.matches.clear();
        self.ultra2 = Some(true);
        self.status = Status::Ready;
        Ok(())
    }
    pub(crate) fn parse<Q: PathProvider, O: PathObserver>(
        &mut self,
        provider: &mut Q,
        observer: &mut O,
    ) -> Result<PathResult> {
        if self.scope != Scope::Scripted {
            return Err(Error::UnsupportedFeature);
        }
        self.parse_with_provider(provider, observer)
    }
    fn parse_with_provider<Q: PathProvider, O: PathObserver>(
        &mut self,
        provider: &mut Q,
        observer: &mut O,
    ) -> Result<PathResult> {
        if self.status != Status::Ready || self.retained_end != 0 {
            return Err(Error::InvalidState);
        }
        self.status = Status::Poisoned;
        let result = self.run(provider, observer)?;
        self.status = Status::Finished;
        Ok(result)
    }
    fn require_ordinary_policy(&self) -> Result<()> {
        if self.frequencies.policy() != PricePolicy::Optimal {
            return Err(Error::UnsupportedFeature);
        }
        Ok(())
    }
    pub(crate) fn fresh_configuration(&self) -> Result<(&'a [u8], usize, usize, bool)> {
        self.require_ordinary_policy()?;
        if !(64..=16384).contains(&self.source.len()) {
            return Err(Error::InvalidParameter);
        }
        if self.status != Status::Ready || self.retained_end != 0 {
            return Err(Error::InvalidState);
        }
        Ok((
            self.source,
            self.minimum,
            self.target,
            self.frequencies.disabled_literals(),
        ))
    }
    /// Inspect an unused owner admitted through the ordinary Ultra constructor.
    pub(crate) fn ultra_fresh_configuration(&self) -> Result<(&'a [u8], usize, usize, bool)> {
        if self.ultra2.is_some()
            || self.frequencies.policy() != PricePolicy::Ultra
            || self.scope != Scope::UltraFresh
        {
            return Err(Error::UnsupportedFeature);
        }
        if !(64..=16384).contains(&self.source.len()) {
            return Err(Error::InvalidParameter);
        }
        if self.status != Status::Ready || self.retained_end != 0 {
            return Err(Error::InvalidState);
        }
        Ok((
            self.source,
            self.minimum,
            self.target,
            self.frequencies.disabled_literals(),
        ))
    }
    /// Inspect only an unused retained-Ultra owner; this grants no parse entry.
    pub(crate) fn ultra_retained_configuration(&self) -> Result<(&'a [u8], usize, usize, bool)> {
        if self.scope != Scope::UltraRetained || self.frequencies.policy() != PricePolicy::Ultra {
            return Err(Error::UnsupportedFeature);
        }
        if self.status != Status::Ready || self.retained_end != 0 {
            return Err(Error::InvalidState);
        }
        Ok((
            self.source,
            self.minimum,
            self.target,
            self.frequencies.disabled_literals(),
        ))
    }
    pub(crate) fn retained_configuration(&self) -> Result<(&'a [u8], usize, usize, bool)> {
        self.require_ordinary_policy()?;
        if !(128..=32768).contains(&self.source.len()) {
            return Err(Error::InvalidParameter);
        }
        if self.status != Status::Ready || self.retained_end != 0 {
            return Err(Error::InvalidState);
        }
        Ok((
            self.source,
            self.minimum,
            self.target,
            self.frequencies.disabled_literals(),
        ))
    }
    pub(crate) fn parse_retained<
        H: Allocator,
        N: Allocator,
        S: Allocator,
        B: Allocator,
        O: PathObserver,
    >(
        &mut self,
        start: usize,
        end: usize,
        repeats: [u32; 3],
        search: &mut OptimalSearch<'a, H, N, S, B>,
        observer: &mut O,
    ) -> Result<PathResult> {
        self.require_ordinary_policy()?;
        if !(128..=32768).contains(&self.source.len())
            || end > self.source.len()
            || !end
                .checked_sub(start)
                .is_some_and(|n| (64..=8192).contains(&n))
            || repeats
                .iter()
                .any(|&v| v == 0 || v as usize > self.source.len())
        {
            return Err(Error::InvalidParameter);
        }
        if start != self.retained_end
            || self.status
                != if start == 0 {
                    Status::Ready
                } else {
                    Status::Finished
                }
        {
            return Err(Error::InvalidState);
        }
        search.begin_retained(start, end)?;
        self.status = Status::Poisoned;
        self.block_start = start;
        self.block_end = end;
        self.anchor = start;
        self.repeats = repeats;
        self.wire_repeats = repeats;
        self.sequences = 0;
        self.last_stretch = PathCell::ZERO;
        self.matches.clear();
        let result = self.run(&mut RetainedProvider(search), observer)?;
        self.retained_end = end;
        self.status = Status::Finished;
        Ok(result)
    }
    fn emit<O: PathObserver>(&self, observer: &mut O, mut event: PathEvent) -> Result<()> {
        event.ip = self.ip;
        event.anchor = self.anchor;
        event.repeats = self.repeats;
        event.sequences = self.sequences;
        event.reverse_phase = self.paths.phase() == PathPhase::Reconstruction;
        let matches = if event.query.is_some() {
            self.matches.as_slice()?
        } else {
            &[]
        };
        observer.event(event, &self.paths, &self.frequencies, matches)
    }
    fn read(&self, index: usize) -> Result<PathCell> {
        self.paths.read(index, self.paths.phase())
    }
    fn write(&mut self, index: usize, cell: PathCell) -> Result<()> {
        self.paths.write(index, cell, self.paths.phase())
    }
    fn ll_price(&self, length: usize) -> Result<u32> {
        Ok(self.frequencies.prices(
            &[],
            u32::try_from(length).map_err(|_| Error::ResourceLimit)?,
            1,
            3,
        )?[1])
    }
    fn match_price(&self, off: u32, length: usize) -> Result<u32> {
        Ok(self.frequencies.prices(
            &[],
            0,
            off,
            u32::try_from(length).map_err(|_| Error::ResourceLimit)?,
        )?[2])
    }
    fn valid_match(
        &self,
        position: usize,
        candidate: OptimalMatch,
        repeats: [u32; 3],
        zero: bool,
    ) -> Result<[u32; 3]> {
        if candidate.length() < self.minimum {
            return Err(Error::CorruptData);
        }
        let (distance, next) = resolved(candidate.off_base(), repeats, zero)?;
        let distance = distance as usize;
        if distance > position {
            return Err(Error::CorruptData);
        }
        let end = position
            .checked_add(candidate.length())
            .ok_or(Error::CorruptData)?;
        if end > self.block_end {
            return Err(Error::CorruptData);
        }
        let current = self.source.get(position..end).ok_or(Error::CorruptData)?;
        let previous = self
            .source
            .get(position - distance..end - distance)
            .ok_or(Error::CorruptData)?;
        if current != previous {
            return Err(Error::CorruptData);
        }
        Ok(next)
    }
    fn find<Q: PathProvider, O: PathObserver>(
        &mut self,
        provider: &mut Q,
        observer: &mut O,
        cur: usize,
        last: usize,
        repeats: [u32; 3],
        zero: bool,
    ) -> Result<usize> {
        let position = self.ip.checked_add(cur).ok_or(Error::CorruptData)?;
        if position == 0 || position > self.block_end - 8 {
            return Err(Error::CorruptData);
        }
        let query = OptimalQuery {
            position,
            repeats,
            zero_literals: zero,
            length_to_beat: self.minimum,
        };
        self.matches.clear();
        provider.query(query, &mut self.matches)?;
        let candidates = self.matches.as_slice()?;
        for &candidate in candidates {
            self.valid_match(position, candidate, repeats, zero)?;
        }
        let count = candidates.len();
        let mut event = PathEvent::at(1, cur, last);
        event.query = Some(query);
        self.emit(observer, event)?;
        Ok(count)
    }
    fn seed<O: PathObserver>(&mut self, observer: &mut O, litlen: usize) -> Result<usize> {
        for pos in 1..self.minimum {
            let cell = self
                .read(pos)?
                .without_match()
                .with_literals((litlen + pos) as u32)?
                .with_price(PathPrice::UNREACHABLE)?;
            self.write(pos, cell)?;
        }
        let mut pos = self.minimum;
        let count = self.matches.as_slice()?.len();
        for index in 0..count {
            let candidate = self.matches.as_slice()?[index];
            while pos <= candidate.length() {
                let price = self.read(0)?.price().adjusted(
                    i64::from(self.match_price(candidate.off_base(), pos)?)
                        + i64::from(self.ll_price(0)?),
                )?;
                let cell = self
                    .read(pos)?
                    .with_match(candidate.off_base(), pos as u32)?
                    .with_literals(0)?
                    .with_price(price)?;
                self.write(pos, cell)?;
                pos += 1;
            }
        }
        self.write(pos, self.read(pos)?.with_price(PathPrice::UNREACHABLE)?)?;
        self.emit(observer, PathEvent::at(3, 0, pos - 1))?;
        Ok(pos - 1)
    }
    fn forward<Q: PathProvider, O: PathObserver>(
        &mut self,
        provider: &mut Q,
        observer: &mut O,
        mut last: usize,
    ) -> Result<(usize, usize, PathCell)> {
        let thorough = self.frequencies.policy() == PricePolicy::Ultra;
        let mut cur = 1;
        while cur <= last {
            let previous = self.read(cur - 1)?;
            let litlen = previous.literal_length() as usize + 1;
            let at = self.ip.checked_add(cur - 1).ok_or(Error::CorruptData)?;
            let literal = self.source.get(at..at + 1).ok_or(Error::CorruptData)?;
            let raw = self.frequencies.prices(literal, 0, 1, 3)?[0];
            let delta = i64::from(raw) + i64::from(self.ll_price(litlen)?)
                - i64::from(self.ll_price(litlen - 1)?);
            let price = previous.price().adjusted(delta)?;
            let replaced = self.read(cur)?;
            if price.word() <= replaced.price().word() {
                self.write(
                    cur,
                    previous.with_literals(litlen as u32)?.with_price(price)?,
                )?;
                if thorough && replaced.literal_length() == 0 {
                    let increment = i64::from(self.ll_price(1)?) - i64::from(self.ll_price(0)?);
                    let next_at = self.ip.checked_add(cur).ok_or(Error::CorruptData)?;
                    if increment < 0 && next_at < self.block_end {
                        let literal = self
                            .source
                            .get(next_at..next_at + 1)
                            .ok_or(Error::CorruptData)?;
                        let raw = i64::from(self.frequencies.prices(literal, 0, 1, 3)?[0]);
                        let with_one = replaced.price().adjusted(raw + increment)?;
                        let with_more = price.adjusted(
                            raw + i64::from(self.ll_price(litlen + 1)?)
                                - i64::from(self.ll_price(litlen)?),
                        )?;
                        let next_price = self.read(cur + 1)?.price();
                        let predecessor = cur
                            .checked_sub(replaced.match_length() as usize)
                            .ok_or(Error::CorruptData)?;
                        let mut alternative = [0; 12];
                        alternative[..7].copy_from_slice(&replaced.words().map(i64::from));
                        alternative[7..].copy_from_slice(&[
                            increment,
                            i64::from(with_one.word()),
                            i64::from(with_more.word()),
                            i64::from(next_price.word()),
                            predecessor as i64,
                        ]);
                        let mut event = PathEvent::at(12, cur, last);
                        event.alternative = Some(alternative);
                        self.emit(observer, event)?;
                        if with_one.word() < with_more.word() && with_one.word() < next_price.word()
                        {
                            let before = self.read(predecessor)?;
                            let (_, repeats) = resolved(
                                replaced.off_base(),
                                before.repeats(),
                                before.literal_length() == 0,
                            )?;
                            self.write(
                                cur + 1,
                                replaced
                                    .with_repeats(repeats)?
                                    .with_literals(1)?
                                    .with_price(with_one)?,
                            )?;
                            last = last.max(cur + 1);
                        }
                    }
                }
            }
            let mut cell = self.read(cur)?;
            if cur < cell.match_length() as usize {
                return Err(Error::CorruptData);
            }
            if cell.literal_length() == 0 {
                let before = self.read(cur - cell.match_length() as usize)?;
                let (_, repeats) = resolved(
                    cell.off_base(),
                    before.repeats(),
                    before.literal_length() == 0,
                )?;
                cell = cell.with_repeats(repeats)?;
                self.write(cur, cell)?;
            }
            self.emit(observer, PathEvent::at(4, cur, last))?;
            if self.ip + cur > self.block_end - 8 {
                cur += 1;
                continue;
            }
            if cur == last {
                break;
            }
            if !thorough
                && self.read(cur + 1)?.price().word()
                    <= cell
                        .price()
                        .value()?
                        .checked_add(128)
                        .ok_or(Error::ResourceLimit)?
            {
                cur += 1;
                continue;
            }
            let count = self.find(
                provider,
                observer,
                cur,
                last,
                cell.repeats(),
                cell.literal_length() == 0,
            )?;
            if count == 0 {
                cur += 1;
                continue;
            }
            let longest = self.matches.as_slice()?[count - 1];
            let end = cur
                .checked_add(longest.length())
                .ok_or(Error::ResourceLimit)?;
            if longest.length() > self.target
                || end >= PATH_HORIZON
                || self.ip + end >= self.block_end
            {
                let stretch = self
                    .last_stretch
                    .with_match(longest.off_base(), longest.length() as u32)?
                    .with_literals(0)?;
                return Ok((cur, end, stretch));
            }
            let base = cell.price().adjusted(i64::from(self.ll_price(0)?))?;
            let mut previous_length = self.minimum - 1;
            for index in 0..count {
                let candidate = self.matches.as_slice()?[index];
                for length in (previous_length + 1..=candidate.length()).rev() {
                    let pos = cur + length;
                    let price =
                        base.adjusted(i64::from(self.match_price(candidate.off_base(), length)?))?;
                    if pos > last || price.word() < self.read(pos)?.price().word() {
                        while last < pos {
                            last += 1;
                            self.write(
                                last,
                                self.read(last)?
                                    .with_price(PathPrice::UNREACHABLE)?
                                    .with_literals(1)?,
                            )?;
                        }
                        self.write(
                            pos,
                            self.read(pos)?
                                .with_match(candidate.off_base(), length as u32)?
                                .with_literals(0)?
                                .with_price(price)?,
                        )?;
                    } else if !thorough {
                        break;
                    }
                }
                previous_length = candidate.length();
            }
            self.write(
                last + 1,
                self.read(last + 1)?.with_price(PathPrice::UNREACHABLE)?,
            )?;
            self.emit(observer, PathEvent::at(5, cur, last))?;
            cur += 1;
        }
        let stretch = self.read(last)?;
        let cur = last
            .checked_sub(stretch.match_length() as usize)
            .ok_or(Error::CorruptData)?;
        Ok((cur, last, stretch))
    }
    fn reconstruct<O: PathObserver>(
        &mut self,
        observer: &mut O,
        mut cur: usize,
        last: usize,
        stretch: PathCell,
    ) -> Result<()> {
        self.emit(observer, PathEvent::selected(6, cur, last, stretch))?;
        if self.read(0)?.match_length() != 0
            || last.checked_sub(stretch.match_length() as usize) != Some(cur)
        {
            return Err(Error::CorruptData);
        }
        if stretch.match_length() == 0 {
            if stretch.literal_length() as usize != self.ip - self.anchor + last {
                return Err(Error::CorruptData);
            }
            self.ip = self
                .ip
                .checked_add(last)
                .filter(|&v| v <= self.block_end)
                .ok_or(Error::CorruptData)?;
            return Ok(());
        }
        if stretch.literal_length() == 0 {
            let before = self.read(cur)?;
            self.repeats = resolved(
                stretch.off_base(),
                before.repeats(),
                before.literal_length() == 0,
            )?
            .1;
        } else {
            self.repeats = stretch.repeats();
            cur = cur
                .checked_sub(stretch.literal_length() as usize)
                .ok_or(Error::CorruptData)?;
        }
        let mut cursor = ReverseCursor::new(PathIndex::new(cur)?)?;
        let end = cursor.end().get();
        self.paths.begin_reconstruction()?;
        if stretch.literal_length() > 0 {
            self.write(
                end,
                self.read(end)?
                    .with_literals(stretch.literal_length())?
                    .without_match(),
            )?;
            self.write(end - 1, stretch)?;
        }
        // Preserve the pinned unconditional block after the preceding if.
        self.write(end, stretch)?;
        let mut event = PathEvent::selected(7, cur, last, stretch);
        event.reverse = Some([end, end, cur]);
        self.emit(observer, event)?;
        let start = loop {
            let at = cursor.predecessor().get();
            let destination = cursor.destination().get();
            if destination <= at {
                return Err(Error::CorruptData);
            }
            let previous = self.read(at)?;
            self.write(
                destination,
                self.read(destination)?
                    .with_literals(previous.literal_length())?,
            )?;
            let mut event = PathEvent::selected(8, cur, last, stretch);
            event.reverse = Some([destination, end, at]);
            self.emit(observer, event)?;
            let Some(next) = cursor.advance(previous.forward()?)? else {
                break destination;
            };
            self.write(next.destination().get(), previous)?;
            cursor = next;
        };
        for index in start..=end {
            let sequence = self.paths.reverse(index)?;
            let cell = sequence.cell();
            let next = sequence.end(self.anchor, self.block_end)?;
            if cell.match_length() == 0 {
                if index != end {
                    return Err(Error::CorruptData);
                }
                self.ip = next;
                continue;
            }
            let position = self.anchor + cell.literal_length() as usize;
            let candidate = OptimalMatch::new(cell.off_base(), cell.match_length() as usize)?;
            let repeats = self.valid_match(
                position,
                candidate,
                self.wire_repeats,
                cell.literal_length() == 0,
            )?;
            let literals = self
                .source
                .get(self.anchor..position)
                .ok_or(Error::CorruptData)?;
            self.frequencies
                .update(literals, cell.off_base(), cell.match_length())?;
            let row = [
                self.anchor as u32,
                cell.literal_length(),
                cell.match_length(),
                cell.off_base(),
            ];
            self.wire_repeats = repeats;
            self.anchor = next;
            self.ip = next;
            self.sequences += 1;
            let mut event = PathEvent::selected(9, cur, last, stretch);
            event.reverse = Some([start, end, index]);
            event.appended = Some(row);
            self.emit(observer, event)?;
        }
        if self.repeats != self.wire_repeats {
            return Err(Error::CorruptData);
        }
        self.frequencies.refresh()?;
        self.emit(observer, PathEvent::selected(10, cur, last, stretch))?;
        self.paths.resume_forward()?;
        Ok(())
    }
    fn run<Q: PathProvider, O: PathObserver>(
        &mut self,
        provider: &mut Q,
        observer: &mut O,
    ) -> Result<PathResult> {
        self.frequencies.begin_block(
            self.source
                .get(self.block_start..self.block_end)
                .ok_or(Error::CorruptData)?,
        )?;
        self.ip = self.block_start.max(1);
        self.emit(observer, PathEvent::default())?;
        while self.ip < self.block_end.saturating_sub(8) {
            let litlen = self.ip.checked_sub(self.anchor).ok_or(Error::CorruptData)?;
            let count = self.find(provider, observer, 0, 0, self.repeats, litlen == 0)?;
            if count == 0 {
                self.ip += 1;
                continue;
            }
            let origin = self
                .read(0)?
                .without_match()
                .with_literals(litlen as u32)?
                .with_price(PathPrice::finite(self.ll_price(litlen)?)?)?
                .with_repeats(self.repeats)?;
            self.write(0, origin)?;
            self.emit(observer, PathEvent::at(2, 0, 0))?;
            let longest = self.matches.as_slice()?[count - 1];
            let (cur, last, stretch) = if longest.length() > self.target {
                (
                    0,
                    longest.length(),
                    self.last_stretch
                        .with_match(longest.off_base(), longest.length() as u32)?
                        .with_literals(0)?,
                )
            } else {
                let last = self.seed(observer, litlen)?;
                self.forward(provider, observer, last)?
            };
            self.last_stretch = stretch;
            self.reconstruct(observer, cur, last, stretch)?;
        }
        let tail = self.block_end - self.anchor;
        let event = PathEvent {
            stage: 11,
            tail: Some(tail),
            ..PathEvent::default()
        };
        self.emit(observer, event)?;
        Ok(PathResult {
            tail,
            repeats: self.wire_repeats,
            sequences: self.sequences,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimal_path_fixture as fixture;
    use crate::optimal_path_layout::PATH_CAPACITY;
    use alloc::alloc::Global;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    struct Script<'a>(&'a [fixture::Script]);
    impl PathProvider for Script<'_> {
        fn query<A: Allocator>(
            &mut self,
            query: OptimalQuery,
            matches: &mut OptimalMatches<A>,
        ) -> Result<()> {
            if let Some(script) = self
                .0
                .iter()
                .find(|s| s.position as usize == query.position)
            {
                for &[off, length] in &script.matches {
                    matches.push(OptimalMatch::new(off, length as usize)?)?;
                }
            }
            Ok(())
        }
    }
    struct Compare<'a> {
        case: &'a fixture::Case<'a>,
        index: usize,
        cells: Vec<PathCell>,
        actual: Vec<PathCell>,
        frequencies: [u32; 386],
    }
    impl PathObserver for Compare<'_> {
        fn event<P: Allocator, F: Allocator>(
            &mut self,
            event: PathEvent,
            paths: &OptimalPaths<P>,
            frequencies: &OptimalFrequencies<F>,
            matches: &[OptimalMatch],
        ) -> Result<()> {
            let expected = &self.case.events[self.index];
            let context = (self.case.name, self.index, event.stage);
            assert_eq!(
                event.stage,
                u8::try_from(expected.stage).unwrap(),
                "{context:?}"
            );
            assert_eq!(event.reverse_phase, expected.reverse_phase, "{context:?}");
            assert_eq!(
                [event.ip, event.anchor],
                [expected.ip as usize, expected.anchor as usize],
                "{context:?}"
            );
            assert_eq!(
                [event.cur, event.last, event.tail],
                [
                    expected.cur.map(|v| v as usize),
                    expected.last.map(|v| v as usize),
                    expected.tail.map(|v| v as usize)
                ],
                "{context:?}"
            );
            assert_eq!(event.repeats, expected.repeats, "{context:?}");
            assert_eq!(
                event.stretch.map(PathCell::words),
                expected.stretch,
                "{context:?}"
            );
            assert_eq!(
                event.reverse,
                expected.reverse.map(|r| r.map(|v| v as usize)),
                "{context:?}"
            );
            assert_eq!(
                event.query.map(|q| [
                    q.position as u32,
                    q.repeats[0],
                    q.repeats[1],
                    q.repeats[2],
                    u32::from(q.zero_literals),
                    q.length_to_beat as u32
                ]),
                expected.query,
                "{context:?}"
            );
            assert_eq!(
                matches
                    .iter()
                    .map(|m| [m.off_base(), m.length() as u32])
                    .collect::<Vec<_>>(),
                expected.matches,
                "{context:?}"
            );
            assert_eq!(event.sequences, expected.sequence_count, "{context:?}");
            assert_eq!(event.appended.as_slice(), expected.appended, "{context:?}");
            for &(at, words) in &expected.path {
                self.cells[at] = PathCell::from_words(words).unwrap();
            }
            paths.snapshot(&mut self.actual)?;
            assert_eq!(self.actual, self.cells, "{context:?}");
            for &(at, value) in &expected.frequencies {
                self.frequencies[at] = value;
            }
            let snapshot = frequencies.snapshot()?;
            assert_eq!(snapshot.counts, self.frequencies[..377], "{context:?}");
            assert_eq!(snapshot.sums, self.frequencies[377..381], "{context:?}");
            assert_eq!(
                snapshot.base_prices,
                self.frequencies[381..385],
                "{context:?}"
            );
            assert_eq!(
                u32::from(snapshot.predefined),
                self.frequencies[385],
                "{context:?}"
            );
            assert!(snapshot.initialized);
            self.index += 1;
            Ok(())
        }
    }
    #[test]
    fn every_captured_scripted_decision_matches_complete_path_frequency_and_output_state() {
        let cases = fixture::read(fixture::PACK).unwrap();
        let mut events = 0;
        let mut sequences = 0;
        for case in &cases {
            let freq = OptimalFrequencies::new_in(case.literal_mode == 1, Global).unwrap();
            let paths = OptimalPaths::new_in(Global).unwrap();
            let matches = OptimalMatches::new_in(Global).unwrap();
            let mut parser = OptimalParser::from_parts(
                case.source,
                case.minimum as u8,
                case.target as usize,
                freq,
                paths,
                matches,
            )
            .unwrap();
            let mut compare = Compare {
                case,
                index: 0,
                cells: vec![PathCell::ZERO; PATH_CAPACITY],
                actual: vec![PathCell::ZERO; PATH_CAPACITY],
                frequencies: [0; 386],
            };
            let result = parser
                .parse(&mut Script(&case.scripts), &mut compare)
                .unwrap();
            assert_eq!(compare.index, case.events.len(), "{}", case.name);
            events += compare.index;
            sequences += result.sequences;
            let last = case.events.last().unwrap();
            assert_eq!(result.tail, last.tail.unwrap() as usize);
            assert_eq!(result.repeats, last.repeats);
            assert_eq!(parser.status, Status::Finished);
        }
        assert_eq!(events, 22_024);
        assert_eq!(sequences, 42);
    }
    struct Empty;
    impl PathProvider for Empty {
        fn query<A: Allocator>(
            &mut self,
            _: OptimalQuery,
            _: &mut OptimalMatches<A>,
        ) -> Result<()> {
            Ok(())
        }
    }
    #[derive(Default)]
    struct Observe {
        events: usize,
        sequences: usize,
        fail_stage: Option<u8>,
    }
    impl PathObserver for Observe {
        fn event<P: Allocator, F: Allocator>(
            &mut self,
            event: PathEvent,
            _: &OptimalPaths<P>,
            _: &OptimalFrequencies<F>,
            _: &[OptimalMatch],
        ) -> Result<()> {
            self.events += 1;
            self.sequences += usize::from(event.appended.is_some());
            if self.fail_stage == Some(event.stage) {
                Err(Error::ResourceLimit)
            } else {
                Ok(())
            }
        }
    }
    fn parser(source: &[u8]) -> OptimalParser<'_, Global, Global, Global> {
        OptimalParser::from_parts(
            source,
            4,
            0,
            OptimalFrequencies::new_in(true, Global).unwrap(),
            OptimalPaths::new_in(Global).unwrap(),
            OptimalMatches::new_in(Global).unwrap(),
        )
        .unwrap()
    }
    struct Supplied {
        off: u32,
        length: usize,
        fail: bool,
    }
    impl PathProvider for Supplied {
        fn query<A: Allocator>(
            &mut self,
            query: OptimalQuery,
            matches: &mut OptimalMatches<A>,
        ) -> Result<()> {
            if self.fail {
                return Err(Error::ResourceLimit);
            }
            if query.position == 1 {
                matches.push(OptimalMatch::new(self.off, self.length)?)?;
            }
            Ok(())
        }
    }
    #[test]
    fn malformed_candidates_and_provider_failures_poison_before_observation() {
        for (source, off, length, fail) in [
            (&b"aaaaaaaaaaaaaaaa"[..], 5, 4, false),
            (&b"abcdefghijklmnop"[..], 4, 4, false),
            (&b"aaaaaaaaaaaaaaaa"[..], 4, 3, false),
            (&b"aaaaaaaaaaaaaaaa"[..], 4, 16, false),
            (&b"aaaaaaaaaaaaaaaa"[..], 4, 4, true),
        ] {
            let mut parser = parser(source);
            let mut observe = Observe::default();
            assert!(
                parser
                    .parse(&mut Supplied { off, length, fail }, &mut observe)
                    .is_err()
            );
            assert_eq!(observe.events, 1);
            assert_eq!(parser.status, Status::Poisoned);
            assert_eq!(
                parser.parse(&mut Empty, &mut observe),
                Err(Error::InvalidState)
            );
            assert_eq!(observe.events, 1);
            parser.reset().unwrap();
            let result = parser.parse(&mut Empty, &mut observe).unwrap();
            assert_eq!(result.tail, source.len());
            assert_eq!(result.repeats, [1, 4, 8]);
        }
    }
    #[test]
    fn observer_failure_after_output_requires_reset_and_does_not_undo_external_output() {
        for stage in [0, 1, 6, 8, 9, 10, 11] {
            let mut parser = parser(b"aaaaaaaaaaaaaaaa");
            let mut observer = Observe {
                fail_stage: Some(stage),
                ..Observe::default()
            };
            assert_eq!(
                parser.parse(
                    &mut Supplied {
                        off: 4,
                        length: 15,
                        fail: false
                    },
                    &mut observer
                ),
                Err(Error::ResourceLimit)
            );
            assert_eq!(parser.status, Status::Poisoned);
            let emitted = observer.sequences;
            assert_eq!(emitted, usize::from(stage >= 9));
            let events = observer.events;
            assert_eq!(
                parser.parse(&mut Empty, &mut observer),
                Err(Error::InvalidState)
            );
            assert_eq!(observer.events, events);
            parser.reset().unwrap();
            observer.fail_stage = None;
            assert_eq!(parser.parse(&mut Empty, &mut observer).unwrap().tail, 16);
            assert_eq!(observer.sequences, emitted);
            assert_eq!(
                parser.parse(&mut Empty, &mut observer),
                Err(Error::InvalidState)
            );
        }
    }
    #[test]
    fn borrowed_allocators_do_not_allocate_during_parse_or_reset() {
        let allocator = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = allocator.monitor();
        let frequencies = OptimalFrequencies::new_in(true, &allocator).unwrap();
        let paths = OptimalPaths::new_in(&allocator).unwrap();
        let matches = OptimalMatches::new_in(&allocator).unwrap();
        let mut parser =
            OptimalParser::from_parts(b"aaaaaaaaaaaaaaaa", 4, 0, frequencies, paths, matches)
                .unwrap();
        assert!(monitor.fail_next());
        let before = monitor.snapshot();
        for _ in 0..3 {
            let result = parser
                .parse(
                    &mut Supplied {
                        off: 4,
                        length: 15,
                        fail: false,
                    },
                    &mut Observe::default(),
                )
                .unwrap();
            assert_eq!(result.sequences, 1);
            parser.reset().unwrap();
        }
        assert_eq!(monitor.snapshot().attempts, before.attempts);
        drop(parser);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        assert!(matches!(
            OptimalPaths::new_in(&allocator),
            Err(Error::AllocationFailed)
        ));
        drop(allocator);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }
    #[test]
    fn rejected_construction_drops_all_exact_moved_owners() {
        for (source, minimum, target) in [
            (&b"12345678"[..], 4, 0),
            (&b"123456789"[..], 2, 0),
            (&b"123456789"[..], 4, 4096),
        ] {
            let a = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let ma = a.monitor();
            let b = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let mb = b.monitor();
            let c = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let mc = c.monitor();
            let result = OptimalParser::from_parts(
                source,
                minimum,
                target,
                OptimalFrequencies::new_in(false, a).unwrap(),
                OptimalPaths::new_in(b).unwrap(),
                OptimalMatches::new_in(c).unwrap(),
            );
            assert!(matches!(result, Err(Error::InvalidParameter)));
            for monitor in [ma, mb, mc] {
                monitor.assert_released();
                assert_eq!(monitor.snapshot().owners_dropped, 1);
                assert_eq!(monitor.snapshot().attempts, 1);
            }
        }
    }
    #[test]
    fn repeat_resolution_distinguishes_zero_literals_and_checked_subtraction() {
        for (off, zero, distance, next) in [
            (1, false, 10, [10, 20, 30]),
            (2, false, 20, [20, 10, 30]),
            (3, false, 30, [30, 10, 20]),
            (1, true, 20, [20, 10, 30]),
            (2, true, 30, [30, 10, 20]),
            (3, true, 9, [9, 10, 20]),
            (7, false, 4, [4, 10, 20]),
        ] {
            assert_eq!(resolved(off, [10, 20, 30], zero), Ok((distance, next)));
        }
        assert!(resolved(3, [1, 4, 8], true).is_err());
        assert!(resolved(0, [1, 4, 8], false).is_err());
        assert!(resolved(4, [0, 4, 8], false).is_err());
    }
    #[test]
    fn ultra_policy_rejection_releases_all_exact_moved_owners() {
        let a = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let ma = a.monitor();
        let b = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let mb = b.monitor();
        let c = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let mc = c.monitor();
        let mut frequencies = OptimalFrequencies::new_with_policy_in(
            crate::optimal_price::PricePolicy::Ultra,
            false,
            a,
        )
        .unwrap();
        frequencies.begin_block(b"aaaaaaaaa").unwrap();
        frequencies.update(b"aa", 1, 3).unwrap();
        let result = OptimalParser::from_parts(
            b"aaaaaaaaaaaaaaaa",
            4,
            0,
            frequencies,
            OptimalPaths::new_in(b).unwrap(),
            OptimalMatches::new_in(c).unwrap(),
        );
        assert!(matches!(result, Err(Error::InvalidParameter)));
        for monitor in [ma, mb, mc] {
            monitor.assert_released();
            let state = monitor.snapshot();
            assert_eq!(state.attempts, 1);
            assert_eq!(state.owners_dropped, 1);
            assert!(!state.violations.any());
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/optimal_fresh_parse.rs"]
mod fresh_tests;

#[cfg(test)]
#[path = "../tests/support/optimal_fresh_wide_parse.rs"]
mod fresh_wide_tests;

#[cfg(test)]
#[path = "../tests/support/ultra_path_parse.rs"]
mod ultra_tests;

#[cfg(test)]
#[path = "../tests/support/ultra_fresh_parse.rs"]
mod ultra_fresh_tests;

#[cfg(test)]
#[path = "../tests/support/ultra_retained_scope.rs"]
mod ultra_retained_scope_tests;
