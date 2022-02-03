use crate::{MockMessage, MockNode};

use fuel_bft::{Error, HeightRound, Message, Network, Node, RoundValidator, State};
use fuel_crypto::PublicKey;

use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct MockNetwork {
    network: HashMap<PublicKey, MockNode>,
}

impl MockNetwork {
    pub fn node(&self, key: &PublicKey) -> Option<&MockNode> {
        self.network.get(key)
    }

    pub fn node_mut(&mut self, key: &PublicKey) -> Option<&mut MockNode> {
        self.network.get_mut(key)
    }

    pub fn insert_node(&mut self, mut node: MockNode) -> Result<(), fuel_bft::Error> {
        let round = node.start();
        let public = node.public_key(round)?.into_owned();

        self.network.values_mut().try_for_each(|n| {
            node.insert_node(n)?;

            n.insert_node(&node)
        })?;

        self.network.insert(public, node);

        Ok(())
    }

    pub fn override_state(&mut self, round_validator: &RoundValidator, state: State) {
        self.network
            .values_mut()
            .for_each(|n| n.set_validator_state(round_validator, state));
    }

    pub fn leader(&self, round: &HeightRound) -> Result<PublicKey, Error> {
        // Leader is the same for all validators
        self.network
            .values()
            .next()
            .ok_or(Error::ValidatorNotFound)
            .and_then(|node| node.leader(round))
    }
}

impl Network for MockNetwork {
    type Error = Error;
    type Message = MockMessage;

    fn broadcast(&mut self, message: &MockMessage) -> Result<(), Self::Error> {
        let round = message.round();

        // Safety: self-contained network won't mutate the nodes set on broadcast
        let nodes = unsafe {
            ((&mut self.network) as *mut HashMap<PublicKey, MockNode>)
                .as_mut()
                .unwrap()
        };

        nodes
            .values_mut()
            .filter(|node| node.is_round_validator(round))
            .try_for_each(|node| node.receive_message(self, message))?;

        Ok(())
    }

    fn block_payload(&self, _round: &HeightRound) -> Result<bool, Self::Error> {
        Ok(true)
    }
}
