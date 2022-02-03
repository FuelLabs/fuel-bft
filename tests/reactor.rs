use fuel_bft::*;

use fuel_crypto::{PublicKey, SecretKey};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use core::time::Duration;

#[test]
fn current_height_round() {
    let reactor = Reactor::default();

    let height = reactor.height();
    let round = reactor.round(Config::DEFAULT_GENESIS);

    assert_eq!(0, height);
    assert_eq!(0, round);

    // Some arbitrarily large round number
    let rounds = 394820;

    let elapsed = rounds as u128 * Config::DEFAULT_CONSENSUS;
    let elapsed = Duration::from_millis(elapsed as u64);
    let elapsed = time::Duration::try_from(elapsed).expect("Failed to convert time primitive");

    let now = Config::DEFAULT_GENESIS + elapsed;
    let round = reactor.round(now);

    assert_eq!(rounds, round);
}

#[test]
fn leader() {
    let rng = &mut StdRng::seed_from_u64(2322u64);

    let validators = 153;
    let validity = 2500;

    let mut reactor = Reactor::default();

    let validators: Vec<PublicKey> = (0..validators)
        .map(|_| SecretKey::random(rng).public_key())
        .map(|p| {
            reactor.add_validator(p, 0, validity);
            p
        })
        .collect();

    let mut validators_sorted = validators.clone();

    validators_sorted.as_mut_slice().sort();

    let mut current_height = 0;
    let mut current_round = 0;
    let mut current_leader = 0;

    while current_height <= validity {
        assert_eq!(current_height, reactor.height());

        let expected_leader = &validators_sorted[current_leader];
        let leader = reactor
            .leader(current_round)
            .expect("Failed to define leader");

        assert_eq!(expected_leader, leader);

        current_leader += 1;
        if current_leader == validators.len() {
            current_leader = 0;
        }

        // Decide at random if should commit
        let should_commit = rng.gen_range(0..10) < 3;
        if should_commit {
            assert!(reactor.commit(current_height, current_round));

            current_height += 1;
            current_round = 0;
        } else {
            current_round += 1;
        }
    }

    reactor
        .leader(0)
        .err()
        .expect("The validators are expired and no leader should be returned");
}
