use fuel_pbft::*;

use curve25519_dalek::constants;
use curve25519_dalek::ristretto::RistrettoPoint;
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::traits::Identity;
use sha2::{Digest, Sha512};

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct MockKey(Scalar);

impl fmt::Debug for MockKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MockKey")
            .field("key", &hex::encode(self.0.as_bytes()))
            .finish()
    }
}

impl MockKey {
    pub fn new<P>(password: P) -> Self
    where
        P: AsRef<[u8]>,
    {
        let h = Sha512::new().chain(password);
        let s = Scalar::from_hash(h);

        Self(s)
    }

    pub fn sign(&self, challenge: &Scalar, message: &[u8]) -> Signature {
        let c = challenge * &constants::RISTRETTO_BASEPOINT_TABLE;
        let h = Sha512::new().chain(c.compress().as_bytes()).chain(message);
        let e = Scalar::from_hash(h);

        let s = challenge - self.0 * e;

        Signature { s, e }
    }
}

impl Key for MockKey {
    type ValidatorId = MockValidator;

    fn validator(&self) -> Self::ValidatorId {
        MockValidator(&self.0 * &constants::RISTRETTO_BASEPOINT_TABLE)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Signature {
    s: Scalar,
    e: Scalar,
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Signature")
            .field("signature", &"(?)")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MockValidator(RistrettoPoint);

impl fmt::Debug for MockValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MockKey")
            .field("key", &hex::encode(self.0.compress().as_bytes()))
            .finish()
    }
}

impl MockValidator {
    pub fn verify(&self, signature: &Signature, message: &[u8]) -> bool {
        let c = RistrettoPoint::vartime_double_scalar_mul_basepoint(
            &signature.e,
            &self.0,
            &signature.s,
        );
        let h = Sha512::new().chain(c.compress().as_bytes()).chain(message);
        let e = Scalar::from_hash(h);

        signature.e == e
    }
}

impl Hash for MockValidator {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.compress().as_bytes().hash(state);
    }
}

impl ValidatorId for MockValidator {}

impl Default for MockValidator {
    fn default() -> MockValidator {
        MockValidator(RistrettoPoint::identity())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPayload {
    is_valid: bool,
}

impl Default for BlockPayload {
    fn default() -> Self {
        Self { is_valid: true }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MockBlock {
    owner: MockValidator,
    payload: BlockPayload,
}

impl MockBlock {
    fn digest(&self) -> Sha512 {
        Sha512::new()
            .chain(self.owner.0.compress().as_bytes())
            .chain(&[self.payload.is_valid as u8])
    }
}

impl Block for MockBlock {
    type Payload = BlockPayload;
    type ValidatorId = MockValidator;

    fn new(owner: MockValidator, payload: BlockPayload) -> Self {
        Self { owner, payload }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MockMessage {
    block: MockBlock,
    round: u64,
    signature: Signature,
    state: State,
    validator: MockValidator,
}

impl MockMessage {
    fn digest(round: u64, state: State, block: &MockBlock) -> Sha512 {
        block
            .digest()
            .chain(round.to_be_bytes())
            .chain(&[state as u8])
    }
}

impl Message for MockMessage {
    type Block = MockBlock;
    type Key = MockKey;
    type Round = u64;
    type ValidatorId = MockValidator;

    fn new(round: u64, key: Self::Key, state: State, block: Self::Block) -> Self {
        let digest = Self::digest(round, state, &block);
        let validator = key.validator();

        // Insecure implementation for test purposes. Don't do this in production.
        let challenge = Scalar::from(round);
        let signature = key.sign(&challenge, &digest.finalize());

        Self {
            block,
            round,
            signature,
            state,
            validator,
        }
    }

    fn block(&self) -> &Self::Block {
        &self.block
    }

    fn owned_block(&self) -> Self::Block {
        self.block.clone()
    }

    fn round(&self) -> u64 {
        self.round
    }

    fn state(&self) -> State {
        self.state
    }

    fn validator(&self) -> &Self::ValidatorId {
        &self.validator
    }

    fn is_valid(&self) -> bool {
        let digest = Self::digest(self.round, self.state, &self.block);

        self.validator.verify(&self.signature, &digest.finalize())
    }
}

#[derive(Debug, Default, Clone)]
pub struct MockNetwork {
    /// Set of validators mapping to a round range
    validators: HashMap<MockValidator, (u64, u64, MockNode)>,
}

impl MockNetwork {
    pub fn key_from_round(round: u64) -> MockKey {
        MockKey::new(format!("round {}", round))
    }

    pub fn add_node(&mut self, round: u64, from: u64, validity: u64) {
        let to = from + validity;

        let key = Self::key_from_round(round);
        let validator = key.validator();
        let node = MockNode::new(key);

        self.validators.insert(validator, (from, to, node));
    }

    pub fn node(&self, round: u64) -> Option<&MockNode> {
        let key = Self::key_from_round(round);
        let validator = key.validator();

        self.validators.get(&validator).map(|(_, _, n)| n)
    }

    pub fn validators(&self, round: u64) -> impl Iterator<Item = &MockValidator> {
        self.validators
            .iter()
            .filter_map(move |(validator, (from, to, _))| {
                (*from <= round && round < *to).then(|| validator)
            })
    }
}

impl Network for MockNetwork {
    type Block = MockBlock;
    type Message = MockMessage;
    type Payload = BlockPayload;
    type Round = u64;
    type ValidatorId = MockValidator;

    fn broadcast(&mut self, message: &Self::Message) {
        let round = message.round();

        let network = self as *mut MockNetwork;

        self.validators
            .iter_mut()
            .filter_map(|(_, (from, to, node))| (*from <= round && round < *to).then(|| node))
            .for_each(|node| node.receive_message(unsafe { network.as_mut().unwrap() }, message));
    }

    fn increment_round(round: u64) -> u64 {
        round + 1
    }

    fn is_validator(&self, round: u64, validator: &Self::ValidatorId) -> bool {
        self.validators(round).any(|p| p == validator)
    }

    fn validators(&self, round: u64) -> usize {
        self.validators(round).count()
    }

    fn proposer(&self, round: u64) -> Option<&Self::ValidatorId> {
        let validator = Self::key_from_round(round).validator();

        self.validators(round).find(|p| p == &&validator)
    }

    fn block_payload(&self) -> Self::Payload {
        BlockPayload { is_valid: true }
    }
}

#[derive(Debug, Clone)]
pub struct MockNode {
    key: MockKey,
    validator_state: HashMap<(u64, MockValidator), State>,
}

impl MockNode {
    pub fn new(key: MockKey) -> Self {
        Self {
            key,
            validator_state: Default::default(),
        }
    }
}

impl Node for MockNode {
    type Block = MockBlock;
    type Key = MockKey;
    type Message = MockMessage;
    type Network = MockNetwork;
    type Payload = BlockPayload;
    type Round = u64;
    type ValidatorId = MockValidator;

    fn id(&self) -> Self::ValidatorId {
        self.key.validator()
    }

    fn key(&self) -> Self::Key {
        self.key
    }

    fn validator_state(&self, round: u64, validator: &Self::ValidatorId) -> Option<State> {
        self.validator_state.get(&(round, *validator)).copied()
    }

    fn set_validator_state(&mut self, round: u64, validator: &Self::ValidatorId, state: State) {
        self.validator_state.insert((round, *validator), state);
    }

    fn state_count(&self, round: u64, state: State) -> usize {
        self.validator_state
            .iter()
            .filter(|((h, _), s)| h == &round && s == &&state)
            .count()
    }

    fn validate_block(&self, block: &Self::Block) -> bool {
        block.payload.is_valid
    }
}
