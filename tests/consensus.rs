use fuel_pbft::*;
use fuel_pbft_mock::*;

#[test]
fn consensus() {
    let nodes = 6;

    let mut network = MockNetwork::default();

    for node in 0..nodes {
        network.add_node(node, 0, nodes);
    }

    for node in 0..nodes {
        assert!(network.node(node).expect("Node added").state(0).is_none());
    }

    // Pick a node and start the engine
    let key = MockNetwork::key_from_height(nodes - 1);
    let message = MockMessage::new(0, key, State::NewHeight, Default::default());

    network.broadcast(&message);

    for height in 0..nodes {
        assert_eq!(
            Some(State::Commit),
            network.node(1).expect("Node added").state(height)
        );
    }

    for node in 0..nodes {
        assert_eq!(
            Some(State::NewHeight),
            network.node(node).expect("Node added").state(nodes)
        );
    }
}

#[test]
fn consensus_fails() {
    let mut network = MockNetwork::default();

    // Not enough nodes for pBFT
    network.add_node(1, 1, 100);
    network.add_node(2, 1, 100);
    network.add_node(3, 1, 100);

    assert!(network.node(1).expect("Node added").state(1).is_none());

    let key = MockNetwork::key_from_height(3);
    let message = MockMessage::new(1, key, State::NewHeight, Default::default());

    network.broadcast(&message);

    assert_eq!(
        Some(State::Reject),
        network.node(1).expect("Node added").state(1)
    );
}
