use crate::{Error, Height};

use fuel_crypto::PublicKey;
use fuel_types::Bytes64;
use hashbrown::HashMap;

use core::ops::{Range, RangeBounds};

mod stake_keys;

use stake_keys::StakeKeys;

/// Registered stake for a validator
///
/// The used key might not reflect the canonical validators set and will be used only to verify the
/// signatures with the chosen protocol.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stake {
    /// One-time key for a height range
    pub key: PublicKey,
    /// Staked value
    pub value: u64,
}

/// A stake pool, mapping a validator identity to a set of bft stakes.
///
/// The validator identity is agnostic to this library and the only requirements is it fits in
/// [`Bytes64`]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct StakePool {
    validators: HashMap<Bytes64, StakeKeys>,
}

impl StakePool {
    /// Add a stake that will be valid within the provided height bounds.
    ///
    /// The validator is the permanent identity of the staker, while the stake key is the volatile
    /// per-height key.
    ///
    /// If the bounds intersect with an existing stake, they will merged if, and only if, the key
    /// and value matches. Otherwise, the function will halt with a duplicated stake error.
    pub fn stake<B>(&mut self, validator: Bytes64, bounds: B, stake: Stake) -> Result<(), Error>
    where
        B: RangeBounds<Height>,
    {
        self.validators
            .entry(validator)
            .or_default()
            .add_stake_range(bounds, stake)
    }

    /// Remove all stake entries for the given validator, if present
    pub fn clear(&mut self, validator: &Bytes64) {
        if let Some(staked) = self.validators.get_mut(validator) {
            staked.clear()
        }
    }

    /// Return a stake for a given height
    pub fn fetch(&self, validator: &Bytes64, height: Height) -> Option<&Stake> {
        self.validators
            .get(validator)
            .and_then(|staked| staked.fetch(height))
    }

    /// Remove all entries that matches the stake key.
    pub fn purge_key(&mut self, key: &PublicKey) {
        self.validators
            .values_mut()
            .for_each(|staked| staked.purge_key(key));
    }

    /// Return the total staked value for a given height.
    pub fn total_staked(&self, height: Height) -> u64 {
        self.iter()
            .filter_map(|(_, range, stake)| range.contains(&height).then(|| stake.value))
            .sum()
    }

    /// Iter the validator, ranges and stakes
    pub fn iter(&self) -> impl Iterator<Item = (&Bytes64, &Range<Height>, &Stake)> {
        self.validators.iter().flat_map(|(validator, staked)| {
            staked
                .iter()
                .map(move |(range, stake)| (validator, range, stake))
        })
    }

    /// Attempt to create a keys set from an iterator, calling [`Self::stake`] for each
    /// item.
    pub fn try_from_iter<B, T>(iter: T) -> Result<Self, Error>
    where
        B: RangeBounds<Height>,
        T: IntoIterator<Item = (Bytes64, B, Stake)>,
    {
        iter.into_iter()
            .try_fold(Self::default(), |mut pool, (validator, range, stake)| {
                pool.stake(validator, range, stake)?;

                Ok(pool)
            })
    }
}
