use crate::PeerId;

/// Block representation.
///
/// The default implementation will be used to start consensus process for non-proposers.
pub trait Block: Default {
    type PeerId: PeerId;
    type Payload;

    fn new(owner: Self::PeerId, payload: Self::Payload) -> Self;
}
