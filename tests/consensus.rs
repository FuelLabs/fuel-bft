use fuel_bft::*;
use fuel_bft_mock::*;

#[test]
fn consensus() {
    let nodes = 6;

    let mut network = MockNetwork::default();

    let start = HeightRound::start(0);

    for node in 0..nodes {
        let key = MockKey::from(node);
        let node = MockNode::new(key, start, nodes);

        network.add_validator(node);
    }

    for node in 0..nodes {
        let round = HeightRound::start(node);
        let state = network.validator(node).expect("Node added").state(&round);

        assert!(state.is_none());
    }

    // Propose a block with the current round leader
    let seed = nodes - 1;
    let key = MockKey::from(seed);
    let id = key.validator();

    let node = network
        .validator(seed)
        .expect("Expected leader for first round");

    let block = node.new_block(&network);

    let round = HeightRound::start(0);
    let message = MockMessage::new(round, key, State::Propose, block);

    network
        .validator_mut(seed)
        .expect("Expected leader for first round")
        .upgrade_validator_state(&round, &id, State::Commit);

    network.broadcast(&message);

    for round in 0..nodes {
        for node in 0..nodes {
            let round = HeightRound::start(round);

            let state = network.validator(node).expect("Node added").state(&round);

            assert_eq!(Some(State::Commit), state);
        }
    }

    let round = HeightRound::start(nodes);

    for node in 0..nodes {
        let state = network.validator(node).expect("Node added").state(&round);

        assert_eq!(Some(State::NewRound), state);
    }
}

#[test]
fn consensus_fails() {
    let nodes = 3;

    let mut network = MockNetwork::default();

    let start = HeightRound::start(0);

    for node in 0..nodes {
        let key = MockKey::from(node);
        let node = MockNode::new(key, start, nodes);

        network.add_validator(node);
    }

    let round = HeightRound::start(1);

    assert!(network
        .validator(1)
        .expect("Node added")
        .state(&round)
        .is_none());

    let key = MockKey::from(2);
    let message = MockMessage::new(round, key, State::NewRound, Default::default());

    network.broadcast(&message);

    let state = network.validator(1).expect("Node added").state(&round);

    assert_eq!(Some(State::Reject), state);
}
