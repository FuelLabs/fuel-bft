use core::cmp::Ordering;
use core::fmt;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeightRound {
    height: u64,
    round: u64,
}

impl PartialOrd for HeightRound {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.height
            .partial_cmp(&other.height)
            .map(|o| match o {
                Ordering::Equal => self.round.partial_cmp(&other.round),

                _ => Some(o),
            })
            .flatten()
    }
}

impl Ord for HeightRound {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.height.cmp(&other.height) {
            Ordering::Less => Ordering::Less,
            Ordering::Greater => Ordering::Greater,

            _ => self.round.cmp(&other.round),
        }
    }
}

impl fmt::Display for HeightRound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HeightRound({}, {})", self.height, self.round)
    }
}

impl HeightRound {
    pub const fn new(height: u64, round: u64) -> Self {
        Self { height, round }
    }

    pub const fn start(height: u64) -> Self {
        Self::new(height, 0)
    }

    pub const fn height(&self) -> u64 {
        self.height
    }

    pub const fn round(&self) -> u64 {
        self.round
    }

    pub const fn increment_height(self) -> Self {
        Self {
            height: self.height + 1,
            round: 0,
        }
    }

    pub const fn increment_round(self) -> Self {
        Self {
            height: self.height,
            round: self.round + 1,
        }
    }
}

impl From<u64> for HeightRound {
    fn from(height: u64) -> Self {
        Self::start(height)
    }
}
