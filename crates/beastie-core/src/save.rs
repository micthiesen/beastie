use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{SAVE_VERSION, SeededRandom, StateValidationError, WorldState};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveGame {
    pub save_version: u32,
    pub world: WorldState,
    pub random: SeededRandom,
}

impl SaveGame {
    #[must_use]
    pub fn capture(world: &WorldState, random: &SeededRandom) -> Self {
        Self {
            save_version: SAVE_VERSION,
            world: world.clone(),
            random: *random,
        }
    }

    pub fn to_json(&self) -> Result<String, SaveError> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(SaveError::Json)
    }

    pub fn from_json(source: &str) -> Result<Self, SaveError> {
        let save = serde_json::from_str::<Self>(source).map_err(SaveError::Json)?;
        save.validate()?;
        Ok(save)
    }

    #[must_use]
    pub fn resume(self) -> (WorldState, SeededRandom) {
        (self.world, self.random)
    }

    fn validate(&self) -> Result<(), SaveError> {
        if self.save_version != SAVE_VERSION {
            return Err(SaveError::Version(self.save_version));
        }
        if self.world.save_version != SAVE_VERSION {
            return Err(SaveError::Version(self.world.save_version));
        }
        self.world.validate().map_err(SaveError::State)
    }
}

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("save JSON is malformed: {0}")]
    Json(serde_json::Error),
    #[error("save version {0} is unsupported")]
    Version(u32),
    #[error("save state is invalid: {0}")]
    State(StateValidationError),
}
