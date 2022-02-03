/// Evaluation of the outcome of a mutation produced by a peer message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Consensus {
    /// The votes weren't enough to produce a consensus
    Inconclusive = 0x00,
    /// The votes achieved BFT consensus
    Consensus = 0x01,
    /// The number of validators in the round are not enough to achieve consensus
    Reject = 0x02,
}

impl Consensus {
    /// Minimum amount of validators for a BFT consensus
    pub const MINIMUM: usize = 4;

    /// Check if the validators count meet the criteria for a BFT consensus
    pub const fn is_bft(validators: usize) -> bool {
        validators >= Self::MINIMUM
    }

    /// Check if the result is a consensus
    pub const fn is_consensus(&self) -> bool {
        matches!(self, Self::Consensus)
    }

    /// Given the number of validators and computed approvals, evaluate the consensus outcome
    pub const fn evaluate(validators: usize, approvals: usize) -> Self {
        let minimum = Self::is_bft(validators);
        let consensus = validators * 2 / 3;

        if !minimum {
            Consensus::Reject
        } else if approvals > consensus {
            Consensus::Consensus
        } else {
            Consensus::Inconclusive
        }
    }
}

#[test]
fn evaluate() {
    assert!(!Consensus::is_bft(3));
    assert!(Consensus::is_bft(4));

    assert!(!Consensus::evaluate(3, 3).is_consensus());
    assert!(!Consensus::evaluate(4, 2).is_consensus());
    assert!(Consensus::evaluate(4, 3).is_consensus());
}
