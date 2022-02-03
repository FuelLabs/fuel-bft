use crate::{MockBlock, MockKeystore, MockNetwork};

use fuel_bft::{Error, HeightRound, Network, Node, RoundValidator, State};
use fuel_crypto::{Keystore, PublicKey};
use fuel_types::Bytes32;

use std::collections::HashMap;
use std::vec::IntoIter;

#[derive(Debug, Clone)]
pub struct MockNode {
    key: Bytes32,
    keystore: MockKeystore,
    /// Key mapping to inclusive range of validity rounds
    rounds: HashMap<PublicKey, Vec<(HeightRound, HeightRound)>>,
    start: HeightRound,
    state: HashMap<RoundValidator, State>,
    validity: HeightRound,
}

impl MockNode {
    pub fn new<P>(
        keystore: MockKeystore,
        password: P,
        start: u64,
        validity: u64,
    ) -> Result<Self, Error>
    where
        P: AsRef<[u8]>,
    {
        let key = keystore.add_secret(password)?;
        let public = keystore
            .public(&key)?
            .ok_or(Error::ValidatorNotFound)?
            .into_owned();

        let start = HeightRound::start(start);
        let validity = (0..validity).fold(start, |r, _| MockNetwork::increment_height(r));

        let rounds = Default::default();
        let state = Default::default();

        let mut node = Self {
            key,
            keystore,
            rounds,
            state,
            start,
            validity,
        };

        node.insert_key(start, validity, public);

        Ok(node)
    }

    pub const fn start(&self) -> &HeightRound {
        &self.start
    }

    pub const fn validity(&self) -> &HeightRound {
        &self.validity
    }

    pub fn insert_key(&mut self, start: HeightRound, validity: HeightRound, public: PublicKey) {
        // TODO optimize to merge ranges instead of naive append
        match self.rounds.get_mut(&public) {
            Some(ranges) => {
                ranges.push((start, validity));
            }

            None => {
                self.rounds.insert(public, vec![(start, validity)]);
            }
        }
    }

    pub fn insert_node(&mut self, node: &MockNode) -> Result<(), Error> {
        let start = *node.start();
        let validity = *node.validity();
        let public = node.public_key(&start)?.into_owned();

        self.insert_key(start, validity, public);

        Ok(())
    }
}

impl Node for MockNode {
    type Error = Error;
    type Filtered = IntoIter<PublicKey>;
    type Network = MockNetwork;
    type PublicKey = PublicKey;
    type Sorted = IntoIter<PublicKey>;

    fn id(&self, _round: &HeightRound) -> Result<&Bytes32, Self::Error> {
        Ok(&self.key)
    }

    fn is_round_validator(&self, round: &HeightRound) -> bool {
        self.start() <= round && round <= self.validity()
    }

    fn signer(&self) -> &MockKeystore {
        &self.keystore
    }

    fn validate_block(&self, block: &MockBlock) -> Result<(), Self::Error> {
        block.is_valid().then(|| ()).ok_or(Error::BlockValidation)
    }

    fn filter_round(&self, round: &HeightRound) -> Option<Self::Filtered> {
        use itertools::Itertools;

        // FIXME using `sorted` to simplify associated type resolution
        let iter = self
            .rounds
            .iter()
            .filter_map(|(key, ranges)| {
                ranges
                    .iter()
                    .any(|(start, end)| start <= round && round <= end)
                    .then(|| *key)
            })
            .sorted();

        Some(iter)
    }

    fn sort_round(&self, round: &HeightRound) -> Option<Self::Sorted> {
        self.filter_round(round).map(|i| itertools::sorted(i))
    }

    fn validator_state(&self, round_key: &RoundValidator) -> Option<State> {
        self.state.get(round_key).copied()
    }

    fn set_validator_state(&mut self, round_key: &RoundValidator, state: State) {
        self.state.insert(*round_key, state);
    }

    fn state_count(&self, round: &HeightRound, state: State) -> usize {
        self.state
            .iter()
            .filter(|(k, v)| {
                k.round() == round && (v.is_reject() && state.is_reject() || v >= &&state)
            })
            .count()
    }
}
