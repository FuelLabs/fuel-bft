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
pub struct EdKey(Scalar);

impl fmt::Debug for EdKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EdKey")
            .field("key", &hex::encode(self.0.as_bytes()))
            .finish()
    }
}

impl EdKey {
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

impl Key for EdKey {
    type PeerId = RistrettoPeerId;

    fn peer(&self) -> Self::PeerId {
        RistrettoPeerId(&self.0 * &constants::RISTRETTO_BASEPOINT_TABLE)
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
pub struct RistrettoPeerId(RistrettoPoint);

impl fmt::Debug for RistrettoPeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EdKey")
            .field("key", &hex::encode(self.0.compress().as_bytes()))
            .finish()
    }
}

impl RistrettoPeerId {
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

impl Hash for RistrettoPeerId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.compress().as_bytes().hash(state);
    }
}

impl PeerId for RistrettoPeerId {}

impl Default for RistrettoPeerId {
    fn default() -> RistrettoPeerId {
        RistrettoPeerId(RistrettoPoint::identity())
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
    owner: RistrettoPeerId,
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
    type PeerId = RistrettoPeerId;
    type Payload = BlockPayload;

    fn new(owner: RistrettoPeerId, payload: BlockPayload) -> Self {
        Self { owner, payload }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MockMessage {
    block: MockBlock,
    round: u64,
    signature: Signature,
    state: State,
    peer: RistrettoPeerId,
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
    type Key = EdKey;
    type PeerId = RistrettoPeerId;
    type Round = u64;

    fn new(round: u64, key: Self::Key, state: State, block: Self::Block) -> Self {
        let digest = Self::digest(round, state, &block);
        let peer = key.peer();

        // Insecure implementation for test purposes. Don't do this in production.
        let challenge = Scalar::from(round);
        let signature = key.sign(&challenge, &digest.finalize());

        Self {
            block,
            round,
            signature,
            state,
            peer,
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

    fn peer(&self) -> &Self::PeerId {
        &self.peer
    }

    fn is_valid(&self) -> bool {
        let digest = Self::digest(self.round, self.state, &self.block);

        self.peer.verify(&self.signature, &digest.finalize())
    }
}

#[derive(Debug, Default, Clone)]
pub struct MockNetwork {
    /// Set of council validators mapping to a round range
    council: HashMap<RistrettoPeerId, (u64, u64, MockNode)>,
}

impl MockNetwork {
    pub fn key_from_round(round: u64) -> EdKey {
        EdKey::new(format!("round {}", round))
    }

    pub fn add_node(&mut self, round: u64, from: u64, validity: u64) {
        let to = from + validity;

        let key = Self::key_from_round(round);
        let peer = key.peer();
        let node = MockNode::new(key);

        self.council.insert(peer, (from, to, node));
    }

    pub fn node(&self, round: u64) -> Option<&MockNode> {
        let key = Self::key_from_round(round);
        let peer = key.peer();

        self.council.get(&peer).map(|(_, _, n)| n)
    }

    pub fn council_members(&self, round: u64) -> impl Iterator<Item = &RistrettoPeerId> {
        self.council
            .iter()
            .filter_map(move |(peer, (from, to, _))| (*from <= round && round < *to).then(|| peer))
    }
}

impl Network for MockNetwork {
    type Block = MockBlock;
    type Message = MockMessage;
    type Payload = BlockPayload;
    type PeerId = RistrettoPeerId;
    type Round = u64;

    fn broadcast(&mut self, message: &Self::Message) {
        let round = message.round();

        let network = self as *mut MockNetwork;

        self.council
            .iter_mut()
            .filter_map(|(_, (from, to, node))| (*from <= round && round < *to).then(|| node))
            .for_each(|node| node.receive_message(unsafe { network.as_mut().unwrap() }, message));
    }

    fn increment_round(round: u64) -> u64 {
        round + 1
    }

    fn is_council(&self, round: u64, peer: &Self::PeerId) -> bool {
        self.council_members(round).any(|p| p == peer)
    }

    fn peers(&self, round: u64) -> usize {
        self.council_members(round).count()
    }

    fn proposer(&self, round: u64) -> Option<&Self::PeerId> {
        let peer = Self::key_from_round(round).peer();

        self.council_members(round).find(|p| p == &&peer)
    }

    fn block_payload(&self) -> Self::Payload {
        BlockPayload { is_valid: true }
    }
}

#[derive(Debug, Clone)]
pub struct MockNode {
    key: EdKey,
    peer_state: HashMap<(u64, RistrettoPeerId), State>,
}

impl MockNode {
    pub fn new(key: EdKey) -> Self {
        Self {
            key,
            peer_state: Default::default(),
        }
    }
}

impl Node for MockNode {
    type Block = MockBlock;
    type Key = EdKey;
    type Message = MockMessage;
    type Network = MockNetwork;
    type Payload = BlockPayload;
    type PeerId = RistrettoPeerId;
    type Round = u64;

    fn id(&self) -> Self::PeerId {
        self.key.peer()
    }

    fn key(&self) -> Self::Key {
        self.key
    }

    fn peer_state(&self, round: u64, peer: &Self::PeerId) -> Option<State> {
        self.peer_state.get(&(round, *peer)).copied()
    }

    fn set_peer_state(&mut self, round: u64, peer: &Self::PeerId, state: State) {
        self.peer_state.insert((round, *peer), state);
    }

    fn state_count(&self, round: u64, state: State) -> usize {
        self.peer_state
            .iter()
            .filter(|((h, _), s)| h == &round && s == &&state)
            .count()
    }

    fn validate_block(&self, block: &Self::Block) -> bool {
        block.payload.is_valid
    }
}
