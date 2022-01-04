use crate::PeerId;

pub trait Key {
    type PeerId: PeerId;

    fn peer(&self) -> Self::PeerId;
}
