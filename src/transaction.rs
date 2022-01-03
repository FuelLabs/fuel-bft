pub trait Transaction {}

pub enum TransactionSet<T> {
    Transaction(T),
    Serial(Vec<Self>),
    Parallel(Vec<Self>),
}

impl<T> Default for TransactionSet<T> {
    fn default() -> Self {
        Self::Serial(vec![])
    }
}
