use crate::{
    Config, Error, MemoryKeychain, Message, Moderator, Notification, Reactor, Request, Response,
};

use async_trait::async_trait;
use tokio::sync::mpsc;

use core::time::Duration;
use std::time::SystemTime;

/// Communication bridge with a consensus reactor.
pub struct TokioReactor {
    timeout: Duration,

    listener: mpsc::Receiver<Message>,
    sender: mpsc::Sender<Message>,

    /// Reactor will dispatch messages to
    outbound: mpsc::Sender<Message>,
}

impl TokioReactor {
    /// Await for the next message sent from a reactor
    pub async fn next_async(&mut self) -> Option<Message> {
        self.listener.recv().await
    }

    /// Send a notification to the reactor
    pub async fn notify(&mut self, notification: Notification) -> Result<(), Error> {
        let notification = Message::Notification(notification);

        self.sender
            .send_timeout(notification, self.timeout)
            .await
            .map_err(|_| Error::ResourceNotAvailable)
    }

    /// Send a request to the reactor
    pub async fn request(&mut self, request: Request) -> Result<Response, Error> {
        let id = request.id();
        let request = Message::Request(request);

        let start = SystemTime::now();
        let mut requeue = Vec::new();

        self.sender
            .send_timeout(request, self.timeout)
            .await
            .map_err(|_| Error::ResourceNotAvailable)?;

        #[cfg(feature = "trace")]
        tracing::debug!(
            "request {:?} sent, awaiting response with timeout {:?}",
            request,
            self.timeout
        );

        loop {
            match start.elapsed() {
                Err(_e) => {
                    requeue.into_iter().for_each(|m| {
                        self.outbound.try_send(m).ok();
                    });

                    #[cfg(feature = "trace")]
                    tracing::debug!("request {} failed with error {}", id, _e);

                    return Err(Error::ResourceNotAvailable);
                }

                Ok(elapsed) if elapsed > self.timeout => {
                    requeue.into_iter().for_each(|m| {
                        self.outbound.try_send(m).ok();
                    });

                    #[cfg(feature = "trace")]
                    tracing::debug!("request {} failed with timeout", id);

                    return Err(Error::ResourceNotAvailable);
                }

                _ => (),
            }

            #[cfg(feature = "trace")]
            tracing::trace!("attempting to receive request {}", id);

            while let Some(m) = self.listener.try_recv().ok() {
                match m {
                    Message::Response(r) if r.id() == id => {
                        requeue.into_iter().for_each(|m| {
                            self.outbound.try_send(m).ok();
                        });

                        return Ok(r);
                    }

                    _ => requeue.push(m),
                }
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    /// Spawn a consensus reactor into a new thread. This struct will communicate with the spawned
    /// reactor.
    pub fn spawn<P>(config: Config, password: P) -> Self
    where
        P: AsRef<[u8]>,
    {
        let Config { heartbeat, .. } = config;

        let password = password.as_ref().to_vec();
        let (mut moderator, bridge) = TokioModerator::new(config);

        tokio::spawn(async move {
            let mut reactor = Reactor::new(config);
            let mut keychain = MemoryKeychain::default();

            keychain.insert(.., password);

            loop {
                let start = std::time::SystemTime::now();

                if let Err(_e) = reactor.heartbeat(&keychain, &mut moderator).await {
                    #[cfg(feature = "trace")]
                    tracing::trace!("heartbeat error: {}", _e);
                }

                if reactor.should_quit() {
                    break;
                }

                let elapsed = start.elapsed().map(|d| d.as_millis()).unwrap_or(0);
                let interval = heartbeat.saturating_sub(elapsed);
                let interval = std::time::Duration::from_millis(interval as u64);

                tokio::time::sleep(interval).await;
            }
        });

        bridge
    }
}

impl Iterator for TokioReactor {
    type Item = Message;

    fn next(&mut self) -> Option<Self::Item> {
        match self.listener.try_recv() {
            Ok(m) => Some(m),
            Err(_) => None,
        }
    }
}

struct TokioModerator {
    /// Reactor will consume messages from
    inbound: mpsc::Receiver<Message>,

    /// Reactor will dispatch messages to
    outbound: mpsc::Sender<Message>,

    /// Reactor will requeue its messages through
    rebound: mpsc::Sender<Message>,
}

impl TokioModerator {
    pub fn new(config: Config) -> (Self, TokioReactor) {
        let Config {
            capacity, timeout, ..
        } = config;

        let (rebound, inbound) = mpsc::channel(capacity);
        let (outbound, listener) = mpsc::channel(capacity);

        let sender = rebound.clone();
        let bridge = TokioReactor {
            timeout,
            listener,
            sender,
            outbound: outbound.clone(),
        };

        let moderator = Self {
            inbound,
            outbound,
            rebound,
        };

        (moderator, bridge)
    }
}

#[async_trait]
impl Moderator for TokioModerator {
    type Error = Error;

    async fn inbound(&mut self) -> Result<Option<Message>, Self::Error> {
        Ok(self.inbound.try_recv().ok())
    }

    fn inbound_blocking(&mut self) -> Result<Option<Message>, Self::Error> {
        Ok(self.inbound.blocking_recv())
    }

    async fn outbound(&mut self, message: Message, timeout: Duration) -> Result<(), Self::Error> {
        self.outbound
            .send_timeout(message, timeout)
            .await
            .map_err(|_| Error::ResourceNotAvailable)
    }

    async fn rebound(&mut self, message: Message, timeout: Duration) -> Result<(), Self::Error> {
        self.rebound
            .send_timeout(message, timeout)
            .await
            .map_err(|_| Error::ResourceNotAvailable)
    }
}
