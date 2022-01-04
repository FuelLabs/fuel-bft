use crate::{Block, Message, Round, ValidatorId};

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

    pub const fn is_inconclusive(&self) -> bool {
        matches!(self, Self::Inconclusive)
    }
}

pub trait Network {
    type Block: Block<Payload = Self::Payload>;
    type Message: Message<ValidatorId = Self::ValidatorId>;
    type Payload;
    type Round: Round;
    type ValidatorId: ValidatorId;

    fn broadcast(&mut self, message: &Self::Message);
    fn increment_round(round: Self::Round) -> Self::Round;
    fn is_validator(&self, round: Self::Round, validator: &Self::ValidatorId) -> bool;
    fn validators(&self, round: Self::Round) -> usize;
    fn proposer(&self, round: Self::Round) -> Option<&Self::ValidatorId>;

    /// Generate the block payload to allow the creation of a new block.
    fn block_payload(&self) -> Self::Payload;

    /// From a round and a count of positive voters, resolves the consensus state.
    ///
    /// Will not evaluate negative voters because this is handled by the protocol timeout.
    fn consensus(&self, round: Self::Round, count: usize) -> Consensus {
        let validators = self.validators(round);

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
