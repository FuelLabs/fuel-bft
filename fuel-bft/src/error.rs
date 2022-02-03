use core::convert::Infallible;
use core::fmt;

/// Consensus error variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    /// The block validation failed
    BlockValidation,

    /// The validator is not included for this round.
    ValidatorNotFound,

    /// Crypto backend error.
    Crypto(fuel_crypto::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<fuel_crypto::Error> for Error {
    fn from(e: fuel_crypto::Error) -> Error {
        Self::Crypto(e)
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
