use crate::{PeerId, Transaction, TransactionSet};

/// Block representation.
///
/// The default implementation will be used to start consensus process for non-proposers.
pub trait Block: Default {
    type PeerId: PeerId;
    type Transaction: Transaction;

    fn new(owner: Self::PeerId, txs: TransactionSet<Self::Transaction>) -> Self;
}
