use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;
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
    /// Known words the creature has answered.
    #[serde(default)]
    pub requests: u32,
    /// Taps on the glass.
    #[serde(default)]
    pub taps: u32,
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
    /// A playful refusal is never repeated for the very next request.
    #[serde(default)]
    pub refused_last_request: bool,
    /// When the creature last asked for something on its own.
    #[serde(default)]
    pub last_asked_ms: u64,
    /// When the creature last named something unprompted.
    #[serde(default)]
    pub last_remark_ms: u64,
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
    #[serde(alias = "red_food_is_a_trick")]
    FoodIsATrick,
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
    /// Velocities are fixed-point units per second.
    pub const MAX_COMPONENT: i32 = 8_000;

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

/// The authoritative reason a creature is travelling to a semantic destination.
///
/// This is persisted separately from the projected intention because intentions may change while
/// a journey is still active. Arrival code must dispatch on this owner, never infer meaning from
/// proximity or the current intention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TravelPurpose {
    IdleVisit {
        visit_id: NonZeroU64,
    },
    ToyInteraction {
        interaction_id: NonZeroU64,
    },
    RefusalStare {
        interaction_id: NonZeroU64,
    },
    CursorSocial {
        action_id: NonZeroU64,
    },
    Relationship {
        beat_id: NonZeroU64,
    },
    Initiative {
        initiative_id: NonZeroU64,
        requested_at_ms: u64,
    },
    PrivateLife {
        activity_id: NonZeroU64,
    },
}

/// The concrete subject an autonomous activity is about.  This intentionally stays distinct
/// from a travel destination: the activity owns the meaning, travel only owns locomotion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivitySubject {
    Toy(ToyId),
    Cave,
    Plant,
    Bottom,
    OpenWater,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivateLifeKind {
    ToyPlay(ToyId),
    CaveSettle,
    PlantInspect,
    BottomForage,
    OpenWaterDrift,
}

impl PrivateLifeKind {
    #[must_use]
    pub const fn subject(self) -> ActivitySubject {
        match self {
            Self::ToyPlay(toy) => ActivitySubject::Toy(toy),
            Self::CaveSettle => ActivitySubject::Cave,
            Self::PlantInspect => ActivitySubject::Plant,
            Self::BottomForage => ActivitySubject::Bottom,
            Self::OpenWaterDrift => ActivitySubject::OpenWater,
        }
    }

    #[must_use]
    pub const fn destination(self) -> Option<SemanticDestination> {
        match self {
            Self::ToyPlay(toy) => Some(SemanticDestination::Toy(toy)),
            Self::CaveSettle => Some(SemanticDestination::Cave),
            Self::PlantInspect => Some(SemanticDestination::Plant),
            Self::BottomForage => Some(SemanticDestination::Bottom),
            Self::OpenWaterDrift => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityRecipe {
    BallNudge,
    BellStrike,
    SockTug,
    CaveShelter,
    PlantOrbit,
    BottomForage,
    OpenWaterDrift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityPhase {
    Notice,
    Approach,
    Act,
    Recover,
    Settle,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityPurpose {
    Autonomous,
    NeedUrgency,
    Routine,
    Preference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityInterruptionOwner {
    Player,
    Food,
    Toy,
    Comfort,
    Sleep,
    Relationship,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivitySelectionEvidence {
    pub need_pressure: u8,
    pub trait_bias: u8,
    pub preference: i8,
    pub routine_hour: Option<u8>,
    #[serde(default)]
    pub relationship_evidence: Vec<RelationshipEvidence>,
    #[serde(default)]
    pub excluded_families: Vec<PrivateLifeKind>,
    #[serde(default)]
    pub excluded_subjects: Vec<ActivitySubject>,
    #[serde(default)]
    pub excluded_recipes: Vec<ActivityRecipe>,
    #[serde(default)]
    pub urgency_overrode_repetition: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateLifeActivity {
    pub id: NonZeroU64,
    pub kind: PrivateLifeKind,
    pub subject: Option<ActivitySubject>,
    pub purpose: ActivityPurpose,
    pub recipe: ActivityRecipe,
    pub phase: ActivityPhase,
    #[serde(default)]
    pub selected_at_ms: u64,
    pub phase_started_at_ms: u64,
    pub selected_from: ActivitySelectionEvidence,
    #[serde(default)]
    pub payoff_reached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecentActivity {
    pub id: NonZeroU64,
    pub kind: PrivateLifeKind,
    pub subject: Option<ActivitySubject>,
    pub recipe: ActivityRecipe,
    pub selected_at_ms: u64,
    pub completed_at_ms: Option<u64>,
    pub interrupted_by: Option<ActivityInterruptionOwner>,
    pub active_day: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateLifeState {
    #[serde(default = "first_action_id")]
    pub next_activity_id: u64,
    #[serde(default)]
    pub active: Option<PrivateLifeActivity>,
    #[serde(default)]
    pub recent: Vec<RecentActivity>,
}

impl Default for PrivateLifeState {
    fn default() -> Self {
        Self {
            next_activity_id: first_action_id(),
            active: None,
            recent: Vec::new(),
        }
    }
}

/// Stable authoritative toy physics and contact history.  The object catalogue keeps authored
/// placement; this state carries the mutable, save-owned response to a particular interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToyResponse {
    None,
    BallNudged,
    BellStruck,
    SockTugged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToyObjectState {
    pub position: NormalizedPosition,
    pub velocity: NormalizedVelocity,
    pub carried: bool,
    pub last_response: ToyResponse,
    pub last_contact_activity: Option<NonZeroU64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TravelTarget {
    pub destination: SemanticDestination,
    pub purpose: TravelPurpose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToyOrigin {
    Player,
    Autonomous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToyInteractionPhase {
    Approach,
    Contact,
    Resolved,
    Recovery,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToyInteractionOutcome {
    Accepted,
    Rejected,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToyInteraction {
    pub id: NonZeroU64,
    pub toy: ToyId,
    pub origin: ToyOrigin,
    pub outcome: ToyInteractionOutcome,
    pub phase: ToyInteractionPhase,
    #[serde(default)]
    pub relationship: Option<ActionRelationshipContext>,
    /// When the post-contact recovery (a held sock, a satisfied pause) ends.
    #[serde(default)]
    pub recovery_until_ms: u64,
    /// Further chase rounds in this play session: a direct offer is a little game, not one tap.
    #[serde(default)]
    pub rounds_left: u8,
    /// Contacts made so far; history and reward are recorded on the first one only.
    #[serde(default)]
    pub contacts: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedToyInteraction {
    pub id: NonZeroU64,
    pub toy: ToyId,
    pub origin: ToyOrigin,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RelationshipSubject {
    Food(FoodId),
    Toy(ToyId),
    Place(SemanticDestination),
    Player,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoodOutcome {
    Consumed,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipExpressionMode {
    ActionBound,
    Standalone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRelationshipContext {
    pub motif: RelationshipMotifKey,
    pub expression_kind: RelationshipExpressionKind,
    #[serde(default)]
    pub evidence: Vec<RelationshipEvidence>,
    pub subject: RelationshipSubject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRelationshipMoment {
    pub action_id: u64,
    pub context: ActionRelationshipContext,
    pub started_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionTimeline {
    #[serde(default)]
    pub action_id: u64,
    pub phase: ActionPhase,
    pub elapsed_ms: u64,
    pub phase_duration_ms: u64,
    pub destination: SemanticDestination,
    pub food_id: Option<u64>,
    #[serde(default)]
    pub food: Option<FoodId>,
    #[serde(default)]
    pub food_outcome: Option<FoodOutcome>,
    #[serde(default)]
    pub relationship: Option<ActionRelationshipContext>,
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
    #[serde(default)]
    pub toy_states: BTreeMap<ToyId, ToyObjectState>,
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
                    position: NormalizedPosition::new(4_800, 7_650),
                },
            ),
            (
                4,
                WorldObject::Toy {
                    toy: ToyId::Bell,
                    position: NormalizedPosition::new(6_650, 8_700),
                },
            ),
            (
                5,
                WorldObject::Toy {
                    toy: ToyId::Sock,
                    position: NormalizedPosition::new(8_250, 10_000),
                },
            ),
        ]);
        Self {
            objects,
            next_object_id: 6,
            cursor: None,
            object_names: BTreeMap::new(),
            toy_states: BTreeMap::from([
                (
                    ToyId::Ball,
                    ToyObjectState {
                        position: NormalizedPosition::new(4_800, 7_650),
                        velocity: NormalizedVelocity::default(),
                        carried: false,
                        last_response: ToyResponse::None,
                        last_contact_activity: None,
                    },
                ),
                (
                    ToyId::Bell,
                    ToyObjectState {
                        position: NormalizedPosition::new(6_650, 8_700),
                        velocity: NormalizedVelocity::default(),
                        carried: false,
                        last_response: ToyResponse::None,
                        last_contact_activity: None,
                    },
                ),
                (
                    ToyId::Sock,
                    ToyObjectState {
                        position: NormalizedPosition::new(8_250, 10_000),
                        velocity: NormalizedVelocity::default(),
                        carried: false,
                        last_response: ToyResponse::None,
                        last_contact_activity: None,
                    },
                ),
            ]),
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

/// Persisted evidence for a repeated, genuine visit during an active-day hour slot.
///
/// This deliberately records only one visit per active day.  A creature pacing back and
/// forth during one short session must not manufacture a routine from a single outing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisitEvidence {
    pub hour_start: u8,
    pub destination: SemanticDestination,
    pub visits: u8,
    pub last_active_day: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct IdleLifeState {
    pub last_arrived_destination: Option<SemanticDestination>,
    pub settled_until_ms: u64,
    #[serde(default)]
    pub visit_evidence: Vec<VisitEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionState {
    /// A short embodied aftermath of comfort, after which the creature returns to its own life.
    pub affectionate_until_ms: u64,
    /// Kept so a sleeping save can resume restoring energy with a bounded wake-up.
    pub sleep_started_at_ms: Option<u64>,
    #[serde(default = "first_action_id")]
    pub next_action_id: u64,
    #[serde(default = "first_action_id")]
    pub next_toy_interaction_id: u64,
    #[serde(default)]
    pub toy_interaction: Option<ToyInteraction>,
    #[serde(default)]
    pub last_resolved_toy_interaction: Option<ResolvedToyInteraction>,
    #[serde(default)]
    pub relationship_moment: Option<ActionRelationshipMoment>,
}

const fn first_action_id() -> u64 {
    1
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            affectionate_until_ms: 0,
            sleep_started_at_ms: None,
            next_action_id: first_action_id(),
            next_toy_interaction_id: first_action_id(),
            toy_interaction: None,
            last_resolved_toy_interaction: None,
            relationship_moment: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RelationshipMotifKey {
    SharedToy(ToyId),
    ComfortRitual,
    TrustedFood(FoodId),
    FoodGrudge(FoodId),
    PlayerReturns,
    FamiliarPlace(SemanticDestination),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipMotif {
    pub key: RelationshipMotifKey,
    pub strength: u8,
    #[serde(default)]
    pub evidence: Vec<RelationshipEvidence>,
    pub last_supported_day: u64,
    #[serde(default)]
    pub eligible_triggers: BTreeSet<RelationshipTriggerKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "evidence_kind", rename_all = "snake_case")]
pub enum RelationshipEvidence {
    Memory {
        id: MemoryId,
    },
    Belief {
        id: BeliefId,
        kind: BeliefKind,
    },
    Visit {
        hour_start: u8,
        destination: SemanticDestination,
        last_active_day: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipExpressionKind {
    Notice,
    Anticipate,
    Seek,
    Ritual,
    Recognize,
    Welcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipTriggerKind {
    PlayerReturn,
    FoodPresented,
    FoodResolved,
    ToyEngaged,
    ComfortCompleted,
    ComfortNeeded,
    RoutineWindow,
    PlaceArrived,
    RelevantUtterance,
    QuietMoment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RelationshipTrigger {
    PlayerReturn,
    FoodPresented {
        food: FoodId,
    },
    FoodResolved {
        food: FoodId,
        outcome: FoodOutcome,
    },
    ToyEngaged {
        toy: ToyId,
    },
    ComfortCompleted,
    ComfortNeeded,
    RoutineWindow {
        hour_start: u8,
        destination: SemanticDestination,
    },
    PlaceArrived {
        destination: SemanticDestination,
    },
    RelevantUtterance {
        subject: Option<RelationshipSubject>,
    },
    QuietMoment,
}

impl RelationshipTrigger {
    #[must_use]
    pub const fn kind(self) -> RelationshipTriggerKind {
        match self {
            Self::PlayerReturn => RelationshipTriggerKind::PlayerReturn,
            Self::FoodPresented { .. } => RelationshipTriggerKind::FoodPresented,
            Self::FoodResolved { .. } => RelationshipTriggerKind::FoodResolved,
            Self::ToyEngaged { .. } => RelationshipTriggerKind::ToyEngaged,
            Self::ComfortCompleted => RelationshipTriggerKind::ComfortCompleted,
            Self::ComfortNeeded => RelationshipTriggerKind::ComfortNeeded,
            Self::RoutineWindow { .. } => RelationshipTriggerKind::RoutineWindow,
            Self::PlaceArrived { .. } => RelationshipTriggerKind::PlaceArrived,
            Self::RelevantUtterance { .. } => RelationshipTriggerKind::RelevantUtterance,
            Self::QuietMoment => RelationshipTriggerKind::QuietMoment,
        }
    }

    #[must_use]
    pub const fn subject(self) -> Option<RelationshipSubject> {
        match self {
            Self::FoodPresented { food } | Self::FoodResolved { food, .. } => {
                Some(RelationshipSubject::Food(food))
            }
            Self::ToyEngaged { toy } => Some(RelationshipSubject::Toy(toy)),
            Self::ComfortCompleted | Self::ComfortNeeded | Self::PlayerReturn => {
                Some(RelationshipSubject::Player)
            }
            Self::RoutineWindow { destination, .. } | Self::PlaceArrived { destination } => {
                Some(RelationshipSubject::Place(destination))
            }
            Self::RelevantUtterance { subject } => subject,
            Self::QuietMoment => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipBeatPhase {
    Notice,
    Anticipate,
    Act,
    Recover,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipBeat {
    pub motif: RelationshipMotifKey,
    pub trigger: RelationshipTrigger,
    pub subject: Option<RelationshipSubject>,
    pub expression_kind: RelationshipExpressionKind,
    #[serde(default)]
    pub evidence: Vec<RelationshipEvidence>,
    pub target: Option<SemanticDestination>,
    pub phase: RelationshipBeatPhase,
    pub started_at_ms: u64,
    pub phase_started_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpressedMotif {
    pub key: RelationshipMotifKey,
    pub expressed_at_ms: u64,
    pub expression_kind: RelationshipExpressionKind,
}

/// The embodied treatment chosen for one relationship callback.
///
/// This is intentionally separate from the motif. A motif establishes what the creature knows;
/// the recipe establishes the particular, repeatable performance that made that knowledge
/// visible. Keeping it typed makes the suppression ledger robust to future presentation work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "expression", rename_all = "snake_case")]
pub enum RelationshipPerformanceRecipe {
    SharedBall(RelationshipExpressionKind),
    SharedBell(RelationshipExpressionKind),
    SharedSock(RelationshipExpressionKind),
    ComfortAttention(RelationshipExpressionKind),
    TrustedFoodReceipt(RelationshipExpressionKind),
    FoodGrudgeReceipt(RelationshipExpressionKind),
    PlayerReturn(RelationshipExpressionKind),
    FamiliarCave(RelationshipExpressionKind),
    FamiliarPlant(RelationshipExpressionKind),
    FamiliarBottom(RelationshipExpressionKind),
    FamiliarPlayer(RelationshipExpressionKind),
    FamiliarToy(RelationshipExpressionKind),
    FamiliarFood(RelationshipExpressionKind),
    FamiliarPosition(RelationshipExpressionKind),
}

/// One authoritative performance receipt. The bounded ledger stops a newly remembered event
/// from immediately replaying as a callback, while preserving the exact history that did speak.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipPerformanceRecord {
    pub motif: RelationshipMotifKey,
    pub subject: RelationshipSubject,
    #[serde(default)]
    pub evidence: Vec<RelationshipEvidence>,
    pub recipe: RelationshipPerformanceRecipe,
    pub performed_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipExpressionState {
    #[serde(default = "relationship_expression_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub recent: Vec<ExpressedMotif>,
    #[serde(default)]
    pub performance_ledger: Vec<RelationshipPerformanceRecord>,
    #[serde(default)]
    pub active: Option<RelationshipBeat>,
    #[serde(default)]
    pub last_expressed_at_ms: Option<u64>,
    #[serde(default)]
    pub count_active_day: u8,
    #[serde(default = "first_active_day")]
    pub count_active_day_index: u64,
}

const fn relationship_expression_schema_version() -> u32 {
    crate::RELATIONSHIP_EXPRESSION_SCHEMA_VERSION
}

const fn first_active_day() -> u64 {
    1
}

impl Default for RelationshipExpressionState {
    fn default() -> Self {
        Self {
            schema_version: crate::RELATIONSHIP_EXPRESSION_SCHEMA_VERSION,
            recent: Vec::new(),
            performance_ledger: Vec::new(),
            active: None,
            last_expressed_at_ms: None,
            count_active_day: 0,
            count_active_day_index: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitiatedBehavior {
    pub reason: InitiativeReason,
    pub nonverbal: Option<NonverbalAct>,
    pub requested_at_ms: u64,
    #[serde(default)]
    pub expires_at_ms: u64,
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
    pub idle_life: IdleLifeState,
    #[serde(default)]
    pub private_life: PrivateLifeState,
    #[serde(default)]
    pub interaction_state: InteractionState,
    #[serde(default)]
    pub relationship_expression: RelationshipExpressionState,
    #[serde(default)]
    pub routines: Vec<Routine>,
    #[serde(default)]
    pub favorite_locations: BTreeMap<SemanticDestination, u32>,
    #[serde(default)]
    pub initiated_behavior: Option<InitiatedBehavior>,
    /// Words heard from the player and the evidence for what they mean.
    #[serde(default)]
    pub lexicon: crate::Lexicon,
    /// Short-lived shared focus: what a word heard right now would most plausibly refer to.
    #[serde(default)]
    pub attention: Vec<crate::FocusMark>,
    /// The thing it most recently refused, shown until the moment passes.
    #[serde(default)]
    pub refusing: Option<(crate::Meaning, u64)>,
    /// A brand-new creature waits shyly in its cave until it has met the player.
    #[serde(default)]
    pub hidden_until_met: bool,
    /// When the player arrived for the first meeting; the shy wait counts from here.
    #[serde(default)]
    pub met_player_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AquariumCreatureState {
    pub position: NormalizedPosition,
    pub velocity: NormalizedVelocity,
    pub facing: Facing,
    pub gaze: GazeTarget,
    pub depth_lane: DepthLane,
    pub steering: SteeringMode,
    pub destination: Option<SemanticDestination>,
    #[serde(default)]
    pub travel_purpose: Option<TravelPurpose>,
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
            travel_purpose: None,
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

/// A new creature has a clear favorite, something it is fine with, and something it dislikes,
/// so preferences are discovered rather than averaged away. Jitter keeps creatures distinct.
fn ranked_preferences<T: Ord + Copy>(seed: u64, items: [(T, u64); 3]) -> BTreeMap<T, f32> {
    let mut ranked = items.map(|(item, key)| {
        (
            crate::deterministic_unit(seed, crate::RandomDomain::Preferences, 1_000 + key),
            crate::deterministic_unit(seed, crate::RandomDomain::Preferences, 2_000 + key),
            item,
        )
    });
    ranked.sort_by(|left, right| right.0.total_cmp(&left.0));
    ranked
        .into_iter()
        .zip([0.75_f32, 0.25, -0.6])
        .map(|((_, jitter, item), base)| (item, base + (jitter - 0.5) * 0.16))
        .collect()
}

impl WorldState {
    /// A new creature for a player who has never met it: it starts tucked in its cave mouth
    /// and comes out to meet them.
    #[must_use]
    pub fn first_meeting(seed: u64, name: impl Into<String>) -> Self {
        let mut world = Self::new(seed, name);
        world.creature.hidden_until_met = true;
        world.aquarium.player_present = false;
        world.creature.aquarium.position = NormalizedPosition::new(1_700, 8_100);
        world.creature.aquarium.facing = Facing::Right;
        world
    }

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
                preferences: ranked_preferences(
                    seed,
                    [
                        (FoodId::Berry, 11),
                        (FoodId::Mushroom, 12),
                        (FoodId::Pellet, 13),
                    ],
                ),
                toy_preferences: ranked_preferences(
                    seed,
                    [(ToyId::Ball, 21), (ToyId::Bell, 22), (ToyId::Sock, 23)],
                ),
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
                idle_life: IdleLifeState::default(),
                private_life: PrivateLifeState::default(),
                interaction_state: InteractionState::default(),
                relationship_expression: RelationshipExpressionState::default(),
                routines: Vec::new(),
                favorite_locations: BTreeMap::new(),
                initiated_behavior: None,
                lexicon: crate::Lexicon::default(),
                attention: Vec::new(),
                refusing: None,
                hidden_until_met: false,
                met_player_at_ms: None,
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
        if self.aquarium.toy_states.len() > 3
            || self.aquarium.toy_states.values().any(|toy| {
                toy.position != toy.position.clamped() || toy.velocity != toy.velocity.clamped()
            })
        {
            return Err(StateValidationError::Aquarium);
        }
        let has_valid_visit_destination = |destination: SemanticDestination| {
            matches!(
                destination,
                SemanticDestination::Cave
                    | SemanticDestination::Plant
                    | SemanticDestination::Bottom
                    | SemanticDestination::Toy(_)
            )
        };
        if self.creature.routines.len() > 8
            || self.creature.routines.iter().any(|routine| {
                routine.hour_start >= 24
                    || routine.strength == 0
                    || !has_valid_visit_destination(routine.destination)
            })
            || self.creature.idle_life.visit_evidence.len() > 32
            || self
                .creature
                .idle_life
                .visit_evidence
                .iter()
                .any(|evidence| {
                    evidence.hour_start >= 24
                        || evidence.visits == 0
                        || evidence.last_active_day > self.active_day()
                        || !has_valid_visit_destination(evidence.destination)
                })
            || self
                .creature
                .idle_life
                .visit_evidence
                .windows(2)
                .any(|pair| {
                    pair[0].hour_start > pair[1].hour_start
                        || (pair[0].hour_start == pair[1].hour_start
                            && pair[0].destination >= pair[1].destination)
                })
            || self
                .creature
                .interaction_state
                .sleep_started_at_ms
                .is_some_and(|started_at| started_at > self.elapsed_ms)
        {
            return Err(StateValidationError::IdleLife);
        }
        let private_life = &self.creature.private_life;
        let valid_private_activity = |activity: &PrivateLifeActivity| {
            activity.id.get() < private_life.next_activity_id
                && activity.subject == Some(activity.kind.subject())
                && activity.selected_at_ms <= self.elapsed_ms
                && activity.phase_started_at_ms <= self.elapsed_ms
        };
        let private_life_invalid = private_life.next_activity_id == 0
            || private_life.recent.len() > 32
            || private_life
                .active
                .as_ref()
                .is_some_and(|activity| !valid_private_activity(activity))
            || private_life.recent.iter().any(|entry| {
                entry.id.get() >= private_life.next_activity_id
                    || entry.subject != Some(entry.kind.subject())
                    || entry.selected_at_ms > self.elapsed_ms
                    || entry
                        .completed_at_ms
                        .is_some_and(|time| time > self.elapsed_ms)
                    || (entry.completed_at_ms.is_some() && entry.interrupted_by.is_some())
            })
            || private_life
                .recent
                .windows(2)
                .any(|pair| pair[0].id >= pair[1].id);
        if private_life_invalid {
            return Err(StateValidationError::PrivateLife);
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
        // Language stage follows learned vocabulary and never regresses, so earlier saves may
        // legitimately be ahead of their current word count.
        if development.active_days_reached == 0
            || u64::from(development.active_days_reached) > self.active_day()
        {
            return Err(StateValidationError::Development);
        }
        let memory_ids = self
            .creature
            .memories
            .iter()
            .map(|memory| memory.id)
            .collect::<BTreeSet<_>>();
        let valid_motif_key = |key: RelationshipMotifKey| match key {
            RelationshipMotifKey::FamiliarPlace(destination) => {
                has_valid_visit_destination(destination)
            }
            RelationshipMotifKey::SharedToy(_)
            | RelationshipMotifKey::ComfortRitual
            | RelationshipMotifKey::TrustedFood(_)
            | RelationshipMotifKey::FoodGrudge(_)
            | RelationshipMotifKey::PlayerReturns => true,
        };
        let valid_relationship_evidence = |evidence: RelationshipEvidence| match evidence {
            RelationshipEvidence::Memory { id } => memory_ids.contains(&id),
            RelationshipEvidence::Belief { id, kind } => self
                .creature
                .beliefs
                .iter()
                .any(|candidate| candidate.id == id && candidate.kind == kind),
            RelationshipEvidence::Visit {
                hour_start,
                destination,
                last_active_day,
            } => {
                hour_start < 24
                    && has_valid_visit_destination(destination)
                    && last_active_day <= self.active_day()
                    && self.creature.idle_life.visit_evidence.iter().any(|visit| {
                        visit.hour_start == hour_start
                            && visit.destination == destination
                            && visit.last_active_day >= last_active_day
                    })
            }
        };
        let interaction = &self.creature.interaction_state;
        let destination = self.creature.aquarium.destination;
        let travel = self.creature.aquarium.travel_purpose;
        let travel_invalid = destination.is_some() != travel.is_some()
            || destination
                .zip(travel)
                .is_some_and(|(destination, purpose)| match purpose {
                    TravelPurpose::IdleVisit { .. } => !has_valid_visit_destination(destination),
                    TravelPurpose::ToyInteraction { interaction_id } => interaction
                        .toy_interaction
                        .as_ref()
                        .is_none_or(|toy_interaction| {
                            toy_interaction.id != interaction_id
                                || toy_interaction.outcome != ToyInteractionOutcome::Accepted
                                || toy_interaction.phase != ToyInteractionPhase::Approach
                                || self.creature.current_intention != Intention::Play
                                || destination != SemanticDestination::Toy(toy_interaction.toy)
                        }),
                    TravelPurpose::RefusalStare { interaction_id } => interaction
                        .toy_interaction
                        .as_ref()
                        .is_none_or(|toy_interaction| {
                            toy_interaction.id != interaction_id
                                || toy_interaction.origin != ToyOrigin::Player
                                || toy_interaction.outcome != ToyInteractionOutcome::Rejected
                                || toy_interaction.phase != ToyInteractionPhase::Approach
                                || self.creature.current_intention != Intention::RefuseAndStare
                                || destination != SemanticDestination::Toy(toy_interaction.toy)
                        }),
                    TravelPurpose::CursorSocial { .. } => !matches!(
                        destination,
                        SemanticDestination::Player | SemanticDestination::Position(_)
                    ),
                    TravelPurpose::Relationship { .. } => {
                        self.creature.relationship_expression.active.is_none()
                    }
                    TravelPurpose::PrivateLife { activity_id } => self
                        .creature
                        .private_life
                        .active
                        .as_ref()
                        .is_none_or(|activity| {
                            activity.id != activity_id
                                || activity.phase != ActivityPhase::Approach
                                || match activity.kind.destination() {
                                    Some(expected) => expected != destination,
                                    // An open-water chase targets a point in the water.
                                    None => {
                                        !matches!(destination, SemanticDestination::Position(_))
                                    }
                                }
                        }),
                    TravelPurpose::Initiative {
                        requested_at_ms, ..
                    } => {
                        requested_at_ms > self.elapsed_ms
                            || self.creature.initiated_behavior.is_none()
                    }
                });
        let toy_interaction_invalid = interaction.next_toy_interaction_id == 0
            || interaction
                .toy_interaction
                .as_ref()
                .is_some_and(|toy_interaction| {
                    let expected_purpose = match toy_interaction.outcome {
                        ToyInteractionOutcome::Accepted => TravelPurpose::ToyInteraction {
                            interaction_id: toy_interaction.id,
                        },
                        ToyInteractionOutcome::Rejected => TravelPurpose::RefusalStare {
                            interaction_id: toy_interaction.id,
                        },
                        ToyInteractionOutcome::Interrupted => return true,
                    };
                    toy_interaction.id.get() >= interaction.next_toy_interaction_id
                        || matches!(
                            toy_interaction.phase,
                            ToyInteractionPhase::Contact
                                | ToyInteractionPhase::Resolved
                                | ToyInteractionPhase::Interrupted
                        )
                        || (toy_interaction.phase == ToyInteractionPhase::Approach
                            && (destination
                                != Some(SemanticDestination::Toy(toy_interaction.toy))
                                || travel != Some(expected_purpose)))
                        || toy_interaction.relationship.as_ref().is_some_and(|context| {
                            toy_interaction.origin != ToyOrigin::Player
                                || context.subject
                                    != RelationshipSubject::Toy(toy_interaction.toy)
                                || context.motif
                                    != RelationshipMotifKey::SharedToy(toy_interaction.toy)
                                || !crate::relationship::action_relationship_context_is_grounded(
                                    self, context,
                                )
                        })
                })
            || interaction
                .last_resolved_toy_interaction
                .is_some_and(|resolved| resolved.id.get() >= interaction.next_toy_interaction_id);
        let action = self.creature.aquarium.action.as_ref();
        let action_invalid = action.is_some_and(|action| {
            let food_identity = action.food.zip(action.food_id);
            let object_matches = food_identity.is_some_and(|(food, id)| {
                self.aquarium.objects.get(&id).is_some_and(|object| {
                    matches!(object, WorldObject::Food(candidate) if candidate.food == food)
                })
            });
            let outcome_matches_phase = match action.food_outcome {
                None => action.phase != ActionPhase::Recover && object_matches,
                Some(FoodOutcome::Consumed) => {
                    action.phase == ActionPhase::Recover
                        && action
                            .food_id
                            .is_some_and(|id| !self.aquarium.objects.contains_key(&id))
                }
                Some(FoodOutcome::Rejected) => {
                    action.phase == ActionPhase::Recover
                        && action.food_id.is_some_and(|id| {
                            matches!(
                                self.aquarium.objects.get(&id),
                                Some(WorldObject::Food(food))
                                    if food.disposition == FoodDisposition::Rejected
                            )
                        })
                }
            };
            let relationship_matches = action.relationship.as_ref().is_none_or(|context| {
                action.food.is_some_and(|food| {
                    context.subject == RelationshipSubject::Food(food)
                        && matches!(
                            context.motif,
                            RelationshipMotifKey::TrustedFood(candidate)
                                | RelationshipMotifKey::FoodGrudge(candidate)
                                if candidate == food
                        )
                        && !matches!(
                            (context.motif, action.food_outcome),
                            (
                                RelationshipMotifKey::TrustedFood(_),
                                Some(FoodOutcome::Rejected)
                            ) | (
                                RelationshipMotifKey::FoodGrudge(_),
                                Some(FoodOutcome::Consumed)
                            )
                        )
                        && crate::relationship::action_relationship_context_is_grounded(
                            self, context,
                        )
                })
            });
            action.action_id == 0
                || action.action_id >= interaction.next_action_id
                || action.food.is_none()
                || action.food_id.is_none()
                || !outcome_matches_phase
                || !relationship_matches
        });
        let relationship_moment_invalid =
            interaction
                .relationship_moment
                .as_ref()
                .is_some_and(|moment| {
                    moment.action_id == 0
                        || moment.action_id >= interaction.next_action_id
                        || moment.started_at_ms > self.elapsed_ms
                        || moment.expires_at_ms <= moment.started_at_ms
                        || moment.expires_at_ms <= self.elapsed_ms
                        || !crate::relationship::action_relationship_context_is_grounded(
                            self,
                            &moment.context,
                        )
                });
        let expression = &self.creature.relationship_expression;
        if travel_invalid || toy_interaction_invalid {
            return Err(StateValidationError::ToyInteraction);
        }
        if interaction.next_action_id == 0
            || action_invalid
            || relationship_moment_invalid
            || expression.schema_version != crate::RELATIONSHIP_EXPRESSION_SCHEMA_VERSION
            || expression.recent.len() > 8
            || expression.performance_ledger.len()
                > crate::relationship::MAX_PERFORMANCE_LEDGER_RECORDS
            || expression
                .last_expressed_at_ms
                .is_some_and(|time| time > self.elapsed_ms)
            || expression.count_active_day > 4
            || expression.count_active_day_index == 0
            || expression.count_active_day_index > self.active_day()
            || expression
                .recent
                .iter()
                .any(|entry| entry.expressed_at_ms > self.elapsed_ms || !valid_motif_key(entry.key))
            || expression
                .recent
                .windows(2)
                .any(|pair| pair[0].expressed_at_ms > pair[1].expressed_at_ms)
            || expression.performance_ledger.iter().any(|record| {
                record.performed_at_ms > self.elapsed_ms
                    || !valid_motif_key(record.motif)
                    || !crate::relationship::performance_record_is_grounded(self, record)
            })
            || expression
                .performance_ledger
                .windows(2)
                .any(|pair| pair[0].performed_at_ms > pair[1].performed_at_ms)
            || expression.active.as_ref().is_some_and(|beat| {
                beat.started_at_ms > self.elapsed_ms
                    || beat.phase_started_at_ms < beat.started_at_ms
                    || beat.phase_started_at_ms > self.elapsed_ms
                    || beat.evidence.len() > 8
                    || beat
                        .evidence
                        .iter()
                        .copied()
                        .any(|evidence| !valid_relationship_evidence(evidence))
                    || !valid_motif_key(beat.motif)
                    || !crate::relationship::relationship_beat_is_grounded(self, beat)
            })
        {
            return Err(StateValidationError::RelationshipExpression);
        }
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
    #[error("idle life, routine, or embodied interaction state is invalid")]
    IdleLife,
    #[error("private-life activity, ledger, or allocator is invalid")]
    PrivateLife,
    #[error("relationship expression state is invalid")]
    RelationshipExpression,
    #[error("toy interaction or travel ownership is invalid")]
    ToyInteraction,
}
