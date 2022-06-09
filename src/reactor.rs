use crate::{Consensus, Error, Height, Keychain, Metadata, Moderator, Round, Step, Vote};

use fuel_crypto::PublicKey;
use fuel_types::Bytes32;
use time::OffsetDateTime;

use alloc::vec::Vec;
use core::time::Duration;

mod config;
mod message;

pub use config::Config;
pub use message::{Event, Message, Notification, Request, Response};

/// State machine of the consensus
#[derive(Debug)]
pub struct Reactor {
    capacity: usize,
    consensus: u128,
    genesis: OffsetDateTime,
    metadata: Metadata,
    timeout: Duration,
    should_quit: bool,
}

impl Default for Reactor {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl Reactor {
    /// Create a new reactor with the provided arguments
    pub fn new(config: Config) -> Self {
        let Config {
            capacity,
            consensus,
            genesis,
            timeout,
            ..
        } = config;

        let metadata = Default::default();
        let should_quit = false;

        Self {
            capacity,
            consensus,
            genesis,
            metadata,
            timeout,
            should_quit,
        }
    }

    /// Flag on whether should quit
    pub const fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Current block height.
    pub const fn height(&self) -> Height {
        self.metadata.committed_height().wrapping_add(1)
    }

    /// Current height round
    pub fn round(&self, now: OffsetDateTime) -> Round {
        let elapsed = now - self.genesis;
        let elapsed = elapsed.whole_milliseconds() as u128;

        let committed_rounds = self.metadata.committed_rounds() as u128;
        let committed_ms = committed_rounds.saturating_sub(1) * self.consensus;

        let remainder_ms = elapsed.saturating_sub(committed_ms);
        let round = remainder_ms / self.consensus;

        round as Round
    }

    /// Evaluate the consensus step of a validator for a given round
    pub fn validator_step(&self, height: Height, round: Round, public: &PublicKey) -> Option<Step> {
        self.metadata.validator_step(height, round, public)
    }

    /// Attempt a forced commit to a round.
    pub async fn commit<M>(&mut self, moderator: &mut M, height: Height, round: Round) -> bool
    where
        M: Moderator,
    {
        let committed = self.metadata.commit(height, round);

        if committed {
            let commit = Message::Event(Event::Commit {
                height,
                round,
                block_id: Bytes32::zeroed(),
            });

            moderator.send(commit, self.timeout).await;
        }

        committed
    }

    /// Compute the round leader for the current height.
    pub fn leader(&self, round: Round) -> Result<&PublicKey, Error> {
        let height = self.height();
        let committed_rounds = self.metadata.committed_rounds();
        let validators = self.metadata.validators_at_height_count(height) as u64;

        #[cfg(feature = "trace")]
        tracing::trace!(
            "choosing leader for height {} round {} with {} validators",
            height,
            round,
            validators
        );

        if validators == 0 {
            return Err(Error::ValidatorNotFound);
        }

        let index = (committed_rounds + round) % validators;
        let leader = self
            .metadata
            .validators_at_height(height)
            .skip(index as usize)
            .next()
            .ok_or(Error::ValidatorNotFound)?;

        #[cfg(feature = "trace")]
        tracing::trace!(
            "leader for height {} round {}: {:08x}",
            height,
            round,
            leader
        );

        Ok(leader)
    }

    /// Add a new validator for the inclusive range `[height..height+validity]`.
    pub fn add_validator(&mut self, validator: PublicKey, height: Height, validity: u64) {
        self.metadata.add_validator(validator, height, validity);
    }

    pub(crate) async fn propose<K, M>(
        &mut self,
        keychain: &K,
        moderator: &mut M,
    ) -> Result<(), Error>
    where
        K: Keychain,
        M: Moderator,
    {
        let height = self.height();
        let now = moderator.now();
        let round = self.round(now);

        #[cfg(feature = "trace")]
        tracing::trace!("propose request for height {} round {}", height, round);

        let public = keychain
            .public(height)
            .map_err(|_| Error::ResourceNotAvailable)?
            .ok_or(Error::NotRoundValidator)?;

        // Sanity check
        debug_assert_eq!(public.as_ref(), self.leader(round)?);

        // If the block is not authorized, send `awaiting` event
        let block_id = match self.metadata.authorized_propose(height) {
            Some(b) => *b,
            None => {
                #[cfg(feature = "trace")]
                tracing::trace!("propose blocked for height {} round {}", height, round);

                let awaiting = Message::Event(Event::AwaitingBlock { height });

                moderator.send(awaiting, self.timeout).await;

                return Ok(());
            }
        };

        if self.metadata.commit(height, round) {
            #[cfg(feature = "trace")]
            tracing::debug!("propose authorized for height {} round {}", height, round);

            let vote = Vote::signed(keychain, height, round, block_id, Step::Propose)?;
            let vote = Message::Event(Event::Broadcast { vote });

            moderator.send(vote, self.timeout).await;

            // Always commit to own blocks
            let vote = Vote::signed(keychain, height, round, block_id, Step::Commit)?;
            let vote = Message::Event(Event::Broadcast { vote });

            moderator.send(vote, self.timeout).await;

            let event = Message::Event(Event::Commit {
                height,
                round,
                block_id,
            });

            moderator.send(event, self.timeout).await;
        }

        Ok(())
    }

    pub(crate) async fn upgrade_step<K, M>(
        &mut self,
        keychain: &K,
        moderator: &mut M,
        height: Height,
        round: Round,
        block_id: Bytes32,
        step: Step,
    ) -> Result<(), Error>
    where
        K: Keychain,
        M: Moderator,
    {
        #[cfg(feature = "trace")]
        tracing::trace!(
            "starting upgrade state request for height {} round {}: {:?}",
            height,
            round,
            step,
        );

        let vote = Vote::signed(keychain, height, round, block_id, step)?;
        let is_upgraded = self.metadata.upgrade_validator_step(&vote);
        if !is_upgraded {
            // State not affected; ignore
            return Ok(());
        }

        let vote = Message::Event(Event::Broadcast { vote });

        moderator.send(vote, self.timeout).await;

        if step.is_commit() && self.metadata.commit(height, round) {
            let event = Message::Event(Event::Commit {
                height,
                round,
                block_id,
            });

            moderator.send(event, self.timeout).await;

            let height = self.height();
            let round = 0;

            let public = keychain
                .public(height)
                .map_err(|_| Error::ResourceNotAvailable)?
                .ok_or(Error::NotRoundValidator)?;

            // Check if its the next round leader
            let leader = self.leader(round)?;
            let is_leader = leader == public.as_ref();

            // If its not the leader, just start a new round
            if !is_leader {
                // async recursion currently not supported without Box hacks
                // Better just update state and broadcast vote - otherwise should call upgrade_step
                // again
                let vote =
                    Vote::signed(keychain, height, round, Bytes32::zeroed(), Step::NewRound)?;
                let is_upgraded = self.metadata.upgrade_validator_step(&vote);

                if is_upgraded {
                    let vote = Message::Event(Event::Broadcast { vote });
                    moderator.send(vote, self.timeout).await;
                }

                return Ok(());
            }

            self.propose(keychain, moderator).await?;
        }

        Ok(())
    }

    pub(crate) async fn receive_vote<K, M>(
        &mut self,
        keychain: &K,
        moderator: &mut M,
        vote: Vote,
    ) -> Result<(), Error>
    where
        K: Keychain,
        M: Moderator,
    {
        let height = vote.height();
        let round = vote.round();
        let validator = vote.validator();
        let block_id = vote.block_id();
        let mut proposed_step = vote.step();
        let public = keychain
            .public(height)
            .map_err(|_| Error::ResourceNotAvailable)?
            .ok_or(Error::NotRoundValidator)?;

        let now = moderator.now();

        let expected_height = self.height();
        let expected_round = self.round(now);

        // Ignore messages produced by self
        if validator == public.as_ref() {
            return Ok(());
        }

        // Ignore old steps
        if height < expected_height || round < expected_round {
            return Ok(());
        }

        // Requeue future steps
        if height > expected_height || round > expected_round {
            let vote = Message::Notification(Notification::Vote { vote });

            // FIXME maybe limit the height different for requeue? It could be an attack vector
            // since more queued votes than the capacity would block the reactor
            moderator.requeue(vote, self.timeout).await;

            return Ok(());
        }

        #[cfg(feature = "trace")]
        tracing::trace!(
            "receiving vote: height {}, round {}, validator {:08x}, step: {:?}",
            height,
            round,
            validator,
            proposed_step
        );

        if !self.metadata.validate::<K>(&vote).is_ok() {
            #[cfg(feature = "trace")]
            tracing::trace!(
                "dropping received invalid vote - height {}, round {}, author {:08x}, step: {:?}",
                height,
                round,
                validator,
                proposed_step
            );

            let bad_vote = Message::Event(Event::BadVote { vote });

            moderator.send(bad_vote, self.timeout).await;

            return Ok(());
        }

        #[cfg(feature = "trace")]
        tracing::trace!(
            "vote validated - height {}, round {}, author {:08x}, step: {:?}",
            height,
            round,
            validator,
            proposed_step
        );

        let validators = self.metadata.validators_at_height_count(height);
        let is_bft = Consensus::is_bft(validators);
        let validator_step = self.validator_step(height, round, validator);

        match validator_step {
            // Can discard any previous state since it won't affect the current consensus state
            Some(s) if s > proposed_step => {
                #[cfg(feature = "trace")]
                tracing::trace!(
                    "vote discarded: height {}, round {}, author {:08x}, state: {:?}",
                    height,
                    round,
                    validator,
                    proposed_step
                );

                return Ok(());
            }

            _ if !is_bft => {
                #[cfg(feature = "trace")]
                tracing::trace!(
                    "vote rejected, not enough validators for the round: height {}, round {}, author {:08x}, state: {:?}, validators: {}",
                    height,
                    round,
                    validator,
                    proposed_step,
                    validators
                );

                return Ok(());
            }

            _ => (),
        }

        if proposed_step.is_propose() {
            let leader = self.leader(round)?;
            let proposer_is_leader = validator == leader;

            // Only the round leader should propose
            if !proposer_is_leader {
                let bad_vote = Message::Event(Event::BadVote { vote });

                moderator.send(bad_vote, self.timeout).await;

                return Ok(());
            }

            if !self.metadata.is_block_authorized(block_id, height) {
                #[cfg(feature = "trace")]
                tracing::trace!(
                    "block not authorized - height {}, round {}, author {:08x}, state: {:?}",
                    height,
                    round,
                    validator,
                    proposed_step
                );

                let vote = Message::Notification(Notification::Vote { vote });

                // Block isn't authorized yet; should wait
                moderator.requeue(vote, self.timeout).await;

                return Ok(());
            }

            #[cfg(feature = "trace")]
            tracing::trace!(
                "block authorized - height {}, round {}, author {:08x}, state: {:?}",
                height,
                round,
                validator,
                proposed_step
            );

            self.metadata.upgrade_validator_step(&vote);

            // Should upgrade to prevote; vote was authorized via block notification
            self.upgrade_step(keychain, moderator, height, round, *block_id, Step::Prevote)
                .await?;

            return Ok(());
        }

        self.metadata.upgrade_validator_step(&vote);

        // Evaluate the count considering the vote of the current node
        let approved = 1 + self
            .metadata
            .evaluate_step_count(height, round, proposed_step);

        let consensus = Consensus::evaluate(validators, approved);

        // Upgrade to highest available consensus
        if consensus.is_consensus() {
            while let Some(next_step) = proposed_step.increment() {
                let approved = 1 + self.metadata.evaluate_step_count(height, round, next_step);
                let next_consensus = Consensus::evaluate(validators, approved);

                if next_consensus.is_consensus() {
                    proposed_step = next_step;
                } else {
                    break;
                }
            }
        }

        #[cfg(feature = "trace")]
        tracing::trace!(
            "receiving vote: height {}, round {}, author {:08x}, step: {:?}, consensus {:?}",
            height,
            round,
            validator,
            proposed_step,
            consensus
        );

        let current_step = self.validator_step(height, round, public.as_ref());

        match consensus {
            Consensus::Inconclusive if current_step.is_none() => {
                self.upgrade_step(
                    keychain,
                    moderator,
                    height,
                    round,
                    *block_id,
                    Step::initial(),
                )
                .await?;
            }

            Consensus::Inconclusive => (),

            Consensus::Consensus if proposed_step.is_precommit() || proposed_step.is_commit() => {
                self.upgrade_step(keychain, moderator, height, round, *block_id, Step::Commit)
                    .await?;
            }

            Consensus::Consensus => {
                if let Some(step) = proposed_step.increment() {
                    self.upgrade_step(keychain, moderator, height, round, *block_id, step)
                        .await?;
                }
            }

            Consensus::Reject => {
                #[cfg(feature = "trace")]
                tracing::trace!(
                    "vote rejected, not enough validators for the round: height {}, round {}, author {:08x}, state: {:?}, validators: {}",
                    height,
                    round,
                    validator,
                    proposed_step,
                    validators
                );
            }
        }

        #[cfg(feature = "trace")]
        tracing::trace!(
            "vote processed: height {}, round {}, author {:08x}, state: {:?}",
            height,
            round,
            validator,
            proposed_step
        );

        Ok(())
    }

    pub(crate) async fn receive_event(&mut self, _event: Event) {
        #[cfg(feature = "trace")]
        tracing::warn!("inbound events are not expected; ignored {:?}", _event);
    }

    pub(crate) async fn receive_notification<K, M>(
        &mut self,
        keychain: &K,
        moderator: &mut M,
        notification: Notification,
    ) where
        K: Keychain,
        M: Moderator,
    {
        match notification {
            Notification::Kill => self.should_quit = true,

            Notification::NewValidator {
                height,
                validity,
                validator,
            } => self.add_validator(validator, height, validity),

            Notification::Vote { vote } => {
                if let Err(_e) = self.receive_vote(keychain, moderator, vote).await {
                    #[cfg(feature = "trace")]
                    tracing::error!("error receiving vote: {}", _e);
                }
            }

            Notification::BlockAuthorized { height, block_id } => {
                #[cfg(feature = "trace")]
                tracing::debug!("block authorized for height {}", height);

                self.metadata.authorize_block(block_id, height)
            }

            Notification::BlockProposeAuthorized { height, block_id } => {
                #[cfg(feature = "trace")]
                tracing::debug!("block propose authorized for height {}", height);

                self.metadata.authorize_block_propose(height, block_id)
            }
        }
    }

    pub(crate) async fn receive_request<K, M>(
        &mut self,
        keychain: &K,
        moderator: &mut M,
        request: Request,
    ) where
        K: Keychain,
        M: Moderator,
    {
        let response = match request {
            Request::Commit { id, height, round } => Response::Commit {
                id,
                committed: self.commit(moderator, height, round).await,
            },

            Request::Identity { id, height } => Response::Identity {
                id,
                public: keychain
                    .public(height)
                    .ok()
                    .flatten()
                    .map(|k| k.into_owned()),
            },

            Request::Initialize {
                id,
                start,
                validity,
            } => Response::Initialize {
                id,
                initialized: keychain
                    .public(start)
                    .ok()
                    .flatten()
                    .map(|public| self.add_validator(public.into_owned(), start, validity))
                    .is_some(),
            },

            Request::Round { id } => {
                let height = self.height();
                let round = self.round(moderator.now());
                let leader = self.leader(round).copied().unwrap_or_default();
                let public = keychain
                    .public(height)
                    .unwrap_or_default()
                    .map(|p| p.into_owned())
                    .unwrap_or_default();
                let step = self.validator_step(height, round, &public);

                Response::Round {
                    id,
                    height,
                    round,
                    leader,
                    step,
                }
            }
        };

        let response = Message::Response(response);

        moderator.send(response, self.timeout).await;
    }

    /// Receive a new message, mutating the internal state
    pub async fn receive<K, M>(&mut self, keychain: &K, moderator: &mut M, message: Message)
    where
        K: Keychain,
        M: Moderator,
    {
        #[cfg(feature = "trace")]
        tracing::trace!("receiving message {:?}", message);

        match message {
            Message::Event(e) => self.receive_event(e).await,
            Message::Notification(n) => self.receive_notification(keychain, moderator, n).await,
            Message::Request(r) => self.receive_request(keychain, moderator, r).await,
            Message::Response(_) => (),
        }
    }

    /// Check the current status of the reactor, producing an event, if applicable
    pub async fn heartbeat<K, M>(&mut self, keychain: &K, moderator: &mut M) -> Result<(), Error>
    where
        K: Keychain,
        M: Moderator,
    {
        let height = self.height();

        #[cfg(feature = "trace")]
        tracing::trace!("heartbeat height {}", height);

        // If no public key is available in the keychain, the node is idle
        let public = match keychain
            .public(height)
            .map_err(|_| Error::ResourceNotAvailable)?
        {
            Some(p) => p,
            None => {
                let idle = Message::Event(Event::Idle);

                // FIXME maybe clean all queues?
                moderator.send(idle, self.timeout).await;

                if let Ok(Some(m)) = moderator.inbound().await {
                    moderator.rebound(m, self.timeout).await.ok();
                }

                return Ok(());
            }
        };

        #[cfg(feature = "trace")]
        tracing::trace!("heartbeat height {} drain queue", height);

        // Clear the inbound queue
        let mut queue = Vec::with_capacity(self.capacity);

        while let Some(m) = moderator.inbound().await.ok().flatten() {
            queue.push(m);
        }

        for m in queue {
            self.receive(keychain, moderator, m).await;

            if self.should_quit() {
                return Ok(());
            }
        }

        let now = moderator.now();
        let round = self.round(now);

        #[cfg(feature = "trace")]
        tracing::trace!("heartbeat height {} check propose", height);

        // Since a node automatically upgrade to commit when it proposes a block, it means that if
        // this node is the current round leader, it didn't propose a block yet
        if let Ok(leader) = self.leader(round) {
            // The consumer of the outbound messages should be aware that this reactor will send the
            // request for a new block multiple times - once per heartbeat
            if leader == public.as_ref() {
                #[cfg(feature = "trace")]
                tracing::trace!("round leader height {} from heartbeat", height);

                self.propose(keychain, moderator).await?;
            }
        }

        Ok(())
    }
}
