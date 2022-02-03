use fuel_crypto::PublicKey;

/// Block representation.
///
/// The default implementation will be used to start consensus process for non-leaders.
pub trait Block: Default + Clone {
    type Payload;

    fn new(owner: PublicKey, payload: Self::Payload) -> Self;
}
