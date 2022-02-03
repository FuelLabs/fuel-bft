use fuel_bft::Block;
use fuel_crypto::{Hasher, PublicKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MockBlock {
    owner: PublicKey,
    payload: bool,
}

impl MockBlock {
    pub fn digest(&self) -> Hasher {
        Hasher::default()
            .chain(self.owner.as_ref())
            .chain(&[self.payload as u8])
    }

    pub const fn is_valid(&self) -> bool {
        self.payload
    }
}

impl Default for MockBlock {
    fn default() -> Self {
        Self {
            owner: PublicKey::default(),
            payload: true,
        }
    }
}

impl Block for MockBlock {
    type Payload = bool;

    fn new(owner: PublicKey, payload: bool) -> Self {
        Self { owner, payload }
    }
}
