use fuel_bft::Error;
use fuel_crypto::borrown::Borrown;
use fuel_crypto::{Hasher, Keystore, PublicKey, SecretKey, Signer};
use fuel_types::Bytes32;
use rand::rngs::StdRng;
use rand::SeedableRng;

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, RwLock};

#[derive(Debug, Default, Clone)]
struct InnerKeystore {
    secret: HashMap<Bytes32, SecretKey>,
    public: HashMap<Bytes32, PublicKey>,
}

impl InnerKeystore {
    fn add_secret(&mut self, id: Bytes32, secret: SecretKey) {
        self.add_public(id, secret.public_key());

        self.secret.insert(id, secret);
    }

    fn add_public(&mut self, id: Bytes32, public: PublicKey) {
        self.public.insert(id, public);
    }
}

impl Keystore for InnerKeystore {
    type Error = Infallible;
    type KeyId = Bytes32;

    fn secret(&self, id: &Self::KeyId) -> Result<Option<Borrown<'_, SecretKey>>, Self::Error> {
        Ok(self.secret.get(id).map(|s| (*s).into()))
    }
}

#[derive(Debug, Default, Clone)]
pub struct MockKeystore {
    store: Arc<RwLock<InnerKeystore>>,
}

impl MockKeystore {
    pub fn add_secret<P>(&self, password: P) -> Result<Bytes32, Error>
    where
        P: AsRef<[u8]>,
    {
        let seed = Self::id(password);
        let rng = &mut StdRng::from_seed(*seed);

        let secret = SecretKey::random(rng);

        self.store
            .write()
            .map_err(|_| fuel_crypto::Error::KeystoreNotAvailable)
            .map(|mut store| store.add_secret(seed, secret))?;

        Ok(seed)
    }

    pub fn add_public(&self, id: Bytes32, public: PublicKey) -> Result<(), Error> {
        self.store
            .write()
            .map_err(|_| fuel_crypto::Error::KeystoreNotAvailable)
            .map(|mut store| store.add_public(id, public))?;

        Ok(())
    }

    pub fn id<P>(password: P) -> Bytes32
    where
        P: AsRef<[u8]>,
    {
        Hasher::hash(password)
    }

    pub fn keystore_public<P>(&self, password: P) -> Result<Option<Borrown<'_, PublicKey>>, Error>
    where
        P: AsRef<[u8]>,
    {
        let id = Self::id(password);

        self.public(&id)
    }

    pub fn keystore_secret<P>(&self, password: P) -> Result<Option<Borrown<'_, SecretKey>>, Error>
    where
        P: AsRef<[u8]>,
    {
        let id = Self::id(password);

        self.secret(&id)
    }
}

impl Keystore for MockKeystore {
    type Error = Error;
    type KeyId = Bytes32;

    fn secret(&self, id: &Self::KeyId) -> Result<Option<Borrown<'_, SecretKey>>, Self::Error> {
        let secret = self
            .store
            .read()
            .map_err(|_| fuel_crypto::Error::KeystoreNotAvailable)
            .and_then(|store| Ok(store.secret(id)?.map(|k| k.into_owned())))?
            .map(|secret| Borrown::Owned(secret));

        Ok(secret)
    }
}

impl Signer for MockKeystore {
    type Error = Error;
    type Keystore = Self;

    fn keystore(&self) -> Result<&Self::Keystore, Self::Error> {
        Ok(&self)
    }
}
