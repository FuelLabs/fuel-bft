use crate::{Block, Consensus, Key, Message, Network, PeerId, State};

pub trait Node {
    type Block: Block<Payload = Self::Payload, PeerId = Self::PeerId>;
    type Key: Key;
    type Message: Message<Block = Self::Block, Key = Self::Key, PeerId = Self::PeerId>;
    type Network: Network<Message = Self::Message, Payload = Self::Payload, PeerId = Self::PeerId>;
    type Payload;
    type PeerId: PeerId;

    /// Network ID of the node
    fn id(&self) -> Self::PeerId;

    /// Secret signature key of the node
    fn key(&self) -> Self::Key;

    /// Fetch the current state of a peer for a given height
    fn peer_state(&self, height: u64, peer: &Self::PeerId) -> Option<State>;

    /// Set the network state of a peer for a given height
    fn set_peer_state(&mut self, height: u64, peer: &Self::PeerId, state: State);

    /// State count for a given height
    fn state_count(&self, height: u64, state: State) -> usize;

    fn validate_block(&self, block: &Self::Block) -> bool;

    /// Upgrade a peer state, returning true if there was a change
    fn upgrade_peer_state(&mut self, height: u64, peer: &Self::PeerId, state: State) -> bool {
        let peer_state = self.peer_state(height, peer);

        match peer_state {
            None => {
                self.set_peer_state(height, peer, state);

                true
            }

            Some(s) if state > s => {
                self.set_peer_state(height, peer, state);

                true
            }

            _ => false,
        }
    }

    /// Upgrade the node state, returning true if there was a change
    fn upgrade_state(
        &mut self,
        height: u64,
        state: State,
        network: &mut Self::Network,
        block: Self::Block,
    ) {
        if self.upgrade_peer_state(height, &self.id(), state) {
            let key = self.key();

            let reply = Self::Message::new(height, key, state, block);

            network.broadcast(&reply);
        }
    }

    /// Current state of the node for a given height
    fn state(&self, height: u64) -> Option<State> {
        let peer = self.id();

        self.peer_state(height, &peer)
    }

    /// Create a new block from a network pool
    fn new_block(&self, network: &Self::Network) -> Self::Block {
        let id = self.id();
        let payload = network.block_payload();

        Self::Block::new(id, payload)
    }

    /// Evaluate the state count for a given height, including the peers that are in subsequent
    /// states.
    fn evaluate_state_count(&self, height: u64, state: State) -> usize {
        let current = self.state_count(height, state);
        let subsequent: usize = state.map(|s| self.state_count(height, s)).sum::<usize>();

        current + subsequent
    }

    fn receive_message(&mut self, network: &mut Self::Network, message: &Self::Message) {
        let block = message.block();
        let height = message.height();
        let peer = message.peer();
        let proposed_state = message.state();

        // Ignore messages produced by self
        if peer == &self.id() {
            return ();
        }

        if !network.is_council(height, peer) || !message.is_valid() {
            return ();
        }

        if let Some(state) = self.state(height) {
            if state.is_reject() {
                return ();
            }

            // Can discard any previous state since it won't affect the current consensus state
            if state > proposed_state {
                return ();
            }
        }

        if !self.validate_block(block) {
            return ();
        }

        self.upgrade_peer_state(height, peer, proposed_state);

        if proposed_state.is_propose() && Some(peer) == network.proposer(height) {
            self.upgrade_state(height, State::Propose, network, message.owned_block());

            return ();
        }

        // Evaluate the count considering the vote of the current node
        let count = 1 + self.evaluate_state_count(height, proposed_state);
        let consensus = network.consensus(height, count);
        let current_state = self.state(height);

        match consensus {
            Consensus::Inconclusive if current_state.is_none() => {
                let state = State::initial();
                let block = Self::Block::default();

                self.upgrade_state(height, state, network, block);
            }

            Consensus::Inconclusive => (),

            Consensus::Consensus if proposed_state.is_commit() => {
                self.upgrade_state(height, proposed_state, network, message.owned_block());

                let state = State::initial();
                let height = Self::Network::increment_height(height);
                let block = Self::Block::default();

                self.upgrade_state(height, state, network, block);
            }

            Consensus::Consensus
                if proposed_state == State::NewHeight
                    && network.proposer(height) == Some(&self.id()) =>
            {
                let state = State::Propose;
                let block = self.new_block(network);

                self.upgrade_state(height, state, network, block);
            }

            // Wait for the proposer to send a block
            Consensus::Consensus if proposed_state == State::NewHeight => (),

            Consensus::Consensus => {
                if let Some(state) = proposed_state.increment() {
                    self.upgrade_state(height, state, network, message.owned_block());
                }
            }

            Consensus::Reject => {
                let state = State::Reject;

                self.upgrade_state(height, state, network, message.owned_block());
            }
        }
    }
}
