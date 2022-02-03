use time::OffsetDateTime;

use core::time::Duration;

/// Config data for the reactor and consensus behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Blocking resources capacity
    pub capacity: usize,

    /// Expected interval between blocks (ms)
    pub consensus: u128,

    /// Genesis instant
    pub genesis: OffsetDateTime,

    /// Frequency of every tick (ms)
    pub heartbeat: u128,

    /// Await timeout for blocking resources
    pub timeout: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            capacity: Self::DEFAULT_CAPACITY,
            consensus: Self::DEFAULT_CONSENSUS,
            genesis: Self::DEFAULT_GENESIS,
            heartbeat: Self::DEFAULT_HEARTBEAT,
            timeout: Self::DEFAULT_TIMEOUT,
        }
    }
}

// TODO tweak
impl Config {
    /// Default capacity
    pub const DEFAULT_CAPACITY: usize = 256;

    /// 10 seconds as default consensus interval
    pub const DEFAULT_CONSENSUS: u128 = 10000;

    /// Set the genesis at unix epoch
    pub const DEFAULT_GENESIS: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    /// 500 ms as default heartbeat interval
    pub const DEFAULT_HEARTBEAT: u128 = 500;

    /// 5s as default timeout
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
}
