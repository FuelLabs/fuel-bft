use crate::Message;

use async_trait::async_trait;
use time::OffsetDateTime;

use alloc::boxed::Box;
use core::fmt;
use core::time::Duration;

/// Reactor I/O handler
#[async_trait]
pub trait Moderator: Sync {
    /// Concrete error of the trait.
    type Error: fmt::Display;

    /// Current timestamp in UTC
    #[cfg(not(feature = "std"))]
    fn now(&self) -> OffsetDateTime;

    /// Current timestamp in UTC
    #[cfg(feature = "std")]
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    /// Messages consumed by the reactor
    async fn inbound(&mut self) -> Result<Option<Message>, Self::Error>;

    /// Messages consumed by the reactor - should block
    fn inbound_blocking(&mut self) -> Result<Option<Message>, Self::Error>;

    /// Messages dispatched from the reactor
    async fn outbound(&self, message: Message, timeout: Duration) -> Result<(), Self::Error>;

    /// Messages consumed by the reactor that need to be rescheduled
    async fn rebound(&self, message: Message, timeout: Duration) -> Result<(), Self::Error>;

    /// Send a message from the reactor.
    async fn send(&self, message: Message, timeout: Duration) {
        #[cfg(feature = "trace")]
        tracing::debug!("sending message {:?}", message);

        if let Err(_e) = self.outbound(message, timeout).await {
            #[cfg(feature = "trace")]
            tracing::error!("error sending outbound message: {}", _e);
        }
    }

    /// Requeue a message that cannot be consumed by the reactor.
    async fn requeue(&self, message: Message, timeout: Duration) {
        if let Err(_e) = self.rebound(message, timeout).await {
            #[cfg(feature = "trace")]
            tracing::error!("error rebounding message: {}", _e);
        }
    }
}
