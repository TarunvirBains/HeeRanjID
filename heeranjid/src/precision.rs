use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RanjPrecision {
    Microseconds = 0b00,
    Nanoseconds = 0b01,
    Picoseconds = 0b10,
    Femtoseconds = 0b11,
}

impl RanjPrecision {
    pub fn from_micros_multiplier(self) -> u128 {
        match self {
            Self::Microseconds => 1,
            Self::Nanoseconds => 1_000,
            Self::Picoseconds => 1_000_000,
            Self::Femtoseconds => 1_000_000_000,
        }
    }

    pub fn to_micros_divisor(self) -> u128 {
        self.from_micros_multiplier()
    }

    pub fn from_millis_multiplier(self) -> u128 {
        self.from_micros_multiplier() * 1_000
    }

    pub fn from_bits(bits: u8) -> Option<Self> {
        match bits & 0b11 {
            0b00 => Some(Self::Microseconds),
            0b01 => Some(Self::Nanoseconds),
            0b10 => Some(Self::Picoseconds),
            0b11 => Some(Self::Femtoseconds),
            _ => None,
        }
    }

    pub fn to_bits(self) -> u8 {
        self as u8
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Microseconds => "us",
            Self::Nanoseconds => "ns",
            Self::Picoseconds => "ps",
            Self::Femtoseconds => "fs",
        }
    }
}

static GENERATION_PRECISION: OnceLock<RanjPrecision> = OnceLock::new();

pub fn generation_precision() -> RanjPrecision {
    *GENERATION_PRECISION.get_or_init(|| match std::env::var("RANJID_PRECISION").as_deref() {
        Ok("us") => RanjPrecision::Microseconds,
        Ok("ns") => RanjPrecision::Nanoseconds,
        Ok("ps") => RanjPrecision::Picoseconds,
        Ok("fs") => RanjPrecision::Femtoseconds,
        Ok(invalid) => panic!(
            "RANJID_PRECISION must be one of: us, ns, ps, fs (got '{invalid}')"
        ),
        Err(_) => RanjPrecision::Nanoseconds, // unset → default
    })
}
