use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Belief, Concept, ConversationState, Creature, Development, FoodId, Intention, Memory, MemoryId,
    MemoryKind, Needs, NonverbalAct, Reaction, Relationship, RoomSpot, RoomState, SAVE_VERSION,
    SeededRandom, SocialAct, SocialHabits, StateValidationError, ToyId, Traits, WorldState,
};

const LEGACY_SAVE_VERSION: u32 = 1;

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
        let header = serde_json::from_str::<SaveVersionHeader>(source).map_err(SaveError::Json)?;
        if header.save_version == LEGACY_SAVE_VERSION {
            return LegacySaveGame::from_json(source);
        }
        if header.save_version != SAVE_VERSION {
            return Err(SaveError::Version(header.save_version));
        }
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

#[derive(Deserialize)]
struct SaveVersionHeader {
    save_version: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacySaveGame {
    save_version: u32,
    world: LegacyWorldState,
    random: SeededRandom,
}

impl LegacySaveGame {
    fn from_json(source: &str) -> Result<SaveGame, SaveError> {
        let legacy = serde_json::from_str::<Self>(source).map_err(SaveError::Json)?;
        if legacy.save_version != LEGACY_SAVE_VERSION {
            return Err(SaveError::Version(legacy.save_version));
        }
        if legacy.world.save_version != LEGACY_SAVE_VERSION {
            return Err(SaveError::Version(legacy.world.save_version));
        }
        let save = SaveGame {
            save_version: SAVE_VERSION,
            world: legacy.world.migrate(),
            random: legacy.random,
        };
        save.validate()?;
        Ok(save)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyWorldState {
    save_version: u32,
    seed: u64,
    elapsed_ms: u64,
    next_memory_id: u64,
    next_belief_id: u64,
    creature: LegacyCreature,
    room: LegacyRoomState,
}

impl LegacyWorldState {
    fn migrate(self) -> WorldState {
        WorldState {
            save_version: SAVE_VERSION,
            seed: self.seed,
            elapsed_ms: self.elapsed_ms,
            simulation_remainder_ms: 0,
            next_memory_id: self.next_memory_id,
            next_belief_id: self.next_belief_id,
            creature: self.creature.migrate(),
            room: self.room.migrate(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyCreature {
    name: String,
    needs: Needs,
    traits: Traits,
    relationship: Relationship,
    preferences: BTreeMap<FoodId, f32>,
    known_concepts: BTreeSet<LegacyConcept>,
    memories: Vec<LegacyMemory>,
    beliefs: Vec<Belief>,
    social_habits: SocialHabits,
    current_intention: Intention,
    last_social_act: Option<SocialAct>,
}

impl LegacyCreature {
    fn migrate(self) -> Creature {
        Creature {
            name: self.name,
            needs: self.needs,
            traits: self.traits,
            relationship: self.relationship,
            preferences: self.preferences,
            toy_preferences: BTreeMap::new(),
            known_concepts: self
                .known_concepts
                .into_iter()
                .map(LegacyConcept::migrate)
                .collect(),
            memories: self
                .memories
                .into_iter()
                .map(LegacyMemory::migrate)
                .collect(),
            beliefs: self.beliefs,
            social_habits: self.social_habits,
            current_intention: self.current_intention,
            last_social_act: self.last_social_act,
            conversation: ConversationState::default(),
            position: RoomSpot::Center,
            movement: None,
            development: Development::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyRoomState {
    food_in_bowl: Option<FoodId>,
    toy_available: bool,
    tidy: bool,
    player_present: bool,
    last_nonverbal_act: Option<LegacyNonverbalAct>,
}

impl LegacyRoomState {
    fn migrate(self) -> RoomState {
        RoomState {
            food_in_bowl: self.food_in_bowl,
            toy_available: self.toy_available,
            toy: ToyId::Ball,
            tidy: self.tidy,
            player_present: self.player_present,
            last_nonverbal_act: self.last_nonverbal_act.map(LegacyNonverbalAct::migrate),
            play_requested: false,
            comfort_requested: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LegacyConcept {
    SelfIdentity,
    You,
    Food,
    Good,
    Bad,
    Here,
    Sleep,
    Toy,
    Again,
    Yesterday,
    Trust,
}

impl LegacyConcept {
    fn migrate(self) -> Concept {
        match self {
            Self::SelfIdentity => Concept::SelfIdentity,
            Self::You => Concept::You,
            Self::Food => Concept::Food,
            Self::Good => Concept::Good,
            Self::Bad => Concept::Bad,
            Self::Here => Concept::Here,
            Self::Sleep => Concept::Sleep,
            Self::Toy => Concept::Toy,
            Self::Again => Concept::Again,
            Self::Yesterday => Concept::Yesterday,
            Self::Trust => Concept::Trust,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyMemory {
    id: MemoryId,
    happened_at_ms: u64,
    kind: LegacyMemoryKind,
    concepts: BTreeSet<LegacyConcept>,
    valence: f32,
    salience: f32,
}

impl LegacyMemory {
    fn migrate(self) -> Memory {
        Memory {
            id: self.id,
            happened_at_ms: self.happened_at_ms,
            kind: self.kind.migrate(),
            concepts: self
                .concepts
                .into_iter()
                .map(LegacyConcept::migrate)
                .collect(),
            valence: self.valence,
            salience: self.salience,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum LegacyMemoryKind {
    WasFed { food: FoodId },
    DislikedFood { food: FoodId },
    RejectedFood { food: FoodId },
    Played,
    WasComforted,
    PlayerReturnedAfterAbsence,
    PlayerReacted { reaction: Reaction, to: SocialAct },
}

impl LegacyMemoryKind {
    fn migrate(self) -> MemoryKind {
        match self {
            Self::WasFed { food } => MemoryKind::WasFed { food },
            Self::DislikedFood { food } => MemoryKind::DislikedFood { food },
            Self::RejectedFood { food } => MemoryKind::RejectedFood { food },
            Self::Played => MemoryKind::PlayedWith { toy: ToyId::Ball },
            Self::WasComforted => MemoryKind::WasComforted,
            Self::PlayerReturnedAfterAbsence => MemoryKind::PlayerReturnedAfterAbsence,
            Self::PlayerReacted { reaction, to } => MemoryKind::PlayerReacted { reaction, to },
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum LegacyNonverbalAct {
    PushFoodAway(FoodId),
    TakeToyAway,
    RefuseToEat,
    UndoTidy,
}

impl LegacyNonverbalAct {
    fn migrate(self) -> NonverbalAct {
        match self {
            Self::PushFoodAway(food) => NonverbalAct::PushFoodAway(food),
            Self::TakeToyAway => NonverbalAct::TakeToyAway(ToyId::Ball),
            Self::RefuseToEat => NonverbalAct::RefuseToEat,
            Self::UndoTidy => NonverbalAct::UndoTidy,
        }
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
