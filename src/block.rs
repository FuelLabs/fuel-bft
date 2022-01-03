use crate::PeerId;

use fuel_tx::Transaction;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TransactionSet {
    Transaction(Transaction),
    Serial(Vec<Self>),
    Parallel(Vec<Self>),
}

impl Default for TransactionSet {
    fn default() -> Self {
        Self::Serial(vec![])
    }
}

/// Block representation.
///
/// The default implementation will be used to start consensus process for non-proposers.
pub trait Block: Default {
    type PeerId: PeerId;

    fn new(owner: Self::PeerId, txs: TransactionSet) -> Self;
}
