use crate::Height;

use fuel_crypto::borrown::Borrown;
use fuel_crypto::{Hasher, Keystore, PublicKey, SecretKey, Signature, Signer};

#[cfg(feature = "memory")]
pub mod memory;

/// Keychain provider for the protocol.
pub trait Keychain {
    /// Concrete error type
    type Error: From<fuel_crypto::Error>
        + From<<Self::Signer as Signer>::Error>
        + From<<<Self::Signer as Signer>::Keystore as Keystore>::Error>;

    /// Signature provider
    type Signer: Signer<Keystore = Self::Keystore>;

    /// Keys provider
    type Keystore: fuel_crypto::Keystore<KeyId = Height>;

    /// Underlying signature provider
    fn signer(&self) -> &Self::Signer;

    /// The node is a validator if the keystore provide a public key for the requested height
    fn is_validator_for(&self, height: Height) -> Result<bool, Self::Error> {
        self.public(height).map(|p| p.is_some())
    }

    /// Fetch the public key of the node for the given round
    fn public(&self, height: Height) -> Result<Option<Borrown<'_, PublicKey>>, Self::Error> {
        let public = self.signer().keystore()?.public(&height)?;

        Ok(public)
    }

    /// Generate a public key from a secret key
    #[cfg(not(feature = "std"))]
    fn public_with_key(secret: &SecretKey) -> PublicKey;

    /// Generate a public key from a secret key
    #[cfg(feature = "std")]
    fn public_with_key(secret: &SecretKey) -> PublicKey {
        secret.public_key()
    }

    /// Sign the result of a given digest
    fn sign(&self, height: Height, digest: Hasher) -> Result<Signature, Self::Error> {
        let normalized = fuel_crypto::Message::from(digest);
        let signature = self.signer().sign(&height, &normalized)?;

        Ok(signature)
    }

    /// Sign the result of a given digest with a provided key
    #[cfg(not(feature = "std"))]
    fn sign_with_key(secret: &SecretKey, digest: Hasher) -> Signature;

    /// Sign the result of a given digest with a provided key
    #[cfg(feature = "std")]
    fn sign_with_key(secret: &SecretKey, digest: Hasher) -> Signature {
        let normalized = fuel_crypto::Message::from(digest);

        Signature::sign(secret, &normalized)
    }

    /// Verify the signature against the result of a given digest
    #[cfg(not(feature = "std"))]
    fn verify(signature: Signature, author: &PublicKey, digest: Hasher) -> Result<(), Self::Error>;

    /// Verify the signature against the result of a given digest
    #[cfg(feature = "std")]
    fn verify(signature: Signature, author: &PublicKey, digest: Hasher) -> Result<(), Self::Error> {
        let normalized = fuel_crypto::Message::from(digest);

        signature.verify(author, &normalized)?;

        Ok(())
    }
}
