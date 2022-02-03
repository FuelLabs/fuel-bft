use crate::{Height, Vote};

use fuel_crypto::PublicKey;
use fuel_types::Bytes32;

/// A notification to be consumed by the reactor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Notification {
    /// Kill command.
    Kill,

    /// A validator was included
    NewValidator {
        /// Initial block height.
        height: Height,
        /// Validity period of the validator.
        validity: u64,
        /// Validator identifier.
        validator: PublicKey,
    },

    /// A new vote was received
    Vote {
        /// Vote to be processed
        vote: Vote,
    },

    /// A block was cleared for consensus.
    ///
    /// The reactor will expect this event before it can upgrade from the Propose phase.
    BlockAuthorized {
        /// Block height
        height: Height,
        /// Block identifier.
        block_id: Bytes32,
    },

    /// A block was generated and is available in the network so the reactor can initiate the
    /// propose protocol.
    BlockProposeAuthorized {
        /// Block height
        height: Height,
        /// Block identifier.
        block_id: Bytes32,
    },
}
