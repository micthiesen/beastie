use serde::{Deserialize, Serialize};

pub trait RandomSource {
    fn next_unit(&mut self) -> f32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SeededRandom(u64);

impl SeededRandom {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
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
