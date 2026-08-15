use serde::{Deserialize, Serialize};

pub trait RandomSource {
    fn next_unit(&mut self) -> f32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RandomDomain {
    Motion,
    Preferences,
    Social,
    Environment,
}

impl RandomDomain {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Motion => "motion",
            Self::Preferences => "preferences",
            Self::Social => "social",
            Self::Environment => "environment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SeededRandom(u64);

impl SeededRandom {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    #[must_use]
    pub fn keyed(seed: u64, domain: RandomDomain, key: u64) -> Self {
        let mut value =
            seed ^ (domain.name().as_bytes().iter().fold(0_u64, |hash, byte| {
                hash.wrapping_mul(31).wrapping_add(u64::from(*byte))
            })) ^ key.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        value ^= value >> 30;
        value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value ^= value >> 27;
        Self(value)
    }
}

#[must_use]
pub fn deterministic_unit(seed: u64, domain: RandomDomain, key: u64) -> f32 {
    let mut random = SeededRandom::keyed(seed, domain, key);
    random.next_unit()
}

impl RandomSource for SeededRandom {
    fn next_unit(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        ((self.0 >> 40) as f32) / ((1_u32 << 24) as f32)
    }
}
