use crate::{Height, Round, Vote};

use fuel_types::Bytes32;

/// Event produced by the reactor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    /// The reactor is awaiting for a block authorization to propose a new consensus round.
    AwaitingBlock {
        /// Height of the expected block.
        height: Height,
    },

    /// The reactor is idle.
    ///
    /// This will be produced when the reactor is not a validator for the current round.
    Idle,

    /// The reactor produced a vote and it should be broadcast to the peers.
    Broadcast {
        /// Vote produced by the reactor
        vote: Vote,
    },

    /// A block was committed.
    Commit {
        /// Committed block height.
        height: Height,
        /// Rounds performed for this height.
        round: Round,
        /// Block identifier.
        block_id: Bytes32,
    },

    /// A bad vote was received - should reduce the karma of the author
    BadVote {
        /// Tampered vote
        vote: Vote,
    },
}
