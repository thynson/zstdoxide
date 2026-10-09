//! Allocation-free inspection records for pinned Zstandard reference parameters.
//!
//! These values describe facebook/zstd v1.5.7 parameter contracts. They do not
//! select an encoder algorithm: the independent [`crate::CompressionStrategy`]
//! policies and [`crate::EncoderOptions`] defaults remain unchanged. Plain
//! scalar records share their safe representation under every backend feature.

use crate::{Error, Result};

mod presets;

// Bounds correspond to lib/zstd.h:1263-1279 and zstd_compress.c:7673-7674
// in facebook/zstd v1.5.7. The pinned C API distinguishes 32-bit size_t from
// other supported (64-bit) targets; these are parameter bounds, not limits on
// the independently supported Rust frame-header representation.
const WINDOW_LOG_MIN: u8 = 10;
const WINDOW_LOG_MAX: u8 = if usize::BITS == 32 { 30 } else { 31 };
const CHAIN_LOG_MIN: u8 = 6;
const CHAIN_LOG_MAX: u8 = if usize::BITS == 32 { 29 } else { 30 };
const HASH_LOG_MIN: u8 = 6;
const HASH_LOG_MAX: u8 = 30;
const SEARCH_LOG_MIN: u8 = 1;
const SEARCH_LOG_MAX: u8 = WINDOW_LOG_MAX - 1;
const MIN_MATCH_MIN: u8 = 3;
const MIN_MATCH_MAX: u8 = 7;
const TARGET_LENGTH_MAX: u32 = crate::frame::MAX_BLOCK_SIZE as u32;
const LEVEL_MIN: i32 = -(TARGET_LENGTH_MAX as i32);
const LEVEL_MAX: i32 = 22;
const LEVEL_DEFAULT: i32 = 3;

/// Source-size knowledge for reference parameter inspection.
///
/// A known empty source is distinct from an unknown source. Adapters for C's
/// public `ZSTD_getCParams`/`ZSTD_adjustCParams` map their zero size argument to
/// `Unknown`; internal typed inspection instead preserves `Known(0)`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SourceSize {
    /// No source size is available.
    #[default]
    Unknown,
    /// An exact source size, including zero for empty input.
    ///
    /// The reference reserves `u64::MAX` as its unknown sentinel. Inspection
    /// and adjustment reject `Known(u64::MAX)` instead of reinterpreting it.
    Known(u64),
}

/// The nine strategies named by the pinned reference's `ZSTD_strategy`.
///
/// This inspection enum is separate from [`crate::CompressionStrategy`]. A
/// variant does not imply that the Rust encoder implements that algorithm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceStrategy {
    /// Reference `ZSTD_fast` (1).
    Fast,
    /// Reference `ZSTD_dfast` (2).
    DoubleFast,
    /// Reference `ZSTD_greedy` (3).
    Greedy,
    /// Reference `ZSTD_lazy` (4).
    Lazy,
    /// Reference `ZSTD_lazy2` (5).
    Lazy2,
    /// Reference `ZSTD_btlazy2` (6).
    BinaryTreeLazy2,
    /// Reference `ZSTD_btopt` (7).
    Optimal,
    /// Reference `ZSTD_btultra` (8).
    Ultra,
    /// Reference `ZSTD_btultra2` (9).
    Ultra2,
}

impl ReferenceStrategy {
    const fn code(self) -> u32 {
        match self {
            Self::Fast => 1,
            Self::DoubleFast => 2,
            Self::Greedy => 3,
            Self::Lazy => 4,
            Self::Lazy2 => 5,
            Self::BinaryTreeLazy2 => 6,
            Self::Optimal => 7,
            Self::Ultra => 8,
            Self::Ultra2 => 9,
        }
    }
}

impl TryFrom<u32> for ReferenceStrategy {
    type Error = Error;

    /// Accept reference codes 1 through 9; zero is not a concrete strategy.
    fn try_from(value: u32) -> Result<Self> {
        match value {
            1 => Ok(Self::Fast),
            2 => Ok(Self::DoubleFast),
            3 => Ok(Self::Greedy),
            4 => Ok(Self::Lazy),
            5 => Ok(Self::Lazy2),
            6 => Ok(Self::BinaryTreeLazy2),
            7 => Ok(Self::Optimal),
            8 => Ok(Self::Ultra),
            9 => Ok(Self::Ultra2),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Unvalidated seven-field input at the reference parameter boundary.
///
/// Corresponds to the scalar fields of `ZSTD_compressionParameters`. Public
/// fields intentionally permit invalid values; use [`Self::validate`] to obtain
/// a validated record. Zero is not an automatic value here, except that a
/// concrete target length of zero is valid. `Default` initializes every field
/// to zero and therefore does not produce validated parameters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RawCompressionParameters {
    /// Base-two window size logarithm.
    pub window_log: u32,
    /// Base-two chain table size logarithm.
    pub chain_log: u32,
    /// Base-two hash table size logarithm.
    pub hash_log: u32,
    /// Base-two search-attempt logarithm.
    pub search_log: u32,
    /// Reference search setting, not a universal emitted-match length floor.
    pub min_match: u32,
    /// Strategy-dependent target length or fast sampling setting.
    pub target_length: u32,
    /// Concrete reference strategy code, in `1..=9`.
    pub strategy: u32,
}

impl RawCompressionParameters {
    /// Clamp raw fields and adjust them for the given source and dictionary.
    ///
    /// Corresponds to `ZSTD_adjustCParams` in its ordinary dictionary mode,
    /// with `Known(0)` preserving the internal reference's empty-source meaning.
    /// A caller adapting the public C API maps its zero size to `Unknown`.
    ///
    /// Raw fields deliberately follow the pinned 32-bit signed clamp: values
    /// whose high bit is set, including `u32::MAX`, clamp to the field's lower
    /// bound. This differs from saturating an unsigned value to its upper bound.
    /// Unlike [`Self::validate`], invalid raw fields are therefore accepted.
    ///
    /// `Known(u64::MAX)` returns [`Error::InvalidParameter`]. Overflow in a
    /// known source plus dictionary or dictionary plus intermediate window
    /// returns [`Error::ResourceLimit`] instead of reproducing C wrapping.
    /// Unknown-source adjustment does not perform the preset selector's
    /// dictionary-plus-499 arithmetic. No allocation or encoder mutation occurs.
    pub fn adjust(
        self,
        source_size: SourceSize,
        dictionary_size: usize,
    ) -> Result<ResolvedCompressionParameters> {
        let clamped = Self {
            window_log: signed_clamp(
                self.window_log,
                WINDOW_LOG_MIN.into(),
                WINDOW_LOG_MAX.into(),
            ),
            chain_log: signed_clamp(self.chain_log, CHAIN_LOG_MIN.into(), CHAIN_LOG_MAX.into()),
            hash_log: signed_clamp(self.hash_log, HASH_LOG_MIN.into(), HASH_LOG_MAX.into()),
            search_log: signed_clamp(
                self.search_log,
                SEARCH_LOG_MIN.into(),
                SEARCH_LOG_MAX.into(),
            ),
            min_match: signed_clamp(self.min_match, MIN_MATCH_MIN.into(), MIN_MATCH_MAX.into()),
            target_length: signed_clamp(self.target_length, 0, TARGET_LENGTH_MAX),
            strategy: signed_clamp(self.strategy, 1, 9),
        };
        adjust_parameters(clamped, source_size, dictionary_size)
    }

    /// Validate all concrete fields without clamping or resolving a level.
    ///
    /// Corresponds to `ZSTD_checkCParams`. Bounds follow the target pointer
    /// width. Successful validation describes reference values only; it does
    /// not promise memory availability or an implemented Rust search algorithm.
    pub fn validate(self) -> Result<ResolvedCompressionParameters> {
        let window_log = checked_log(self.window_log, WINDOW_LOG_MIN, WINDOW_LOG_MAX)?;
        let chain_log = checked_log(self.chain_log, CHAIN_LOG_MIN, CHAIN_LOG_MAX)?;
        let hash_log = checked_log(self.hash_log, HASH_LOG_MIN, HASH_LOG_MAX)?;
        let search_log = checked_log(self.search_log, SEARCH_LOG_MIN, SEARCH_LOG_MAX)?;
        let min_match = checked_log(self.min_match, MIN_MATCH_MIN, MIN_MATCH_MAX)?;
        if self.target_length > TARGET_LENGTH_MAX {
            return Err(Error::InvalidParameter);
        }
        let strategy = ReferenceStrategy::try_from(self.strategy)?;
        Ok(ResolvedCompressionParameters {
            window_log,
            chain_log,
            hash_log,
            search_log,
            min_match,
            target_length: self.target_length,
            strategy,
        })
    }
}

/// Inspect the pinned reference's parameters for a level, source and dictionary.
///
/// Corresponds to `ZSTD_getCParams_internal` in no-attached-dictionary mode.
/// Zero selects reference level three; other levels clamp to the reference
/// bounds. `Known(0)` represents empty input, while `Unknown` leaves its size
/// unspecified. Adapters for the public C query map its zero argument to
/// `Unknown` explicitly. Reference frame defaults are available separately via
/// [`ReferenceFrameParameters`].
///
/// This allocation-free query does not select a Rust encoder algorithm or
/// promise its memory requirements. `Known(u64::MAX)` is invalid. Checked size
/// arithmetic returns [`Error::ResourceLimit`] on overflow, including the
/// unknown-source dictionary-plus-499 tier expression. The separate raw
/// adjustment method checks only its own expressions.
pub fn inspect_compression_level(
    level: i32,
    source_size: SourceSize,
    dictionary_size: usize,
) -> Result<ResolvedCompressionParameters> {
    adjust_parameters(
        presets::select(level, source_size, dictionary_size)?,
        source_size,
        dictionary_size,
    )
}

// ZSTD_clampCParams interprets each u32 field as a 32-bit signed int first.
// Reinterpreting the bit pattern states this compatibility policy explicitly;
// every caller passes nonnegative bounds at most TARGET_LENGTH_MAX (or 31).
fn signed_clamp(value: u32, minimum: u32, maximum: u32) -> u32 {
    i32::from_ne_bytes(value.to_ne_bytes()).clamp(minimum as i32, maximum as i32) as u32
}

// The input and result satisfy public scalar bounds, but intermediate window
// logs may be below ten. Preserve that smaller value until hash and chain
// downsizing completes, matching ZSTD_adjustCParams_internal's order.
fn adjust_parameters(
    parameters: RawCompressionParameters,
    source_size: SourceSize,
    dictionary_size: usize,
) -> Result<ResolvedCompressionParameters> {
    let mut working = parameters.validate()?.to_raw();
    let dictionary = u64::try_from(dictionary_size).map_err(|_| Error::ResourceLimit)?;
    if let SourceSize::Known(source) = source_size {
        if source == u64::MAX {
            return Err(Error::InvalidParameter);
        }
        let total = source.checked_add(dictionary).ok_or(Error::ResourceLimit)?;
        let resize_limit = 1_u64 << (WINDOW_LOG_MAX - 1);
        if source <= resize_limit && dictionary <= resize_limit {
            let source_log = if total < (1_u64 << HASH_LOG_MIN) {
                u32::from(HASH_LOG_MIN)
            } else {
                // total is nonzero in this branch. The subtraction is exact.
                u64::BITS - (total - 1).leading_zeros()
            };
            working.window_log = working.window_log.min(source_log);
        }
        let dictionary_window_log = if dictionary == 0 {
            working.window_log
        } else {
            // Validated window logs only decrease, so the shift is at most 31.
            let window = 1_u64 << working.window_log;
            let combined = dictionary.checked_add(window).ok_or(Error::ResourceLimit)?;
            if window >= total {
                working.window_log
            } else if combined >= (1_u64 << WINDOW_LOG_MAX) {
                u32::from(WINDOW_LOG_MAX)
            } else {
                // The nonempty dictionary and positive window make combined > 1.
                u64::BITS - (combined - 1).leading_zeros()
            }
        };
        working.hash_log = working.hash_log.min(dictionary_window_log + 1);
        let tree_scale = u32::from(working.strategy >= ReferenceStrategy::BinaryTreeLazy2.code());
        working.chain_log = working.chain_log.min(dictionary_window_log + tree_scale);
    }
    working.window_log = working.window_log.max(u32::from(WINDOW_LOG_MIN));
    // The reference's automatic row finder applies only to Greedy/Lazy/Lazy2.
    // Each row stores eight-bit tags; its row log is search_log clamped to 4..6.
    if (ReferenceStrategy::Greedy.code()..=ReferenceStrategy::Lazy2.code())
        .contains(&working.strategy)
    {
        working.hash_log = working.hash_log.min(24 + working.search_log.clamp(4, 6));
    }
    working.validate()
}

fn normalized_level(level: i32) -> i32 {
    if level == 0 {
        LEVEL_DEFAULT
    } else {
        level.clamp(LEVEL_MIN, LEVEL_MAX)
    }
}

fn checked_log(value: u32, minimum: u8, maximum: u8) -> Result<u8> {
    if (u32::from(minimum)..=u32::from(maximum)).contains(&value) {
        // The inclusive upper bound is a u8, so this conversion is exact.
        Ok(value as u8)
    } else {
        Err(Error::InvalidParameter)
    }
}

/// Concrete reference parameters whose individual fields satisfy target bounds.
///
/// Fields remain private so invalid raw values cannot bypass validation. This
/// record owns no memory and does not enable numeric compression in an encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedCompressionParameters {
    window_log: u8,
    chain_log: u8,
    hash_log: u8,
    search_log: u8,
    min_match: u8,
    target_length: u32,
    strategy: ReferenceStrategy,
}

impl ResolvedCompressionParameters {
    /// Base-two window size logarithm.
    pub const fn window_log(&self) -> u8 {
        self.window_log
    }

    /// Base-two chain table size logarithm.
    pub const fn chain_log(&self) -> u8 {
        self.chain_log
    }

    /// Base-two hash table size logarithm.
    pub const fn hash_log(&self) -> u8 {
        self.hash_log
    }

    /// Base-two search-attempt logarithm.
    pub const fn search_log(&self) -> u8 {
        self.search_log
    }

    /// Reference minimum-match search setting.
    pub const fn min_match(&self) -> u8 {
        self.min_match
    }

    /// Strategy-dependent target length or fast sampling setting.
    pub const fn target_length(&self) -> u32 {
        self.target_length
    }

    /// Reference strategy, without a Rust encoder support claim.
    pub const fn strategy(&self) -> ReferenceStrategy {
        self.strategy
    }

    /// Return the exact seven-field raw representation without allocation.
    pub const fn to_raw(&self) -> RawCompressionParameters {
        RawCompressionParameters {
            window_log: self.window_log as u32,
            chain_log: self.chain_log as u32,
            hash_log: self.hash_log as u32,
            search_log: self.search_log as u32,
            min_match: self.min_match as u32,
            target_length: self.target_length,
            strategy: self.strategy.code(),
        }
    }
}

/// Requested reference level and explicit scalar overrides for inspection.
///
/// Corresponds to the supported scalar request portion of `ZSTD_CCtx_params`.
/// Getters return requested values, not effective per-frame settings. This
/// allocation-free record is independent of a live encoder. Every failed
/// setter preserves the complete record, including previously selected values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompressionParameterRequest {
    level: i32,
    window_log: Option<u8>,
    chain_log: Option<u8>,
    hash_log: Option<u8>,
    search_log: Option<u8>,
    min_match: Option<u8>,
    target_length: Option<u32>,
    strategy: Option<ReferenceStrategy>,
    source_hint: Option<u32>,
}

impl CompressionParameterRequest {
    /// Lowest level accepted by the pinned reference, including fast acceleration.
    pub const MIN_LEVEL: i32 = LEVEL_MIN;
    /// Highest level defined by the pinned reference's preset table.
    pub const MAX_LEVEL: i32 = LEVEL_MAX;
    /// Pinned reference default level, also selected by a request for zero.
    pub const DEFAULT_LEVEL: i32 = LEVEL_DEFAULT;

    /// Resolve the requested level and overrides without changing this record.
    ///
    /// Corresponds to `ZSTD_getCParamsFromCCtxParams` in no-attached-dictionary
    /// mode with automatic row matching and no long-distance matching. A source
    /// hint supplies size knowledge only for `Unknown`; known sizes, including
    /// zero, take precedence. The preset is adjusted first, explicit overrides
    /// are applied second, and the result is adjusted again. This preserves
    /// source-dependent reductions made before a strategy or window override.
    ///
    /// Errors and checked size policy match [`inspect_compression_level`].
    /// No numeric compression algorithm or live context setting is enabled.
    pub fn resolve(
        &self,
        source_size: SourceSize,
        dictionary_size: usize,
    ) -> Result<ResolvedCompressionParameters> {
        let source_size = match (source_size, self.source_hint) {
            (SourceSize::Unknown, Some(hint)) => SourceSize::Known(u64::from(hint)),
            (source_size, _) => source_size,
        };
        let mut parameters =
            inspect_compression_level(self.level, source_size, dictionary_size)?.to_raw();
        if let Some(value) = self.window_log {
            parameters.window_log = value.into();
        }
        if let Some(value) = self.chain_log {
            parameters.chain_log = value.into();
        }
        if let Some(value) = self.hash_log {
            parameters.hash_log = value.into();
        }
        if let Some(value) = self.search_log {
            parameters.search_log = value.into();
        }
        if let Some(value) = self.min_match {
            parameters.min_match = value.into();
        }
        if let Some(value) = self.target_length {
            parameters.target_length = value;
        }
        if let Some(value) = self.strategy {
            parameters.strategy = value.code();
        }
        adjust_parameters(parameters, source_size, dictionary_size)
    }

    /// Request reference level three, with automatic fields and no source hint.
    pub const fn new() -> Self {
        Self {
            level: LEVEL_DEFAULT,
            window_log: None,
            chain_log: None,
            hash_log: None,
            search_log: None,
            min_match: None,
            target_length: None,
            strategy: None,
            source_hint: None,
        }
    }

    /// Construct a request using the same normalization as [`Self::set_level`].
    pub fn with_level(level: i32) -> Self {
        let mut request = Self::new();
        request.set_level(level);
        request
    }

    /// Normalize zero to three and clamp to the reference level range.
    ///
    /// Corresponds to the compression-level branch of
    /// `ZSTD_CCtxParams_setParameter`. Explicit overrides and source hints are
    /// preserved. No negation is performed, so `i32::MIN` is accepted safely.
    pub fn set_level(&mut self, level: i32) {
        self.level = normalized_level(level);
    }

    /// Return the normalized requested reference level.
    pub const fn level(&self) -> i32 {
        self.level
    }

    /// Set a checked window log; zero restores automatic selection.
    pub fn set_window_log(&mut self, value: u8) -> Result<()> {
        self.window_log = optional_log(value, WINDOW_LOG_MIN, WINDOW_LOG_MAX)?;
        Ok(())
    }

    /// Set a checked chain log; zero restores automatic selection.
    pub fn set_chain_log(&mut self, value: u8) -> Result<()> {
        self.chain_log = optional_log(value, CHAIN_LOG_MIN, CHAIN_LOG_MAX)?;
        Ok(())
    }

    /// Set a checked hash log; zero restores automatic selection.
    pub fn set_hash_log(&mut self, value: u8) -> Result<()> {
        self.hash_log = optional_log(value, HASH_LOG_MIN, HASH_LOG_MAX)?;
        Ok(())
    }

    /// Set a checked search log; zero restores automatic selection.
    pub fn set_search_log(&mut self, value: u8) -> Result<()> {
        self.search_log = optional_log(value, SEARCH_LOG_MIN, SEARCH_LOG_MAX)?;
        Ok(())
    }

    /// Set a checked minimum-match search value; zero restores automatic selection.
    pub fn set_min_match(&mut self, value: u8) -> Result<()> {
        self.min_match = optional_log(value, MIN_MATCH_MIN, MIN_MATCH_MAX)?;
        Ok(())
    }

    /// Set a checked target length; zero restores automatic selection.
    pub fn set_target_length(&mut self, value: u32) -> Result<()> {
        if value > TARGET_LENGTH_MAX {
            return Err(Error::InvalidParameter);
        }
        self.target_length = if value == 0 { None } else { Some(value) };
        Ok(())
    }

    /// Select a reference strategy override, or `None` for automatic selection.
    pub fn set_strategy(&mut self, value: Option<ReferenceStrategy>) {
        self.strategy = value;
    }

    /// Requested window log, or `None` for automatic selection.
    pub const fn window_log(&self) -> Option<u8> {
        self.window_log
    }

    /// Requested chain log, or `None` for automatic selection.
    pub const fn chain_log(&self) -> Option<u8> {
        self.chain_log
    }

    /// Requested hash log, or `None` for automatic selection.
    pub const fn hash_log(&self) -> Option<u8> {
        self.hash_log
    }

    /// Requested search log, or `None` for automatic selection.
    pub const fn search_log(&self) -> Option<u8> {
        self.search_log
    }

    /// Requested minimum-match search value, or `None` for automatic selection.
    pub const fn min_match(&self) -> Option<u8> {
        self.min_match
    }

    /// Requested target length, or `None` for automatic selection.
    pub const fn target_length(&self) -> Option<u32> {
        self.target_length
    }

    /// Requested reference strategy, or `None` for automatic selection.
    pub const fn strategy(&self) -> Option<ReferenceStrategy> {
        self.strategy
    }

    /// Set a source-size hint in `0..=i32::MAX`; zero disables it.
    ///
    /// Corresponds to the `ZSTD_c_srcSizeHint` request. A hint is separate from
    /// a source pledge and is relevant only when the source size is unknown.
    pub fn set_source_hint(&mut self, bytes: u32) -> Result<()> {
        if bytes > i32::MAX as u32 {
            return Err(Error::InvalidParameter);
        }
        self.source_hint = if bytes == 0 { None } else { Some(bytes) };
        Ok(())
    }

    /// Requested positive source hint, or `None` when disabled.
    pub const fn source_hint(&self) -> Option<u32> {
        self.source_hint
    }
}

impl Default for CompressionParameterRequest {
    fn default() -> Self {
        Self::new()
    }
}

fn optional_log(value: u8, minimum: u8, maximum: u8) -> Result<Option<u8>> {
    if value == 0 {
        Ok(None)
    } else {
        checked_log(u32::from(value), minimum, maximum).map(Some)
    }
}

/// Reference frame defaults returned by `ZSTD_getParams`.
///
/// These defaults enable content size and dictionary ID, and disable checksum.
/// They do not alter the checksum-enabled [`crate::EncoderOptions`] defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReferenceFrameParameters {
    include_content_size: bool,
    checksum: bool,
    include_dictionary_id: bool,
}

impl ReferenceFrameParameters {
    /// Construct the pinned reference's default frame settings.
    pub const fn new() -> Self {
        Self {
            include_content_size: true,
            checksum: false,
            include_dictionary_id: true,
        }
    }

    /// Whether the reference includes an available content size by default.
    pub const fn include_content_size(&self) -> bool {
        self.include_content_size
    }

    /// Whether the reference enables a content checksum by default.
    pub const fn checksum(&self) -> bool {
        self.checksum
    }

    /// Whether the reference includes an available dictionary ID by default.
    pub const fn include_dictionary_id(&self) -> bool {
        self.include_dictionary_id
    }
}

impl Default for ReferenceFrameParameters {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod fixture {
    use alloc::vec::Vec;

    pub(super) const BOUNDS: &str = include_str!("../tests/upstream/parameters/bounds.tsv");
    pub(super) const WIDTHS: &str = include_str!("../tests/upstream/parameters/width-bounds.tsv");
    #[cfg(target_pointer_width = "64")]
    pub(super) const ADJUSTMENTS: &str =
        include_str!("../tests/upstream/parameters/adjustments.tsv");
    #[cfg(target_pointer_width = "64")]
    pub(super) const REQUESTS: &str = include_str!("../tests/upstream/parameters/requests.tsv");
    pub(super) const QUERIES: &str = include_str!("../tests/upstream/parameters/queries.tsv");

    pub(super) fn rows(source: &str) -> Vec<Vec<&str>> {
        let mut lines = source.lines();
        let columns = lines.next().unwrap().split('\t').count();
        lines
            .map(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                assert_eq!(fields.len(), columns, "malformed fixture row: {line}");
                fields
            })
            .collect()
    }

    pub(super) fn number<T: core::str::FromStr>(text: &str) -> T
    where
        T::Err: core::fmt::Debug,
    {
        text.parse().unwrap()
    }
}

#[cfg(test)]
mod validation_tests {
    use super::{fixture::*, *};

    fn valid_raw() -> RawCompressionParameters {
        RawCompressionParameters {
            window_log: 17,
            chain_log: 16,
            hash_log: 17,
            search_log: 1,
            min_match: 5,
            target_length: 0,
            strategy: 2,
        }
    }

    fn replace(raw: &mut RawCompressionParameters, field: usize, value: u32) {
        match field {
            0 => raw.window_log = value,
            1 => raw.chain_log = value,
            2 => raw.hash_log = value,
            3 => raw.search_log = value,
            4 => raw.min_match = value,
            5 => raw.target_length = value,
            6 => raw.strategy = value,
            _ => panic!("unknown test field"),
        }
    }

    #[test]
    fn source_size_keeps_unknown_distinct_from_known_empty_and_reserved_value() {
        const EMPTY: SourceSize = SourceSize::Known(0);
        assert_eq!(SourceSize::default(), SourceSize::Unknown);
        assert_ne!(EMPTY, SourceSize::Unknown);
        // Stage 1 stores knowledge only. Reserved-known rejection belongs to
        // the later query boundary, not a silent enum conversion to Unknown.
        assert_ne!(SourceSize::Known(u64::MAX), SourceSize::Unknown);
    }

    #[test]
    fn reference_strategy_codes_are_explicit_and_reject_non_strategies() {
        let strategies = [
            ReferenceStrategy::Fast,
            ReferenceStrategy::DoubleFast,
            ReferenceStrategy::Greedy,
            ReferenceStrategy::Lazy,
            ReferenceStrategy::Lazy2,
            ReferenceStrategy::BinaryTreeLazy2,
            ReferenceStrategy::Optimal,
            ReferenceStrategy::Ultra,
            ReferenceStrategy::Ultra2,
        ];
        for (index, strategy) in strategies.into_iter().enumerate() {
            let code = u32::try_from(index + 1).unwrap();
            assert_eq!(ReferenceStrategy::try_from(code), Ok(strategy));
            let raw = RawCompressionParameters {
                strategy: code,
                ..valid_raw()
            };
            assert_eq!(raw.validate().unwrap().to_raw(), raw);
        }
        for code in [0, 10, 255, i32::MAX as u32, 0x8000_0000, u32::MAX] {
            assert_eq!(
                ReferenceStrategy::try_from(code),
                Err(Error::InvalidParameter)
            );
        }
    }

    #[test]
    fn bounds_match_host_oracle_and_separately_labeled_width_macros() {
        let widths = rows(WIDTHS);
        assert_eq!(widths.len(), 2);
        assert_eq!(
            widths[0],
            ["32", "macro_derived_not_executed", "30", "29", "30", "29"]
        );
        assert_eq!(
            widths[1],
            ["64", "host_executed_bounds", "31", "30", "30", "30"]
        );
        let width_row = &widths[usize::from(usize::BITS != 32)];
        assert_eq!(u32::from(WINDOW_LOG_MAX), number(width_row[2]));
        assert_eq!(u32::from(CHAIN_LOG_MAX), number(width_row[3]));
        assert_eq!(u32::from(HASH_LOG_MAX), number(width_row[4]));
        assert_eq!(u32::from(SEARCH_LOG_MAX), number(width_row[5]));

        let mut scalar_bounds = 0;
        let mut frame_flags = 0;
        let mut unsupported = 0;
        for row in rows(BOUNDS) {
            let expected = match row[0] {
                "level" => (LEVEL_MIN, LEVEL_MAX),
                "window_log" => (i32::from(WINDOW_LOG_MIN), 31),
                "chain_log" => (i32::from(CHAIN_LOG_MIN), 30),
                "hash_log" => (i32::from(HASH_LOG_MIN), i32::from(HASH_LOG_MAX)),
                "search_log" => (i32::from(SEARCH_LOG_MIN), 30),
                "min_match" => (i32::from(MIN_MATCH_MIN), i32::from(MIN_MATCH_MAX)),
                "target_length" => (0, TARGET_LENGTH_MAX as i32),
                "source_hint" => (0, i32::MAX),
                "strategy" => (1, 9),
                "content_size" | "checksum" | "dictionary_id" => {
                    frame_flags += 1;
                    assert_eq!(&row[2..], ["0", "1", "0"]);
                    continue;
                }
                "unsupported" => {
                    unsupported += 1;
                    assert_eq!(row[4], "40");
                    continue;
                }
                _ => panic!("unclassified bounds row"),
            };
            assert_eq!((number(row[2]), number(row[3])), expected);
            assert_eq!(row[4], "0");
            scalar_bounds += 1;
        }
        assert_eq!((scalar_bounds, frame_flags, unsupported), (9, 3, 1));
    }

    #[test]
    fn each_raw_field_validates_boundaries_without_clamping() {
        let bounds = [
            (u32::from(WINDOW_LOG_MIN), u32::from(WINDOW_LOG_MAX)),
            (u32::from(CHAIN_LOG_MIN), u32::from(CHAIN_LOG_MAX)),
            (u32::from(HASH_LOG_MIN), u32::from(HASH_LOG_MAX)),
            (u32::from(SEARCH_LOG_MIN), u32::from(SEARCH_LOG_MAX)),
            (u32::from(MIN_MATCH_MIN), u32::from(MIN_MATCH_MAX)),
            (0, TARGET_LENGTH_MAX),
            (1, 9),
        ];
        for (field, (minimum, maximum)) in bounds.into_iter().enumerate() {
            for value in [minimum, maximum] {
                let mut raw = valid_raw();
                replace(&mut raw, field, value);
                assert_eq!(raw.validate().unwrap().to_raw(), raw);
            }
            for value in [maximum + 1, 0x8000_0000, u32::MAX] {
                let mut raw = valid_raw();
                replace(&mut raw, field, value);
                assert_eq!(raw.validate(), Err(Error::InvalidParameter));
            }
            if minimum != 0 {
                let mut raw = valid_raw();
                replace(&mut raw, field, minimum - 1);
                assert_eq!(raw.validate(), Err(Error::InvalidParameter));
            }
        }
        assert_eq!(
            RawCompressionParameters::default().validate(),
            Err(Error::InvalidParameter)
        );
    }

    #[test]
    fn resolved_getters_preserve_all_concrete_fields_including_zero_target() {
        let raw = valid_raw();
        let valid = raw.validate().unwrap();
        assert_eq!(valid.window_log(), 17);
        assert_eq!(valid.chain_log(), 16);
        assert_eq!(valid.hash_log(), 17);
        assert_eq!(valid.search_log(), 1);
        assert_eq!(valid.min_match(), 5);
        assert_eq!(valid.target_length(), 0);
        assert_eq!(valid.strategy(), ReferenceStrategy::DoubleFast);
        assert_eq!(valid.to_raw(), raw);
    }

    // The recorded C validation ran with 64-bit size_t. Narrower-target bounds
    // are checked above against explicitly macro-derived rows, not relabeled
    // as executed 32-bit C outcomes.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn all_225_recorded_raw_inputs_match_c_validation_only() {
        let fixtures = rows(ADJUSTMENTS);
        assert_eq!(fixtures.len(), 225);
        for (ordinal, row) in fixtures.iter().enumerate() {
            let raw = RawCompressionParameters {
                window_log: number(row[5]),
                chain_log: number(row[6]),
                hash_log: number(row[7]),
                search_log: number(row[8]),
                min_match: number(row[9]),
                target_length: number(row[10]),
                strategy: number(row[11]),
            };
            match raw.validate() {
                Ok(valid) => {
                    assert_eq!(row[12], "0", "fixture ordinal {ordinal}");
                    assert_eq!(valid.to_raw(), raw);
                }
                Err(error) => {
                    assert_eq!(error, Error::InvalidParameter);
                    assert_eq!(row[12], "42", "fixture ordinal {ordinal}");
                }
            }
        }
        // Adjusted fields, domain-specific query rejection, and signed clamp
        // outcomes belong to stage 3, not this validation-only fixture replay.
    }
}

#[cfg(test)]
mod request_tests {
    #[cfg(target_pointer_width = "64")]
    use super::fixture::*;
    use super::*;

    fn configured() -> CompressionParameterRequest {
        let mut request = CompressionParameterRequest::with_level(7);
        request.set_window_log(20).unwrap();
        request.set_chain_log(16).unwrap();
        request.set_hash_log(18).unwrap();
        request.set_search_log(4).unwrap();
        request.set_min_match(4).unwrap();
        request.set_target_length(25).unwrap();
        request.set_strategy(Some(ReferenceStrategy::Lazy));
        request.set_source_hint(16384).unwrap();
        request
    }

    fn snapshot(request: &CompressionParameterRequest) -> [i64; 9] {
        [
            i64::from(request.level()),
            i64::from(request.window_log().unwrap_or(0)),
            i64::from(request.chain_log().unwrap_or(0)),
            i64::from(request.hash_log().unwrap_or(0)),
            i64::from(request.search_log().unwrap_or(0)),
            i64::from(request.min_match().unwrap_or(0)),
            i64::from(request.target_length().unwrap_or(0)),
            i64::from(request.strategy().map_or(0, ReferenceStrategy::code)),
            i64::from(request.source_hint().unwrap_or(0)),
        ]
    }

    #[test]
    fn default_and_const_construction_leave_every_override_automatic() {
        const REQUEST: CompressionParameterRequest = CompressionParameterRequest::new();
        assert_eq!(REQUEST, CompressionParameterRequest::default());
        assert_eq!(snapshot(&REQUEST), [3, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn level_clamps_extremes_without_negation_or_clearing_overrides() {
        let mut request = configured();
        let original = snapshot(&request);
        for (input, expected) in [
            (i32::MIN, -131072),
            (-131073, -131072),
            (-131072, -131072),
            (-1, -1),
            (0, 3),
            (1, 1),
            (22, 22),
            (23, 22),
            (i32::MAX, 22),
        ] {
            request.set_level(input);
            assert_eq!(request.level(), expected);
            assert_eq!(&snapshot(&request)[1..], &original[1..]);
            assert_eq!(
                CompressionParameterRequest::with_level(input).level(),
                expected
            );
        }
    }

    #[test]
    fn zero_clears_overrides_and_hint_without_changing_level() {
        let mut request = configured();
        request.set_window_log(0).unwrap();
        request.set_chain_log(0).unwrap();
        request.set_hash_log(0).unwrap();
        request.set_search_log(0).unwrap();
        request.set_min_match(0).unwrap();
        request.set_target_length(0).unwrap();
        request.set_strategy(None);
        request.set_source_hint(0).unwrap();
        assert_eq!(request, CompressionParameterRequest::with_level(7));
    }

    #[test]
    fn invalid_scalar_setters_preserve_the_complete_request() {
        let mut request = configured();
        let before = request;
        type ScalarSetter = fn(&mut CompressionParameterRequest, u8) -> Result<()>;
        let setters: [(ScalarSetter, u8, u8); 5] = [
            (
                CompressionParameterRequest::set_window_log,
                WINDOW_LOG_MIN,
                WINDOW_LOG_MAX,
            ),
            (
                CompressionParameterRequest::set_chain_log,
                CHAIN_LOG_MIN,
                CHAIN_LOG_MAX,
            ),
            (
                CompressionParameterRequest::set_hash_log,
                HASH_LOG_MIN,
                HASH_LOG_MAX,
            ),
            (
                CompressionParameterRequest::set_search_log,
                SEARCH_LOG_MIN,
                SEARCH_LOG_MAX,
            ),
            (
                CompressionParameterRequest::set_min_match,
                MIN_MATCH_MIN,
                MIN_MATCH_MAX,
            ),
        ];
        for (set, minimum, maximum) in setters {
            for invalid in [maximum + 1, u8::MAX] {
                assert_eq!(set(&mut request, invalid), Err(Error::InvalidParameter));
                assert_eq!(request, before);
            }
            if minimum > 1 {
                assert_eq!(set(&mut request, minimum - 1), Err(Error::InvalidParameter));
                assert_eq!(request, before);
            }
        }
        for invalid in [TARGET_LENGTH_MAX + 1, 0x8000_0000, u32::MAX] {
            assert_eq!(
                request.set_target_length(invalid),
                Err(Error::InvalidParameter)
            );
            assert_eq!(request, before);
        }
        for invalid in [i32::MAX as u32 + 1, u32::MAX] {
            assert_eq!(
                request.set_source_hint(invalid),
                Err(Error::InvalidParameter)
            );
            assert_eq!(request, before);
        }
    }

    #[test]
    fn typed_setters_accept_target_width_bounds_and_keep_hint_independent() {
        let mut request = CompressionParameterRequest::new();
        request.set_window_log(WINDOW_LOG_MAX).unwrap();
        request.set_chain_log(CHAIN_LOG_MAX).unwrap();
        request.set_hash_log(HASH_LOG_MAX).unwrap();
        request.set_search_log(SEARCH_LOG_MAX).unwrap();
        request.set_min_match(MIN_MATCH_MIN).unwrap();
        request.set_target_length(TARGET_LENGTH_MAX).unwrap();
        request.set_source_hint(i32::MAX as u32).unwrap();
        assert_eq!(request.window_log(), Some(WINDOW_LOG_MAX));
        assert_eq!(request.chain_log(), Some(CHAIN_LOG_MAX));
        assert_eq!(request.hash_log(), Some(HASH_LOG_MAX));
        assert_eq!(request.search_log(), Some(SEARCH_LOG_MAX));
        assert_eq!(request.min_match(), Some(MIN_MATCH_MIN));
        assert_eq!(request.target_length(), Some(TARGET_LENGTH_MAX));
        assert_eq!(request.source_hint(), Some(i32::MAX as u32));
        request.set_source_hint(16384).unwrap();
        assert_eq!(request.source_hint(), Some(16384));
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn c_scalar_trace_accounts_for_typed_inputs_and_outside_surface_rows() {
        let mut request = CompressionParameterRequest::new();
        let mut scalar_rows = 0;
        let mut frame_rows = 0;
        let mut unsupported_rows = 0;
        let mut typed_boundary_rejections = 0;
        let mut setter_calls = 0;
        let fixtures = rows(REQUESTS);
        assert_eq!(fixtures.len(), 150);
        for (ordinal, row) in fixtures.iter().enumerate() {
            if matches!(row[0], "content_size" | "checksum" | "dictionary_id") {
                // C flag truth conversion is deliberately outside this scalar
                // request API. Default frame flags have independent tests.
                frame_rows += 1;
                continue;
            }
            if row[0] == "unsupported" {
                // There is no untyped parameter-ID setter in the Rust API.
                assert_eq!(row[5], "40");
                unsupported_rows += 1;
                continue;
            }
            let before = request;
            let value: i32 = number(row[4]);
            let outcome = match row[2] {
                "init" => {
                    request = CompressionParameterRequest::with_level(value);
                    Ok(())
                }
                "reset" => {
                    request = CompressionParameterRequest::new();
                    Ok(())
                }
                "set" => {
                    let parameter: u32 = number(row[3]);
                    let representable = match parameter {
                        100 => true,
                        101..=105 => u8::try_from(value).is_ok(),
                        106 | 1004 => u32::try_from(value).is_ok(),
                        107 => {
                            value == 0
                                || u32::try_from(value)
                                    .ok()
                                    .and_then(|code| ReferenceStrategy::try_from(code).ok())
                                    .is_some()
                        }
                        _ => panic!("unclassified scalar parameter"),
                    };
                    if !representable {
                        // These C integers cannot reach the typed Rust setter.
                        // Count them as boundary rejections, not setter calls.
                        typed_boundary_rejections += 1;
                        Err(Error::InvalidParameter)
                    } else {
                        setter_calls += 1;
                        match parameter {
                            100 => {
                                request.set_level(value);
                                Ok(())
                            }
                            101 => request.set_window_log(u8::try_from(value).unwrap()),
                            102 => request.set_hash_log(u8::try_from(value).unwrap()),
                            103 => request.set_chain_log(u8::try_from(value).unwrap()),
                            104 => request.set_search_log(u8::try_from(value).unwrap()),
                            105 => request.set_min_match(u8::try_from(value).unwrap()),
                            106 => request.set_target_length(u32::try_from(value).unwrap()),
                            107 => {
                                request.set_strategy(if value == 0 {
                                    None
                                } else {
                                    Some(
                                        ReferenceStrategy::try_from(u32::try_from(value).unwrap())
                                            .unwrap(),
                                    )
                                });
                                Ok(())
                            }
                            1004 => request.set_source_hint(u32::try_from(value).unwrap()),
                            _ => unreachable!(),
                        }
                    }
                }
                _ => panic!("unclassified request operation"),
            };
            let expected_error: u32 = number(row[5]);
            if expected_error == 0 {
                assert_eq!(outcome, Ok(()), "fixture ordinal {ordinal}");
            } else {
                assert_eq!(expected_error, 42);
                assert_eq!(
                    outcome,
                    Err(Error::InvalidParameter),
                    "fixture ordinal {ordinal}"
                );
                assert_eq!(request, before);
            }
            let expected: [i64; 9] = core::array::from_fn(|index| number(row[7 + index]));
            assert_eq!(snapshot(&request), expected, "fixture ordinal {ordinal}");
            scalar_rows += 1;
        }
        assert_eq!((scalar_rows, frame_rows, unsupported_rows), (119, 30, 1));
        assert_eq!((setter_calls, typed_boundary_rejections), (72, 25));
        // Resolved columns and C integer success return values are not Rust
        // scalar setter contracts; stage 3 separately checks resolution.
    }
}

#[cfg(test)]
mod frame_tests {
    use super::{fixture::*, *};

    #[test]
    fn reference_defaults_match_query_flags_without_changing_encoder_defaults() {
        const FRAME: ReferenceFrameParameters = ReferenceFrameParameters::new();
        assert_eq!(FRAME, ReferenceFrameParameters::default());
        let flags = [
            u8::from(FRAME.include_content_size()),
            u8::from(FRAME.checksum()),
            u8::from(FRAME.include_dictionary_id()),
        ];
        assert_eq!(flags, [1, 0, 1]);
        let fixtures = rows(QUERIES);
        assert_eq!(fixtures.len(), 266);
        for row in fixtures {
            assert_eq!(flags, [number(row[14]), number(row[15]), number(row[16])]);
        }
        assert!(crate::EncoderOptions::default().checksum);
        assert_eq!(
            crate::EncoderOptions::default().strategy,
            crate::CompressionStrategy::Greedy
        );
    }
}

#[cfg(test)]
mod resolution_tests {
    use super::*;

    fn baseline() -> RawCompressionParameters {
        RawCompressionParameters {
            window_log: 21,
            chain_log: 16,
            hash_log: 17,
            search_log: 1,
            min_match: 5,
            target_length: 0,
            strategy: 2,
        }
    }

    #[cfg(target_pointer_width = "64")]
    fn raw_record(fields: &[&str]) -> RawCompressionParameters {
        assert_eq!(fields.len(), 7);
        RawCompressionParameters {
            window_log: fixture::number(fields[0]),
            chain_log: fixture::number(fields[1]),
            hash_log: fixture::number(fields[2]),
            search_log: fixture::number(fields[3]),
            min_match: fixture::number(fields[4]),
            target_length: fixture::number(fields[5]),
            strategy: fixture::number(fields[6]),
        }
    }

    #[cfg(target_pointer_width = "64")]
    fn request_record(
        level: i32,
        raw: RawCompressionParameters,
        hint: u32,
    ) -> CompressionParameterRequest {
        let mut request = CompressionParameterRequest::with_level(level);
        request
            .set_window_log(raw.window_log.try_into().unwrap())
            .unwrap();
        request
            .set_chain_log(raw.chain_log.try_into().unwrap())
            .unwrap();
        request
            .set_hash_log(raw.hash_log.try_into().unwrap())
            .unwrap();
        request
            .set_search_log(raw.search_log.try_into().unwrap())
            .unwrap();
        request
            .set_min_match(raw.min_match.try_into().unwrap())
            .unwrap();
        request.set_target_length(raw.target_length).unwrap();
        request.set_strategy(if raw.strategy == 0 {
            None
        } else {
            Some(ReferenceStrategy::try_from(raw.strategy).unwrap())
        });
        request.set_source_hint(hint).unwrap();
        request
    }

    #[test]
    fn associated_level_bounds_match_normalization_and_frozen_bounds() {
        assert_eq!(CompressionParameterRequest::MIN_LEVEL, -131_072);
        assert_eq!(CompressionParameterRequest::MAX_LEVEL, 22);
        assert_eq!(CompressionParameterRequest::DEFAULT_LEVEL, 3);
        for (input, expected) in [
            (i32::MIN, CompressionParameterRequest::MIN_LEVEL),
            (i32::MAX, CompressionParameterRequest::MAX_LEVEL),
            (0, CompressionParameterRequest::DEFAULT_LEVEL),
        ] {
            assert_eq!(
                inspect_compression_level(input, SourceSize::Unknown, 0),
                inspect_compression_level(expected, SourceSize::Unknown, 0)
            );
        }
    }

    #[test]
    fn raw_adjustment_uses_signed32_clamps_and_target_width_bounds() {
        for high_bit in [0x8000_0000, u32::MAX] {
            let raw = RawCompressionParameters {
                window_log: high_bit,
                chain_log: high_bit,
                hash_log: high_bit,
                search_log: high_bit,
                min_match: high_bit,
                target_length: high_bit,
                strategy: high_bit,
            };
            assert_eq!(raw.validate(), Err(Error::InvalidParameter));
            assert_eq!(
                raw.adjust(SourceSize::Unknown, 0).unwrap().to_raw(),
                RawCompressionParameters {
                    window_log: 10,
                    chain_log: 6,
                    hash_log: 6,
                    search_log: 1,
                    min_match: 3,
                    target_length: 0,
                    strategy: 1
                }
            );
        }
        let upper = RawCompressionParameters {
            window_log: i32::MAX as u32,
            chain_log: i32::MAX as u32,
            hash_log: i32::MAX as u32,
            search_log: i32::MAX as u32,
            min_match: i32::MAX as u32,
            target_length: i32::MAX as u32,
            strategy: i32::MAX as u32,
        };
        assert_eq!(
            upper.adjust(SourceSize::Unknown, 0).unwrap().to_raw(),
            RawCompressionParameters {
                window_log: WINDOW_LOG_MAX.into(),
                chain_log: CHAIN_LOG_MAX.into(),
                hash_log: HASH_LOG_MAX.into(),
                search_log: SEARCH_LOG_MAX.into(),
                min_match: 7,
                target_length: 131_072,
                strategy: 9
            }
        );
    }

    #[test]
    fn empty_adjustment_downsizes_hash_and_chain_before_flooring_window() {
        let value = inspect_compression_level(3, SourceSize::Known(0), 0).unwrap();
        assert_eq!(
            value.to_raw(),
            RawCompressionParameters {
                window_log: 10,
                chain_log: 6,
                hash_log: 7,
                search_log: 2,
                min_match: 4,
                target_length: 0,
                strategy: 2
            }
        );
        let raw = RawCompressionParameters {
            strategy: 9,
            ..baseline()
        };
        assert_eq!(raw.adjust(SourceSize::Known(0), 0).unwrap().chain_log(), 7);
        assert_eq!(raw.adjust(SourceSize::Unknown, 0).unwrap().chain_log(), 16);
    }

    #[test]
    fn row_hash_caps_apply_only_to_the_three_reference_row_strategies() {
        for strategy in 1..=9 {
            for search in [1, 4, 5, 6, SEARCH_LOG_MAX] {
                let raw = RawCompressionParameters {
                    hash_log: 30,
                    search_log: search.into(),
                    strategy,
                    ..baseline()
                };
                let adjusted = raw.adjust(SourceSize::Unknown, 0).unwrap();
                let expected = if (3..=5).contains(&strategy) {
                    24 + search.clamp(4, 6)
                } else {
                    30
                };
                assert_eq!(
                    adjusted.hash_log(),
                    expected,
                    "strategy {strategy}, search {search}"
                );
            }
        }
    }

    #[test]
    fn request_resolution_preserves_initial_adjustment_before_strategy_override() {
        let mut request = CompressionParameterRequest::new();
        request.set_strategy(Some(ReferenceStrategy::Ultra2));
        let before = request;
        let resolved = request.resolve(SourceSize::Known(0), 0).unwrap();
        assert_eq!(
            (
                resolved.window_log(),
                resolved.chain_log(),
                resolved.hash_log(),
                resolved.strategy()
            ),
            (10, 6, 7, ReferenceStrategy::Ultra2)
        );
        let unadjusted_override = RawCompressionParameters {
            strategy: 9,
            ..presets::select(3, SourceSize::Known(0), 0).unwrap()
        };
        assert_eq!(
            unadjusted_override
                .adjust(SourceSize::Known(0), 0)
                .unwrap()
                .chain_log(),
            7
        );
        assert_eq!(request, before);
    }

    #[test]
    fn source_hints_change_unknown_resolution_only_and_never_mutate_requests() {
        let mut request = CompressionParameterRequest::new();
        let unknown = request.resolve(SourceSize::Unknown, 0).unwrap();
        let empty = request.resolve(SourceSize::Known(0), 0).unwrap();
        let known = request.resolve(SourceSize::Known(16_385), 0).unwrap();
        request.set_source_hint(16_384).unwrap();
        let before = request;
        assert_ne!(request.resolve(SourceSize::Unknown, 0).unwrap(), unknown);
        assert_eq!(
            request.resolve(SourceSize::Unknown, 0),
            request.resolve(SourceSize::Known(16_384), 0)
        );
        assert_eq!(request.resolve(SourceSize::Known(0), 0), Ok(empty));
        assert_eq!(request.resolve(SourceSize::Known(16_385), 0), Ok(known));
        assert_eq!(
            request.resolve(SourceSize::Known(u64::MAX), 0),
            Err(Error::InvalidParameter)
        );
        assert_eq!(request, before);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn checked_size_policy_is_specific_to_the_arithmetic_each_operation_uses() {
        let raw = baseline();
        assert_eq!(
            raw.adjust(SourceSize::Unknown, usize::MAX)
                .unwrap()
                .to_raw(),
            raw
        );
        assert_eq!(
            inspect_compression_level(3, SourceSize::Unknown, usize::MAX),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            raw.adjust(SourceSize::Known(u64::MAX), 0),
            Err(Error::InvalidParameter)
        );
        assert_eq!(
            raw.adjust(SourceSize::Known(u64::MAX - 1), 2),
            Err(Error::ResourceLimit)
        );
        assert!(raw.adjust(SourceSize::Known(u64::MAX - 1), 1).is_ok());
        let small_window = RawCompressionParameters {
            window_log: 10,
            ..raw
        };
        assert!(
            small_window
                .adjust(SourceSize::Known(1), usize::MAX - 1024)
                .is_ok()
        );
        assert_eq!(
            small_window.adjust(SourceSize::Known(1), usize::MAX - 1023),
            Err(Error::ResourceLimit)
        );
        let mut request = CompressionParameterRequest::new();
        request.set_source_hint(16_384).unwrap();
        request.set_hash_log(25).unwrap();
        let before = request;
        assert_eq!(
            request.resolve(SourceSize::Unknown, usize::MAX - 1000),
            Err(Error::ResourceLimit)
        );
        assert_eq!(request, before);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn all_266_query_rows_compare_or_explicitly_reject_c_modular_domains() {
        let mut exact = 0;
        let mut rejected = 0;
        for row in fixture::rows(fixture::QUERIES) {
            let source = match row[6] {
                "known" | "reserved_known" => SourceSize::Known(fixture::number(row[3])),
                "unknown" | "public_zero_unknown" => SourceSize::Unknown,
                other => panic!("unclassified source kind: {other}"),
            };
            assert!(matches!(row[1], "public" | "typed_no_attach"));
            let result =
                inspect_compression_level(fixture::number(row[2]), source, fixture::number(row[4]));
            match row[5] {
                "ordinary" => {
                    assert_eq!(
                        result.map(|p| p.to_raw()),
                        Ok(raw_record(&row[7..14])),
                        "{row:?}"
                    );
                    exact += 1;
                }
                "rust_reject_reserved_known" => {
                    assert_eq!(result, Err(Error::InvalidParameter), "{row:?}");
                    rejected += 1;
                }
                "c_modular_known_sum"
                | "c_modular_dictionary_plus499"
                | "c_modular_dictionary_window" => {
                    assert_eq!(result, Err(Error::ResourceLimit), "{row:?}");
                    rejected += 1;
                }
                other => panic!("unclassified query domain: {other}"),
            }
        }
        assert_eq!((exact, rejected), (262, 4));
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn all_225_adjustment_rows_compare_signed_clamps_or_explicitly_reject_overflow() {
        let mut exact = 0;
        let mut rejected = 0;
        for row in fixture::rows(fixture::ADJUSTMENTS) {
            let size = fixture::number::<u64>(row[2]);
            assert!(matches!(row[1], "public" | "typed_no_attach_adapter"));
            let source = if size == u64::MAX || (row[1] == "public" && size == 0) {
                SourceSize::Unknown
            } else {
                SourceSize::Known(size)
            };
            let result = raw_record(&row[5..12]).adjust(source, fixture::number(row[3]));
            match row[4] {
                "ordinary" | "ordinary_signed32_raw" => {
                    assert_eq!(
                        result.map(|p| p.to_raw()),
                        Ok(raw_record(&row[13..20])),
                        "{row:?}"
                    );
                    exact += 1;
                }
                "c_modular_known_sum" | "c_modular_dictionary_window" => {
                    assert_eq!(result, Err(Error::ResourceLimit), "{row:?}");
                    rejected += 1;
                }
                other => panic!("unclassified adjustment domain: {other}"),
            }
        }
        assert_eq!((exact, rejected), (223, 2));
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn all_150_requested_states_resolve_without_claiming_frame_or_unsupported_setters() {
        let mut scalar = 0;
        let mut frame_projection = 0;
        let mut unsupported_projection = 0;
        let mut profiles = 0;
        for row in fixture::rows(fixture::REQUESTS) {
            // Stage-one tests replay supported mutation operations. Here every
            // recorded scalar state is resolved, including projections from
            // frame-only and unsupported operations without claiming those APIs.
            match row[0] {
                "content_size" | "checksum" | "dictionary_id" => frame_projection += 1,
                "unsupported" => unsupported_projection += 1,
                _ => scalar += 1,
            }
            let request = request_record(
                fixture::number(row[7]),
                raw_record(&row[8..15]),
                fixture::number(row[15]),
            );
            let before = request;
            for (source, offset) in [
                (SourceSize::Known(0), 19),
                (SourceSize::Unknown, 26),
                (SourceSize::Known(16_385), 33),
            ] {
                assert_eq!(
                    request.resolve(source, 0).map(|p| p.to_raw()),
                    Ok(raw_record(&row[offset..offset + 7])),
                    "{row:?} {source:?}"
                );
                assert_eq!(request, before);
                profiles += 1;
            }
        }
        assert_eq!(
            (scalar, frame_projection, unsupported_projection, profiles),
            (119, 30, 1, 450)
        );
    }

    #[cfg(all(feature = "std", target_pointer_width = "64"))]
    #[test]
    #[ignore = "requires the separately generated C review probe via ZSTDOXIDE_PARAMETER_REVIEW_PROBE"]
    fn external_review_probe_matches_independent_c_queries_adjustments_and_requests() {
        let path = std::env::var("ZSTDOXIDE_PARAMETER_REVIEW_PROBE")
            .expect("set path to the 46-row external review TSV");
        let input = std::fs::read_to_string(path).unwrap();
        let mut exact = 0;
        let mut rejected = 0;
        for row in fixture::rows(&input) {
            assert_eq!(row.len(), 21);
            let size = fixture::number::<u64>(row[3]);
            let source = if row[2] == "rust_reject_reserved_known" {
                SourceSize::Known(u64::MAX)
            } else if size == u64::MAX || (row[1].starts_with("public_") && size == 0) {
                SourceSize::Unknown
            } else {
                SourceSize::Known(size)
            };
            let dictionary = fixture::number(row[4]);
            let raw = raw_record(&row[7..14]);
            let result = match row[1] {
                "public_query" => {
                    inspect_compression_level(fixture::number(row[5]), source, dictionary)
                }
                "public_adjust" => raw.adjust(source, dictionary),
                "typed_request" => {
                    let request =
                        request_record(fixture::number(row[5]), raw, fixture::number(row[6]));
                    let before = request;
                    let result = request.resolve(source, dictionary);
                    assert_eq!(request, before);
                    result
                }
                other => panic!("unclassified review operation: {other}"),
            };
            match row[2] {
                "ordinary_known" | "ordinary_unknown" => {
                    assert_eq!(
                        result.map(|p| p.to_raw()),
                        Ok(raw_record(&row[14..21])),
                        "{row:?}"
                    );
                    exact += 1;
                }
                "rust_reject_reserved_known" => {
                    assert_eq!(result, Err(Error::InvalidParameter), "{row:?}");
                    rejected += 1;
                }
                "rust_reject_sum_overflow"
                | "rust_reject_plus499_overflow"
                | "rust_reject_hint_sum_overflow"
                | "rust_reject_dictionary_window_overflow" => {
                    assert_eq!(result, Err(Error::ResourceLimit), "{row:?}");
                    rejected += 1;
                }
                other => panic!("unclassified review domain: {other}"),
            }
        }
        assert_eq!((exact, rejected), (41, 5));
    }
}
