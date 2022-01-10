use crate::ValidatorId;

/// Block representation.
///
/// The default implementation will be used to start consensus process for non-leaders.
pub trait Block: Default {
    type Payload;
    type ValidatorId: ValidatorId;

    fn new(owner: Self::ValidatorId, payload: Self::Payload) -> Self;
}
