use fuel_bft::*;
use fuel_bft_mock::*;

fn password(seed: u64) -> Vec<u8> {
    seed.to_be_bytes().to_vec()
}

fn simulate_network(keystore: &MockKeystore, network: &mut MockNetwork, nodes: u64) {
    let start = 0;

    for node in 0..nodes {
        let pass = password(node);
        let node =
            MockNode::new(keystore.clone(), pass, start, nodes).expect("Failed to create node");

        network
            .insert_node(node)
            .expect("Failed to add validator to the network");
    }

    for node in 0..nodes {
        let pass = password(node);
        let public = keystore
            .keystore_public(pass)
            .expect("Node was included")
            .expect("Key was not found");

        let round = HeightRound::start(node);
        let round_validator = RoundValidator::new(round, public.clone().into_owned());
        let node = network.node(public.as_ref()).expect("Node added");
        let state = node.validator_state(&round_validator);

        assert!(state.is_none());
    }

    // Propose a block with the current round leader
    let round = HeightRound::start(0);
    let leader = network.leader(&round).expect("Failed to determine leader");

    let node = network
        .node(&leader)
        .expect("Expected leader for first round");

    let block = network
        .generate_block(&round, leader)
        .expect("Failed to create block");

    let key = node.id(&round).expect("Expected ID");
    let message = MockMessage::signed(&keystore, &key, round, State::Propose, block)
        .expect("Failed to create message");

    let round_validator = RoundValidator::new(round, leader);

    network.override_state(&round_validator, State::Commit);

    network
        .broadcast(&message)
        .expect("Failed to broadcast message");
}

#[test]
fn consensus() {
    let nodes = 6u64;

    let keystore = MockKeystore::default();
    let mut network = MockNetwork::default();

    simulate_network(&keystore, &mut network, nodes);

    for round in 0..nodes {
        for node in 0..nodes {
            let round = HeightRound::start(round);

            let pass = password(node);
            let public = keystore
                .keystore_public(pass)
                .expect("Node was included")
                .expect("Key was not found")
                .into_owned();

            let round_validator = RoundValidator::new(round, public);
            let state = network
                .node(&public)
                .expect("Node was included")
                .validator_state(&round_validator);

            assert_eq!(Some(State::Commit), state);
        }
    }
}

#[test]
fn consensus_fails() {
    let nodes = 3u64;

    let keystore = MockKeystore::default();
    let mut network = MockNetwork::default();

    simulate_network(&keystore, &mut network, nodes);

    // Assert all nodes rejected the round due to insufficient validators
    for node in 0..nodes {
        let round = HeightRound::start(0);

        let pass = password(node);
        let public = keystore
            .keystore_public(pass)
            .expect("Node was included")
            .expect("Key was not found")
            .into_owned();

        let round_validator = RoundValidator::new(round, public);
        let state = network
            .node(&public)
            .expect("Node was included")
            .validator_state(&round_validator);

        assert_eq!(Some(State::Reject), state);
    }
}
