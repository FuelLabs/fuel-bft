#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Consensus {
    Inconclusive = 0x00,
    Consensus = 0x01,
    Reject = 0x02,
}

impl Consensus {
    pub const fn from_u8(byte: u8) -> Self {
        match byte {
            0x00 => Self::Inconclusive,
            0x01 => Self::Consensus,
            _ => Self::Reject,
        }
    }

    pub const fn is_consensus(&self) -> bool {
        matches!(self, Self::Consensus)
    }

    pub const fn is_inconclusive(&self) -> bool {
        matches!(self, Self::Inconclusive)
    }

    pub const fn evaluate(validators: usize, approvals: usize) -> Self {
        let minimum = validators > 3;
        let consensus = validators * 2 / 3;

        if !minimum {
            Consensus::Reject
        } else if approvals > consensus {
            Consensus::Consensus
        } else {
            Consensus::Inconclusive
        }
    }
}
