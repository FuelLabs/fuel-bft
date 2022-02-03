use crate::{MockBlock, MockKeystore};

use fuel_bft::{Error, HeightRound, Message, RoundValidator, State};
use fuel_crypto::{PublicKey, Signature};
use fuel_types::Bytes32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MockMessage {
    author: RoundValidator,
    block: MockBlock,
    round: HeightRound,
    signature: Signature,
    state: State,
}

impl Message for MockMessage {
    type Block = MockBlock;
    type Error = Error;
    type Signer = MockKeystore;

    fn author(&self) -> &RoundValidator {
        &self.author
    }

    fn block(&self) -> &Self::Block {
        &self.block
    }

    fn hash(&self) -> Bytes32 {
        self.block
            .digest()
            .chain(self.round.height().to_le_bytes())
            .chain(self.round.round().to_le_bytes())
            .chain([self.state as u8])
            .finalize()
    }

    fn set_signature(&mut self, author: PublicKey, signature: Signature) {
        self.author.set_validator(author);
        self.signature = signature;
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn state(&self) -> State {
        self.state
    }

    fn unsigned(round: HeightRound, state: State, block: Self::Block) -> Self {
        let author = RoundValidator::from(round);
        let signature = Default::default();

        Self {
            author,
            block,
            round,
            signature,
            state,
        }
    }
}
