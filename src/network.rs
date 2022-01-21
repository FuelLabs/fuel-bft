use crate::{Block, HeightRound, Message, ValidatorId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Consensus {
    Inconclusive = 0x00,
    Consensus = 0x01,
    Reject = 0x02,
}

impl Consensus {
    pub const fn from_u8(byte: u8) -> Self {
        match byte {
            0x00 => Self::Inconclusive,
            0x01 => Self::Consensus,
            _ => Self::Reject,
        }
    }

    pub const fn is_consensus(&self) -> bool {
        matches!(self, Self::Consensus)
    }

    pub const fn is_inconclusive(&self) -> bool {
        matches!(self, Self::Inconclusive)
    }
}

pub trait Network {
    type Block: Block<Payload = Self::Payload>;
    type Message: Message<ValidatorId = Self::ValidatorId>;
    type Payload;
    type ValidatorId: ValidatorId;
    type ValidatorIdRef: AsRef<Self::ValidatorId>;
    type ValidatorIdSorted: Iterator<Item = Self::ValidatorIdRef>;

    fn broadcast(&mut self, message: &Self::Message);
    fn validators_sorted(&self, round: &HeightRound) -> Self::ValidatorIdSorted;

    /// Generate the block payload to allow the creation of a new block.
    fn block_payload(&self) -> Self::Payload;

    fn leader(&self, round: &HeightRound) -> Option<Self::ValidatorIdRef> {
        let count = self.validators_count(round) as u64;

        (count > 0)
            .then(|| {
                let h = round.height();
                let r = round.round();

                let index = (h + r) % count;

                self.validators_sorted(round).skip(index as usize).next()
            })
            .flatten()
    }

    fn is_validator(&self, round: &HeightRound, validator: &Self::ValidatorId) -> bool {
        self.validators_sorted(round)
            .any(|p| p.as_ref() == validator)
    }

    fn validators_count(&self, round: &HeightRound) -> usize {
        self.validators_sorted(round).count()
    }

    fn increment_height(round: HeightRound) -> HeightRound {
        round.increment_height()
    }

    fn increment_round(round: HeightRound) -> HeightRound {
        round.increment_round()
    }

    /// From a round and a count of positive voters, resolves the consensus state.
    ///
    /// Will not evaluate negative voters because this is handled by the protocol timeout.
    fn consensus(&self, round: &HeightRound, count: usize) -> Consensus {
        let validators = self.validators_count(round);

        let minimum = validators > 3;
        let consensus = validators * 2 / 3;

        if !minimum {
            Consensus::Reject
        } else if count > consensus {
            Consensus::Consensus
        } else {
            Consensus::Inconclusive
        }
    }
}
