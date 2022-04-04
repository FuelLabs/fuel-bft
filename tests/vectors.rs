use fuel_bft::*;

use async_trait::async_trait;
use fuel_crypto::{Hasher, PublicKey, SecretKey};
use fuel_types::Bytes32;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use time::OffsetDateTime;
use tokio::runtime::Runtime;
use yaml_rust::{Yaml, YamlLoader};

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

fn named_secret<P>(password: P) -> SecretKey
where
    P: AsRef<[u8]>,
{
    let seed = Hasher::hash(password);
    let rng = &mut StdRng::from_seed(*seed);

    SecretKey::random(rng)
}

pub struct DummyModerator {
    pub time: OffsetDateTime,
    pub rng: StdRng,

    inbound: Vec<Message>,
    outbound: Vec<Message>,
}

impl Default for DummyModerator {
    fn default() -> Self {
        Self {
            time: OffsetDateTime::UNIX_EPOCH,
            rng: StdRng::seed_from_u64(8586),
            inbound: Vec::with_capacity(Config::DEFAULT_CAPACITY),
            outbound: Vec::with_capacity(Config::DEFAULT_CAPACITY),
        }
    }
}

impl DummyModerator {
    pub async fn flush(&mut self, keychain: &mut MemoryKeychain, reactor: &mut Reactor) {
        let inbound: Vec<Message> = self.inbound.drain(..).collect();

        for m in inbound {
            reactor.receive(keychain, self, m).await;
        }

        let outbound: Vec<Message> = self.outbound.drain(..).collect();

        for m in outbound {
            match m {
                _ => (),
            }
        }
    }

    pub fn notify(
        &mut self,
        runtime: &Runtime,
        keychain: &MemoryKeychain,
        reactor: &mut Reactor,
        notification: Notification,
    ) {
        runtime.block_on(async {
            reactor
                .receive(keychain, self, Message::Notification(notification))
                .await;
        });
    }

    pub fn request(
        &mut self,
        runtime: &Runtime,
        keychain: &MemoryKeychain,
        reactor: &mut Reactor,
        request: Request,
    ) -> Response {
        let id = request.id();

        runtime.block_on(async {
            reactor
                .receive(keychain, self, Message::Request(request))
                .await;
        });

        let index = self
            .outbound
            .as_slice()
            .iter()
            .enumerate()
            .find_map(|(i, x)| matches!(x, Message::Response(r) if r.id() == id).then(|| i))
            .unwrap_or(usize::MAX);

        let response = self.outbound.swap_remove(index);

        match response {
            Message::Response(r) => r,
            _ => unreachable!(),
        }
    }

    pub fn take_event<F>(&mut self, f: F) -> Option<Message>
    where
        F: Fn(&Event) -> bool,
    {
        self.outbound
            .iter()
            .enumerate()
            .find_map(|(i, m)| {
                match m {
                    Message::Event(e) => f(e),
                    _ => false,
                }
                .then(|| i)
            })
            .map(|i| self.outbound.remove(i))
    }
}

#[async_trait]
impl Moderator for DummyModerator {
    type Error = Error;

    fn now(&self) -> OffsetDateTime {
        self.time
    }

    async fn inbound(&mut self) -> Result<Option<Message>, Self::Error> {
        self.inbound_blocking()
    }

    fn inbound_blocking(&mut self) -> Result<Option<Message>, Self::Error> {
        Ok(self.inbound.pop())
    }

    async fn outbound(
        &mut self,
        message: Message,
        _timeout: std::time::Duration,
    ) -> Result<(), Self::Error> {
        self.outbound.push(message);

        Ok(())
    }

    async fn rebound(
        &mut self,
        message: Message,
        _timeout: std::time::Duration,
    ) -> Result<(), Self::Error> {
        self.inbound.push(message);

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validator {
    Keyed(String),
    Named(String),
}

impl Validator {
    pub const DEFAULT_NODE: &'static str = "some-hard-password";
    pub const DEFAULT_VALIDATOR_A: &'static str = "validator_1";
    pub const DEFAULT_VALIDATOR_B: &'static str = "validator_2";
    pub const DEFAULT_VALIDATOR_C: &'static str = "validator_3";

    pub fn default_sorted() -> Vec<Self> {
        // Convenience list that happens to be sorted as public keys
        vec![
            Self::Named(Self::DEFAULT_VALIDATOR_A.into()),
            Self::Named(Self::DEFAULT_VALIDATOR_B.into()),
            Self::Named(Self::DEFAULT_VALIDATOR_C.into()),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Integer(u64),
    String(String),
    Secret(SecretKey),
    Step(Step),
    Validator(PublicKey),
}

impl Token {
    pub fn get(mut y: &Yaml, path: &str) -> Option<Self> {
        let split = path.split('.');
        let count = split.clone().count();

        for (i, x) in split.enumerate() {
            match y
                .as_hash()
                .map(|h| h.get(&Yaml::String(x.into())))
                .flatten()
            {
                Some(v) => y = v,
                None => return None,
            };

            let is_last = i == count - 1;
            if is_last && x == "validator" {
                let y = y
                    .as_hash()
                    .expect("a validator token must be a dictionary with name or key");

                if let Some(Yaml::String(key)) = y.get(&Yaml::String("key".into())) {
                    return Some(Self::Validator(
                        PublicKey::from_str(key).expect("invalid validator key"),
                    ));
                }

                if let Some(Yaml::String(name)) = y.get(&Yaml::String("name".into())) {
                    return Some(match name.as_str() {
                        "defaultNode" => {
                            Self::Validator(named_secret(Validator::DEFAULT_NODE).public_key())
                        }
                        "defaultValidatorA" => Self::Validator(
                            named_secret(Validator::DEFAULT_VALIDATOR_A).public_key(),
                        ),
                        "defaultValidatorB" => Self::Validator(
                            named_secret(Validator::DEFAULT_VALIDATOR_B).public_key(),
                        ),
                        "defaultValidatorC" => Self::Validator(
                            named_secret(Validator::DEFAULT_VALIDATOR_C).public_key(),
                        ),
                        _ => Self::Validator(named_secret(name).public_key()),
                    });
                }

                panic!("a validator token must contain either key or name");
            }

            if is_last && x == "secret" {
                let y = y
                    .as_hash()
                    .expect("a secret token must be a dictionary with name or key");

                if let Some(Yaml::String(key)) = y.get(&Yaml::String("key".into())) {
                    return Some(Self::Secret(
                        SecretKey::from_str(key).expect("invalid secret key"),
                    ));
                }

                if let Some(Yaml::String(name)) = y.get(&Yaml::String("name".into())) {
                    return Some(match name.as_str() {
                        "defaultNode" => Self::Secret(named_secret(Validator::DEFAULT_NODE)),
                        "defaultValidatorA" => {
                            Self::Secret(named_secret(Validator::DEFAULT_VALIDATOR_A))
                        }
                        "defaultValidatorB" => {
                            Self::Secret(named_secret(Validator::DEFAULT_VALIDATOR_B))
                        }
                        "defaultValidatorC" => {
                            Self::Secret(named_secret(Validator::DEFAULT_VALIDATOR_C))
                        }
                        _ => Self::Validator(named_secret(name).public_key()),
                    });
                }

                panic!("a secret token must contain either key or name");
            }
        }

        let t = match y {
            Yaml::Integer(i) => Self::Integer(*i as u64),

            Yaml::String(s) if s.as_str() == "newRound" => Self::Step(Step::NewRound),
            Yaml::String(s) if s.as_str() == "propose" => Self::Step(Step::Propose),
            Yaml::String(s) if s.as_str() == "prevote" => Self::Step(Step::Prevote),
            Yaml::String(s) if s.as_str() == "precommit" => Self::Step(Step::Precommit),
            Yaml::String(s) if s.as_str() == "commit" => Self::Step(Step::Commit),

            Yaml::String(s) => Self::String(s.clone()),

            _ => unimplemented!(),
        };

        Some(t)
    }

    pub fn integer(self) -> u64 {
        match self {
            Self::Integer(n) => n,
            _ => panic!("expected integer"),
        }
    }

    pub fn secret(self) -> SecretKey {
        match self {
            Self::Secret(k) => k,
            _ => panic!("expected secret"),
        }
    }

    pub fn step(self) -> Step {
        match self {
            Self::Step(s) => s,
            _ => panic!("expected step"),
        }
    }

    pub fn string(self) -> String {
        match self {
            Self::String(s) => s,
            _ => panic!("expected string"),
        }
    }

    pub fn validator(self) -> PublicKey {
        match self {
            Self::Validator(k) => k,
            _ => panic!("expected validator"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    AddValidator {
        validator: PublicKey,
        height: Height,
        validity: u64,
    },
    AddDefaultValidators {
        height: Height,
        validity: u64,
    },
    AssertHeight {
        height: Height,
    },
    AssertNoValidators,
    AssertRound {
        round: Round,
    },
    AssertRoundValidatorWasLeader {
        validator: PublicKey,
        round: Round,
    },
    AssertStep {
        validator: PublicKey,
        height: Height,
        round: Round,
        step: Step,
    },
    AssertValidatorIsLeader {
        validator: PublicKey,
    },
    AuthorizeBlock {
        block_id: Bytes32,
        height: Height,
    },
    AuthorizeBlockPropose {
        block_id: Bytes32,
        height: Height,
    },
    Commit,
    ExpectBlockRequest {
        height: Height,
    },
    ExpectCommit {
        block_id: Bytes32,
        height: Height,
        round: Round,
    },
    ExpectVote {
        block_id: Bytes32,
        height: Height,
        round: Round,
        step: Step,
        validator: PublicKey,
    },
    Flush,
    Heartbeat,
    InitializeDefault,
    Initialize {
        password: String,
    },
    SkipRounds {
        rounds: u64,
    },
    Vote {
        block_id: Bytes32,
        height: Height,
        round: Round,
        secret: SecretKey,
        step: Step,
    },
}

impl From<&Yaml> for Statement {
    fn from(y: &Yaml) -> Statement {
        match y {
            Yaml::Hash(h) => {
                if let Some(t) = h.get(&Yaml::String("addValidator".into())) {
                    return Self::AddValidator {
                        validator: Token::get(t, "validator")
                            .expect("addValidator expects a validator argument")
                            .validator(),
                        height: Token::get(t, "height")
                            .expect("addValidator expects a height argument")
                            .integer(),
                        validity: Token::get(t, "validity")
                            .expect("addValidator expects a validity argument")
                            .integer(),
                    };
                }

                if let Some(t) = h.get(&Yaml::String("addDefaultValidators".into())) {
                    return Self::AddDefaultValidators {
                        height: Token::get(t, "height")
                            .expect("addDefaultValidators expects a height argument")
                            .integer(),
                        validity: Token::get(t, "validity")
                            .expect("addDefaultValidators expects a validity argument")
                            .integer(),
                    };
                }

                if let Some(Token::Integer(height)) = Token::get(y, "assertHeight") {
                    return Self::AssertHeight { height };
                }

                if let Some(Token::Integer(round)) = Token::get(y, "assertRound") {
                    return Self::AssertRound { round };
                }

                if let Some(t) = h.get(&Yaml::String("assertRoundValidatorWasLeader".into())) {
                    return Self::AssertRoundValidatorWasLeader {
                        validator: Token::get(t, "validator")
                            .expect("assertRoundValidatorWasLeader expects a validator argument")
                            .validator(),
                        round: Token::get(t, "round")
                            .expect("assertRoundValidatorWasLeader expects a round argument")
                            .integer(),
                    };
                }

                if let Some(t) = h.get(&Yaml::String("assertStep".into())) {
                    return Self::AssertStep {
                        validator: Token::get(t, "validator")
                            .expect("assertStep expects a validator argument")
                            .validator(),
                        height: Token::get(t, "height")
                            .expect("assertStep expects a height argument")
                            .integer(),
                        round: Token::get(t, "round")
                            .expect("assertStep expects a round argument")
                            .integer(),
                        step: Token::get(t, "step")
                            .expect("assertStep expects a step argument")
                            .step(),
                    };
                }

                if let Some(Token::Validator(validator)) =
                    Token::get(y, "assertValidatorIsLeader.validator")
                {
                    return Self::AssertValidatorIsLeader { validator };
                }

                if let Some(Token::Integer(round)) = Token::get(y, "assertRound") {
                    return Self::AssertRound { round };
                }

                if let Some(t) = h.get(&Yaml::String("authorizeBlock".into())) {
                    return Self::AuthorizeBlock {
                        block_id: Hasher::hash(
                            Token::get(t, "blockSeed")
                                .expect("authorizeBlock expects a blockSeed argument")
                                .string(),
                        ),
                        height: Token::get(t, "height")
                            .expect("authorizeBlock expects a height argument")
                            .integer(),
                    };
                }

                if let Some(t) = h.get(&Yaml::String("authorizeBlockPropose".into())) {
                    return Self::AuthorizeBlockPropose {
                        block_id: Hasher::hash(
                            Token::get(t, "blockSeed")
                                .expect("authorizeBlockPropose expects a blockSeed argument")
                                .string(),
                        ),
                        height: Token::get(t, "height")
                            .expect("authorizeBlockPropose expects a height argument")
                            .integer(),
                    };
                }

                if let Some(t) = h.get(&Yaml::String("expectBlockRequest".into())) {
                    return Self::ExpectBlockRequest {
                        height: Token::get(t, "height")
                            .expect("expectBlockRequest expects a height argument")
                            .integer(),
                    };
                }

                if let Some(t) = h.get(&Yaml::String("expectCommit".into())) {
                    return Self::ExpectCommit {
                        block_id: Hasher::hash(
                            Token::get(t, "blockSeed")
                                .expect("expectCommit expects a blockSeed argument")
                                .string(),
                        ),
                        height: Token::get(t, "height")
                            .expect("expectCommit expects a height argument")
                            .integer(),
                        round: Token::get(t, "round")
                            .expect("expectCommit expects a round argument")
                            .integer(),
                    };
                }

                if let Some(t) = h.get(&Yaml::String("expectVote".into())) {
                    return Self::ExpectVote {
                        block_id: Hasher::hash(
                            Token::get(t, "blockSeed")
                                .expect("expectVote expects a blockSeed argument")
                                .string(),
                        ),
                        height: Token::get(t, "height")
                            .expect("expectVote expects a height argument")
                            .integer(),
                        round: Token::get(t, "round")
                            .expect("expectVote expects a round argument")
                            .integer(),
                        step: Token::get(t, "step")
                            .expect("expectVote expects a step argument")
                            .step(),
                        validator: Token::get(t, "validator")
                            .expect("assertRoundValidatorWasLeader expects a validator argument")
                            .validator(),
                    };
                }

                if let Some(Token::String(password)) = Token::get(y, "initialize.password") {
                    return Self::Initialize { password };
                }

                if let Some(Token::Integer(rounds)) = Token::get(y, "skipRounds") {
                    return Self::SkipRounds { rounds };
                }

                if let Some(t) = h.get(&Yaml::String("vote".into())) {
                    return Self::Vote {
                        block_id: Hasher::hash(
                            Token::get(t, "blockSeed")
                                .expect("expectVote expects a blockSeed argument")
                                .string(),
                        ),
                        height: Token::get(t, "height")
                            .expect("expectVote expects a height argument")
                            .integer(),
                        round: Token::get(t, "round")
                            .expect("expectVote expects a round argument")
                            .integer(),
                        secret: Token::get(t, "secret")
                            .expect("vote expects a secret argument")
                            .secret(),
                        step: Token::get(t, "step")
                            .expect("expectVote expects a step argument")
                            .step(),
                    };
                }

                panic!("invalid statement {:?}", h)
            }

            Yaml::String(s) if s == "assertNoValidators" => Self::AssertNoValidators,

            Yaml::String(s) if s == "commit" => Self::Commit,

            Yaml::String(s) if s == "flush" => Self::Flush,

            Yaml::String(s) if s == "heartbeat" => Self::Heartbeat,

            Yaml::String(s) if s == "initializeDefault" => Self::InitializeDefault,

            _ => panic!("invalid statement {:?}", y),
        }
    }
}

impl Statement {
    pub fn execute(
        self,
        runtime: &Runtime,
        moderator: &mut DummyModerator,
        keychain: &mut MemoryKeychain,
        reactor: &mut Reactor,
    ) {
        match self {
            Statement::AddValidator {
                validator,
                height,
                validity,
            } => {
                reactor.add_validator(validator, height, validity);
            }

            Statement::AddDefaultValidators { height, validity } => [
                Validator::DEFAULT_VALIDATOR_A,
                Validator::DEFAULT_VALIDATOR_B,
                Validator::DEFAULT_VALIDATOR_C,
            ]
            .iter()
            .for_each(|v| {
                Self::execute(
                    Self::AddValidator {
                        validator: named_secret(v).public_key(),
                        height,
                        validity,
                    },
                    runtime,
                    moderator,
                    keychain,
                    reactor,
                )
            }),

            Statement::AssertHeight { height } => {
                assert_eq!(height, reactor.height(), "unexpected height");
            }

            Statement::AssertNoValidators => {
                let round = reactor.round(moderator.time);

                let err = reactor.leader(round).err().expect("no validators expected");
                assert_eq!(Error::ValidatorNotFound, err, "unexpected validator");
            }

            Statement::AssertRound { round } => {
                assert_eq!(round, reactor.round(moderator.time), "unexpected round");
            }

            Statement::AssertRoundValidatorWasLeader { validator, round } => {
                assert_eq!(
                    &validator,
                    reactor
                        .leader(round)
                        .expect("failed to define round leader"),
                    "unexpected leader"
                );
            }

            Statement::AssertStep {
                validator,
                height,
                round,
                step,
            } => {
                let validator_step = reactor
                    .validator_step(height, round, &validator)
                    .expect("The validator is not being tracked");

                assert_eq!(step, validator_step);
            }

            Statement::AssertValidatorIsLeader { validator } => Self::execute(
                Self::AssertRoundValidatorWasLeader {
                    validator,
                    round: reactor.round(moderator.time),
                },
                runtime,
                moderator,
                keychain,
                reactor,
            ),

            Statement::AuthorizeBlock { block_id, height } => {
                moderator.notify(
                    runtime,
                    keychain,
                    reactor,
                    Notification::BlockAuthorized { block_id, height },
                );
            }

            Statement::AuthorizeBlockPropose { block_id, height } => {
                moderator.notify(
                    runtime,
                    keychain,
                    reactor,
                    Notification::BlockProposeAuthorized { block_id, height },
                );
            }

            Statement::Commit => {
                let height = reactor.height();
                let round = reactor.round(moderator.time);
                let id = moderator.rng.gen();

                let response = moderator.request(
                    runtime,
                    keychain,
                    reactor,
                    Request::Commit { id, height, round },
                );

                match response {
                    Response::Commit { committed, .. } if committed => {
                        Self::execute(Self::Flush, runtime, moderator, keychain, reactor);
                    }
                    _ => panic!("unexpected commit response: {:?}", response),
                }
            }

            Statement::ExpectBlockRequest { height } => {
                moderator
                    .take_event(|e| e == &Event::AwaitingBlock { height })
                    .expect("the `AwaitingBlock` event wasn't emitted by the reactor");
            }

            Statement::ExpectCommit {
                block_id,
                height,
                round,
            } => {
                moderator
                    .take_event(|e| {
                        e == &Event::Commit {
                            height,
                            round,
                            block_id,
                        }
                    })
                    .expect("the `Commit` event wasn't emitted by the reactor");
            }

            Statement::ExpectVote {
                block_id,
                height,
                round,
                step,
                validator,
            } => {
                let vote = moderator
                    .take_event(|e| match e {
                        Event::Broadcast { vote } => {
                            vote.block_id() == &block_id
                                && vote.height() == height
                                && vote.round() == round
                                && vote.step() == step
                                && vote.validator() == &validator
                        }

                        _ => false,
                    })
                    .expect("the `Broadcast` event wasn't emitted by the reactor");

                let vote = match vote {
                    Message::Event(Event::Broadcast { vote }) => vote,
                    _ => unreachable!(),
                };

                vote.validate::<MemoryKeychain>()
                    .expect("the received vote isn't valid");
            }

            Statement::Flush => runtime.block_on(async {
                moderator.flush(keychain, reactor).await;
            }),

            Statement::Heartbeat => runtime.block_on(async {
                reactor
                    .heartbeat(keychain, moderator)
                    .await
                    .expect("heartbeat command failed");
            }),

            Statement::InitializeDefault => Self::execute(
                Statement::Initialize {
                    password: Validator::DEFAULT_NODE.into(),
                },
                runtime,
                moderator,
                keychain,
                reactor,
            ),

            Statement::Initialize { password } => {
                keychain.insert(.., password);
                let id = moderator.rng.gen();

                let response = moderator.request(
                    runtime,
                    keychain,
                    reactor,
                    Request::Initialize {
                        id,
                        start: 0,
                        validity: u64::MAX,
                    },
                );

                match response {
                    Response::Initialize { initialized, .. } if initialized => (),
                    _ => panic!("unexpected initialize response: {:?}", response),
                }
            }

            Statement::SkipRounds { rounds } => {
                moderator.time = moderator.time.saturating_add(time::Duration::milliseconds(
                    (Config::DEFAULT_CONSENSUS as u64 * rounds) as i64,
                ))
            }

            Statement::Vote {
                block_id,
                height,
                round,
                secret,
                step,
            } => moderator.notify(
                runtime,
                keychain,
                reactor,
                Notification::Vote {
                    vote: Vote::signed_with_key::<MemoryKeychain>(
                        &secret, height, round, block_id, step,
                    ),
                },
            ),
        }
    }
}

#[test]
fn vectors() {
    let vectors = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("vectors");

    let vectors = fs::read_dir(vectors)
        .expect("failed to read test vectors")
        .filter_map(|d| {
            let path = d.expect("failed to read dir").path();
            let extension = path.extension().and_then(|x| x.to_str()).unwrap_or("");

            match extension.to_lowercase().as_str() {
                "yaml" => Some(path),
                _ => None,
            }
        });

    for program in vectors {
        println!(
            "executing test vector {}",
            program.file_name().and_then(|f| f.to_str()).unwrap_or("")
        );

        let program = fs::read_to_string(program).expect("failed to read program");
        let program = YamlLoader::load_from_str(program.as_str()).expect("invalid yaml");

        let program = program
            .first()
            .expect("no statements found")
            .as_vec()
            .expect("the program expects an array of statements");

        let runtime = &Runtime::new().expect("failed to create async runtime");
        let moderator = &mut DummyModerator::default();
        let keychain = &mut MemoryKeychain::default();
        let reactor = &mut Reactor::default();

        program
            .iter()
            .map(Statement::from)
            .for_each(|s| s.execute(runtime, moderator, keychain, reactor));
    }
}
