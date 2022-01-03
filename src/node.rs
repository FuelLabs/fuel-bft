use crate::{Block, Consensus, Key, Message, Network, PeerId, State, Transaction};

pub trait Node {
    type Block: Block<PeerId = Self::PeerId, Transaction = Self::Transaction>;
    type Key: Key;
    type Message: Message<Block = Self::Block, Key = Self::Key, PeerId = Self::PeerId>;
    type Network: Network<
        PeerId = Self::PeerId,
        Message = Self::Message,
        Transaction = Self::Transaction,
    >;
    type PeerId: PeerId;
    type Transaction: Transaction;

    /// Network ID of the node
    fn id(&self) -> Self::PeerId;

    /// Secret signature key of the node
    fn key(&self) -> Self::Key;

    /// Current state of the node for a given height
    fn state(&self, height: u64) -> Option<State>;

    /// Set the network state of a peer for a given height
    fn set_state(&mut self, height: u64, state: State);

    /// Fetch the current state of a peer for a given height
    fn peer_state(&self, height: u64, peer: &Self::PeerId) -> Option<State>;

    /// Set the network state of a peer for a given height
    fn set_peer_state(&mut self, height: u64, peer: &Self::PeerId, state: State);

    /// State count for a given height
    fn state_count(&self, height: u64, state: State) -> usize;

    fn validate_block(&self, block: &Self::Block) -> bool;

    /// Create a new block from a network pool
    fn new_block(&self, network: &Self::Network) -> Self::Block {
        let id = self.id();
        let txs = network.transaction_set();

        Self::Block::new(id, txs)
    }

    /// Evaluate the state count for a given height, including the peers that are in subsequent
    /// states.
    fn evaluate_state_count(&self, height: u64, state: State) -> usize {
        let current = self.state_count(height, state);
        let subsequent: usize = state.map(|s| self.state_count(height, s)).sum::<usize>();

        current + subsequent
    }

    fn receive_message(&mut self, network: &Self::Network, message: &Self::Message) {
        let block = message.block();
        let height = message.height();
        let peer = message.peer();
        let proposed_state = message.state();

        if !network.is_council(height, peer) || !message.is_valid() {
            return ();
        }

        if proposed_state.is_propose() && peer != network.proposer(height) {
            return ();
        }

        let current_state = self.state(height).unwrap_or(State::initial());

        if current_state.is_reject() {
            return ();
        }

        // Can discard any previous state since it won't affect the current consensus state
        if current_state > proposed_state {
            return ();
        }

        if !self.validate_block(block) {
            return ();
        }

        self.set_peer_state(height, peer, proposed_state);

        let next_height = Self::Network::increment_height(height);

        // Attempt to upgrade the state
        loop {
            let current_state = self.state(height).unwrap_or_else(|| {
                let initial = State::initial();

                self.set_state(height, initial);

                initial
            });

            if current_state > proposed_state {
                return ();
            }

            if current_state.is_commit() {
                return ();
            }

            let count = self.evaluate_state_count(height, current_state);
            let consensus = network.consensus(height, count);

            let new_state = current_state.increment();

            let id = self.id();
            let key = self.key();

            match (consensus, new_state) {
                (Consensus::Inconclusive, _) => break,

                (Consensus::Consensus, Some(State::Propose))
                    if network.proposer(next_height) == &id =>
                {
                    let block = self.new_block(network);
                    let state = State::Propose;

                    let proposal = Self::Message::new(next_height, key, id, state, block);

                    self.set_peer_state(next_height, &self.id(), state);
                    network.broadcast(&proposal);

                    self.set_state(next_height, state);
                }

                (Consensus::Consensus, None) if current_state.is_commit() => {
                    let block = Self::Block::default();
                    let state = State::NewHeight;

                    let reply = Self::Message::new(next_height, key, id, state, block);

                    self.set_peer_state(next_height, &self.id(), state);
                    network.broadcast(&reply);

                    self.set_state(next_height, state);
                }

                // The onyl two variants that can produce `None` as next step are `commit` or
                // `reject`, and both are already checked.
                (Consensus::Consensus, None) => unreachable!(),

                (Consensus::Consensus, Some(state)) => {
                    let block = message.owned_block();
                    let reply = Self::Message::new(height, key, id, state, block);

                    self.set_peer_state(height, &self.id(), state);
                    network.broadcast(&reply);

                    self.set_state(height, state);
                }

                (Consensus::Reject, _) => {
                    let block = message.owned_block();
                    let state = State::Reject;

                    let reply = Self::Message::new(height, key, id, state, block);

                    self.set_peer_state(height, &self.id(), state);

                    network.broadcast(&reply);

                    self.set_state(height, state);

                    break;
                }
            }
        }
    }
}
