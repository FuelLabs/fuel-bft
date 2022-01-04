use core::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord)]
#[repr(u8)]
pub enum State {
    Reject = 0x00,
    NewHeight = 0x01,
    Propose = 0x02,
    Prevote = 0x03,
    Precommit = 0x04,
    Commit = 0x05,
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Reject, _) => Some(Ordering::Greater),
            (_, Self::Reject) => Some(Ordering::Less),

            (Self::NewHeight, Self::NewHeight) => Some(Ordering::Equal),
            (Self::NewHeight, _) => Some(Ordering::Less),

            (Self::Propose, Self::NewHeight) => Some(Ordering::Greater),
            (Self::Propose, Self::Propose) => Some(Ordering::Equal),
            (Self::Propose, _) => Some(Ordering::Less),

            (Self::Prevote, Self::NewHeight) => Some(Ordering::Greater),
            (Self::Prevote, Self::Propose) => Some(Ordering::Greater),
            (Self::Prevote, Self::Prevote) => Some(Ordering::Equal),
            (Self::Prevote, _) => Some(Ordering::Less),

            (Self::Precommit, Self::NewHeight) => Some(Ordering::Greater),
            (Self::Precommit, Self::Propose) => Some(Ordering::Greater),
            (Self::Precommit, Self::Prevote) => Some(Ordering::Greater),
            (Self::Precommit, Self::Precommit) => Some(Ordering::Equal),
            (Self::Precommit, _) => Some(Ordering::Less),

            (Self::Commit, Self::NewHeight) => Some(Ordering::Greater),
            (Self::Commit, Self::Propose) => Some(Ordering::Greater),
            (Self::Commit, Self::Prevote) => Some(Ordering::Greater),
            (Self::Commit, Self::Precommit) => Some(Ordering::Greater),
            (Self::Commit, Self::Commit) => Some(Ordering::Equal),
        }
    }
}

impl State {
    pub const fn from_u8(byte: u8) -> Self {
        match byte {
            0x01 => Self::NewHeight,
            0x02 => Self::Propose,
            0x03 => Self::Prevote,
            0x04 => Self::Precommit,
            0x05 => Self::Commit,
            _ => Self::Reject,
        }
    }

    pub const fn initial() -> Self {
        Self::NewHeight
    }

    pub const fn is_commit(&self) -> bool {
        matches!(self, Self::Commit)
    }

    pub const fn is_initial(&self) -> bool {
        const INITIAL: State = State::initial();

        matches!(self, &INITIAL)
    }

    pub const fn is_propose(&self) -> bool {
        matches!(self, Self::Propose)
    }

    pub const fn is_reject(&self) -> bool {
        matches!(self, Self::Reject)
    }

    /// Increment the current step to the next one of the consensus flow.
    pub const fn increment(self) -> Option<Self> {
        match self {
            Self::Reject => None,
            Self::NewHeight => Some(Self::Propose),
            Self::Propose => Some(Self::Prevote),
            Self::Prevote => Some(Self::Precommit),
            Self::Precommit => Some(Self::Commit),
            Self::Commit => None,
        }
    }
}

impl Iterator for State {
    type Item = State;

    fn next(&mut self) -> Option<State> {
        self.increment().map(|s| *self = s).map(|_| *self)
    }
}
