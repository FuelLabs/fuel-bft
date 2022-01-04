use crate::{Block, Message, PeerId, Round};

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
    type Message: Message<PeerId = Self::PeerId>;
    type Payload;
    type PeerId: PeerId;
    type Round: Round;

    fn broadcast(&mut self, message: &Self::Message);
    fn increment_round(round: Self::Round) -> Self::Round;
    fn is_council(&self, round: Self::Round, peer: &Self::PeerId) -> bool;
    fn peers(&self, round: Self::Round) -> usize;
    fn proposer(&self, round: Self::Round) -> Option<&Self::PeerId>;

    /// Generate the block payload to allow the creation of a new block.
    fn block_payload(&self) -> Self::Payload;

    /// From a round and a count of positive voters, resolves the consensus state.
    ///
    /// Will not evaluate negative voters because this is handled by the protocol timeout.
    fn consensus(&self, round: Self::Round, count: usize) -> Consensus {
        let peers = self.peers(round);

        let minimum = peers > 3;
        let consensus = peers * 2 / 3;

        if !minimum {
            Consensus::Reject
        } else if count > consensus {
            Consensus::Consensus
        } else {
            Consensus::Inconclusive
        }
    }
}
