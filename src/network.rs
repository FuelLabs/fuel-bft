use crate::{Block, Message, PeerId};

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
}

pub trait Network {
    type Block: Block<Payload = Self::Payload>;
    type Message: Message<PeerId = Self::PeerId>;
    type Payload;
    type PeerId: PeerId;

    fn broadcast(&self, message: &Self::Message);
    fn increment_height(height: u64) -> u64;
    fn is_council(&self, height: u64, peer: &Self::PeerId) -> bool;
    fn peers(&self, height: u64) -> usize;
    fn proposer(&self, height: u64) -> &Self::PeerId;

    /// Generate the block payload to allow the creation of a new block.
    fn block_payload(&self) -> Self::Payload;

    /// From a height and a count of positive voters, resolves the consensus state.
    ///
    /// Will not evaluate negative voters because this is handled by the protocol timeout.
    fn consensus(&self, height: u64, count: usize) -> Consensus {
        let peers = self.peers(height);

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
