use crate::{Block, Consensus, Key, Message, Network, PeerId, Round, State};

pub trait Node {
    type Block: Block<Payload = Self::Payload, PeerId = Self::PeerId>;
    type Key: Key;
    type Message: Message<
        Block = Self::Block,
        Key = Self::Key,
        PeerId = Self::PeerId,
        Round = Self::Round,
    >;
    type Network: Network<
        Message = Self::Message,
        Payload = Self::Payload,
        PeerId = Self::PeerId,
        Round = Self::Round,
    >;
    type Payload;
    type PeerId: PeerId;
    type Round: Round;

    /// Network ID of the node
    fn id(&self) -> Self::PeerId;

    /// Secret signature key of the node
    fn key(&self) -> Self::Key;

    /// Fetch the current state of a peer for a given round
    fn peer_state(&self, round: Self::Round, peer: &Self::PeerId) -> Option<State>;

    /// Set the network state of a peer for a given round
    fn set_peer_state(&mut self, round: Self::Round, peer: &Self::PeerId, state: State);

    /// State count for a given round
    fn state_count(&self, round: Self::Round, state: State) -> usize;

    fn validate_block(&self, block: &Self::Block) -> bool;

    /// Upgrade a peer state, returning true if there was a change
    fn upgrade_peer_state(
        &mut self,
        round: Self::Round,
        peer: &Self::PeerId,
        state: State,
    ) -> bool {
        let peer_state = self.peer_state(round, peer);

        match peer_state {
            None => {
                self.set_peer_state(round, peer, state);

                true
            }

            Some(s) if state > s => {
                self.set_peer_state(round, peer, state);

                true
            }

            _ => false,
        }
    }

    /// Upgrade the node state, returning true if there was a change
    fn upgrade_state(
        &mut self,
        round: Self::Round,
        state: State,
        network: &mut Self::Network,
        block: Self::Block,
    ) {
        if self.upgrade_peer_state(round, &self.id(), state) {
            let key = self.key();

            let reply = Self::Message::new(round, key, state, block);

            network.broadcast(&reply);
        }
    }

    /// Current state of the node for a given round
    fn state(&self, round: Self::Round) -> Option<State> {
        let peer = self.id();

        self.peer_state(round, &peer)
    }

    /// Create a new block from a network pool
    fn new_block(&self, network: &Self::Network) -> Self::Block {
        let id = self.id();
        let payload = network.block_payload();

        Self::Block::new(id, payload)
    }

    /// Evaluate the state count for a given round, including the peers that are in subsequent
    /// states.
    fn evaluate_state_count(&self, round: Self::Round, state: State) -> usize {
        let current = self.state_count(round, state);
        let subsequent: usize = state.map(|s| self.state_count(round, s)).sum::<usize>();

        current + subsequent
    }

    fn receive_message(&mut self, network: &mut Self::Network, message: &Self::Message) {
        let block = message.block();
        let round = message.round();
        let peer = message.peer();
        let proposed_state = message.state();

        // Ignore messages produced by self
        if peer == &self.id() {
            return ();
        }

        if !network.is_council(round, peer) || !message.is_valid() {
            return ();
        }

        if let Some(state) = self.state(round) {
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

        self.upgrade_peer_state(round, peer, proposed_state);

        if proposed_state.is_propose() && Some(peer) == network.proposer(round) {
            self.upgrade_state(round, State::Propose, network, message.owned_block());

            return ();
        }

        // Evaluate the count considering the vote of the current node
        let count = 1 + self.evaluate_state_count(round, proposed_state);
        let consensus = network.consensus(round, count);
        let current_state = self.state(round);

        match consensus {
            Consensus::Inconclusive if current_state.is_none() => {
                let state = State::initial();
                let block = Self::Block::default();

                self.upgrade_state(round, state, network, block);
            }

            Consensus::Inconclusive => (),

            Consensus::Consensus if proposed_state.is_commit() => {
                self.upgrade_state(round, proposed_state, network, message.owned_block());

                let state = State::initial();
                let round = Self::Network::increment_round(round);
                let block = Self::Block::default();

                self.upgrade_state(round, state, network, block);
            }

            Consensus::Consensus
                if proposed_state == State::NewRound
                    && network.proposer(round) == Some(&self.id()) =>
            {
                let state = State::Propose;
                let block = self.new_block(network);

                self.upgrade_state(round, state, network, block);
            }

            // Wait for the proposer to send a block
            Consensus::Consensus if proposed_state == State::NewRound => (),

            Consensus::Consensus => {
                if let Some(state) = proposed_state.increment() {
                    self.upgrade_state(round, state, network, message.owned_block());
                }
            }

            Consensus::Reject => {
                let state = State::Reject;

                self.upgrade_state(round, state, network, message.owned_block());
            }
        }
    }
}
