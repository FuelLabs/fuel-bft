use crate::{Consensus, Error, HeightRound, Message, Network, RoundValidator, State};

use fuel_crypto::borrown::Borrown;
use fuel_crypto::{Keystore, PublicKey, Signer};

use core::borrow::Borrow;

pub trait Node {
    type Error: From<Error>
        + From<fuel_crypto::Error>
        + From<<Self::Network as Network>::Error>
        + From<<<<Self::Network as Network>::Message as Message>::Signer as Signer>::Error>
        + From<<<Self::Network as Network>::Message as Message>::Error>;
    type Filtered: Iterator<Item = Self::PublicKey>;
    type Network: Network;
    /// Borrowed public key for iterators of validators
    type PublicKey: Borrow<PublicKey>;
    type Sorted: Iterator<Item = Self::PublicKey>;

    /// Validators filtered per round
    fn filter_round(&self, round: &HeightRound) -> Option<Self::Filtered>;

    fn id(
        &self,
        round: &HeightRound,
    ) -> Result<&<<<<Self::Network as Network>::Message as Message>::Signer as Signer>::Keystore as Keystore>::KeyId, Self::Error>;

    fn is_round_validator(&self, round: &HeightRound) -> bool;

    fn signer(&self) -> &<<Self::Network as Network>::Message as Message>::Signer;

    /// Sorted list of validators filtered per round to elect the leader
    fn sort_round(&self, round: &HeightRound) -> Option<Self::Sorted>;

    /// State count for a given round
    fn state_count(&self, round: &HeightRound, state: State) -> usize;

    fn validate_block(
        &self,
        block: &<<Self::Network as Network>::Message as Message>::Block,
    ) -> Result<(), Self::Error>;

    /// Fetch the current state of a validator for a given round
    fn validator_state(&self, round_key: &RoundValidator) -> Option<State>;

    /// Set the network state of a validator for a given round
    fn set_validator_state(&mut self, round_key: &RoundValidator, state: State);

    fn public_key(&self, round: &HeightRound) -> Result<Borrown<'_, PublicKey>, Self::Error> {
        let id = self.id(round)?;

        let public = self.signer().id_public(id)?;

        Ok(public)
    }

    fn upgrade_state(
        &mut self,
        network: &mut Self::Network,
        round: HeightRound,
        state: State,
        block: <<Self::Network as Network>::Message as Message>::Block,
    ) -> Result<(), Self::Error> {
        #[cfg(feature = "trace")]
        tracing::trace!(
            "starting upgrade state request for round {}: {:?}",
            round,
            state
        );

        let public = self.public_key(&round)?.into_owned();
        let round_key = RoundValidator::new(round, public);

        #[cfg(feature = "trace")]
        tracing::trace!(
            "{:04x} upgrade state request for round {}: {:?}",
            round_key.validator(),
            round,
            state
        );

        if self.upgrade_validator_state(&round_key, state) {
            #[cfg(feature = "trace")]
            tracing::debug!(
                "{:04x} self-upgrade to round {}, state {:?}",
                round_key.validator(),
                round,
                state
            );

            let signer = self.signer();
            let id = self.id(&round)?;

            let reply =
                <Self::Network as Network>::Message::signed(signer, id, round, state, block)?;

            #[cfg(feature = "trace")]
            tracing::trace!(
                "{:04x} self-upgrade reply broadcasting for round {}/{}/{}, state {:?}",
                round_key.validator(),
                round_key.round(),
                round,
                reply.round(),
                state
            );

            network.broadcast(&reply)?;

            #[cfg(feature = "trace")]
            tracing::trace!(
                "{:04x} self-upgrade reply broadcasted for round {}, state {:?}",
                round_key.validator(),
                round_key.round(),
                state
            );

            if state.is_commit() {
                #[cfg(feature = "trace")]
                tracing::debug!(
                    "{:04x} self-upgrade commit for round {}, state {:?}",
                    round_key.validator(),
                    round_key.round(),
                    state
                );

                let next_round = Self::Network::increment_height(round);
                let is_next_round_validator = self.is_round_validator(&next_round);

                // FIXME this broadcast should be async for when the new block is ready
                let is_next_round_leader = is_next_round_validator
                    && self.leader(&next_round).map(|l| l.borrow() == &public)?;

                if is_next_round_leader {
                    let block = network.generate_block(&next_round, public)?;

                    #[cfg(feature = "trace")]
                    tracing::debug!(
                        "{:04x} self-upgrade committing new round {}",
                        round_key.validator(),
                        next_round
                    );

                    self.upgrade_state(network, next_round, State::Commit, block)?;
                } else if is_next_round_validator {
                    let block = Default::default();

                    #[cfg(feature = "trace")]
                    tracing::debug!(
                        "{:04x} self-upgrade starting new round {}",
                        round_key.validator(),
                        next_round
                    );

                    self.upgrade_state(network, next_round, State::NewRound, block)?;
                }
            }
        }

        Ok(())
    }

    /// Current state of the node for a given round
    fn state(&self, round: &HeightRound) -> Result<Option<State>, Self::Error> {
        self.public_key(round)
            .map(|public| RoundValidator::new(*round, public.into_owned()))
            .map(|round_key| self.validator_state(&round_key))
    }

    #[cfg(not(feature = "std"))]
    // FIXME `Signature::verify` requires `std`. For this lib, `std` is optional.
    fn validate(&self, message: &<Self::Network as Network>::Message) -> Result<(), Self::Error>;

    #[cfg(feature = "std")]
    fn validate(&self, message: &<Self::Network as Network>::Message) -> Result<(), Self::Error> {
        #[cfg(feature = "trace")]
        tracing::trace!("validating message");

        let round = message.round();
        let public = message.author_key();
        let signature = *message.signature();
        let message = message.to_signature_message();

        let author_exists = self
            .filter_round(round)
            .map(|mut i| i.any(|k| k.borrow() == public))
            .unwrap_or(false);

        if !author_exists {
            return Err(Error::ValidatorNotFound.into());
        }

        signature.verify(public, &message)?;

        #[cfg(feature = "trace")]
        tracing::trace!("message validated");

        Ok(())
    }

    /// Evaluate the state count for a given round, including the validators that are in subsequent
    /// states.
    fn evaluate_state_count(&self, round: &HeightRound, state: State) -> usize {
        let current = self.state_count(round, state);
        let subsequent: usize = state.map(|s| self.state_count(round, s)).sum::<usize>();

        current + subsequent
    }

    /// Upgrade a validator state, returning true if there was a change
    fn upgrade_validator_state(&mut self, round_key: &RoundValidator, state: State) -> bool {
        let current_state = self.validator_state(round_key);

        #[cfg(feature = "trace")]
        tracing::trace!(
            "{:04x?} upgrade state attempt; validator: {:04x}, current: {:?}, round: {}, state: {:?}",
            self.public_key(round_key.round()).ok(),
            round_key.validator(),
            current_state,
            round_key.round(),
            state
        );

        match current_state {
            None => {
                self.set_validator_state(round_key, state);

                #[cfg(feature = "trace")]
                tracing::debug!(
                    "{:04x?} upgrading state; validator: {:04x}, round: {}, state: {:?}",
                    self.public_key(round_key.round()).ok(),
                    round_key.validator(),
                    round_key.round(),
                    state
                );

                true
            }

            Some(s) if state > s => {
                self.set_validator_state(round_key, state);

                #[cfg(feature = "trace")]
                tracing::debug!(
                    "{:04x?} upgrading state; validator: {:04x}, round: {}, state: {:?}",
                    self.public_key(round_key.round()).ok(),
                    round_key.validator(),
                    round_key.round(),
                    state
                );

                true
            }

            _ => {
                #[cfg(feature = "trace")]
                tracing::debug!(
                    "{:04x?} upgrading state skipped; validator: {:04x}, round: {}, state: {:?}",
                    self.public_key(round_key.round()).ok(),
                    round_key.validator(),
                    round_key.round(),
                    state
                );

                false
            }
        }
    }

    fn round_count(&self, round: &HeightRound) -> usize {
        self.filter_round(round)
            .map(|iter| iter.count())
            .unwrap_or(0)
    }

    fn is_validator(&self, round_key: &RoundValidator) -> bool {
        self.filter_round(round_key.round())
            .map(|mut iter| iter.any(|p| p.borrow() == round_key.validator()))
            .unwrap_or(false)
    }

    fn leader(&self, round: &HeightRound) -> Result<Self::PublicKey, Self::Error> {
        #[cfg(feature = "trace")]
        tracing::trace!("choosing leader for round {}", round);

        let count = self.round_count(round) as u64;

        if count == 0 {
            return Err(Error::ValidatorNotFound.into());
        }

        let h = round.height();
        let r = round.round();

        let index = (h + r) % count;

        let leader = self
            .sort_round(round)
            .map(|iter| iter.skip(index as usize).next())
            .flatten()
            .ok_or(Error::ValidatorNotFound)?;

        #[cfg(feature = "trace")]
        tracing::debug!("{:04x} leader for round {}", leader.borrow(), round);

        Ok(leader)
    }

    fn receive_message(
        &mut self,
        network: &mut Self::Network,
        message: &<Self::Network as Network>::Message,
    ) -> Result<(), Self::Error> {
        let block = message.block();
        let round = message.round();
        let validator = message.author_key();
        let round_key = message.author();
        let mut proposed_state = message.state();
        let public = self.public_key(round)?.into_owned();

        #[cfg(feature = "trace")]
        tracing::debug!(
            "{:04x} receiving message: round {}, author {:04x}, state: {:?}",
            public,
            round,
            validator,
            proposed_state
        );

        // Ignore messages produced by self
        if validator == &public {
            #[cfg(feature = "trace")]
            tracing::trace!(
                "{:04x} skipping received message: round {}, author {:04x}, state: {:?}",
                public,
                round,
                validator,
                proposed_state
            );

            return Ok(());
        }

        self.validate(message)?;

        #[cfg(feature = "trace")]
        tracing::trace!("message validated");

        let state = self.state(round)?;

        match state {
            Some(s) if s.is_reject() => {
                #[cfg(feature = "trace")]
                tracing::debug!(
                    "{:04x} message rejected: round {}, author {:04x}, state: {:?}",
                    public,
                    round,
                    validator,
                    proposed_state
                );

                return Ok(());
            }

            // Can discard any previous state since it won't affect the current consensus state
            Some(s) if s > proposed_state => {
                #[cfg(feature = "trace")]
                tracing::debug!(
                    "{:04x} message discarded: round {}, author {:04x}, state: {:?}",
                    public,
                    round,
                    validator,
                    proposed_state
                );

                return Ok(());
            }

            _ => (),
        }

        self.validate_block(block)?;

        #[cfg(feature = "trace")]
        tracing::trace!("block validated");

        self.upgrade_validator_state(round_key, proposed_state);

        let proposer_is_leader = self.leader(round).map(|v| v.borrow() == validator)?;

        if let Some(s) = state {
            if s < proposed_state && proposed_state.is_propose() && proposer_is_leader {
                self.upgrade_state(network, *round, State::Propose, message.block().clone())?;

                // Block proposers are automatically committed to their own blocks
                self.upgrade_validator_state(round_key, State::Commit);

                return Ok(());
            }
        }

        // Evaluate the count considering the vote of the current node
        let validators = self.round_count(round);
        let approved = 1 + self.state_count(round, proposed_state);
        let consensus = Consensus::evaluate(validators, approved);
        let current_state = self.state(round)?;

        #[cfg(feature = "trace")]
        tracing::trace!(
            "{:04x} receiving message: round {}, author {:04x}, state: {:?}, consensus {:?}",
            public,
            round,
            validator,
            proposed_state,
            consensus
        );

        // Upgrade to latest consensus, if available
        if consensus.is_consensus() {
            while let Some(next_state) = proposed_state.increment() {
                let approved = 1 + self.state_count(round, next_state);
                let next_consensus = Consensus::evaluate(validators, approved);

                if next_consensus.is_consensus() {
                    proposed_state = next_state;
                } else {
                    break;
                }
            }
        }

        #[cfg(feature = "trace")]
        tracing::debug!(
            "{:04x} receiving message: round {}, author {:04x}, state: {:?}, consensus {:?}, evaluated {:?}",
            public, round, validator, proposed_state, consensus, proposed_state
        );

        match consensus {
            Consensus::Inconclusive if current_state.is_none() => {
                self.upgrade_state(network, *round, State::initial(), block.clone())?;
            }

            Consensus::Inconclusive => (),

            Consensus::Consensus if proposed_state.is_precommit() || proposed_state.is_commit() => {
                self.upgrade_state(network, *round, State::Commit, block.clone())?;
            }

            Consensus::Consensus => {
                if let Some(state) = proposed_state.increment() {
                    self.upgrade_state(network, *round, state, block.clone())?;
                }
            }

            Consensus::Reject => {
                self.upgrade_state(network, *round, State::Reject, block.clone())?;
            }
        }

        #[cfg(feature = "trace")]
        tracing::debug!(
            "{:04x} message processed: round {}, author {:04x}, state: {:?}",
            public,
            round,
            validator,
            proposed_state
        );

        Ok(())
    }
}
