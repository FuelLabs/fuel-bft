use fuel_bft::*;

use fuel_crypto::PublicKey;
use fuel_types::Bytes32;

// FIXME CI hardware may not have the expected behavior regarding the timeouts and async order.
//
// Need to refactor this test and make it reproductible in any environment/hardware
#[ignore]
#[tokio::test]
async fn consensus() {
    let validators = 4;
    let validity = validators;

    let validators: Vec<(MemoryKeychain, PublicKey)> = (0..validators - 1)
        .map(|i| {
            let mut k = MemoryKeychain::default();

            k.insert(.., format!("some-hard-password-{}", i));

            let height = 0;
            let public = k
                .public(height)
                .expect("failed to query keychain")
                .expect("failed to extract public key")
                .into_owned();

            (k, public)
        })
        .collect();

    let config = Config::default();
    let password = "some-harder-password";
    let mut reactor = TokioReactor::spawn(config, password);

    // Query the public identity for the initial height
    let public = reactor
        .request(Request::Identity { id: 0, height: 0 })
        .await
        .expect("Failed to request node identity from the reactor");

    let public = match public {
        Response::Identity { public, .. } => public,
        _ => panic!("unexpected response"),
    }
    .expect("The identity of the reactor should be available");

    // Add all validators of the network
    for (_, validator) in validators.iter() {
        let height = 0;
        let validator = *validator;

        reactor
            .notify(Notification::NewValidator {
                height,
                validity,
                validator,
            })
            .await
            .expect("notification failed");
    }

    let response = reactor
        .request(Request::Initialize {
            id: 1,
            start: 0,
            validity,
        })
        .await
        .expect("failed to initialize the reactor");

    match response {
        Response::Initialize { initialized, .. } if initialized => (),
        _ => panic!("unexpected response"),
    }

    let mut current_height = 0;

    while current_height < validity {
        let response = reactor
            .request(Request::Round { id: 2 })
            .await
            .expect("failed to query the round");

        let (round, leader) = match response {
            Response::Round {
                height,
                round,
                leader,
                ..
            } if height == current_height => (round, leader),
            _ => panic!("unexpected height"),
        };

        let block_id = Bytes32::from([current_height as u8; Bytes32::LEN]);

        let current_round = round;
        let current_block_id = block_id;
        let mut commit_found = false;
        let mut event_found = false;

        if leader == public {
            let mut propose_found = false;

            // Expect reactor to request block propose authorization
            while let Some(m) = reactor.next_async().await {
                match m {
                    Message::Event(Event::AwaitingBlock { height }) if height == current_height => {
                        break
                    }
                    _ => (),
                }
            }

            reactor
                .notify(Notification::BlockProposeAuthorized {
                    height: current_height,
                    block_id,
                })
                .await
                .expect("failed to notify");

            // Block authorized, expecting propose and commit
            while let Some(m) = reactor.next_async().await {
                match m {
                    Message::Event(Event::Broadcast { vote })
                        if vote.height() == current_height
                            && vote.round() == round
                            && vote.step() == Step::Propose
                            && vote.validator() == &public
                            && vote.block_id() == &block_id =>
                    {
                        propose_found = true;
                    }
                    Message::Event(Event::Broadcast { vote })
                        if vote.height() == current_height
                            && vote.round() == round
                            && vote.step() == Step::Commit
                            && vote.validator() == &public
                            && vote.block_id() == &block_id =>
                    {
                        commit_found = true;
                    }
                    Message::Event(Event::Commit {
                        height,
                        round,
                        block_id,
                    }) if height == current_height
                        && round == current_round
                        && block_id == current_block_id =>
                    {
                        event_found = true;
                    }
                    _ => (),
                }

                if propose_found && commit_found && event_found {
                    break;
                }
            }
        } else {
            let keychain = validators
                .iter()
                .find_map(|(k, p)| (p == &leader).then(|| k))
                .expect("failed to fetch validator keychain");

            let propose = Vote::signed(keychain, current_height, round, block_id, Step::Propose)
                .expect("failed to create vote");

            let proposer_commit =
                Vote::signed(keychain, current_height, round, block_id, Step::Propose)
                    .expect("failed to create vote");

            reactor
                .notify(Notification::Vote { vote: propose })
                .await
                .expect("failed to notify reactor");

            reactor
                .notify(Notification::Vote {
                    vote: proposer_commit,
                })
                .await
                .expect("failed to notify reactor");

            // Reactor should not update state until block is authorized
            let response = reactor
                .request(Request::Round { id: 3 })
                .await
                .expect("failed to query the round");

            match response {
                Response::Round {
                    height,
                    round,
                    step: None,
                    ..
                } if height == current_height && round == current_round => (),
                _ => panic!("unexpected step"),
            };

            // Authorize the block
            reactor
                .notify(Notification::BlockAuthorized {
                    height: current_height,
                    block_id: current_block_id,
                })
                .await
                .expect("notification failed");

            // Authorized block should move to prevote
            loop {
                let response = reactor
                    .request(Request::Round { id: 4 })
                    .await
                    .expect("failed to query the round");

                match response {
                    Response::Round {
                        height,
                        round,
                        step: Some(Step::Prevote),
                        ..
                    } if height == current_height && round == current_round => break,
                    _ => (),
                };
            }

            // One peer prevote should be enough since proposer is `commit` and node is `prevote`
            // This is BFT consensus for 4 validators
            let keychain = validators
                .iter()
                .find_map(|(k, p)| (p != &leader).then(|| k))
                .expect("failed to fetch validator keychain");

            let prevote = Vote::signed(keychain, current_height, round, block_id, Step::Prevote)
                .expect("failed to create vote");

            reactor
                .notify(Notification::Vote { vote: prevote })
                .await
                .expect("failed to notify reactor");

            // Reactor should be precommit after prevote is done
            let response = reactor
                .request(Request::Round { id: 5 })
                .await
                .expect("failed to query the round");

            match response {
                Response::Round {
                    height,
                    round,
                    step: Some(Step::Precommit),
                    ..
                } if height == current_height && round == current_round => (),
                _ => panic!("unexpected step"),
            };

            // One precommit vote should be enough to commit BFT
            let precommit =
                Vote::signed(keychain, current_height, round, block_id, Step::Precommit)
                    .expect("failed to create vote");

            reactor
                .notify(Notification::Vote { vote: precommit })
                .await
                .expect("failed to notify reactor");

            // Expecting commit
            while let Some(m) = reactor.next_async().await {
                match m {
                    Message::Event(Event::Broadcast { vote })
                        if vote.height() == current_height
                            && vote.round() == current_round
                            && vote.step() == Step::Commit
                            && vote.validator() == &public
                            && vote.block_id() == &current_block_id =>
                    {
                        commit_found = true;
                    }
                    Message::Event(Event::Commit {
                        height,
                        round,
                        block_id,
                    }) if height == current_height
                        && round == current_round
                        && block_id == current_block_id =>
                    {
                        event_found = true;
                    }
                    _ => (),
                }

                if commit_found && event_found {
                    break;
                }
            }

            // After commit, height should be incremented
            let response = reactor
                .request(Request::Round { id: 6 })
                .await
                .expect("failed to query the round");

            match response {
                Response::Round { height, round, .. }
                    if height == current_height.wrapping_add(1) && round == 0 =>
                {
                    ()
                }
                _ => panic!("unexpected round"),
            };
        }

        current_height += 1;
    }
}
