/// Step of the consensus protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Step {
    /// A round just started without a proposal from a leader.
    NewRound = 0x00,
    /// The block proposal from the leader was accepted in the network.
    Propose = 0x01,
    /// The block acceptance is ready to commit in the network.
    Prevote = 0x02,
    /// The commit is performed locally and should be accepted by the peers.
    Precommit = 0x03,
    /// The round is finalized with a commit.
    Commit = 0x04,
}

impl Step {
    /// Deserialize the step from a byte.
    pub const fn from_u8(byte: u8) -> Self {
        match byte {
            0x01 => Self::Propose,
            0x02 => Self::Prevote,
            0x03 => Self::Precommit,
            0x04 => Self::Commit,

            _ => Self::NewRound,
        }
    }

    /// Beginning of a round.
    pub const fn initial() -> Self {
        Self::NewRound
    }

    /// Check if round is in precommit step.
    pub const fn is_precommit(&self) -> bool {
        matches!(self, Self::Precommit)
    }

    /// Check if round is finalized with a commit step.
    pub const fn is_commit(&self) -> bool {
        matches!(self, Self::Commit)
    }

    /// Check if round is in the initial step.
    pub const fn is_initial(&self) -> bool {
        const INITIAL: Step = Step::initial();

        matches!(self, &INITIAL)
    }

    /// Check if round is waiting for a proposal from the leader.
    pub const fn is_propose(&self) -> bool {
        matches!(self, Self::Propose)
    }

    /// Increment the current step to the next one of the consensus flow.
    pub const fn increment(self) -> Option<Self> {
        match self {
            Self::NewRound => Some(Self::Propose),
            Self::Propose => Some(Self::Prevote),
            Self::Prevote => Some(Self::Precommit),
            Self::Precommit => Some(Self::Commit),
            Self::Commit => None,
        }
    }
}

impl Iterator for Step {
    type Item = Step;

    fn next(&mut self) -> Option<Step> {
        self.increment().map(|s| *self = s).map(|_| *self)
    }
}
