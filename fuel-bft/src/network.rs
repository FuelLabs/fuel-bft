use crate::{Block, HeightRound, Message};

use fuel_crypto::PublicKey;

pub trait Network {
    type Error;
    type Message: Message;

    fn broadcast(&mut self, message: &Self::Message) -> Result<(), Self::Error>;

    /// Generate the block payload to allow the creation of a new block.
    fn block_payload(
        &self,
        round: &HeightRound,
    ) -> Result<<<Self::Message as Message>::Block as Block>::Payload, Self::Error>;

    fn generate_block(
        &self,
        round: &HeightRound,
        owner: PublicKey,
    ) -> Result<<Self::Message as Message>::Block, Self::Error> {
        #[cfg(feature = "trace")]
        tracing::debug!(
            "{:04x} owner; creating new block for round {}",
            owner,
            round
        );

        let payload = self.block_payload(round)?;
        let block = <Self::Message as Message>::Block::new(owner, payload);

        Ok(block)
    }

    // Will offer the option to the user to override the increment implementation
    fn increment_height(round: HeightRound) -> HeightRound {
        round.increment_height()
    }

    fn increment_round(round: HeightRound) -> HeightRound {
        round.increment_round()
    }
}
