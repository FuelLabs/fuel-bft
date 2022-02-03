use crate::{Error, Height, Keychain, Round, Step, Vote};

use fuel_crypto::PublicKey;
use fuel_types::Bytes32;

use alloc::collections::BTreeMap;

/// Consensus metadata
#[derive(Debug, Clone)]
pub struct Metadata {
    committed_height: Height,
    committed_rounds: u64,

    /// Set of authorized blocks for commit
    authorized_blocks: BTreeMap<Bytes32, Height>,

    /// Blocks authorized for the propose protocol.
    propose_blocks: BTreeMap<Height, Bytes32>,

    /// key -> (from, to) inclusive height range
    validators: BTreeMap<PublicKey, (Height, Height)>,

    /// (height, round, key) -> step
    step: BTreeMap<(Height, Round, PublicKey), Step>,
}

impl Default for Metadata {
    fn default() -> Self {
        let committed_height = Self::HEIGHT_NEVER;
        let committed_rounds = 0;

        let authorized_blocks = Default::default();
        let propose_blocks = Default::default();
        let step = Default::default();
        let validators = Default::default();

        Self {
            authorized_blocks,
            committed_height,
            committed_rounds,
            propose_blocks,
            validators,
            step,
        }
    }
}

impl Metadata {
    /// Height representing a `never` step
    pub const HEIGHT_NEVER: Height = Height::MAX;

    pub fn add_validator(&mut self, validator: PublicKey, height: Height, validity: u64) {
        let validity = height + validity;

        match self.validators.get_mut(&validator) {
            Some((from, to)) => {
                *from = height;
                *to = validity;
            }

            None => {
                self.validators.insert(validator, (height, validity));
            }
        }
    }

    /// Authorize the provided block in the given height
    pub fn authorize_block(&mut self, block_id: Bytes32, height: Height) {
        if self.committed_height.wrapping_add(1) <= height {
            self.authorized_blocks.insert(block_id, height);
        }
    }

    /// Authorize the propose protocol for the given height.
    pub fn authorize_block_propose(&mut self, height: Height, block_id: Bytes32) {
        if self.committed_height.wrapping_add(1) <= height {
            self.propose_blocks.insert(height, block_id);
        }
    }

    /// Check if the block is authorized for the given height
    pub fn is_block_authorized(&self, block_id: &Bytes32, height: Height) -> bool {
        self.authorized_blocks
            .get(block_id)
            .filter(|&&h| h == height)
            .is_some()
    }

    /// Return the authorized block for propose for the given height, if present
    pub fn authorized_propose(&self, height: Height) -> Option<&Bytes32> {
        self.propose_blocks.get(&height)
    }

    /// Sorted validators filtered per height.
    pub fn validators_at_height(&self, height: Height) -> impl Iterator<Item = &PublicKey> {
        self.validators
            .iter()
            .filter_map(move |(k, (from, to))| (*from <= height && height <= *to).then(|| k))
    }

    /// Validators count per height.
    pub fn validators_at_height_count(&self, height: Height) -> usize {
        self.validators_at_height(height).count()
    }

    /// Evaluate the step count for a given round, including the validators that are in subsequent
    /// steps.
    pub fn evaluate_step_count(&self, height: Height, round: Round, step: Step) -> usize {
        let current = self.step_count(height, round, step);

        // FIXME optimize
        let subsequent: usize = step
            .map(|s| self.step_count(height, round, s))
            .sum::<usize>();

        current + subsequent
    }

    /// Block height of the last commit
    pub const fn committed_height(&self) -> Height {
        self.committed_height
    }

    /// Total committed rounds
    pub const fn committed_rounds(&self) -> u64 {
        self.committed_rounds
    }

    pub fn commit(&mut self, height: Height, round: Round) -> bool {
        // Commit only to the subsequent block
        if !self.committed_height.wrapping_add(1) == height {
            return false;
        }

        // Remove all expired content
        self.authorized_blocks.retain(|_, h| height < *h);
        self.propose_blocks.retain(|h, _| height < *h);
        self.validators.retain(|_, &mut (_, to)| height < to);
        self.step.retain(|(h, _, _), _| height < *h);

        self.committed_rounds += 1 + round;
        self.committed_height = height;

        true
    }

    /// Step count for a given round
    pub fn step_count(&self, height: Height, round: Round, step: Step) -> usize {
        self.step
            .iter()
            .filter(|((h, r, _), s)| h == &height && r == &round && s == &&step)
            .count()
    }

    /// Validate a vote, checking if the author is a validator of the round, and if the signature is valid.
    pub fn validate<K>(&self, vote: &Vote) -> Result<(), Error>
    where
        K: Keychain,
    {
        let height = vote.height();
        let validator = vote.validator();

        let is_height_validator = self.validators_at_height(height).any(|v| v == validator);
        if !is_height_validator {
            return Err(Error::ValidatorNotFound);
        }

        vote.validate::<K>().map_err(|_| Error::InvalidSignature)?;

        Ok(())
    }

    /// Fetch the current step of a validator for a given round
    pub fn validator_step(&self, height: Height, round: Round, key: &PublicKey) -> Option<Step> {
        self.step.get(&(height, round, *key)).copied()
    }

    /// Upgrade a validator step, returning true if there was a change
    pub fn upgrade_validator_step(&mut self, vote: &Vote) -> bool {
        let height = vote.height();
        let round = vote.round();
        let validator = *vote.validator();
        let step = vote.step();

        let updated = match self.step.get_mut(&(height, round, validator)) {
            Some(s) if &step > s => {
                #[cfg(feature = "trace")]
                tracing::debug!(
                    "upgrading step; validator: {:08x}, height: {}, round: {}, step: {:?}",
                    validator,
                    height,
                    round,
                    step
                );

                *s = step;
                true
            }

            None => {
                #[cfg(feature = "trace")]
                tracing::debug!(
                    "upgrading step; validator: {:08x}, height: {}, round: {}, step: {:?}",
                    validator,
                    height,
                    round,
                    step
                );

                self.step.insert((height, round, validator), step);
                true
            }

            _ => {
                #[cfg(feature = "trace")]
                tracing::trace!(
                    "upgrading step skipped; validator: {:08x}, height: {}, round: {}, step: {:?}",
                    validator,
                    height,
                    round,
                    step
                );

                false
            }
        };

        updated
    }
}
