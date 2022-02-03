use crate::{Height, Keychain};

use fuel_crypto::borrown::Borrown;
use fuel_crypto::{Hasher, Keystore, SecretKey, Signer};
use rand::rngs::StdRng;
use rand::SeedableRng;

use core::convert::Infallible;
use core::ops::{Range, RangeBounds};
use std::collections::HashMap;

/// Default in-memory implementation of a keychain
#[derive(Debug, Default, Clone)]
pub struct MemoryKeychain {
    keys: HashMap<Range<Height>, SecretKey>,
}

impl MemoryKeychain {
    /// Add a new password generated secret to the keychain
    pub fn insert<H, P>(&mut self, _height: H, password: P)
    where
        H: RangeBounds<Height>,
        P: AsRef<[u8]>,
    {
        let seed = Hasher::hash(password);
        let rng = &mut StdRng::from_seed(*seed);
        let secret = SecretKey::random(rng);

        // TODO implement range split?
        self.keys.insert(
            Range {
                start: Height::MIN,
                end: Height::MAX,
            },
            secret,
        );
    }
}

impl Keystore for MemoryKeychain {
    type Error = Infallible;
    type KeyId = Height;

    fn secret(&self, id: &Height) -> Result<Option<Borrown<'_, SecretKey>>, Self::Error> {
        self.keys
            .iter()
            .find_map(|(range, key)| range.contains(id).then(|| Ok(key.into())))
            .transpose()
    }
}

impl Signer for MemoryKeychain {
    type Error = fuel_crypto::Error;
    type Keystore = Self;

    fn keystore(&self) -> Result<&Self::Keystore, Self::Error> {
        Ok(self)
    }
}

impl Keychain for MemoryKeychain {
    type Error = <Self::Signer as Signer>::Error;
    type Signer = Self;
    type Keystore = Self;

    fn signer(&self) -> &Self::Signer {
        self
    }
}
