use crate::ValidatorId;

pub trait Key {
    type ValidatorId: ValidatorId;

    fn validator(&self) -> Self::ValidatorId;
}
