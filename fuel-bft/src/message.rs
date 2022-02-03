use crate::{Block, HeightRound, RoundValidator, State};

use fuel_crypto::{Keystore, Message as CryptoMessage, PublicKey, Signature, Signer};
use fuel_types::Bytes32;

pub trait Message: Sized {
    type Block: Block;
    type Error: From<<Self::Signer as Signer>::Error>;
    type Signer: Signer;

    fn author(&self) -> &RoundValidator;
    fn block(&self) -> &Self::Block;
    fn hash(&self) -> Bytes32;
    fn set_signature(&mut self, author: PublicKey, signature: Signature);
    fn signature(&self) -> &Signature;
    fn state(&self) -> State;
    fn unsigned(round: HeightRound, state: State, block: Self::Block) -> Self;

    fn author_key(&self) -> &PublicKey {
        self.author().validator()
    }

    fn round(&self) -> &HeightRound {
        self.author().round()
    }

    fn to_signature_message(&self) -> CryptoMessage {
        let hash = self.hash();

        // Safety: cryptographically secure hash used
        unsafe { CryptoMessage::from_bytes_unchecked(*hash) }
    }

    fn signed(
        signer: &Self::Signer,
        key: &<<Self::Signer as Signer>::Keystore as Keystore>::KeyId,
        round: HeightRound,
        state: State,
        block: Self::Block,
    ) -> Result<Self, Self::Error> {
        let mut message = Self::unsigned(round, state, block);

        let signature = signer.sign(key, &message.to_signature_message())?;
        let public = signer.id_public(key)?;

        message.set_signature(public.into_owned(), signature);

        Ok(message)
    }
}
