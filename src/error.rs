use core::convert::Infallible;
use core::fmt;

/// Consensus error variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    /// The block validation failed
    BlockValidation,

    /// Failed to define elapsed time since genesis
    ElapsedTimeFailure,

    /// The provided signature is invalid
    InvalidSignature,

    /// The node isn't a round validator
    NotRoundValidator,

    /// The requested resource is not available
    ResourceNotAvailable,

    /// The validator is not included for this round.
    ValidatorNotFound,

    /// Vote is missing either the block id or round.
    VoteInconsistent,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<Error> for Infallible {
    fn from(_: Error) -> Infallible {
        unreachable!()
    }
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Error {
        unreachable!()
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
