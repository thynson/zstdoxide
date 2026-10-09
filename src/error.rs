use core::fmt;

/// A checked codec operation failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An option is outside the supported range.
    InvalidParameter,
    /// An operation is incompatible with the context phase; reset is required.
    InvalidState,
    /// The input ended before a complete value could be read.
    UnexpectedEof,
    /// A frame magic number is not recognized.
    InvalidMagic,
    /// A field, state, or stream violates the Zstandard format.
    CorruptData,
    /// The frame requires a dictionary that the caller has not supplied.
    DictionaryRequired,
    /// A formatted dictionary contains invalid or incomplete metadata.
    InvalidDictionary,
    /// The supplied dictionary ID does not match the frame's nonzero ID.
    DictionaryMismatch,
    /// Samples contain insufficient usable data for the requested training budget.
    TrainingFailed,
    /// A valid format feature is not implemented yet.
    UnsupportedFeature,
    /// An operation would exceed a checked size or caller resource limit.
    ResourceLimit,
    /// An allocation could not be satisfied.
    AllocationFailed,
    /// The decoded size does not match the declared content size.
    ContentSizeMismatch,
    /// The frame content checksum does not match.
    ChecksumMismatch,
}

/// The result type used throughout the codec.
pub type Result<T> = core::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidParameter => "invalid codec parameter",
            Self::InvalidState => "invalid codec context state",
            Self::UnexpectedEof => "unexpected end of input",
            Self::InvalidMagic => "invalid frame magic",
            Self::CorruptData => "corrupt Zstandard data",
            Self::DictionaryRequired => "a dictionary is required",
            Self::InvalidDictionary => "invalid Zstandard dictionary",
            Self::DictionaryMismatch => "dictionary ID mismatch",
            Self::TrainingFailed => "insufficient dictionary training samples",
            Self::UnsupportedFeature => "unsupported Zstandard feature",
            Self::ResourceLimit => "resource limit exceeded",
            Self::AllocationFailed => "allocation failed",
            Self::ContentSizeMismatch => "frame content size mismatch",
            Self::ChecksumMismatch => "frame checksum mismatch",
        })
    }
}

impl core::error::Error for Error {}
