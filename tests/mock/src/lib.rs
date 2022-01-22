use fuel_pbft::*;

use elliptic_curve::group::prime::PrimeCurveAffine;
use k256::ecdsa::signature::{Signer, Verifier};
use k256::ecdsa::{Signature, SigningKey, VerifyingKey};
use k256::AffinePoint;
use rand::rngs::StdRng;
use rand::SeedableRng;
use sha2::{Digest, Sha256};

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::vec::IntoIter;

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

impl From<u64> for MockKey {
    fn from(seed: u64) -> Self {
        Self::new(seed.to_be_bytes())
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    round: HeightRound,
    signature: MockSignature,
    state: State,
    validator: MockValidatorId,
}

impl MockMessage {
    fn digest(round: &HeightRound, state: State, block: &MockBlock) -> impl Digest {
        block
            .digest()
            .chain_update(round.height().to_be_bytes())
            .chain_update(round.round().to_be_bytes())
            .chain_update(&[state as u8])
    }
}

impl Message for MockMessage {
    type Block = MockBlock;
    type Key = MockKey;
    type ValidatorId = MockValidatorId;

    fn new(round: HeightRound, key: Self::Key, state: State, block: Self::Block) -> Self {
        let digest = Self::digest(&round, state, &block);
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

    fn round(&self) -> &HeightRound {
        &self.round
    }

    fn state(&self) -> State {
        self.state
    }

    fn validator(&self) -> &Self::ValidatorId {
        &self.validator
    }

    fn is_valid(&self) -> bool {
        let digest = Self::digest(&self.round, self.state, &self.block);

        self.validator.verify(&self.signature, digest)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MockRoundId {
    round: HeightRound,
    id: MockValidatorId,
}

impl MockRoundId {
    pub const fn new(round: HeightRound, id: MockValidatorId) -> Self {
        Self { round, id }
    }
}

#[derive(Debug, Clone)]
pub struct MockNode {
    key: MockKey,
    id: MockValidatorId,
    start: HeightRound,
    validity: HeightRound,
    validator_state: HashMap<MockRoundId, State>,
}

impl MockNode {
    pub fn new(key: MockKey, start: HeightRound, validity: u64) -> Self {
        let id = key.validator();

        let validity = (0..validity).fold(start, |r, _| MockNetwork::increment_height(r));
        let validator_state = Default::default();

        Self {
            key,
            id,
            start,
            validity,
            validator_state,
        }
    }

    pub const fn id(&self) -> &MockValidatorId {
        &self.id
    }

    pub const fn start(&self) -> &HeightRound {
        &self.start
    }

    pub const fn validity(&self) -> &HeightRound {
        &self.validity
    }
}

#[derive(Debug, Default, Clone)]
pub struct MockNetwork {
    /// Set of validators mapping to a round range
    validators: HashMap<MockValidatorId, MockNode>,
}

impl MockNetwork {
    pub fn add_validator(&mut self, node: MockNode) {
        let id = *node.id();

        self.validators.insert(id, node);
    }

    pub fn validator(&self, seed: u64) -> Option<&MockNode> {
        let key = MockKey::from(seed);
        let id = key.validator();

        self.validators.get(&id)
    }

    pub fn validator_mut(&mut self, seed: u64) -> Option<&mut MockNode> {
        let key = MockKey::from(seed);
        let id = key.validator();

        self.validators.get_mut(&id)
    }

    pub fn validators(&self, round: &HeightRound) -> impl Iterator<Item = &MockValidatorId> {
        let round = *round;

        self.validators.iter().filter_map(move |(id, node)| {
            (node.start() <= &round && &round < node.validity()).then(|| id)
        })
    }

    pub fn validators_mut(&mut self, round: &HeightRound) -> impl Iterator<Item = &mut MockNode> {
        let round = *round;

        self.validators
            .values_mut()
            .filter(move |node| node.start() <= &round && &round < node.validity())
    }
}

impl Network for MockNetwork {
    type Block = MockBlock;
    type Message = MockMessage;
    type Payload = BlockPayload;
    type ValidatorId = MockValidatorId;
    type ValidatorIdRef = Cow<'static, MockValidatorId>;
    type ValidatorIdSorted = IntoIter<Cow<'static, MockValidatorId>>;

    fn broadcast(&mut self, message: &Self::Message) {
        let network = self as *mut MockNetwork;
        let round = message.round();

        self.validators_mut(round)
            .for_each(|node| node.receive_message(unsafe { network.as_mut().unwrap() }, message));
    }

    fn validators_sorted(&self, round: &HeightRound) -> Self::ValidatorIdSorted {
        itertools::sorted(self.validators(round).copied().map(Cow::Owned))
    }

    fn block_payload(&self) -> Self::Payload {
        BlockPayload { is_valid: true }
    }
}

impl Node for MockNode {
    type Block = MockBlock;
    type Key = MockKey;
    type Message = MockMessage;
    type Network = MockNetwork;
    type Payload = BlockPayload;
    type ValidatorId = MockValidatorId;

    fn id(&self) -> Self::ValidatorId {
        self.key.validator()
    }

    fn key(&self) -> Self::Key {
        self.key.clone()
    }

    fn validator_state(&self, round: &HeightRound, validator: &Self::ValidatorId) -> Option<State> {
        let id = MockRoundId::new(*round, *validator);

        self.validator_state.get(&id).copied()
    }

    fn set_validator_state(
        &mut self,
        round: &HeightRound,
        validator: &Self::ValidatorId,
        state: State,
    ) {
        let id = MockRoundId::new(*round, *validator);

        self.validator_state.insert(id, state);
    }

    fn state_count(&self, round: &HeightRound, state: State) -> usize {
        self.validator_state
            .iter()
            .filter(|(r, s)| &r.round == round && s == &&state)
            .count()
    }

    fn validate_block(&self, block: &Self::Block) -> bool {
        block.payload.is_valid
    }
}
