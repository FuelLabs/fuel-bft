use fuel_bft::*;

#[test]
fn increment() {
    assert!(Step::Commit.increment().is_none());
    assert_eq!(Some(Step::Propose), Step::NewRound.increment());
    assert_eq!(Some(Step::Prevote), Step::Propose.increment());
    assert_eq!(Some(Step::Precommit), Step::Prevote.increment());
    assert_eq!(Some(Step::Commit), Step::Precommit.increment());
}

#[test]
fn ord() {
    assert!(Step::Commit > Step::NewRound);
    assert!(Step::Commit > Step::Propose);
    assert!(Step::Commit > Step::Prevote);
    assert!(Step::Commit > Step::Precommit);
    assert!(Step::Commit == Step::Commit);

    assert!(Step::Precommit > Step::NewRound);
    assert!(Step::Precommit > Step::Propose);
    assert!(Step::Precommit > Step::Prevote);
    assert!(Step::Precommit == Step::Precommit);
    assert!(Step::Precommit < Step::Commit);

    assert!(Step::Prevote > Step::NewRound);
    assert!(Step::Prevote > Step::Propose);
    assert!(Step::Prevote == Step::Prevote);
    assert!(Step::Prevote < Step::Precommit);
    assert!(Step::Prevote < Step::Commit);

    assert!(Step::Propose > Step::NewRound);
    assert!(Step::Propose == Step::Propose);
    assert!(Step::Propose < Step::Prevote);
    assert!(Step::Propose < Step::Precommit);
    assert!(Step::Propose < Step::Commit);

    assert!(Step::NewRound == Step::NewRound);
    assert!(Step::NewRound < Step::Propose);
    assert!(Step::NewRound < Step::Prevote);
    assert!(Step::NewRound < Step::Precommit);
    assert!(Step::NewRound < Step::Commit);
}
