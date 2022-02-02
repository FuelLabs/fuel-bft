use crate::{Block, Consensus, HeightRound, Key, Message, Network, State, ValidatorId};

pub trait Node {
    type Block: Block<Payload = Self::Payload, ValidatorId = Self::ValidatorId>;
    type Key: Key;
    type Message: Message<Block = Self::Block, Key = Self::Key, ValidatorId = Self::ValidatorId>;
    type Network: Network<
        Message = Self::Message,
        Payload = Self::Payload,
        ValidatorId = Self::ValidatorId,
    >;
    type Payload;
    type ValidatorId: ValidatorId;

    /// Network ID of the node
    fn id(&self) -> Self::ValidatorId;

    /// Secret signature key of the node
    fn key(&self) -> Self::Key;

    /// Fetch the current state of a validator for a given round
    fn validator_state(&self, round: &HeightRound, validator: &Self::ValidatorId) -> Option<State>;

    /// Set the network state of a validator for a given round
    fn set_validator_state(
        &mut self,
        round: &HeightRound,
        validator: &Self::ValidatorId,
        state: State,
    );

    /// State count for a given round
    fn state_count(&self, round: &HeightRound, state: State) -> usize;

    fn validate_block(&self, block: &Self::Block) -> bool;

    /// Upgrade a validator state, returning true if there was a change
    fn upgrade_validator_state(
        &mut self,
        round: &HeightRound,
        validator: &Self::ValidatorId,
        state: State,
    ) -> bool {
        let validator_state = self.validator_state(round, validator);

        match validator_state {
            None => {
                self.set_validator_state(round, validator, state);

                true
            }

            Some(s) if state > s => {
                self.set_validator_state(round, validator, state);

                true
            }

            _ => false,
        }
    }

    /// Upgrade the node state, returning true if there was a change
    fn upgrade_state(
        &mut self,
        round: HeightRound,
        state: State,
        network: &mut Self::Network,
        block: Self::Block,
    ) {
        if self.upgrade_validator_state(&round, &self.id(), state) {
            let key = self.key();

            let reply = Self::Message::new(round, key, state, block);

            network.broadcast(&reply);
        }
    }

    /// Current state of the node for a given round
    fn state(&self, round: &HeightRound) -> Option<State> {
        let validator = self.id();

        self.validator_state(round, &validator)
    }

    /// Create a new block from a network pool
    fn new_block(&self, network: &Self::Network) -> Self::Block {
        let id = self.id();
        let payload = network.block_payload();

        Self::Block::new(id, payload)
    }

    /// Evaluate the state count for a given round, including the validators that are in subsequent
    /// states.
    fn evaluate_state_count(&self, round: &HeightRound, state: State) -> usize {
        let current = self.state_count(round, state);
        let subsequent: usize = state.map(|s| self.state_count(round, s)).sum::<usize>();

        current + subsequent
    }

    fn receive_message(&mut self, network: &mut Self::Network, message: &Self::Message) {
        let block = message.block();
        let round = message.round();
        let validator = message.validator();
        let mut proposed_state = message.state();

        // Ignore messages produced by self
        if validator == &self.id() {
            return ();
        }

        if !network.is_validator(round, validator) || !message.is_valid() {
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

        self.upgrade_validator_state(round, validator, proposed_state);

        if proposed_state.is_propose()
            && network
                .leader(round)
                .filter(|v| v.as_ref() == validator)
                .is_some()
        {
            self.upgrade_state(*round, State::Propose, network, message.owned_block());

            // Block proposers are automatically committed to their own blocks
            self.upgrade_validator_state(round, validator, State::Commit);

            return ();
        }

        // Evaluate the count considering the vote of the current node
        let count = 1 + self.evaluate_state_count(round, proposed_state);
        let consensus = network.consensus(round, count);
        let current_state = self.state(round);

        // Upgrade to latest consensus, if available
        if consensus.is_consensus() {
            while let Some(next_state) = proposed_state.increment() {
                let count = 1 + self.evaluate_state_count(round, next_state);
                let next_consensus = network.consensus(round, count);

                if next_consensus.is_consensus() {
                    proposed_state = next_state;
                } else {
                    break;
                }
            }
        }

        match consensus {
            Consensus::Inconclusive if current_state.is_none() => {
                let state = State::initial();
                let block = Self::Block::default();

                self.upgrade_state(*round, state, network, block);
            }

            Consensus::Inconclusive => (),

            Consensus::Consensus if proposed_state.is_precommit() || proposed_state.is_commit() => {
                let next_round = Self::Network::increment_height(*round);

                self.upgrade_state(*round, State::Commit, network, message.owned_block());

                if self.state(&next_round).is_some() {
                    // Do nothing, state is already tracked
                } else if network
                    .leader(&next_round)
                    .filter(|l| l.as_ref() == &self.id())
                    .is_some()
                {
                    let block = self.new_block(network);

                    // Automatically commit to own proposed block
                    self.upgrade_state(next_round, State::Commit, network, block);
                } else {
                    let block = Self::Block::default();

                    self.upgrade_state(next_round, State::NewRound, network, block);
                }
            }

            Consensus::Consensus => {
                if let Some(state) = proposed_state.increment() {
                    self.upgrade_state(*round, state, network, message.owned_block());
                }
            }

            Consensus::Reject => {
                let state = State::Reject;

                self.upgrade_state(*round, state, network, message.owned_block());
            }
        }
    }
}
