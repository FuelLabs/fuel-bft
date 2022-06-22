use crate::{
    Config, Error, MemoryKeychain, Message, Moderator, Notification, Reactor, Request, Response,
};

use async_trait::async_trait;
use tokio::sync::mpsc;

use core::time::Duration;
use std::time::Instant;

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

        tokio::time::timeout(self.timeout, self._request(id))
            .await
            .map_err(|_e| Error::ResourceNotAvailable)?
    }

    async fn _request(&mut self, id: u64) -> Result<Response, Error> {
        loop {
            match self.listener.recv().await {
                Some(Message::Response(r)) if r.id() == id => return Ok(r),
                Some(m) => {
                    if let Err(_e) = self.outbound.send(m).await {
                        #[cfg(feature = "trace")]
                        tracing::error!(
                            "message {:?} discarded; outbound resource exhausted: {}",
                            m,
                            _e
                        );

                        return Err(Error::ResourceNotAvailable);
                    }
                }
                None => {
                    #[cfg(feature = "trace")]
                    tracing::trace!("attempting to receive request {}", id);
                }
            }
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
                let start = Instant::now();

                if let Err(_e) = reactor.heartbeat(&keychain, &mut moderator).await {
                    #[cfg(feature = "trace")]
                    tracing::trace!("heartbeat error: {}", _e);
                }

                if reactor.should_quit() {
                    break;
                }

                let elapsed = start.elapsed().as_millis();
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
        self.listener.try_recv().ok()
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
