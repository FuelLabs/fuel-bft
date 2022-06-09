use super::Stake;
use crate::{Error, Height};

use fuel_crypto::PublicKey;
use hashbrown::HashMap;

use alloc::borrow::ToOwned;
use core::cmp;
use core::ops::{Bound, Range, RangeBounds};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct StakeKeys {
    keys: HashMap<Range<Height>, Stake>,
}

impl StakeKeys {
    /// The provided key will be used to verify the consensus signatures. It might diverge from the
    /// constant validator identifier that exists for the canonical stake contract since a key
    /// rotation strategy is possible.
    pub(super) fn add_stake_range<B>(&mut self, bounds: B, stake: Stake) -> Result<(), Error>
    where
        B: RangeBounds<Height>,
    {
        let bounds = normalize_range(bounds);

        // Check if an equivalent stake intersects the arguments - if yes, extend it
        if let Some(range) = self
            .keys
            .iter()
            .filter(|(_, &staked)| staked == stake)
            .find_map(|(range, _)| {
                (range.contains(&bounds.start) || range.contains(&bounds.end)).then(|| range)
            })
        {
            let range = range.to_owned();

            let start = cmp::min(range.start, bounds.start);
            let end = cmp::max(range.end, bounds.end);
            let bounds = Range { start, end };

            // Not using the entry manipulation because there is no inherent optimization in doing
            // that for the cost of manually replacing the hash entry bucket of the key.
            let _was_removed = self.keys.remove(&range).is_some();

            debug_assert!(_was_removed);

            self.keys.insert(bounds, stake);

            return Ok(());
        }

        // Extend is not possible because either the key or value differs - intersection is forbidden
        // from this point
        if let Some(range) = self
            .keys
            .keys()
            .find(|range| range.contains(&bounds.start) || range.contains(&bounds.end))
        {
            return Err(Error::DuplicatedStake {
                height: range.start,
                valid_before: range.end,
            });
        }

        self.keys.insert(bounds, stake);

        Ok(())
    }

    /// Remove all stake entries
    pub(super) fn clear(&mut self) {
        self.keys.clear();
    }

    /// Return a stake for a given height
    pub(super) fn fetch(&self, height: Height) -> Option<&Stake> {
        self.keys
            .iter()
            .filter_map(|(range, stake)| range.contains(&height).then(|| stake))
            .next()
    }

    /// Remove all entries with the provided key
    pub(super) fn purge_key(&mut self, key: &PublicKey) {
        self.keys.retain(|_, stake| &stake.key != key);
    }

    /// Iter the ranges and stakes
    pub(super) fn iter(&self) -> impl Iterator<Item = (&Range<Height>, &Stake)> {
        self.keys.iter()
    }
}

fn normalize_range<R>(bounds: R) -> Range<Height>
where
    R: RangeBounds<Height>,
{
    let start = match bounds.start_bound() {
        Bound::Included(s) => *s,
        Bound::Excluded(s) if s == &Height::MIN => Height::MIN,
        Bound::Excluded(s) => s.saturating_add(1),
        Bound::Unbounded => Height::MIN,
    };

    let end = match bounds.end_bound() {
        Bound::Included(e) => e.saturating_add(1),
        Bound::Excluded(e) => *e,
        Bound::Unbounded => Height::MAX,
    };

    Range { start, end }
}

#[test]
#[cfg(feature = "std")]
fn stake_keys_intersect_and_merge() {
    use fuel_crypto::SecretKey;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let rng = &mut StdRng::seed_from_u64(0xbeef);

    // keys
    let a = SecretKey::random(rng).public_key();
    let b = SecretKey::random(rng).public_key();

    // values
    let x = rng.gen();
    let y = rng.gen();

    let ax = Stake { key: a, value: x };
    let ay = Stake { key: a, value: y };
    let by = Stake { key: b, value: y };

    let mut keys = StakeKeys::default();
    keys.add_stake_range(0..2, ax).expect("no intersect");
    keys.add_stake_range(3..5, ay).expect("no intersect");

    let mut keys = StakeKeys::default();
    keys.add_stake_range(0..2, ax).expect("no intersect");
    keys.add_stake_range(3..5, by).expect("no intersect");

    let mut keys = StakeKeys::default();
    keys.add_stake_range(0..2, ax).expect("no intersect");
    keys.add_stake_range(2..4, ay).expect("no intersect");

    let mut keys = StakeKeys::default();
    keys.add_stake_range(0..2, ax).expect("no intersect");
    keys.add_stake_range(2..4, by).expect("no intersect");

    let mut keys = StakeKeys::default();
    keys.add_stake_range(0..2, ax).expect("no intersect");
    keys.add_stake_range(1..3, ay).err().expect("intersect");

    let mut keys = StakeKeys::default();
    keys.add_stake_range(1..3, ax).expect("no intersect");
    keys.add_stake_range(0..2, ay).err().expect("intersect");

    let mut keys = StakeKeys::default();
    keys.add_stake_range(0..2, ax).expect("no intersect");
    keys.add_stake_range(1..3, ax).expect("merge");
    let stake = keys.keys.get(&(0..3)).expect("merged stake");
    assert_eq!(&Stake { key: a, value: x }, stake);

    let mut keys = StakeKeys::default();
    keys.add_stake_range(1..3, ax).expect("no intersect");
    keys.add_stake_range(0..2, ax).expect("merge");
    let stake = keys.keys.get(&(0..3)).expect("merged stake");
    assert_eq!(&Stake { key: a, value: x }, stake);
}
