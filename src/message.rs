use crate::{Block, Key, PeerId, Round, State};

pub trait Message {
    type Block: Block;
    type Key: Key;
    type PeerId: PeerId;
    type Round: Round;

    fn new(round: Self::Round, key: Self::Key, state: State, block: Self::Block) -> Self;

    fn block(&self) -> &Self::Block;
    fn owned_block(&self) -> Self::Block;
    fn round(&self) -> Self::Round;
    fn state(&self) -> State;

    /// Author of the message
    fn peer(&self) -> &Self::PeerId;

    /// Validate the signature of the message.
    ///
    /// The suggested structure is to have a field for the signature, a field for the peer id,
    /// function to hash the message - excluding the signature - into a digest, and check the
    /// signature using a protocol that maps [`Self::Key`] into [`Self::PeerId`] (e.g. ECC) and
    /// verify the signature against the peer id and the digest.
    fn is_valid(&self) -> bool;
}
