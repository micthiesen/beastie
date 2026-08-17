use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Belief, Concept, ConversationState, Creature, Development, FoodId, IdleLifeState, Intention,
    InteractionState, Memory, MemoryId, MemoryKind, Needs, Reaction, Relationship, SAVE_VERSION,
    SeededRandom, SocialAct, SocialHabits, StateValidationError, ToyId, Traits, WorldState,
};

const LEGACY_SAVE_VERSION: u32 = 1;
const AQUARIUM_SAVE_VERSION: u32 = 2;
const PRE_RELATIONSHIP_SAVE_VERSION: u32 = 3;
const RELATIONSHIP_SAVE_VERSION: u32 = 4;
const PRE_TRAVEL_OWNERSHIP_SAVE_VERSION: u32 = 5;

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
        if matches!(
            header.save_version,
            AQUARIUM_SAVE_VERSION
                | PRE_RELATIONSHIP_SAVE_VERSION
                | RELATIONSHIP_SAVE_VERSION
                | PRE_TRAVEL_OWNERSHIP_SAVE_VERSION
        ) {
            return PreviousSaveGame::from_json(source, header.save_version);
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
struct PreviousSaveGame {
    save_version: u32,
    world: WorldState,
    random: SeededRandom,
}

impl PreviousSaveGame {
    fn from_json(source: &str, source_version: u32) -> Result<SaveGame, SaveError> {
        let mut value =
            serde_json::from_str::<serde_json::Value>(source).map_err(SaveError::Json)?;
        if source_version == AQUARIUM_SAVE_VERSION
            && let Some(world) = value
                .get_mut("world")
                .and_then(serde_json::Value::as_object_mut)
        {
            world.remove("room");
            if let Some(aquarium) = world
                .get_mut("aquarium")
                .and_then(serde_json::Value::as_object_mut)
            {
                aquarium.remove("action");
                aquarium.remove("creature_position");
                aquarium.remove("creature_velocity");
                aquarium.remove("facing");
                aquarium.remove("gaze");
                aquarium.remove("depth_lane");
                aquarium.remove("steering");
                aquarium.remove("destination");
            }
            if let Some(creature) = world
                .get_mut("creature")
                .and_then(serde_json::Value::as_object_mut)
            {
                creature.remove("position");
                creature.remove("movement");
            }
        }
        if source_version == RELATIONSHIP_SAVE_VERSION
            && let Some(expression) = value
                .get_mut("world")
                .and_then(|world| world.get_mut("creature"))
                .and_then(|creature| creature.get_mut("relationship_expression"))
                .and_then(serde_json::Value::as_object_mut)
        {
            // V4 persisted broad triggers that no longer have an exact semantic subject. The
            // expression ledger is history-safe to retain, but the transient active beat is not.
            expression.insert("active".into(), serde_json::Value::Null);
            expression.insert(
                "schema_version".into(),
                crate::RELATIONSHIP_EXPRESSION_SCHEMA_VERSION.into(),
            );
        }
        let mut previous = serde_json::from_value::<Self>(value).map_err(SaveError::Json)?;
        if previous.save_version != source_version || previous.world.save_version != source_version
        {
            return Err(SaveError::Version(previous.save_version));
        }
        previous.save_version = SAVE_VERSION;
        previous.world = migrate_world(previous.world)?;
        let save = SaveGame {
            save_version: SAVE_VERSION,
            world: previous.world,
            random: previous.random,
        };
        save.validate()?;
        Ok(save)
    }
}

/// Upgrade an embedded core world that was deserialized by a containing save format.
///
/// Production session saves embed `WorldState` directly rather than nesting `SaveGame`, so their
/// loader must use this function instead of merely overwriting `save_version`.
pub fn migrate_world(mut world: WorldState) -> Result<WorldState, SaveError> {
    let source_version = world.save_version;
    if source_version == SAVE_VERSION {
        world.validate().map_err(SaveError::State)?;
        return Ok(world);
    }
    if !matches!(
        source_version,
        AQUARIUM_SAVE_VERSION
            | PRE_RELATIONSHIP_SAVE_VERSION
            | RELATIONSHIP_SAVE_VERSION
            | PRE_TRAVEL_OWNERSHIP_SAVE_VERSION
    ) {
        return Err(SaveError::Version(source_version));
    }
    world.save_version = SAVE_VERSION;
    if source_version == RELATIONSHIP_SAVE_VERSION {
        world.creature.relationship_expression.active = None;
        world.creature.relationship_expression.schema_version =
            crate::RELATIONSHIP_EXPRESSION_SCHEMA_VERSION;
        migrate_v4_action(&mut world);
    }
    migrate_pre_travel_ownership(&mut world);
    world.validate().map_err(SaveError::State)?;
    Ok(world)
}

fn migrate_pre_travel_ownership(world: &mut WorldState) {
    world.creature.interaction_state.next_toy_interaction_id = 1;
    world.creature.interaction_state.toy_interaction = None;
    world
        .creature
        .interaction_state
        .last_resolved_toy_interaction = None;
    world.creature.aquarium.travel_purpose = None;

    let Some(destination) = world.creature.aquarium.destination else {
        return;
    };
    if let crate::SemanticDestination::Toy(toy) = destination
        && world.creature.current_intention == Intention::RefuseAndStare
    {
        let interaction_id = NonZeroU64::new(1).expect("one is nonzero");
        world.creature.interaction_state.next_toy_interaction_id = 2;
        world.creature.interaction_state.toy_interaction = Some(crate::ToyInteraction {
            id: interaction_id,
            toy,
            origin: crate::ToyOrigin::Player,
            outcome: crate::ToyInteractionOutcome::Rejected,
            phase: crate::ToyInteractionPhase::Approach,
            relationship: None,
        });
        world.creature.aquarium.travel_purpose =
            Some(crate::TravelPurpose::RefusalStare { interaction_id });
        return;
    }

    // V5's destination did not carry its causal owner. In particular, `Play` may already have
    // eagerly applied its social mutation. Retain canonical history and clear the ambiguous trip
    // rather than manufacturing a second outcome on load.
    world.creature.aquarium.destination = None;
    world.creature.aquarium.steering = crate::SteeringMode::Hover;
    world.creature.aquarium.velocity = crate::NormalizedVelocity::default();
}

fn migrate_v4_action(world: &mut WorldState) {
    world.creature.interaction_state.next_action_id = 1;
    world.creature.interaction_state.relationship_moment = None;
    let Some(mut action) = world.creature.aquarium.action.take() else {
        return;
    };
    let Some(object_id) = action.food_id else {
        return;
    };
    let Some(crate::WorldObject::Food(food)) = world.aquarium.objects.get(&object_id) else {
        // A consumed V4 action no longer carries its semantic FoodId, so its transient recovery
        // cannot be represented exactly. Canonical memories and outcomes are already retained.
        return;
    };
    action.action_id = 1;
    action.food = Some(food.food);
    action.food_outcome = match food.disposition {
        crate::FoodDisposition::Rejected => Some(crate::FoodOutcome::Rejected),
        _ => None,
    };
    action.relationship = None;
    world.creature.interaction_state.next_action_id = 2;
    world.creature.aquarium.action = Some(action);
}

#[derive(Deserialize)]
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
            aquarium: crate::AquariumState {
                player_present: self.room.player_present(),
                ..crate::AquariumState::default()
            },
            random_domains: BTreeMap::new(),
            absence_days: 0,
        }
    }
}

#[derive(Deserialize)]
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
            development: Development::default(),
            aquarium: crate::AquariumCreatureState::default(),
            idle_life: IdleLifeState::default(),
            interaction_state: InteractionState::default(),
            relationship_expression: crate::RelationshipExpressionState::default(),
            routines: Vec::new(),
            favorite_locations: BTreeMap::new(),
            initiated_behavior: None,
        }
    }
}

#[derive(Deserialize)]
struct LegacyRoomState {
    player_present: bool,
}

impl LegacyRoomState {
    fn player_present(&self) -> bool {
        self.player_present
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

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("save JSON is malformed: {0}")]
    Json(serde_json::Error),
    #[error("save version {0} is unsupported")]
    Version(u32),
    #[error("save state is invalid: {0}")]
    State(StateValidationError),
}
