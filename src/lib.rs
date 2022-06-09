#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

extern crate alloc;

// FIXME replace `async_trait` for stdlib when it becomes stable
// https://github.com/FuelLabs/fuel-bft/issues/11

/// Block height representation.
pub type Height = u64;

/// Round representation.
pub type Round = u64;

pub(crate) use consensus::Consensus;
pub(crate) use metadata::Metadata;

#[doc(no_inline)]
pub use fuel_crypto;

#[doc(no_inline)]
pub use fuel_types;

#[doc(no_inline)]
pub use time;

mod consensus;
mod error;
mod keychain;
mod metadata;
mod moderator;
mod reactor;
mod stake;
mod step;
mod vote;

pub use error::Error;
pub use keychain::Keychain;
pub use moderator::Moderator;
pub use reactor::{Config, Event, Message, Notification, Reactor, Request, Response};
pub use stake::{Stake, StakePool};
pub use step::Step;
pub use vote::Vote;

#[cfg(feature = "tokio-reactor")]
mod tokio_reactor;

#[cfg(feature = "tokio-reactor")]
pub use tokio_reactor::TokioReactor;

#[cfg(feature = "memory")]
pub use keychain::memory::MemoryKeychain;
