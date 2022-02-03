mod event;
mod notification;
mod request;

pub use event::Event;
pub use notification::Notification;
pub use request::{Request, Response};

/// I/O interface with the reactor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Message {
    /// Event produced by the reactor
    Event(Event),
    /// Notification to be consumed by the reactor
    Notification(Notification),
    /// Request-response to be executed by the reactor
    Request(Request),
    /// Response generated from a request
    Response(Response),
}

impl From<Event> for Message {
    fn from(e: Event) -> Self {
        Self::Event(e)
    }
}

impl From<Notification> for Message {
    fn from(n: Notification) -> Self {
        Self::Notification(n)
    }
}

impl From<Request> for Message {
    fn from(r: Request) -> Self {
        Self::Request(r)
    }
}
