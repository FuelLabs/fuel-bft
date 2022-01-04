use fuel_pbft::*;

use elliptic_curve::group::prime::PrimeCurveAffine;
use k256::ecdsa::signature::{Signer, Verifier};
use k256::ecdsa::{Signature, SigningKey, VerifyingKey};
use k256::AffinePoint;
use rand::rngs::StdRng;
use rand::SeedableRng;
use sha2::{Digest, Sha256};

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Clone, PartialEq, Eq)]
pub struct MockKey(SigningKey);

impl fmt::Debug for MockKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MockKey")
            .field("key", &hex::encode(self.0.to_bytes()))
            .finish()
    }
}

impl MockKey {
    pub fn new<P>(password: P) -> Self
    where
        P: AsRef<[u8]>,
    {
        let mut b = [0u8; 32];

        Sha256::new()
            .chain_update(password)
            .finalize_into((&mut b).into());

        let rng = &mut StdRng::from_seed(b);
        let key = SigningKey::random(rng);

        Self(key)
    }

    pub fn sign<D>(&self, digest: D) -> MockSignature
    where
        D: Digest,
    {
        let signature = self.0.sign(&digest.finalize());

        MockSignature(signature)
    }
}

impl Key for MockKey {
    type ValidatorId = MockValidatorId;

    fn validator(&self) -> Self::ValidatorId {
        MockValidatorId(self.0.verifying_key())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MockSignature(Signature);

impl fmt::Debug for MockSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MockSignature")
            .field("sig", &hex::encode(self.0.as_ref()))
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MockValidatorId(VerifyingKey);

impl ValidatorId for MockValidatorId {}

impl Default for MockValidatorId {
    fn default() -> Self {
        let g = &AffinePoint::generator();

        Self(g.into())
    }
}

impl fmt::Debug for MockValidatorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MockValidatorId")
            .field("public", &hex::encode(self.0.to_bytes()))
            .finish()
    }
}

impl Hash for MockValidatorId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bytes().hash(state);
    }
}

impl MockValidatorId {
    pub fn verify<D>(&self, signature: &MockSignature, digest: D) -> bool
    where
        D: Digest,
    {
        self.0.verify(&digest.finalize(), &signature.0).is_ok()
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
    owner: MockValidatorId,
    payload: BlockPayload,
}

impl MockBlock {
    fn digest(&self) -> impl Digest {
        Sha256::new()
            .chain_update(self.owner.0.to_bytes())
            .chain_update(&[self.payload.is_valid as u8])
    }
}

impl Block for MockBlock {
    type Payload = BlockPayload;
    type ValidatorId = MockValidatorId;

    fn new(owner: MockValidatorId, payload: BlockPayload) -> Self {
        Self { owner, payload }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MockMessage {
    block: MockBlock,
    round: u64,
    signature: MockSignature,
    state: State,
    validator: MockValidatorId,
}

impl MockMessage {
    fn digest(round: u64, state: State, block: &MockBlock) -> impl Digest {
        block
            .digest()
            .chain_update(round.to_be_bytes())
            .chain_update(&[state as u8])
    }
}

impl Message for MockMessage {
    type Block = MockBlock;
    type Key = MockKey;
    type Round = u64;
    type ValidatorId = MockValidatorId;

    fn new(round: u64, key: Self::Key, state: State, block: Self::Block) -> Self {
        let digest = Self::digest(round, state, &block);
        let validator = key.validator();
        let signature = key.sign(digest);

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

        self.validator.verify(&self.signature, digest)
    }
}

#[derive(Debug, Default, Clone)]
pub struct MockNetwork {
    /// Set of validators mapping to a round range
    validators: HashMap<MockValidatorId, (u64, u64, MockNode)>,
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

    pub fn validators(&self, round: u64) -> impl Iterator<Item = &MockValidatorId> {
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
    type ValidatorId = MockValidatorId;

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
    validator_state: HashMap<(u64, MockValidatorId), State>,
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
    type ValidatorId = MockValidatorId;

    fn id(&self) -> Self::ValidatorId {
        self.key.validator()
    }

    fn key(&self) -> Self::Key {
        self.key.clone()
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
