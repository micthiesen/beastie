use crate::{
    ActionPhase, ActionTimeline, BeliefKind, Concept, DevelopmentMilestone, FoodBuoyancy,
    FoodDisposition, FoodDropRejectionReason, FoodId, FoodObject, GazeTarget, InitiativeReason,
    Intention, LanguageExposure, LanguageStage, MemoryId, MemoryKind, NamingTarget, NonverbalAct,
    NormalizedPosition, NormalizedVelocity, RandomDomain, RandomSource, Reaction,
    SemanticDestination, SocialAct, SteeringMode, ToyId, WorldObject, WorldState,
    deterministic_unit,
};
use serde::{Deserialize, Serialize};

pub const SIMULATION_TICK_MS: u64 = 1_000;
pub const MAX_OFFLINE_MS: u64 = crate::ACTIVE_DAY_MS * 8;
pub const TALK_COOLDOWN_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PlayerEvent {
    Feed(FoodId),
    DropFood {
        food: FoodId,
        position: NormalizedPosition,
    },
    Cursor(Option<NormalizedPosition>),
    Name {
        target: NamingTarget,
        name: String,
    },
    Play(ToyId),
    Comfort,
    Tidy,
    ReturnedAfterAbsence,
    SpeechStarted,
    Talk,
    React(Reaction),
    LanguageExposure(LanguageExposure),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum GameEvent {
    NeedChanged,
    MemoryCreated(MemoryId),
    FoodConsumed(FoodId),
    FoodRejected(FoodId),
    ToyRejected(ToyId),
    Comforted,
    SleepStarted,
    SleepEnded,
    SocialActExpressed(SocialAct),
    TalkAccepted {
        contextual_follow_up: bool,
    },
    TalkIgnored,
    /// Language was heard and understood as an attempt to engage, but the creature declined.
    UtteranceRefused,
    /// Language was noticed without cancelling the creature's current embodied action.
    UtteranceDeferred,
    SpeechPerceived(SpeechAttention),
    LanguageExposureRegistered(LanguageExposure),
    NonverbalAct(NonverbalAct),
    ConceptLearned(Concept),
    LanguageAdvanced(LanguageStage),
    IntentionChanged {
        from: crate::Intention,
        to: crate::Intention,
    },
    FoodDropped {
        id: u64,
        food: FoodId,
        position: NormalizedPosition,
    },
    FoodDropRejected(FoodDropRejectionReason),
    FoodSettled(u64),
    FoodExpired(u64),
    ActionPhaseChanged {
        from: Option<ActionPhase>,
        to: ActionPhase,
    },
    InitiatedTalk(InitiativeReason),
    NonverbalRequest(InitiativeReason),
    NameAssigned {
        target: NamingTarget,
        name: String,
    },
}

/// The creature's immediate, word-independent response to speech in its environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechAttention {
    Ignored,
    Glanced,
    Attended,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineProgress {
    pub requested_ms: u64,
    pub applied_ms: u64,
    pub events: Vec<GameEvent>,
}

pub fn step(
    state: &mut WorldState,
    input: &[PlayerEvent],
    dt_ms: u64,
    rng: &mut impl RandomSource,
) -> Vec<GameEvent> {
    let mut events = Vec::new();
    for event in input {
        let before = state.creature.memories.len();
        apply_player_event(state, event, &mut events);
        events.extend(
            state.creature.memories[before..]
                .iter()
                .map(|memory| GameEvent::MemoryCreated(memory.id)),
        );
    }
    let accumulated = state.simulation_remainder_ms.saturating_add(dt_ms);
    let ticks = accumulated / SIMULATION_TICK_MS;
    state.simulation_remainder_ms = accumulated % SIMULATION_TICK_MS;
    for _ in 0..ticks {
        let before = state.creature.memories.len();
        fixed_tick(state, rng, &mut events);
        events.extend(
            state.creature.memories[before..]
                .iter()
                .map(|memory| GameEvent::MemoryCreated(memory.id)),
        );
    }
    events
}

pub fn advance_offline(
    state: &mut WorldState,
    requested_ms: u64,
    rng: &mut impl RandomSource,
) -> OfflineProgress {
    let applied_ms = requested_ms.min(MAX_OFFLINE_MS);
    if applied_ms == 0 {
        return OfflineProgress {
            requested_ms,
            applied_ms,
            events: Vec::new(),
        };
    }
    state.aquarium.player_present = false;
    state.absence_days = state
        .absence_days
        .saturating_add((applied_ms / crate::ACTIVE_DAY_MS) as u32)
        .min(8);
    let minutes = applied_ms as f32 / 60_000.0;
    state.creature.needs.hunger = (state.creature.needs.hunger + 0.025 * minutes).clamp(0.0, 1.0);
    state.creature.needs.energy = (state.creature.needs.energy + 0.03 * minutes).clamp(0.2, 1.0);
    state.creature.needs.comfort = (state.creature.needs.comfort - 0.004 * minutes).clamp(0.2, 1.0);
    let mut expired = Vec::new();
    for (id, object) in &mut state.aquarium.objects {
        if let WorldObject::Food(food) = object {
            food.age_ms = food.age_ms.saturating_add(applied_ms);
            if food.age_ms >= food.lifetime_ms {
                expired.push(*id);
            }
        }
    }
    for id in expired {
        state.aquarium.objects.remove(&id);
        state.aquarium.object_names.remove(&id);
    }
    let mut events = vec![GameEvent::NeedChanged];
    events.extend(step(state, &[PlayerEvent::ReturnedAfterAbsence], 0, rng));
    OfflineProgress {
        requested_ms,
        applied_ms,
        events,
    }
}

fn fixed_tick(state: &mut WorldState, _rng: &mut impl RandomSource, events: &mut Vec<GameEvent>) {
    state.elapsed_ms = state.elapsed_ms.saturating_add(SIMULATION_TICK_MS);
    let minutes = SIMULATION_TICK_MS as f32 / 60_000.0;
    state.creature.needs.hunger += 0.025 * minutes;
    state.creature.needs.energy -= 0.018 * minutes;
    state.creature.needs.comfort -= 0.008 * minutes;
    state.creature.needs.curiosity += 0.012 * minutes;
    advance_aquarium(state, events);
    maybe_initiate(state, events);
    state.creature.needs.clamp();
    state.creature.relationship.clamp();
    state.creature.social_habits.clamp();
    events.push(GameEvent::NeedChanged);
    update_development(state, events);
    if state.creature.needs.energy < 0.1
        && state.creature.current_intention != crate::Intention::Sleep
    {
        let previous = state.creature.current_intention;
        state.creature.current_intention = crate::Intention::Sleep;
        events.push(GameEvent::SleepStarted);
        events.push(GameEvent::IntentionChanged {
            from: previous,
            to: crate::Intention::Sleep,
        });
    }
}

fn apply_player_event(state: &mut WorldState, event: &PlayerEvent, events: &mut Vec<GameEvent>) {
    match event {
        PlayerEvent::DropFood { food, position } => {
            if drop_food(state, *food, *position, events) {
                state.creature.development.interactions.feeds = state
                    .creature
                    .development
                    .interactions
                    .feeds
                    .saturating_add(1);
            }
        }
        PlayerEvent::Cursor(position) => {
            state.aquarium.cursor = position.map(NormalizedPosition::clamped);
            state.creature.aquarium.gaze = if position.is_some() {
                GazeTarget::Cursor
            } else {
                GazeTarget::None
            };
            if let Some(cursor) = state.aquarium.cursor {
                if state.creature.relationship.resentment > 0.65 {
                    state.creature.aquarium.steering = SteeringMode::Flee;
                    state.creature.aquarium.destination = None;
                } else if state.creature.relationship.trust > 0.5
                    && state.creature.traits.sociability > 0.4
                {
                    state.creature.aquarium.steering = SteeringMode::Approach;
                    state.creature.aquarium.destination =
                        Some(SemanticDestination::Position(cursor));
                } else {
                    state.creature.aquarium.steering = SteeringMode::Hover;
                }
            } else {
                state.creature.aquarium.destination = None;
            }
        }
        PlayerEvent::Name { target, name } => assign_name(state, *target, name, events),
        PlayerEvent::Feed(food) => {
            let _ = drop_food(state, *food, NormalizedPosition::new(5_000, 3_000), events);
        }
        PlayerEvent::Play(toy) => {
            state.creature.development.interactions.plays = state
                .creature
                .development
                .interactions
                .plays
                .saturating_add(1);
            play_with_toy(state, *toy, events);
        }
        PlayerEvent::Comfort => {
            state.creature.needs.comfort += 0.2;
            state.creature.development.interactions.comforts = state
                .creature
                .development
                .interactions
                .comforts
                .saturating_add(1);
            events.push(GameEvent::Comforted);
        }
        PlayerEvent::Tidy => tidy_aquarium(state, events),
        PlayerEvent::ReturnedAfterAbsence => {
            state.aquarium.player_present = true;
            state.creature.development.interactions.returns = state
                .creature
                .development
                .interactions
                .returns
                .saturating_add(1);
            let memory = state.remember(
                MemoryKind::PlayerReturnedAfterAbsence,
                &[Concept::You, Concept::Again],
                state.creature.relationship.bond,
                0.8,
            );
            state.revise_belief(BeliefKind::PlayerReturnsAfterSleep, memory, true);
            set_intention(state, Intention::ApproachPlayer, events);
            state.creature.aquarium.gaze = GazeTarget::Player;
            state.creature.aquarium.destination = Some(SemanticDestination::Player);
            state.creature.aquarium.steering = SteeringMode::Approach;
            events.push(GameEvent::NonverbalAct(NonverbalAct::LeanAgainstPlayer));
        }
        PlayerEvent::SpeechStarted => {
            let attention = speech_attention(state);
            if !matches!(attention, SpeechAttention::Ignored) {
                // Hearing may redirect the creature's eyes without replacing its intention,
                // destination, steering, or in-progress action.
                state.creature.aquarium.gaze = GazeTarget::Player;
            }
            events.push(GameEvent::SpeechPerceived(attention));
        }
        PlayerEvent::Talk => {
            let follow_up = state.creature.conversation.contextual_follow_up_available;
            if state.elapsed_ms < state.creature.conversation.next_talk_at_ms && !follow_up {
                events.push(GameEvent::TalkIgnored);
                return;
            }
            state.creature.development.interactions.talks = state
                .creature
                .development
                .interactions
                .talks
                .saturating_add(1);
            state.creature.conversation.next_talk_at_ms =
                state.elapsed_ms.saturating_add(TALK_COOLDOWN_MS);
            state.creature.conversation.contextual_follow_up_available = false;
            state.creature.conversation.contextual_follow_up_used = follow_up;
            let act = choose_social_act(state);
            state.creature.last_social_act = Some(act);
            events.push(GameEvent::TalkAccepted {
                contextual_follow_up: follow_up,
            });
            events.push(GameEvent::SocialActExpressed(act));
        }
        PlayerEvent::React(reaction) => apply_reaction(state, *reaction),
        PlayerEvent::LanguageExposure(exposure) => {
            match exposure {
                LanguageExposure::Profanity => state.creature.social_habits.profanity += 0.08,
                LanguageExposure::Crudeness => state.creature.social_habits.crudeness += 0.07,
                LanguageExposure::Innuendo => state.creature.social_habits.sexual_innuendo += 0.06,
            }
            state.creature.social_habits.clamp();
            events.push(GameEvent::LanguageExposureRegistered(*exposure));
        }
    }
}

#[must_use]
pub fn speech_attention(state: &WorldState) -> SpeechAttention {
    let creature = &state.creature;
    if matches!(creature.current_intention, Intention::Sleep) || creature.needs.energy < 0.12 {
        return SpeechAttention::Ignored;
    }

    // Resentment changes willingness, not hearing. The session layer turns this attended
    // perception into a legible refusal after it has creature-bounded words.
    if state.mood() == crate::Mood::Resentful {
        return SpeechAttention::Attended;
    }

    let occupied =
        creature.aquarium.action.is_some() || matches!(creature.current_intention, Intention::Play);
    if occupied {
        SpeechAttention::Glanced
    } else {
        // An unoccupied creature attends instead of randomly dropping player language.
        SpeechAttention::Attended
    }
}

fn drop_food(
    state: &mut WorldState,
    food: FoodId,
    position: NormalizedPosition,
    events: &mut Vec<GameEvent>,
) -> bool {
    let count = state.aquarium.objects.values().filter(|object| matches!(object, WorldObject::Food(food) if !matches!(food.disposition, FoodDisposition::Consumed))).count();
    if count >= usize::from(state.aquarium.max_food) {
        events.push(GameEvent::FoodDropRejected(
            FoodDropRejectionReason::AquariumFull,
        ));
        return false;
    }
    let id = state.aquarium.next_object_id;
    state.aquarium.next_object_id = id.saturating_add(1);
    let buoyancy = match food {
        FoodId::Berry => FoodBuoyancy::Sink,
        FoodId::Mushroom => FoodBuoyancy::Float,
        FoodId::Pellet => FoodBuoyancy::Drift,
    };
    let velocity = match buoyancy {
        FoodBuoyancy::Float => NormalizedVelocity { x: 0, y: -20 },
        FoodBuoyancy::Sink => NormalizedVelocity { x: 0, y: 35 },
        FoodBuoyancy::Drift => NormalizedVelocity { x: 8, y: 8 },
    };
    let position = position.clamped();
    state.aquarium.objects.insert(
        id,
        WorldObject::Food(FoodObject {
            id,
            food,
            position,
            velocity,
            buoyancy,
            disposition: FoodDisposition::Falling,
            age_ms: 0,
            lifetime_ms: 120_000,
        }),
    );
    state.creature.aquarium.gaze = GazeTarget::Food(id);
    state.creature.aquarium.steering = SteeringMode::Brake;
    state.creature.aquarium.action = Some(ActionTimeline {
        phase: ActionPhase::Notice,
        elapsed_ms: 0,
        phase_duration_ms: 1_000,
        destination: SemanticDestination::Food(id),
        food_id: Some(id),
    });
    events.push(GameEvent::FoodDropped { id, food, position });
    events.push(GameEvent::ActionPhaseChanged {
        from: None,
        to: ActionPhase::Notice,
    });
    true
}

fn assign_name(
    state: &mut WorldState,
    target: NamingTarget,
    name: &str,
    events: &mut Vec<GameEvent>,
) {
    let normalized = name.trim().chars().take(32).collect::<String>();
    if normalized.is_empty() {
        return;
    }
    if let NamingTarget::Creature = target {
        state.creature.name = normalized.clone();
    }
    if let NamingTarget::Object(id) = target
        && state.aquarium.objects.contains_key(&id)
    {
        state.aquarium.object_names.insert(id, normalized.clone());
    }
    events.push(GameEvent::NameAssigned {
        target,
        name: normalized,
    });
}

fn advance_aquarium(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    advance_creature_motion(state);
    let mut settled = Vec::new();
    let mut expired = Vec::new();
    for (id, object) in &mut state.aquarium.objects {
        let WorldObject::Food(food) = object else {
            continue;
        };
        food.age_ms = food.age_ms.saturating_add(SIMULATION_TICK_MS);
        if matches!(
            food.disposition,
            FoodDisposition::Falling | FoodDisposition::Floating
        ) {
            food.position = NormalizedPosition::new(
                food.position.x.saturating_add(food.velocity.x),
                food.position.y.saturating_add(food.velocity.y),
            )
            .clamped();
            if food.position.y >= 9_500 && food.buoyancy != FoodBuoyancy::Float {
                food.position.y = 9_500;
                food.velocity = NormalizedVelocity::default();
                food.disposition = FoodDisposition::Settled;
                settled.push(*id);
            }
            if food.buoyancy == FoodBuoyancy::Float && food.position.y <= 500 {
                food.position.y = 500;
                food.velocity = NormalizedVelocity::default();
                food.disposition = FoodDisposition::Floating;
            }
        }
        if food.age_ms >= food.lifetime_ms {
            expired.push(*id);
        }
    }
    for id in settled {
        state.record_favorite(SemanticDestination::Bottom);
        events.push(GameEvent::FoodSettled(id));
    }
    for id in expired {
        state.aquarium.objects.remove(&id);
        state.aquarium.object_names.remove(&id);
        events.push(GameEvent::FoodExpired(id));
    }
    let Some(mut timeline) = state.creature.aquarium.action else {
        choose_idle_behavior(state, events);
        return;
    };
    timeline.elapsed_ms = timeline.elapsed_ms.saturating_add(SIMULATION_TICK_MS);
    if timeline.elapsed_ms < timeline.phase_duration_ms
        || (timeline.phase == ActionPhase::Approach
            && !food_arrived_and_braked(state, timeline.food_id))
    {
        state.creature.aquarium.action = Some(timeline);
        return;
    }
    let old = timeline.phase;
    if old == ActionPhase::Act {
        resolve_food(state, timeline.food_id, events);
    }
    let next = match old {
        ActionPhase::Notice => ActionPhase::Brake,
        ActionPhase::Brake => ActionPhase::Gaze,
        ActionPhase::Gaze => ActionPhase::Turn,
        ActionPhase::Turn => ActionPhase::Approach,
        ActionPhase::Approach => ActionPhase::Inspect,
        ActionPhase::Inspect => ActionPhase::Act,
        ActionPhase::Act => ActionPhase::Recover,
        ActionPhase::Recover => {
            state.creature.aquarium.action = None;
            state.creature.aquarium.steering = SteeringMode::Hover;
            return;
        }
    };
    timeline.phase = next;
    timeline.elapsed_ms = 0;
    state.creature.aquarium.action = Some(timeline);
    state.creature.aquarium.steering = match next {
        ActionPhase::Notice | ActionPhase::Brake => SteeringMode::Brake,
        ActionPhase::Gaze => SteeringMode::Hover,
        ActionPhase::Turn => SteeringMode::Turn,
        ActionPhase::Approach => SteeringMode::Approach,
        ActionPhase::Inspect => SteeringMode::Inspect,
        ActionPhase::Act | ActionPhase::Recover => SteeringMode::Settle,
    };
    events.push(GameEvent::ActionPhaseChanged {
        from: Some(old),
        to: next,
    });
}

fn advance_creature_motion(state: &mut WorldState) {
    let action_target =
        state
            .creature
            .aquarium
            .action
            .and_then(|action| match action.destination {
                SemanticDestination::Food(id) if action.phase == ActionPhase::Approach => state
                    .aquarium
                    .objects
                    .get(&id)
                    .and_then(|object| match object {
                        WorldObject::Food(food) => Some(food.position),
                        _ => None,
                    }),
                _ => None,
            });
    let target = action_target.or_else(|| steering_target(state));
    if let Some(target) = target {
        steer_toward(state, target);
    } else if state.creature.aquarium.action.is_none() {
        drift_with_cause(state);
    }
}

const ARRIVAL_DISTANCE: i32 = 220;
const APPROACH_SPEED: i32 = 650;
const FLEE_DISTANCE: i32 = 2_000;

fn food_arrived_and_braked(state: &WorldState, food_id: Option<u64>) -> bool {
    let Some(id) = food_id else { return false };
    let Some(WorldObject::Food(food)) = state.aquarium.objects.get(&id) else {
        return false;
    };
    let position = state.creature.aquarium.position;
    let distance = manhattan_distance(position, food.position);
    distance <= ARRIVAL_DISTANCE
        && state.creature.aquarium.velocity.x.abs() <= ARRIVAL_DISTANCE
        && state.creature.aquarium.velocity.y.abs() <= ARRIVAL_DISTANCE
}

fn steering_target(state: &mut WorldState) -> Option<NormalizedPosition> {
    if state.creature.aquarium.action.is_some() {
        return None;
    }
    if state.creature.aquarium.steering == SteeringMode::Flee
        && let Some(cursor) = state.aquarium.cursor
    {
        let position = state.creature.aquarium.position;
        if manhattan_distance(position, cursor) >= FLEE_DISTANCE {
            state.creature.aquarium.steering = SteeringMode::Hover;
            state.creature.aquarium.destination = None;
            state.creature.aquarium.velocity = NormalizedVelocity::default();
            return None;
        }
        return Some(
            NormalizedPosition::new(
                position
                    .x
                    .saturating_add((position.x - cursor.x).signum() * APPROACH_SPEED * 2),
                position
                    .y
                    .saturating_add((position.y - cursor.y).signum() * APPROACH_SPEED),
            )
            .clamped(),
        );
    }
    let destination = state.creature.aquarium.destination?;
    let target = destination_position(state, destination)?;
    if manhattan_distance(state.creature.aquarium.position, target) <= ARRIVAL_DISTANCE {
        state.creature.aquarium.velocity = NormalizedVelocity::default();
        if !matches!(
            destination,
            SemanticDestination::Player | SemanticDestination::Position(_)
        ) {
            state.record_favorite(destination);
        }
        state.creature.aquarium.destination = None;
        state.creature.aquarium.steering = SteeringMode::Hover;
        return None;
    }
    Some(target)
}

fn destination_position(
    state: &WorldState,
    destination: SemanticDestination,
) -> Option<NormalizedPosition> {
    match destination {
        SemanticDestination::Position(position) => Some(position),
        SemanticDestination::Player => Some(
            state
                .aquarium
                .cursor
                .unwrap_or(NormalizedPosition::new(5_000, 3_000)),
        ),
        SemanticDestination::Bottom => Some(NormalizedPosition::new(5_000, 9_200)),
        SemanticDestination::Food(id) => {
            state
                .aquarium
                .objects
                .get(&id)
                .and_then(|object| match object {
                    WorldObject::Food(food) => Some(food.position),
                    _ => None,
                })
        }
        SemanticDestination::Toy(toy) => {
            state
                .aquarium
                .objects
                .values()
                .find_map(|object| match object {
                    WorldObject::Toy {
                        toy: candidate,
                        position,
                    } if *candidate == toy => Some(*position),
                    _ => None,
                })
        }
        SemanticDestination::Cave | SemanticDestination::Plant => state
            .aquarium
            .objects
            .values()
            .find_map(|object| match (destination, object) {
                (SemanticDestination::Cave, WorldObject::Cave { position })
                | (SemanticDestination::Plant, WorldObject::Plant { position }) => Some(*position),
                _ => None,
            }),
    }
}

fn steer_toward(state: &mut WorldState, target: NormalizedPosition) {
    let position = state.creature.aquarium.position;
    let dx = target.x - position.x;
    let dy = target.y - position.y;
    let speed = if state.creature.aquarium.steering == SteeringMode::Flee {
        APPROACH_SPEED.saturating_mul(2)
    } else {
        APPROACH_SPEED
    };
    let movement = NormalizedVelocity {
        x: dx.clamp(-speed, speed),
        y: dy.clamp(-speed, speed),
    };
    state.creature.aquarium.position = NormalizedPosition::new(
        position.x.saturating_add(movement.x),
        position.y.saturating_add(movement.y),
    )
    .clamped();
    state.creature.aquarium.velocity = movement;
    if movement.x != 0 {
        state.creature.aquarium.facing = if movement.x < 0 {
            crate::Facing::Left
        } else {
            crate::Facing::Right
        };
    }
}

fn drift_with_cause(state: &mut WorldState) {
    let key = state.elapsed_ms / SIMULATION_TICK_MS;
    let vigor = if state.creature.needs.energy < 0.25 {
        20
    } else {
        80
    };
    let curiosity = if state.creature.needs.curiosity > 0.7 {
        90
    } else {
        35
    };
    let trait_bias = (state.creature.traits.boldness * 45.0) as i32;
    let dx = ((deterministic_unit(state.seed, RandomDomain::Motion, key) * vigor as f32) as i32
        - vigor / 2)
        + trait_bias / 4;
    let dy = (deterministic_unit(state.seed, RandomDomain::Motion, key + 1) * curiosity as f32)
        as i32
        - curiosity / 2;
    let position = state.creature.aquarium.position;
    state.creature.aquarium.position =
        NormalizedPosition::new(position.x.saturating_add(dx), position.y.saturating_add(dy))
            .clamped();
    state.creature.aquarium.velocity = NormalizedVelocity { x: dx, y: dy }.clamped();
}

fn choose_idle_behavior(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    if state.creature.aquarium.action.is_some()
        || matches!(state.creature.aquarium.steering, SteeringMode::Flee)
    {
        return;
    }
    if state.aquarium.cursor.is_some()
        && state.creature.relationship.trust > 0.5
        && state.creature.traits.sociability > 0.4
    {
        set_intention(state, Intention::ApproachPlayer, events);
        return;
    }
    if state.creature.aquarium.destination.is_some() {
        return;
    }
    let hour = ((state.elapsed_ms / 3_600_000) % 24) as u8;
    let routine = state
        .creature
        .routines
        .iter()
        .filter(|routine| routine.hour_start == hour)
        .max_by_key(|routine| routine.strength)
        .map(|routine| routine.destination);
    let favorite = state.favorite_destination();
    let destination = if state.creature.needs.energy < 0.2 {
        Some(SemanticDestination::Cave)
    } else if state.creature.needs.hunger > 0.78 {
        Some(SemanticDestination::Bottom)
    } else if let Some(routine) = routine {
        Some(routine)
    } else if let Some(favorite) = favorite.filter(|_| {
        state.creature.current_intention == Intention::Idle
            && (state.elapsed_ms / SIMULATION_TICK_MS).is_multiple_of(8)
    }) {
        Some(favorite)
    } else if state.creature.needs.curiosity > 0.65 {
        Some(if state.creature.traits.fussiness > 0.6 {
            SemanticDestination::Plant
        } else {
            SemanticDestination::Toy(preferred_toy(state))
        })
    } else {
        Some(if state.creature.traits.boldness > 0.55 {
            SemanticDestination::Plant
        } else {
            SemanticDestination::Cave
        })
    };
    if let Some(destination) = destination {
        state.creature.aquarium.destination = Some(destination);
        state.creature.aquarium.steering = SteeringMode::Approach;
        state.creature.aquarium.gaze = match destination {
            SemanticDestination::Toy(toy) => GazeTarget::Toy(toy),
            SemanticDestination::Cave => GazeTarget::Cave,
            SemanticDestination::Plant => GazeTarget::Plant,
            _ => GazeTarget::None,
        };
        let intention = match destination {
            SemanticDestination::Bottom => Intention::WaitAtBowl,
            SemanticDestination::Toy(_) => Intention::Play,
            _ => Intention::Idle,
        };
        set_intention(state, intention, events);
    }
}

fn play_with_toy(state: &mut WorldState, toy: ToyId, events: &mut Vec<GameEvent>) {
    if destination_position(state, SemanticDestination::Toy(toy)).is_none() {
        return;
    }
    let preference = *state
        .creature
        .toy_preferences
        .entry(toy)
        .or_insert_with(|| {
            deterministic_unit(state.seed, RandomDomain::Preferences, toy_key(toy))
                .mul_add(2.0, -1.0)
        });
    state.creature.aquarium.gaze = GazeTarget::Toy(toy);
    state.creature.aquarium.destination = Some(SemanticDestination::Toy(toy));
    state.creature.aquarium.steering = SteeringMode::Approach;
    if preference < -0.35 {
        set_intention(state, Intention::RefuseAndStare, events);
        let memory = state.remember(MemoryKind::DislikedToy { toy }, &[Concept::Bad], -0.5, 0.7);
        state.revise_belief(BeliefKind::ToyIsJealous, memory, true);
        events.push(GameEvent::ToyRejected(toy));
        events.push(GameEvent::NonverbalAct(NonverbalAct::TakeToyAway(toy)));
    } else {
        set_intention(state, Intention::Play, events);
        state.creature.needs.curiosity = (state.creature.needs.curiosity - 0.18).max(0.0);
        state.record_favorite(SemanticDestination::Toy(toy));
        state.remember(
            MemoryKind::PlayedWith { toy },
            &[Concept::Good],
            preference,
            0.6,
        );
    }
}

fn tidy_aquarium(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let removed = state
        .aquarium
        .objects
        .iter()
        .filter_map(|(id, object)| match object {
            WorldObject::Food(food)
                if matches!(
                    food.disposition,
                    FoodDisposition::Settled | FoodDisposition::Rejected
                ) =>
            {
                Some(*id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for id in &removed {
        state.aquarium.objects.remove(id);
        state.aquarium.object_names.remove(id);
    }
    if !removed.is_empty() {
        state.creature.needs.comfort = (state.creature.needs.comfort + 0.04).min(1.0);
        if state.creature.relationship.resentment > 0.7 {
            set_intention(state, Intention::UndoTidy, events);
            events.push(GameEvent::NonverbalAct(NonverbalAct::UndoTidy));
        } else {
            set_intention(state, Intention::Idle, events);
        }
    }
}

fn preferred_toy(state: &WorldState) -> ToyId {
    [ToyId::Ball, ToyId::Bell, ToyId::Sock]
        .into_iter()
        .max_by(|left, right| {
            state
                .creature
                .toy_preferences
                .get(left)
                .unwrap_or(&0.0)
                .total_cmp(state.creature.toy_preferences.get(right).unwrap_or(&0.0))
        })
        .unwrap_or(ToyId::Ball)
}

fn toy_key(toy: ToyId) -> u64 {
    match toy {
        ToyId::Ball => 1,
        ToyId::Bell => 2,
        ToyId::Sock => 3,
    }
}

fn set_intention(state: &mut WorldState, intention: Intention, events: &mut Vec<GameEvent>) {
    if state.creature.current_intention != intention {
        let from = state.creature.current_intention;
        state.creature.current_intention = intention;
        events.push(GameEvent::IntentionChanged {
            from,
            to: intention,
        });
    }
}

fn manhattan_distance(left: NormalizedPosition, right: NormalizedPosition) -> i32 {
    (left.x - right.x)
        .abs()
        .saturating_add((left.y - right.y).abs())
}

fn resolve_food(state: &mut WorldState, food_id: Option<u64>, events: &mut Vec<GameEvent>) {
    let Some(id) = food_id else { return };
    let Some(WorldObject::Food(object)) = state.aquarium.objects.get(&id).cloned() else {
        return;
    };
    let food = object.food;
    let seed = state.seed;
    let preference = *state.creature.preferences.entry(food).or_insert_with(|| {
        deterministic_unit(seed, RandomDomain::Preferences, id).mul_add(2.0, -1.0)
    });
    if preference < -0.35 {
        if let Some(WorldObject::Food(object)) = state.aquarium.objects.get_mut(&id) {
            object.disposition = FoodDisposition::Rejected;
        }
        let memory = state.remember(
            MemoryKind::RejectedFood { food },
            &[Concept::Food, Concept::Bad, Concept::Again],
            -0.75,
            0.85,
        );
        state.revise_belief(BeliefKind::RedFoodIsATrick, memory, true);
        events.push(GameEvent::FoodRejected(food));
        events.push(GameEvent::NonverbalAct(NonverbalAct::PushFoodAway(food)));
    } else {
        state.aquarium.objects.remove(&id);
        state.aquarium.object_names.remove(&id);
        state.creature.needs.hunger -= 0.45;
        let memory = state.remember(
            MemoryKind::WasFed { food },
            &[Concept::Food, Concept::You],
            preference,
            0.75,
        );
        state.revise_belief(BeliefKind::RedFoodIsATrick, memory, false);
        events.push(GameEvent::FoodConsumed(food));
    }
}

fn maybe_initiate(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    if state.creature.initiated_behavior.is_some() {
        return;
    }
    let candidate = if state.creature.needs.hunger > 0.82 {
        Some((InitiativeReason::Hunger, None))
    } else if state.aquarium.player_present
        && state.creature.needs.comfort < 0.2
        && state.creature.traits.sociability > 0.45
    {
        Some((
            InitiativeReason::Loneliness,
            Some(NonverbalAct::LeanAgainstPlayer),
        ))
    } else {
        None
    };
    let Some((reason, nonverbal)) = candidate else {
        return;
    };
    state.creature.initiated_behavior = Some(crate::InitiatedBehavior {
        reason,
        nonverbal,
        requested_at_ms: state.elapsed_ms,
    });
    if nonverbal.is_some() {
        events.push(GameEvent::NonverbalRequest(reason));
    } else {
        events.push(GameEvent::InitiatedTalk(reason));
    }
}

fn apply_reaction(state: &mut WorldState, reaction: Reaction) {
    let Some(act) = state.creature.last_social_act.take() else {
        return;
    };
    if !state.creature.conversation.contextual_follow_up_used {
        state.creature.conversation.contextual_follow_up_available = true;
    }
    match reaction {
        Reaction::Laugh => {
            reinforce_act(state, act, 0.12);
            state.creature.social_habits.provocation += 0.1;
        }
        Reaction::Disapprove => reinforce_act(state, act, -0.08),
        Reaction::Comfort => state.creature.relationship.bond += 0.04,
    }
    state.remember(
        MemoryKind::PlayerReacted { reaction, to: act },
        &[Concept::You, Concept::Again],
        0.3,
        0.75,
    );
}

fn reinforce_act(state: &mut WorldState, act: SocialAct, delta: f32) {
    match act {
        SocialAct::Profanity => state.creature.social_habits.profanity += delta,
        SocialAct::Crudeness => state.creature.social_habits.crudeness += delta,
        SocialAct::Insult => state.creature.social_habits.spite += delta,
        SocialAct::Provocation => state.creature.social_habits.provocation += delta,
        SocialAct::Innuendo => state.creature.social_habits.sexual_innuendo += delta,
        SocialAct::Neutral => {}
    }
}
fn choose_social_act(state: &WorldState) -> SocialAct {
    let h = state.creature.social_habits;
    if h.provocation >= 0.1 {
        SocialAct::Provocation
    } else if state.creature.relationship.resentment + h.spite >= 0.1 {
        SocialAct::Insult
    } else if h.profanity >= 0.25 {
        SocialAct::Profanity
    } else if h.crudeness >= 0.25 {
        SocialAct::Crudeness
    } else if h.sexual_innuendo >= 0.25 {
        SocialAct::Innuendo
    } else {
        SocialAct::Neutral
    }
}
fn update_development(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let days = state.active_day() as u32;
    state.creature.development.active_days_reached =
        state.creature.development.active_days_reached.max(days);
    if days >= 4 {
        state
            .creature
            .development
            .milestones
            .insert(DevelopmentMilestone::SettledRoutine);
    }
    if !state.creature.favorite_locations.is_empty() {
        state
            .creature
            .development
            .milestones
            .insert(DevelopmentMilestone::FavoriteFound);
    }
    let interactions = state.creature.development.interactions;
    if days >= 2 && interactions.total() > 0 {
        learn(state, Concept::Again, events);
        advance_language(state, LanguageStage::Words, events);
    }
    if days >= 3 && interactions.total() >= 3 {
        learn(state, Concept::Yesterday, events);
        advance_language(state, LanguageStage::Phrases, events);
    }
}
fn learn(state: &mut WorldState, concept: Concept, events: &mut Vec<GameEvent>) {
    if state.creature.known_concepts.insert(concept) {
        events.push(GameEvent::ConceptLearned(concept));
    }
}
fn advance_language(state: &mut WorldState, stage: LanguageStage, events: &mut Vec<GameEvent>) {
    if stage > state.creature.development.language_stage {
        state.creature.development.language_stage = stage;
        events.push(GameEvent::LanguageAdvanced(stage));
    }
}
