use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ACTIVE_DAY_MS, RandomDomain, RandomSource, SAVE_VERSION, SeededRandom, deterministic_unit,
};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Development {
    pub active_days_reached: u32,
    pub language_stage: LanguageStage,
    pub interactions: InteractionCounters,
    #[serde(default)]
    pub milestones: BTreeSet<DevelopmentMilestone>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentMilestone {
    SettledRoutine,
    FavoriteFound,
    NameRecognized,
    MatureExpression,
}

impl Default for Development {
    fn default() -> Self {
        Self {
            active_days_reached: 1,
            language_stage: LanguageStage::Hatch,
            interactions: InteractionCounters::default(),
            milestones: BTreeSet::new(),
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
    WaitAtBowl,
    RejectFood,
    Sleep,
    Play,
    ApproachPlayer,
    SeekComfort,
    UndoTidy,
    RefuseAndStare,
    ShowAffection,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationState {
    pub next_talk_at_ms: u64,
    pub contextual_follow_up_available: bool,
    pub contextual_follow_up_used: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageExposure {
    Profanity,
    Crudeness,
    Innuendo,
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
    RefuseAndStare,
    LeanAgainstPlayer,
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
    #[serde(default)]
    pub contradicting_memories: BTreeSet<MemoryId>,
    pub confidence: f32,
}

/// A normalized coordinate stored as fixed-point units. One unit is 1/10,000 of the
/// aquarium width or height, so simulation results never depend on floating point rounding.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedPosition {
    pub x: i32,
    pub y: i32,
}

impl NormalizedPosition {
    pub const SCALE: i32 = 10_000;
    pub const MIN: Self = Self { x: 0, y: 0 };
    pub const MAX: Self = Self {
        x: Self::SCALE,
        y: Self::SCALE,
    };

    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            x: self.x.clamp(0, Self::SCALE),
            y: self.y.clamp(0, Self::SCALE),
        }
    }
}

pub type AquariumPosition = NormalizedPosition;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedVelocity {
    pub x: i32,
    pub y: i32,
}

impl NormalizedVelocity {
    pub const MAX_COMPONENT: i32 = 2_000;

    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            x: self.x.clamp(-Self::MAX_COMPONENT, Self::MAX_COMPONENT),
            y: self.y.clamp(-Self::MAX_COMPONENT, Self::MAX_COMPONENT),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Facing {
    Left,
    #[default]
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DepthLane {
    Foreground,
    #[default]
    Middle,
    Background,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GazeTarget {
    Cursor,
    Player,
    Food(u64),
    Toy(ToyId),
    Cave,
    Plant,
    #[default]
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SteeringMode {
    #[default]
    Hover,
    Drift,
    Approach,
    Flee,
    Orbit,
    Inspect,
    Settle,
    Brake,
    Turn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDestination {
    Cave,
    Plant,
    Bottom,
    Player,
    Toy(ToyId),
    Food(u64),
    Position(NormalizedPosition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPhase {
    Notice,
    Brake,
    Gaze,
    Turn,
    Approach,
    Inspect,
    Act,
    Recover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionTimeline {
    pub phase: ActionPhase,
    pub elapsed_ms: u64,
    pub phase_duration_ms: u64,
    pub destination: SemanticDestination,
    pub food_id: Option<u64>,
}

impl ActionTimeline {
    #[must_use]
    pub fn progress(self) -> f32 {
        (self.elapsed_ms as f32 / self.phase_duration_ms.max(1) as f32).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoodBuoyancy {
    Float,
    Drift,
    Sink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoodDisposition {
    Falling,
    Floating,
    Settled,
    Consumed,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoodDropRejectionReason {
    AquariumFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoodObject {
    pub id: u64,
    pub food: FoodId,
    pub position: NormalizedPosition,
    pub velocity: NormalizedVelocity,
    pub buoyancy: FoodBuoyancy,
    pub disposition: FoodDisposition,
    pub age_ms: u64,
    pub lifetime_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorldObject {
    Food(FoodObject),
    Toy {
        toy: ToyId,
        position: NormalizedPosition,
    },
    Plant {
        position: NormalizedPosition,
    },
    Cave {
        position: NormalizedPosition,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AquariumState {
    pub objects: BTreeMap<u64, WorldObject>,
    pub next_object_id: u64,
    pub cursor: Option<NormalizedPosition>,
    #[serde(default)]
    pub object_names: BTreeMap<u64, String>,
    pub player_present: bool,
    pub max_food: u16,
}

impl Default for AquariumState {
    fn default() -> Self {
        let objects = BTreeMap::from([
            (
                1,
                WorldObject::Cave {
                    position: NormalizedPosition::new(1_500, 8_500),
                },
            ),
            (
                2,
                WorldObject::Plant {
                    position: NormalizedPosition::new(3_000, 8_800),
                },
            ),
            (
                3,
                WorldObject::Toy {
                    toy: ToyId::Ball,
                    position: NormalizedPosition::new(5_000, 8_900),
                },
            ),
            (
                4,
                WorldObject::Toy {
                    toy: ToyId::Bell,
                    position: NormalizedPosition::new(6_500, 8_900),
                },
            ),
            (
                5,
                WorldObject::Toy {
                    toy: ToyId::Sock,
                    position: NormalizedPosition::new(8_000, 8_900),
                },
            ),
        ]);
        Self {
            objects,
            next_object_id: 6,
            cursor: None,
            object_names: BTreeMap::new(),
            player_present: true,
            max_food: 12,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Routine {
    pub hour_start: u8,
    pub destination: SemanticDestination,
    pub strength: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitiatedBehavior {
    pub reason: InitiativeReason,
    pub nonverbal: Option<NonverbalAct>,
    pub requested_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitiativeReason {
    Hunger,
    Loneliness,
    Curiosity,
    Ritual,
    Request,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamingTarget {
    Creature,
    Food(FoodId),
    Toy(ToyId),
    Object(u64),
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
    pub conversation: ConversationState,
    #[serde(default)]
    pub development: Development,
    #[serde(default)]
    pub aquarium: AquariumCreatureState,
    #[serde(default)]
    pub routines: Vec<Routine>,
    #[serde(default)]
    pub favorite_locations: BTreeMap<SemanticDestination, u32>,
    #[serde(default)]
    pub initiated_behavior: Option<InitiatedBehavior>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AquariumCreatureState {
    pub position: NormalizedPosition,
    pub velocity: NormalizedVelocity,
    pub facing: Facing,
    pub gaze: GazeTarget,
    pub depth_lane: DepthLane,
    pub steering: SteeringMode,
    pub destination: Option<SemanticDestination>,
    pub action: Option<ActionTimeline>,
}

impl Default for AquariumCreatureState {
    fn default() -> Self {
        Self {
            position: NormalizedPosition::new(5_000, 4_500),
            velocity: NormalizedVelocity::default(),
            facing: Facing::Right,
            gaze: GazeTarget::None,
            depth_lane: DepthLane::Middle,
            steering: SteeringMode::Hover,
            destination: None,
            action: None,
        }
    }
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
    #[serde(default)]
    pub aquarium: AquariumState,
    #[serde(default)]
    pub random_domains: BTreeMap<String, u64>,
    #[serde(default)]
    pub absence_days: u32,
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
                conversation: ConversationState::default(),
                development: Development::default(),
                aquarium: AquariumCreatureState::default(),
                routines: Vec::new(),
                favorite_locations: BTreeMap::new(),
                initiated_behavior: None,
            },
            aquarium: AquariumState::default(),
            random_domains: BTreeMap::new(),
            absence_days: 0,
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
        let development = &self.creature.development;
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
        } else if !self.aquarium.player_present && creature.relationship.bond > 0.3 {
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
            contradicting_memories: BTreeSet::new(),
            confidence: confidence_delta.clamp(0.0, 1.0),
        });
    }

    /// Record evidence against a belief and revise its confidence without deleting its history.
    pub(crate) fn contradict_belief(&mut self, kind: BeliefKind, memory: MemoryId, delta: f32) {
        if let Some(belief) = self
            .creature
            .beliefs
            .iter_mut()
            .find(|belief| belief.kind == kind)
        {
            belief.contradicting_memories.insert(memory);
            belief.confidence = (belief.confidence - delta.abs()).clamp(0.0, 1.0);
        }
    }

    #[must_use]
    pub fn favorite_destination(&self) -> Option<SemanticDestination> {
        self.creature
            .favorite_locations
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(destination, _)| *destination)
    }

    pub fn revise_belief(&mut self, kind: BeliefKind, memory: MemoryId, supports: bool) {
        if supports {
            self.reinforce_belief(kind, memory, 0.15);
        } else {
            self.contradict_belief(kind, memory, 0.15);
        }
    }

    pub fn record_favorite(&mut self, destination: SemanticDestination) {
        if !matches!(
            destination,
            SemanticDestination::Cave | SemanticDestination::Plant | SemanticDestination::Bottom
        ) {
            return;
        }
        let count = self
            .creature
            .favorite_locations
            .entry(destination)
            .or_default();
        *count = count.saturating_add(1);
    }

    pub fn set_routine(&mut self, routine: Routine) {
        if let Some(existing) = self
            .creature
            .routines
            .iter_mut()
            .find(|existing| existing.hour_start == routine.hour_start)
        {
            *existing = routine;
        } else if self.creature.routines.len() < 8 {
            self.creature.routines.push(routine);
            self.creature
                .routines
                .sort_by_key(|routine| routine.hour_start);
        }
    }

    /// Draw from a named persisted stream. Callers that need replay-stable randomness should use
    /// this rather than sharing the simulation stream with presentation micro-motion.
    pub fn domain_draw(&mut self, domain: RandomDomain) -> f32 {
        let key = self
            .random_domains
            .entry(domain.name().to_owned())
            .or_default();
        let draw = deterministic_unit(self.seed, domain, *key);
        *key = key.saturating_add(1);
        draw
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
        if self.aquarium.next_object_id == 0 || self.aquarium.max_food == 0 {
            return Err(StateValidationError::Aquarium);
        }
        if self.creature.aquarium.position != self.creature.aquarium.position.clamped()
            || self.creature.aquarium.velocity != self.creature.aquarium.velocity.clamped()
            || self
                .aquarium
                .cursor
                .is_some_and(|position| position != position.clamped())
            || self.aquarium.objects.values().any(|object| match object {
                WorldObject::Food(food) => {
                    food.position != food.position.clamped()
                        || food.velocity != food.velocity.clamped()
                        || food.id == 0
                }
                WorldObject::Toy { position, .. }
                | WorldObject::Plant { position }
                | WorldObject::Cave { position } => *position != position.clamped(),
            })
        {
            return Err(StateValidationError::Aquarium);
        }
        if self
            .aquarium
            .objects
            .keys()
            .any(|id| *id == 0 || *id >= self.aquarium.next_object_id)
        {
            return Err(StateValidationError::Aquarium);
        }
        if self.creature.beliefs.iter().any(|belief| {
            belief
                .supporting_memories
                .intersection(&belief.contradicting_memories)
                .next()
                .is_some()
        }) {
            return Err(StateValidationError::Belief);
        }
        let development = &self.creature.development;
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
    #[error("aquarium state is invalid")]
    Aquarium,
}
