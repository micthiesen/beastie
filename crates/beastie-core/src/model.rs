use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ACTIVE_DAY_MS, RandomSource, SAVE_VERSION, SeededRandom};

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut mixed = value;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MemoryId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BeliefId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoodId {
    Berry,
    Mushroom,
    Pellet,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToyId {
    #[default]
    Ball,
    Bell,
    Sock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Concept {
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
    Give,
    Friend,
    Why,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood {
    Content,
    Curious,
    Hungry,
    Sleepy,
    Lonely,
    Resentful,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageStage {
    #[default]
    Hatch,
    Words,
    Phrases,
}

/// A small, save-derived speech fingerprint.
///
/// This is intentionally a projection rather than another mutable simulation field.  The
/// persisted seed and traits are enough to recover it after a save/reload, and the early
/// language stages remain plain until the creature has reached the Individuality slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdiolectQuirk {
    Plain,
    ArticleDrop,
    Echo,
    HmPrefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Idiolect {
    pub quirk: IdiolectQuirk,
}

impl Default for Idiolect {
    fn default() -> Self {
        Self {
            quirk: IdiolectQuirk::Plain,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionCounters {
    pub feeds: u32,
    pub plays: u32,
    pub comforts: u32,
    pub returns: u32,
    pub talks: u32,
}

impl InteractionCounters {
    #[must_use]
    pub fn total(self) -> u32 {
        self.feeds
            .saturating_add(self.plays)
            .saturating_add(self.comforts)
            .saturating_add(self.returns)
            .saturating_add(self.talks)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Development {
    pub active_days_reached: u32,
    pub language_stage: LanguageStage,
    pub interactions: InteractionCounters,
}

impl Default for Development {
    fn default() -> Self {
        Self {
            active_days_reached: 1,
            language_stage: LanguageStage::Hatch,
            interactions: InteractionCounters::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomSpot {
    Bed,
    Bowl,
    Toy,
    Player,
    #[default]
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Movement {
    pub from: RoomSpot,
    pub to: RoomSpot,
    pub elapsed_ms: u64,
    pub duration_ms: u64,
}

impl Movement {
    #[must_use]
    pub fn progress(self) -> f32 {
        if self.duration_ms == 0 {
            1.0
        } else {
            (self.elapsed_ms as f32 / self.duration_ms as f32).clamp(0.0, 1.0)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Needs {
    pub hunger: f32,
    pub energy: f32,
    pub comfort: f32,
    pub curiosity: f32,
}

impl Needs {
    pub(crate) fn clamp(&mut self) {
        self.hunger = self.hunger.clamp(0.0, 1.0);
        self.energy = self.energy.clamp(0.0, 1.0);
        self.comfort = self.comfort.clamp(0.0, 1.0);
        self.curiosity = self.curiosity.clamp(0.0, 1.0);
    }

    fn values(self) -> [f32; 4] {
        [self.hunger, self.energy, self.comfort, self.curiosity]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Traits {
    pub sociability: f32,
    pub boldness: f32,
    pub fussiness: f32,
    pub stubbornness: f32,
    pub literalness: f32,
    pub repetitiveness: f32,
    pub sentence_complexity: f32,
    pub question_tendency: f32,
}

impl Traits {
    fn values(self) -> [f32; 8] {
        [
            self.sociability,
            self.boldness,
            self.fussiness,
            self.stubbornness,
            self.literalness,
            self.repetitiveness,
            self.sentence_complexity,
            self.question_tendency,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    pub bond: f32,
    pub trust: f32,
    pub respect: f32,
    pub resentment: f32,
}

impl Relationship {
    pub(crate) fn clamp(&mut self) {
        self.bond = self.bond.clamp(0.0, 1.0);
        self.trust = self.trust.clamp(0.0, 1.0);
        self.respect = self.respect.clamp(0.0, 1.0);
        self.resentment = self.resentment.clamp(0.0, 1.0);
    }

    fn values(self) -> [f32; 4] {
        [self.bond, self.trust, self.respect, self.resentment]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SocialHabits {
    pub profanity: f32,
    pub crudeness: f32,
    pub spite: f32,
    pub provocation: f32,
    pub sexual_innuendo: f32,
}

impl SocialHabits {
    pub(crate) fn clamp(&mut self) {
        self.profanity = self.profanity.clamp(0.0, 1.0);
        self.crudeness = self.crudeness.clamp(0.0, 1.0);
        self.spite = self.spite.clamp(0.0, 1.0);
        self.provocation = self.provocation.clamp(0.0, 1.0);
        self.sexual_innuendo = self.sexual_innuendo.clamp(0.0, 1.0);
    }

    fn values(self) -> [f32; 5] {
        [
            self.profanity,
            self.crudeness,
            self.spite,
            self.provocation,
            self.sexual_innuendo,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intention {
    Idle,
    Eat,
    RejectFood,
    Sleep,
    Play,
    ApproachPlayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reaction {
    Laugh,
    Disapprove,
    Comfort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SocialAct {
    Neutral,
    Insult,
    Profanity,
    Crudeness,
    Provocation,
    Innuendo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NonverbalAct {
    PushFoodAway(FoodId),
    TakeToyAway(ToyId),
    RefuseToEat,
    UndoTidy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeliefKind {
    RedFoodIsATrick,
    PlayerReturnsAfterSleep,
    ToyIsJealous,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemoryKind {
    WasFed { food: FoodId },
    DislikedFood { food: FoodId },
    RejectedFood { food: FoodId },
    PlayedWith { toy: ToyId },
    DislikedToy { toy: ToyId },
    WasComforted,
    PlayerReturnedAfterAbsence,
    PlayerReacted { reaction: Reaction, to: SocialAct },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    pub id: MemoryId,
    pub happened_at_ms: u64,
    pub kind: MemoryKind,
    pub concepts: BTreeSet<Concept>,
    pub valence: f32,
    pub salience: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Belief {
    pub id: BeliefId,
    pub kind: BeliefKind,
    pub supporting_memories: BTreeSet<MemoryId>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Creature {
    pub name: String,
    pub needs: Needs,
    pub traits: Traits,
    pub relationship: Relationship,
    pub preferences: BTreeMap<FoodId, f32>,
    #[serde(default)]
    pub toy_preferences: BTreeMap<ToyId, f32>,
    pub known_concepts: BTreeSet<Concept>,
    pub memories: Vec<Memory>,
    pub beliefs: Vec<Belief>,
    pub social_habits: SocialHabits,
    pub current_intention: Intention,
    pub last_social_act: Option<SocialAct>,
    #[serde(default)]
    pub position: RoomSpot,
    #[serde(default)]
    pub movement: Option<Movement>,
    #[serde(default)]
    pub development: Development,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomState {
    pub food_in_bowl: Option<FoodId>,
    pub toy_available: bool,
    #[serde(default)]
    pub toy: ToyId,
    pub tidy: bool,
    pub player_present: bool,
    pub last_nonverbal_act: Option<NonverbalAct>,
    #[serde(default)]
    pub play_requested: bool,
    #[serde(default)]
    pub comfort_requested: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldState {
    pub save_version: u32,
    pub seed: u64,
    pub elapsed_ms: u64,
    #[serde(default)]
    pub simulation_remainder_ms: u64,
    pub next_memory_id: u64,
    pub next_belief_id: u64,
    pub creature: Creature,
    pub room: RoomState,
}

impl WorldState {
    #[must_use]
    pub fn new(seed: u64, name: impl Into<String>) -> Self {
        let mut genome = SeededRandom::new(seed ^ 0xa076_1d64_78bd_642f);
        let requested_name = name.into();
        let normalized_name = requested_name.trim().chars().take(64).collect::<String>();
        let name = if normalized_name.is_empty() {
            "Beastie".to_owned()
        } else {
            normalized_name
        };
        Self {
            save_version: SAVE_VERSION,
            seed,
            elapsed_ms: 0,
            simulation_remainder_ms: 0,
            next_memory_id: 1,
            next_belief_id: 1,
            creature: Creature {
                name,
                needs: Needs {
                    hunger: 0.25,
                    energy: 0.85,
                    comfort: 0.75,
                    curiosity: 0.55,
                },
                traits: Traits {
                    sociability: genome.next_unit(),
                    boldness: genome.next_unit(),
                    fussiness: genome.next_unit(),
                    stubbornness: genome.next_unit(),
                    literalness: genome.next_unit(),
                    repetitiveness: genome.next_unit(),
                    sentence_complexity: genome.next_unit(),
                    question_tendency: genome.next_unit(),
                },
                relationship: Relationship {
                    bond: 0.1,
                    trust: 0.15,
                    respect: 0.1,
                    resentment: 0.02,
                },
                preferences: BTreeMap::new(),
                toy_preferences: BTreeMap::new(),
                known_concepts: BTreeSet::from([
                    Concept::SelfIdentity,
                    Concept::You,
                    Concept::Food,
                    Concept::Good,
                    Concept::Bad,
                    Concept::Here,
                    Concept::Sleep,
                ]),
                memories: Vec::new(),
                beliefs: Vec::new(),
                social_habits: SocialHabits {
                    profanity: 0.02,
                    crudeness: 0.02,
                    spite: 0.03,
                    provocation: 0.03,
                    sexual_innuendo: 0.0,
                },
                current_intention: Intention::Idle,
                last_social_act: None,
                position: RoomSpot::Center,
                movement: None,
                development: Development::default(),
            },
            room: RoomState {
                food_in_bowl: None,
                toy_available: true,
                toy: ToyId::Ball,
                tidy: true,
                player_present: true,
                last_nonverbal_act: None,
                play_requested: false,
                comfort_requested: false,
            },
        }
    }

    #[must_use]
    pub fn active_day(&self) -> u64 {
        self.elapsed_ms / ACTIVE_DAY_MS + 1
    }

    /// Projects a stable speech quirk from persisted identity and progression.
    ///
    /// Individuality is deliberately gated behind the phrase stage and three meaningful
    /// interactions.  `to_bits` makes the projection independent of float formatting and
    /// preserves the exact persisted trait values across a JSON round trip.
    #[must_use]
    pub fn idiolect(&self) -> Idiolect {
        let development = self.creature.development;
        if development.language_stage < LanguageStage::Phrases
            || development.active_days_reached < 3
            || development.interactions.total() < 3
        {
            return Idiolect::default();
        }

        let mut hash = self.seed ^ 0x9e37_79b9_7f4a_7c15;
        for value in self.creature.traits.values() {
            hash = splitmix64(hash ^ u64::from(value.to_bits()));
        }
        let quirk = match hash % 3 {
            0 => IdiolectQuirk::ArticleDrop,
            1 => IdiolectQuirk::Echo,
            _ => IdiolectQuirk::HmPrefix,
        };
        Idiolect { quirk }
    }

    #[must_use]
    pub fn mood(&self) -> Mood {
        let creature = &self.creature;
        if creature.relationship.resentment > 0.4 {
            Mood::Resentful
        } else if creature.needs.energy < 0.25 {
            Mood::Sleepy
        } else if creature.needs.hunger > 0.72 {
            Mood::Hungry
        } else if !self.room.player_present && creature.relationship.bond > 0.3 {
            Mood::Lonely
        } else if creature.needs.curiosity > 0.7 {
            Mood::Curious
        } else {
            Mood::Content
        }
    }

    pub(crate) fn remember(
        &mut self,
        kind: MemoryKind,
        concepts: &[Concept],
        valence: f32,
        salience: f32,
    ) -> MemoryId {
        let id = MemoryId(self.next_memory_id);
        self.next_memory_id = self.next_memory_id.saturating_add(1);
        self.creature.memories.push(Memory {
            id,
            happened_at_ms: self.elapsed_ms,
            kind,
            concepts: concepts.iter().copied().collect(),
            valence: valence.clamp(-1.0, 1.0),
            salience: salience.clamp(0.0, 1.0),
        });
        id
    }

    pub(crate) fn reinforce_belief(
        &mut self,
        kind: BeliefKind,
        memory: MemoryId,
        confidence_delta: f32,
    ) {
        if let Some(belief) = self
            .creature
            .beliefs
            .iter_mut()
            .find(|belief| belief.kind == kind)
        {
            belief.supporting_memories.insert(memory);
            belief.confidence = (belief.confidence + confidence_delta).clamp(0.0, 1.0);
            return;
        }
        let id = BeliefId(self.next_belief_id);
        self.next_belief_id = self.next_belief_id.saturating_add(1);
        self.creature.beliefs.push(Belief {
            id,
            kind,
            supporting_memories: BTreeSet::from([memory]),
            confidence: confidence_delta.clamp(0.0, 1.0),
        });
    }

    pub fn validate(&self) -> Result<(), StateValidationError> {
        if self.save_version != SAVE_VERSION {
            return Err(StateValidationError::Version(self.save_version));
        }
        if self.creature.name.is_empty() || self.creature.name.chars().count() > 64 {
            return Err(StateValidationError::Name);
        }
        let scalar_values = self
            .creature
            .needs
            .values()
            .into_iter()
            .chain(self.creature.traits.values())
            .chain(self.creature.relationship.values())
            .chain(self.creature.social_habits.values())
            .chain(self.creature.preferences.values().copied())
            .chain(self.creature.toy_preferences.values().copied());
        if scalar_values.clone().any(|value| !value.is_finite()) {
            return Err(StateValidationError::NonFiniteScalar);
        }
        if self
            .creature
            .traits
            .values()
            .into_iter()
            .any(|value| !(0.0..=1.0).contains(&value))
        {
            return Err(StateValidationError::TraitScalar);
        }
        if self
            .creature
            .needs
            .values()
            .into_iter()
            .chain(self.creature.relationship.values())
            .chain(self.creature.social_habits.values())
            .any(|value| !(0.0..=1.0).contains(&value))
        {
            return Err(StateValidationError::UnitScalar);
        }
        if self
            .creature
            .preferences
            .values()
            .chain(self.creature.toy_preferences.values())
            .any(|value| !(-1.0..=1.0).contains(value))
        {
            return Err(StateValidationError::Preference);
        }
        if self.simulation_remainder_ms >= crate::SIMULATION_TICK_MS {
            return Err(StateValidationError::SimulationRemainder);
        }
        if let Some(movement) = self.creature.movement
            && (movement.duration_ms == 0
                || movement.elapsed_ms >= movement.duration_ms
                || movement.from == movement.to
                || movement.from != self.creature.position)
        {
            return Err(StateValidationError::Movement);
        }
        let development = self.creature.development;
        if development.active_days_reached == 0
            || u64::from(development.active_days_reached) > self.active_day()
            || (development.language_stage >= LanguageStage::Words
                && (development.active_days_reached < 2
                    || !self.creature.known_concepts.contains(&Concept::Again)))
            || (development.language_stage >= LanguageStage::Phrases
                && (development.active_days_reached < 3
                    || !self.creature.known_concepts.contains(&Concept::Yesterday)))
        {
            return Err(StateValidationError::Development);
        }
        let memory_ids = self
            .creature
            .memories
            .iter()
            .map(|memory| memory.id)
            .collect::<BTreeSet<_>>();
        if self.next_memory_id == 0 {
            return Err(StateValidationError::MemoryOrder);
        }
        if memory_ids.len() != self.creature.memories.len() {
            return Err(StateValidationError::DuplicateMemory);
        }
        if self
            .creature
            .memories
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
            || self.creature.memories.iter().any(|memory| memory.id.0 == 0)
            || self
                .creature
                .memories
                .last()
                .is_some_and(|memory| memory.id.0 >= self.next_memory_id)
        {
            return Err(StateValidationError::MemoryOrder);
        }
        if self.creature.memories.iter().any(|memory| {
            !memory.valence.is_finite()
                || !(-1.0..=1.0).contains(&memory.valence)
                || !memory.salience.is_finite()
                || !(0.0..=1.0).contains(&memory.salience)
                || memory.happened_at_ms > self.elapsed_ms
        }) {
            return Err(StateValidationError::MemoryValue);
        }
        let belief_ids = self
            .creature
            .beliefs
            .iter()
            .map(|belief| belief.id)
            .collect::<BTreeSet<_>>();
        if self.next_belief_id == 0
            || belief_ids.len() != self.creature.beliefs.len()
            || self
                .creature
                .beliefs
                .iter()
                .any(|belief| belief.id.0 == 0 || belief.id.0 >= self.next_belief_id)
        {
            return Err(StateValidationError::BeliefId);
        }
        if self.creature.beliefs.iter().any(|belief| {
            !belief.confidence.is_finite()
                || !(0.0..=1.0).contains(&belief.confidence)
                || belief
                    .supporting_memories
                    .iter()
                    .any(|memory| !memory_ids.contains(memory))
        }) {
            return Err(StateValidationError::Belief);
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StateValidationError {
    #[error("save version {0} is unsupported")]
    Version(u32),
    #[error("creature name must contain 1..=64 characters")]
    Name,
    #[error("state contains a non-finite scalar")]
    NonFiniteScalar,
    #[error("trait values must be within 0..=1")]
    TraitScalar,
    #[error("need, relationship, or habit values must be within 0..=1")]
    UnitScalar,
    #[error("food preferences must be within -1..=1")]
    Preference,
    #[error("simulation remainder must be below one simulation tick")]
    SimulationRemainder,
    #[error("movement must be between distinct spots and strictly in progress")]
    Movement,
    #[error("development stage, active day, or required concepts are inconsistent")]
    Development,
    #[error("memory IDs must be unique")]
    DuplicateMemory,
    #[error("memory IDs must be strictly increasing and below the next ID")]
    MemoryOrder,
    #[error("memory time, valence, or salience is invalid")]
    MemoryValue,
    #[error("belief IDs must be unique and below the next ID")]
    BeliefId,
    #[error("belief confidence or supporting memory is invalid")]
    Belief,
}
