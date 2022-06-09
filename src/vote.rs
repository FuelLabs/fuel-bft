use crate::{Error, Height, Keychain, Round, Step};

use fuel_crypto::{Hasher, PublicKey, SecretKey, Signature};
use fuel_types::Bytes32;

/// A vote from a validator.
///
/// These votes are consumed to produce state change in the reactor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Vote {
    block_id: Bytes32,
    height: Height,
    round: Round,
    signature: Signature,
    step: Step,
    validator: PublicKey,
}

impl Vote {
    /// Create a new vote from a given signature
    pub const fn new(
        validator: PublicKey,
        signature: Signature,
        height: Height,
        round: Round,
        block_id: Bytes32,
        step: Step,
    ) -> Self {
        Self {
            block_id,
            height,
            round,
            signature,
            step,
            validator,
        }
    }

    fn _digest(h: Hasher, height: Height, round: Round, block_id: &Bytes32, step: Step) -> Hasher {
        h.chain(height.to_be_bytes())
            .chain(round.to_be_bytes())
            .chain(block_id)
            .chain(&[step as u8])
    }

    /// Compute the digest of the vote. Will be used by the signature
    pub fn digest(&self, h: Hasher) -> Hasher {
        Self::_digest(h, self.height, self.round, &self.block_id, self.step)
    }

    /// Block Id of the step
    pub const fn block_id(&self) -> &Bytes32 {
        &self.block_id
    }

    /// Target block height.
    pub const fn height(&self) -> Height {
        self.height
    }

    /// Target height round.
    pub const fn round(&self) -> Round {
        self.round
    }

    /// Signature provided by the owner of the vote
    pub const fn signature(&self) -> &Signature {
        &self.signature
    }

    /// Proposed step
    pub const fn step(&self) -> Step {
        self.step
    }

    /// Network identification of the author
    pub const fn validator(&self) -> &PublicKey {
        &self.validator
    }

    /// Produce a guaranteed correctness signed vote
    pub fn signed<K>(
        keychain: &K,
        height: Height,
        round: Round,
        block_id: Bytes32,
        step: Step,
    ) -> Result<Self, Error>
    where
        K: Keychain,
    {
        let digest = Self::_digest(Hasher::default(), height, round, &block_id, step);
        let signature = keychain
            .sign(height, digest)
            .map_err(|_| Error::ResourceNotAvailable)?;

        let validator = keychain
            .public(height)
            .map_err(|_| Error::ResourceNotAvailable)?
            .ok_or(Error::NotRoundValidator)?
            .into_owned();

        let vote = Self::new(validator, signature, height, round, block_id, step);

        Ok(vote)
    }

    /// Produce a guaranteed correctness signed vote
    pub fn signed_with_key<K>(
        secret: &SecretKey,
        height: Height,
        round: Round,
        block_id: Bytes32,
        step: Step,
    ) -> Self
    where
        K: Keychain,
    {
        let digest = Self::_digest(Hasher::default(), height, round, &block_id, step);
        let validator = K::public_with_key(secret);
        let signature = K::sign_with_key(secret, digest);

        Self::new(validator, signature, height, round, block_id, step)
    }

    /// Validate the signature of the vote
    pub fn validate<K>(&self) -> Result<(), Error>
    where
        K: Keychain,
    {
        let digest = self.digest(Hasher::default());

        K::verify(self.signature, &self.validator, digest).map_err(|_| Error::InvalidSignature)
    }
}
