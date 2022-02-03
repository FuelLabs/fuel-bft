use crate::{Height, Round, Step};

use fuel_crypto::PublicKey;

/// A request to be responded by the reactor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Request {
    /// The reactor should attempt to commit the provided block height
    Commit {
        /// Id of the request used to track its response
        id: u64,
        /// Block height
        height: Height,
        /// Height round
        round: Round,
    },

    /// Query the identity of the node for the provided height
    Identity {
        /// Id of the request used to track its response
        id: u64,
        /// Block height for the identity
        height: Height,
    },

    /// Attempt to initialize the node to be a validator of the given interval
    Initialize {
        /// Id of the request used to track its response
        id: u64,
        /// Height in which the node will start to act as validator
        start: Height,
        /// The duration, from `start`, that the node will act as validator
        validity: u64,
    },

    /// Query the current round
    Round {
        /// Id of the request used to track its response
        id: u64,
    },
}

impl Request {
    /// Request id to trace the response
    pub const fn id(&self) -> u64 {
        match self {
            Self::Commit { id, .. } => *id,
            Self::Identity { id, .. } => *id,
            Self::Initialize { id, .. } => *id,
            Self::Round { id, .. } => *id,
        }
    }
}

/// Response from the reactor as result of a request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Response {
    /// The reactor acknowledged a commit request
    Commit {
        /// Id of the request used to track its response
        id: u64,
        /// Flag stating whether the commit was accepted by the reactor
        committed: bool,
    },

    /// Query the identity of the node for the provided height
    Identity {
        /// Id of the request used to track its response
        id: u64,
        /// Public identity of the node for the provided height, if present.
        public: Option<PublicKey>,
    },

    /// Attempt to initialize the node to be a validator of the given interval
    Initialize {
        /// Id of the request used to track its response
        id: u64,
        /// The initialize request was successful
        initialized: bool,
    },

    /// Query the current round
    Round {
        /// Id of the request used to track its response
        id: u64,
        /// Bloch height
        height: Height,
        /// Height round
        round: Round,
        /// Public key of the leader
        leader: PublicKey,
        /// Current step of the node for the round.
        step: Option<Step>,
    },
}

impl Response {
    /// Request id to trace the response
    pub const fn id(&self) -> u64 {
        match self {
            Self::Commit { id, .. } => *id,
            Self::Identity { id, .. } => *id,
            Self::Initialize { id, .. } => *id,
            Self::Round { id, .. } => *id,
        }
    }
}
