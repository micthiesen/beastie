use crate::{
    ActionPhase, ActionTimeline, ActivityInterruptionOwner, ActivityPhase, ActivityPurpose,
    ActivityRecipe, ActivitySelectionEvidence, ActivitySubject, BeliefKind, Concept,
    DevelopmentMilestone, FoodBuoyancy, FoodDisposition, FoodDropRejectionReason, FoodId,
    FoodObject, FoodOutcome, GazeTarget, InitiativeReason, Intention, LanguageExposure,
    LanguageStage, MemoryId, MemoryKind, NamingTarget, NonverbalAct, NormalizedPosition,
    NormalizedVelocity, PrivateLifeActivity, PrivateLifeKind, RandomDomain, RandomSource, Reaction,
    RecentActivity, RelationshipSubject, RelationshipTrigger, SemanticDestination, SocialAct,
    SteeringMode, ToyId, ToyResponse, UtteranceInterpretation, UtteranceReference, WorldObject,
    WorldState, deterministic_unit,
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

/// Ten fixed ticks per second keep acknowledgement, steering and arrival responsive. Velocities
/// remain fixed-point units per second; `tick_step` converts them without rounding drift.
pub const SIMULATION_TICK_MS: u64 = 100;
pub const MAX_OFFLINE_MS: u64 = crate::ACTIVE_DAY_MS * 8;
pub const TALK_COOLDOWN_MS: u64 = 30_000;
pub const SOCK_RELEASE_SPEED: i32 = 900;
// Private life needs room to read as lived time, not a showcase playlist. Arrival begins a
// state-shaped quiet span that outlasts the authored payoff and leaves genuine observation
// between bouts.
const IDLE_BOUT_MIN_MS: u64 = 1_500;
const IDLE_BOUT_MAX_MS: u64 = 4_500;
const AFFECTION_DURATION_MS: u64 = 7_000;
/// How long a direct toy contact holds before recovery ends: long enough to see a sock tug.
const TOY_RECOVERY_MS: u64 = 1_200;
const MAX_SLEEP_MS: u64 = 30_000;
const INITIATIVE_DURATION_MS: u64 = 45_000;
const FIRST_MEETING_PEEK_MS: u64 = 2_500;
const FIRST_MEETING_LINGER_MS: u64 = 6_000;
const ASK_WITH_WORD_INTERVAL_MS: u64 = 20_000;
const REMARK_INTERVAL_MS: u64 = 18_000;
const ASK_WITHOUT_WORD_INTERVAL_MS: u64 = 45_000;
const ACTIVE_DAY_HOURS: u64 = 24;
const MAX_VISIT_EVIDENCE: usize = 32;
const DIRECT_RELATIONSHIP_MOMENT_MS: u64 = 3_000;
const MAX_RECENT_ACTIVITIES: usize = 32;
const PRIVATE_NOTICE_MS: u64 = 500;
const PRIVATE_ACT_MS: u64 = 1_600;
const BUBBLE_SNAP_MS: u64 = 600;
const PRIVATE_RECOVER_MS: u64 = 900;
const PRIVATE_SETTLE_MS: u64 = 3_500;

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
    UnderstoodUtterance(UtteranceInterpretation),
    /// Words addressed to the creature, perceived at once and used for learning.
    Utterance(String),
    /// A tap on the glass at a point in the water.
    Tap(NormalizedPosition),
    /// The player has arrived at the aquarium (left the title screen).
    Arrived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum GameEvent {
    NeedChanged,
    MemoryCreated(MemoryId),
    FoodConsumed(FoodId),
    FoodRejected(FoodId),
    ToyPlayAccepted {
        toy: ToyId,
        interaction_id: NonZeroU64,
        origin: crate::ToyOrigin,
    },
    ToyRejected {
        toy: ToyId,
        interaction_id: NonZeroU64,
        origin: crate::ToyOrigin,
    },
    ToyContacted {
        toy: ToyId,
        interaction_id: NonZeroU64,
        origin: crate::ToyOrigin,
    },
    ToyPlayed {
        toy: ToyId,
        interaction_id: NonZeroU64,
        origin: crate::ToyOrigin,
    },
    ToyInteractionInterrupted {
        toy: ToyId,
        interaction_id: NonZeroU64,
        origin: crate::ToyOrigin,
    },
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
    RelationshipBeatStarted {
        motif: crate::RelationshipMotifKey,
        expression: crate::RelationshipExpressionKind,
        trigger: RelationshipTrigger,
        subject: RelationshipSubject,
        evidence: Vec<crate::RelationshipEvidence>,
    },
    RelationshipBeatPhaseChanged {
        motif: crate::RelationshipMotifKey,
        from: Option<crate::RelationshipBeatPhase>,
        to: crate::RelationshipBeatPhase,
    },
    RelationshipBeatCompleted(crate::RelationshipMotifKey),
    RelationshipBeatInterrupted(crate::RelationshipMotifKey),
    ActionRelationshipStarted {
        action_id: u64,
        motif: crate::RelationshipMotifKey,
        expression: crate::RelationshipExpressionKind,
        subject: RelationshipSubject,
        evidence: Vec<crate::RelationshipEvidence>,
    },
    ActionRelationshipResolved {
        action_id: u64,
        motif: crate::RelationshipMotifKey,
        subject: RelationshipSubject,
        outcome: FoodOutcome,
    },
    ActionRelationshipCompleted {
        action_id: u64,
        motif: crate::RelationshipMotifKey,
        subject: RelationshipSubject,
    },
    ActionRelationshipInterrupted {
        action_id: u64,
        motif: crate::RelationshipMotifKey,
        subject: RelationshipSubject,
    },
    ActionAborted {
        destination: SemanticDestination,
    },
    PrivateLifeStarted {
        activity_id: NonZeroU64,
        kind: PrivateLifeKind,
        recipe: ActivityRecipe,
    },
    PrivateLifePhaseChanged {
        activity_id: NonZeroU64,
        from: ActivityPhase,
        to: ActivityPhase,
    },
    PrivateLifeCompleted {
        activity_id: NonZeroU64,
        kind: PrivateLifeKind,
        recipe: ActivityRecipe,
    },
    PrivateLifeInterrupted {
        activity_id: NonZeroU64,
        phase: ActivityPhase,
        by: ActivityInterruptionOwner,
    },
    ToyObjectResponded {
        toy: ToyId,
        activity_id: NonZeroU64,
        response: ToyResponse,
    },
    ToyInteractionResponded {
        toy: ToyId,
        interaction_id: NonZeroU64,
        response: ToyResponse,
    },
    /// A new creature came out of its cave to meet the player.
    Emerged,
    /// The creature named something it is enjoying, in a word the player taught it.
    Remarked(crate::Meaning),
    /// A tap on the glass, and whether the creature came to look.
    TapNoticed {
        position: NormalizedPosition,
        approached: bool,
    },
    /// Words were heard without any known word to act on. `echo` is a curious repeat attempt.
    WordHeard {
        word: Option<String>,
        echo: Option<String>,
    },
    /// The creature learned what a word means from the situations it was heard in.
    WordLearned {
        word: String,
        meaning: crate::Meaning,
    },
    /// A known word was understood and answered.
    Understood {
        word: String,
        meaning: crate::Meaning,
        response: crate::RequestResponse,
    },
}

/// Exact simulation-owned body that a deferred utterance must wait for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum DialogueActionOwner {
    Food(NonZeroU64),
    Toy(NonZeroU64),
    Refusal(NonZeroU64),
    PrivateLife(NonZeroU64),
    Relationship(NonZeroU64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogueHandoffState {
    Ready,
    WaitingForContact,
    WaitingForRecovery,
    SafeBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueHandoff {
    pub owner: Option<DialogueActionOwner>,
    pub state: DialogueHandoffState,
}

/// Return the only authoritative dialogue handoff boundary.  Callers bind a deferred utterance
/// to `owner`; a new owner means the original action ended, not that its replacement is ready.
/// New direct interactions must reach their own safe boundary before deferred language may
/// interrupt them. Unrelated autonomous activity must not indefinitely capture waiting words.
#[must_use]
pub fn dialogue_handoff(state: &WorldState) -> DialogueHandoff {
    if let Some(action) = state.creature.aquarium.action.as_ref() {
        let owner = NonZeroU64::new(action.action_id).map(DialogueActionOwner::Food);
        return DialogueHandoff {
            owner,
            state: match action.phase {
                ActionPhase::Act => DialogueHandoffState::WaitingForRecovery,
                ActionPhase::Recover => DialogueHandoffState::SafeBoundary,
                _ => DialogueHandoffState::WaitingForContact,
            },
        };
    }
    if let Some(interaction) = state.creature.interaction_state.toy_interaction.as_ref() {
        let owner = match interaction.outcome {
            crate::ToyInteractionOutcome::Rejected => DialogueActionOwner::Refusal(interaction.id),
            _ => DialogueActionOwner::Toy(interaction.id),
        };
        return DialogueHandoff {
            owner: Some(owner),
            state: match interaction.phase {
                crate::ToyInteractionPhase::Approach => DialogueHandoffState::WaitingForContact,
                crate::ToyInteractionPhase::Contact | crate::ToyInteractionPhase::Resolved => {
                    DialogueHandoffState::WaitingForRecovery
                }
                crate::ToyInteractionPhase::Recovery | crate::ToyInteractionPhase::Interrupted => {
                    DialogueHandoffState::SafeBoundary
                }
            },
        };
    }
    if let Some(activity) = state.creature.private_life.active.as_ref() {
        return DialogueHandoff {
            owner: Some(DialogueActionOwner::PrivateLife(activity.id)),
            state: match activity.phase {
                ActivityPhase::Notice | ActivityPhase::Approach => {
                    DialogueHandoffState::WaitingForContact
                }
                ActivityPhase::Act => DialogueHandoffState::WaitingForRecovery,
                ActivityPhase::Recover | ActivityPhase::Settle | ActivityPhase::Interrupted => {
                    DialogueHandoffState::SafeBoundary
                }
            },
        };
    }
    if let Some(beat) = state.creature.relationship_expression.active.as_ref() {
        return DialogueHandoff {
            owner: NonZeroU64::new(beat.started_at_ms.saturating_add(1))
                .map(DialogueActionOwner::Relationship),
            state: if beat.phase == crate::RelationshipBeatPhase::Recover {
                DialogueHandoffState::SafeBoundary
            } else {
                DialogueHandoffState::WaitingForRecovery
            },
        };
    }
    DialogueHandoff {
        owner: None,
        state: DialogueHandoffState::Ready,
    }
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
    // Normalize loaded stationary phases before input can replace their owner.
    clear_stationary_velocity(state);
    let mut events = Vec::new();
    for event in input {
        let before = state.creature.memories.len();
        if should_interrupt_for_player_event(event) {
            interrupt_for_player(state, &mut events);
        }
        let first = events.len();
        apply_player_event(state, event, &mut events);
        events.extend(
            state.creature.memories[before..]
                .iter()
                .map(|memory| GameEvent::MemoryCreated(memory.id)),
        );
        crate::teaching::mark_attention_from_events(state, &events[first..]);
    }
    // Input may have started a stationary phase without advancing a tick.
    clear_stationary_velocity(state);
    let accumulated = state.simulation_remainder_ms.saturating_add(dt_ms);
    let ticks = accumulated / SIMULATION_TICK_MS;
    state.simulation_remainder_ms = accumulated % SIMULATION_TICK_MS;
    for _ in 0..ticks {
        let before = state.creature.memories.len();
        let first = events.len();
        fixed_tick(state, rng, &mut events);
        events.extend(
            state.creature.memories[before..]
                .iter()
                .map(|memory| GameEvent::MemoryCreated(memory.id)),
        );
        crate::teaching::mark_attention_from_events(state, &events[first..]);
        maybe_remark(state, first, &mut events);
    }
    events
}

/// What any direct player request does first: the creature drops what it was doing for it.
pub(crate) fn interrupt_for_player(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    interrupt_private_life(state, ActivityInterruptionOwner::Player, events);
    interrupt_toy_interaction(state, events);
    interrupt_action_relationship_moment(state, events);
    interrupt_relationship_beat(state, events);
    interrupt_travel(state, events);
}

fn clear_stationary_velocity(state: &mut WorldState) {
    // Velocity describes current locomotion, not the last displacement before a stop.
    // Clear it even on input-only frames so entering a stationary action cannot leave
    // presentation extrapolating an old swim during every simulation interval.
    if state.creature.current_intention == Intention::Sleep
        || state
            .creature
            .aquarium
            .action
            .as_ref()
            .is_some_and(|action| action.phase != ActionPhase::Approach)
    {
        state.creature.aquarium.velocity = NormalizedVelocity::default();
    }
}

/// Apply a grounded semantic trigger without accepting arbitrary model-authored facts.
///
/// This is the session boundary for dialogue and other adapters that already have a typed,
/// creature-bounded interpretation. It returns only authoritative simulation events.
pub fn trigger_relationship_beat(
    state: &mut WorldState,
    trigger: RelationshipTrigger,
) -> Vec<GameEvent> {
    let mut events = Vec::new();
    interrupt_relationship_beat(state, &mut events);
    maybe_start_relationship_beat(state, trigger, &mut events);
    events
}

/// Convert a previously grounded utterance into a bounded relationship trigger.
pub fn apply_grounded_utterance(
    state: &mut WorldState,
    interpretation: &UtteranceInterpretation,
) -> Vec<GameEvent> {
    let subject = interpretation
        .references
        .iter()
        .next()
        .map(|reference| match reference {
            UtteranceReference::Toy(toy) => RelationshipSubject::Toy(*toy),
            UtteranceReference::Food(food) => RelationshipSubject::Food(*food),
            UtteranceReference::Creature | UtteranceReference::Player => {
                RelationshipSubject::Player
            }
        });
    trigger_relationship_beat(state, RelationshipTrigger::RelevantUtterance { subject })
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
    resolve_toy_interaction_offline(state);
    resolve_private_life_offline(state);
    state.elapsed_ms = state.elapsed_ms.saturating_add(applied_ms);
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
    let sock_was_carried = state
        .aquarium
        .toy_states
        .get(&ToyId::Sock)
        .is_some_and(|sock| sock.carried);
    advance_embodied_state(state, events);
    advance_private_life(state, events);
    advance_relationship_beat(state, events);
    let minutes = SIMULATION_TICK_MS as f32 / 60_000.0;
    // Needs move fast enough that wants surface within the first minutes of a visit, so there
    // is always something worth doing, and slow enough that caring is not a chore.
    state.creature.needs.hunger += 0.05 * minutes;
    state.creature.needs.energy -= 0.018 * minutes;
    state.creature.needs.comfort -= 0.03 * minutes;
    state.creature.needs.curiosity += 0.05 * minutes;
    if state.creature.needs.energy < 0.1
        && state.creature.current_intention != crate::Intention::Sleep
    {
        start_sleep(state, events);
    }
    let sock_just_released = sock_was_carried
        && state
            .aquarium
            .toy_states
            .get(&ToyId::Sock)
            .is_some_and(|sock| !sock.carried);
    advance_aquarium(state, sock_just_released, events);
    clear_satisfied_or_expired_initiative(state);
    maybe_initiate(state, events);
    state.creature.needs.clamp();
    state.creature.relationship.clamp();
    state.creature.social_habits.clamp();
    if state.aquarium.player_present && state.creature.needs.comfort < 0.35 {
        maybe_start_relationship_beat(state, RelationshipTrigger::ComfortNeeded, events);
    }
    events.push(GameEvent::NeedChanged);
    update_development(state, events);
    // Keep batched ticks identical to individual steps, including the first tick
    // after a stationary phase has ended.
    clear_stationary_velocity(state);
}

/// Fall asleep where the creature is, ending any toy or relationship moment first.
pub(crate) fn start_sleep(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    interrupt_toy_interaction(state, events);
    interrupt_relationship_beat(state, events);
    let previous = state.creature.current_intention;
    state.creature.current_intention = crate::Intention::Sleep;
    state.creature.interaction_state.sleep_started_at_ms = Some(state.elapsed_ms);
    events.push(GameEvent::SleepStarted);
    events.push(GameEvent::IntentionChanged {
        from: previous,
        to: crate::Intention::Sleep,
    });
}

fn advance_embodied_state(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    finish_toy_recovery(state);
    if state
        .creature
        .interaction_state
        .relationship_moment
        .as_ref()
        .is_some_and(|moment| state.elapsed_ms >= moment.expires_at_ms)
        && let Some(moment) = state.creature.interaction_state.relationship_moment.take()
    {
        events.push(GameEvent::ActionRelationshipCompleted {
            action_id: moment.action_id,
            motif: moment.context.motif,
            subject: moment.context.subject,
        });
    }
    if state.creature.current_intention == Intention::Sleep {
        let started_at = state
            .creature
            .interaction_state
            .sleep_started_at_ms
            .get_or_insert(state.elapsed_ms);
        let seconds = SIMULATION_TICK_MS as f32 / 1_000.0;
        state.creature.needs.energy = (state.creature.needs.energy + 0.08 * seconds).min(1.0);
        state.creature.needs.comfort = (state.creature.needs.comfort + 0.01 * seconds).min(1.0);
        if state.creature.needs.energy >= 0.68
            || state.elapsed_ms.saturating_sub(*started_at) >= MAX_SLEEP_MS
        {
            state.creature.interaction_state.sleep_started_at_ms = None;
            set_intention(state, Intention::Idle, events);
            clear_travel_target(state);
            state.creature.aquarium.steering = SteeringMode::Hover;
            events.push(GameEvent::SleepEnded);
        }
    }

    if state.creature.current_intention == Intention::ShowAffection
        && state.elapsed_ms >= state.creature.interaction_state.affectionate_until_ms
    {
        set_intention(state, Intention::Idle, events);
        // Intention is expression, not travel ownership. Comfort alone creates this
        // player-directed CursorSocial journey; a newer private/relationship/cursor
        // journey can already own locomotion while its old affection timer is running.
        if matches!(
            (
                state.creature.aquarium.destination,
                state.creature.aquarium.travel_purpose,
            ),
            (
                Some(SemanticDestination::Player),
                Some(crate::TravelPurpose::CursorSocial { .. })
            )
        ) {
            clear_travel_target(state);
            state.creature.aquarium.steering = SteeringMode::Hover;
        }
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
            // The pointer may catch an unoccupied creature's eye. It never replaces an owned
            // target's gaze, travel or action, so intent stays readable while the player moves.
            if creature_is_occupied(state) {
                return;
            }
            let near = state.aquarium.cursor.is_some_and(|cursor| {
                manhattan_distance(cursor, state.creature.aquarium.position)
                    <= CURSOR_NOTICE_DISTANCE
            });
            if near {
                state.creature.aquarium.gaze = GazeTarget::Cursor;
                if state.creature.relationship.resentment > 0.65
                    && state.creature.aquarium.destination.is_none()
                {
                    state.creature.aquarium.steering = SteeringMode::Flee;
                }
            } else if state.creature.aquarium.gaze == GazeTarget::Cursor {
                state.creature.aquarium.gaze = GazeTarget::None;
            }
        }
        PlayerEvent::Name { target, name } => assign_name(state, *target, name, events),
        PlayerEvent::Feed(food) => {
            let _ = drop_food(state, *food, NormalizedPosition::new(5_000, 3_000), events);
        }
        PlayerEvent::Play(toy) => {
            let relationship = crate::select_action_relationship_context(
                state,
                RelationshipTrigger::ToyEngaged { toy: *toy },
            );
            state.creature.development.interactions.plays = state
                .creature
                .development
                .interactions
                .plays
                .saturating_add(1);
            play_with_toy(state, *toy, relationship, events);
        }
        PlayerEvent::Comfort => {
            let relationship = crate::select_action_relationship_context(
                state,
                RelationshipTrigger::ComfortCompleted,
            );
            state.creature.needs.comfort = (state.creature.needs.comfort + 0.18).min(1.0);
            improve_relationship(state, 0.015, 0.01, 0.035);
            state.creature.development.interactions.comforts = state
                .creature
                .development
                .interactions
                .comforts
                .saturating_add(1);
            state.remember(
                MemoryKind::WasComforted,
                &[Concept::You, Concept::Good],
                0.55,
                0.7,
            );
            state.creature.interaction_state.affectionate_until_ms =
                state.elapsed_ms.saturating_add(AFFECTION_DURATION_MS);
            set_intention(state, Intention::ShowAffection, events);
            state.creature.aquarium.gaze = GazeTarget::Player;
            let action_id =
                NonZeroU64::new(allocate_action_id(state)).expect("action IDs start at one");
            set_travel_target(
                state,
                SemanticDestination::Player,
                crate::TravelPurpose::CursorSocial { action_id },
                events,
            );
            clear_satisfied_or_expired_initiative(state);
            events.push(GameEvent::Comforted);
            if let Some(context) = relationship {
                start_action_relationship_moment(state, context, events);
            }
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
            state.creature.aquarium.gaze = GazeTarget::Player;
            maybe_start_relationship_beat(state, RelationshipTrigger::PlayerReturn, events);
        }
        PlayerEvent::SpeechStarted => {
            let attention = speech_attention(state);
            if !matches!(attention, SpeechAttention::Ignored) {
                interrupt_relationship_beat(state, events);
                // Hearing may redirect the creature's eyes without replacing its intention,
                // destination, steering, or in-progress action.
                state.creature.aquarium.gaze = GazeTarget::Player;
            }
            if attention == SpeechAttention::Attended {
                interrupt_toy_interaction(state, events);
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
        PlayerEvent::Utterance(text) => crate::teaching::hear_utterance(state, text, events),
        PlayerEvent::Tap(position) => tap_glass(state, position.clamped(), events),
        PlayerEvent::Arrived => {
            state.aquarium.player_present = true;
            if state.creature.hidden_until_met && state.creature.met_player_at_ms.is_none() {
                state.creature.met_player_at_ms = Some(state.elapsed_ms);
            }
        }
        PlayerEvent::LanguageExposure(exposure) => {
            match exposure {
                LanguageExposure::Profanity => state.creature.social_habits.profanity += 0.08,
                LanguageExposure::Crudeness => state.creature.social_habits.crudeness += 0.07,
                LanguageExposure::Innuendo => state.creature.social_habits.sexual_innuendo += 0.06,
            }
            state.creature.social_habits.clamp();
            events.push(GameEvent::LanguageExposureRegistered(*exposure));
        }
        PlayerEvent::UnderstoodUtterance(interpretation) => {
            interrupt_relationship_beat(state, events);
            let subject =
                interpretation
                    .references
                    .iter()
                    .next()
                    .map(|reference| match reference {
                        UtteranceReference::Toy(toy) => RelationshipSubject::Toy(*toy),
                        UtteranceReference::Food(food) => RelationshipSubject::Food(*food),
                        UtteranceReference::Creature | UtteranceReference::Player => {
                            RelationshipSubject::Player
                        }
                    });
            maybe_start_relationship_beat(
                state,
                RelationshipTrigger::RelevantUtterance { subject },
                events,
            );
        }
    }
}

/// Every tap gets an answer. An idle creature looks, and a curious one swims over to see; a
/// busy one only glances so its own intent stays readable.
fn tap_glass(state: &mut WorldState, position: NormalizedPosition, events: &mut Vec<GameEvent>) {
    let counters = &mut state.creature.development.interactions;
    counters.taps = counters.taps.saturating_add(1);
    let asleep = state.creature.current_intention == Intention::Sleep;
    let busy = creature_is_occupied(state);
    let approached = !asleep
        && !busy
        && !state.creature.hidden_until_met
        && state.creature.needs.curiosity + state.creature.traits.boldness * 0.3 > 0.35;
    if !asleep && !busy {
        state.aquarium.cursor = Some(position);
        state.creature.aquarium.gaze = GazeTarget::Cursor;
    }
    if approached {
        // Stop a little short, looking at the spot rather than nosing into it.
        let head = state.creature.aquarium.position;
        let toward = NormalizedPosition::new(
            position.x - (position.x - head.x).signum() * 900,
            position.y - (position.y - head.y).signum() * 400,
        )
        .clamped();
        let action_id =
            NonZeroU64::new(allocate_action_id(state)).expect("action IDs start at one");
        set_travel_target(
            state,
            SemanticDestination::Position(toward),
            crate::TravelPurpose::CursorSocial { action_id },
            events,
        );
        state.creature.needs.curiosity = (state.creature.needs.curiosity - 0.03).max(0.0);
    }
    events.push(GameEvent::TapNoticed {
        position,
        approached,
    });
}

/// Pointer distance, in fixed-point units, within which an idle creature looks at it.
const CURSOR_NOTICE_DISTANCE: i32 = 3_200;

/// Whether the creature currently owns a target that its gaze and body must keep showing.
pub(crate) fn creature_is_occupied(state: &WorldState) -> bool {
    let creature = &state.creature;
    creature.current_intention == Intention::Sleep
        || creature.aquarium.action.is_some()
        || creature.interaction_state.toy_interaction.is_some()
        || creature.relationship_expression.active.is_some()
        || creature
            .private_life
            .active
            .as_ref()
            .is_some_and(|activity| {
                matches!(
                    activity.phase,
                    ActivityPhase::Notice | ActivityPhase::Approach | ActivityPhase::Act
                )
            })
        || creature.aquarium.destination.is_some()
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

    // A chosen toy destination may project `Play` while the creature is merely travelling or
    // hovering. Authored play has an action timeline; the destination check also preserves old
    // saves whose active play predates that timeline.
    let travelling_to_toy = match (
        creature.aquarium.destination,
        creature.aquarium.travel_purpose,
    ) {
        (
            Some(SemanticDestination::Toy(_)),
            Some(crate::TravelPurpose::ToyInteraction { interaction_id }),
        ) => creature
            .interaction_state
            .toy_interaction
            .as_ref()
            .is_some_and(|interaction| {
                interaction.id == interaction_id
                    && interaction.origin == crate::ToyOrigin::Autonomous
            }),
        _ => false,
    };
    let settled_at_toy = matches!(
        creature.idle_life.last_arrived_destination,
        Some(SemanticDestination::Toy(_))
    ) && state.elapsed_ms < creature.idle_life.settled_until_ms;
    let autonomous_toy_interest = creature.aquarium.action.is_none()
        && (travelling_to_toy
            || (matches!(creature.current_intention, Intention::Play) && settled_at_toy));
    let occupied = creature.aquarium.action.is_some()
        || (matches!(creature.current_intention, Intention::Play) && !autonomous_toy_interest);
    if occupied {
        SpeechAttention::Glanced
    } else {
        // An unoccupied creature attends instead of randomly dropping player language.
        SpeechAttention::Attended
    }
}

fn should_interrupt_for_player_event(event: &PlayerEvent) -> bool {
    // Pointer motion is attention, never a command: it must not cancel accepted actions.
    !matches!(
        event,
        PlayerEvent::Cursor(_)
            | PlayerEvent::Arrived
            | PlayerEvent::Tap(_)
            | PlayerEvent::Utterance(_)
            | PlayerEvent::SpeechStarted
            | PlayerEvent::React(_)
            | PlayerEvent::LanguageExposure(_)
            | PlayerEvent::UnderstoodUtterance(_)
    )
}

fn maybe_start_relationship_beat(
    state: &mut WorldState,
    trigger: RelationshipTrigger,
    events: &mut Vec<GameEvent>,
) {
    if state.creature.current_intention == Intention::Sleep
        || state.creature.aquarium.action.is_some()
        || state.creature.interaction_state.toy_interaction.is_some()
        || state.creature.relationship_expression.active.is_some()
    {
        return;
    }
    let Some(beat) = crate::select_relationship_beat(state, trigger) else {
        return;
    };
    let motif = beat.motif;
    let expression = beat.expression_kind;
    let subject = beat
        .subject
        .expect("selected standalone beat has a subject");
    let evidence = beat.evidence.clone();
    state.creature.relationship_expression.active = Some(beat.clone());
    record_relationship_expression(state, motif, subject, &evidence, expression);
    apply_relationship_pose(state, &beat, events);
    events.push(GameEvent::RelationshipBeatStarted {
        motif,
        expression,
        trigger,
        subject,
        evidence,
    });
}

fn record_relationship_expression(
    state: &mut WorldState,
    motif: crate::RelationshipMotifKey,
    subject: crate::RelationshipSubject,
    evidence: &[crate::RelationshipEvidence],
    expression_kind: crate::RelationshipExpressionKind,
) {
    state.creature.relationship_expression.last_expressed_at_ms = Some(state.elapsed_ms);
    if state
        .creature
        .relationship_expression
        .count_active_day_index
        != state.active_day()
    {
        state
            .creature
            .relationship_expression
            .count_active_day_index = state.active_day();
        state.creature.relationship_expression.count_active_day = 0;
    }
    state.creature.relationship_expression.count_active_day = state
        .creature
        .relationship_expression
        .count_active_day
        .saturating_add(1);
    state
        .creature
        .relationship_expression
        .recent
        .push(crate::ExpressedMotif {
            key: motif,
            expressed_at_ms: state.elapsed_ms,
            expression_kind,
        });
    state
        .creature
        .relationship_expression
        .performance_ledger
        .push(crate::RelationshipPerformanceRecord {
            motif,
            subject,
            evidence: evidence.to_vec(),
            recipe: crate::relationship::performance_recipe_for(motif, expression_kind),
            performed_at_ms: state.elapsed_ms,
        });
    let excess = state
        .creature
        .relationship_expression
        .recent
        .len()
        .saturating_sub(crate::relationship::MAX_RECENT_EXPRESSIONS);
    if excess > 0 {
        state
            .creature
            .relationship_expression
            .recent
            .drain(..excess);
    }
    let ledger_excess = state
        .creature
        .relationship_expression
        .performance_ledger
        .len()
        .saturating_sub(crate::relationship::MAX_PERFORMANCE_LEDGER_RECORDS);
    if ledger_excess > 0 {
        state
            .creature
            .relationship_expression
            .performance_ledger
            .drain(..ledger_excess);
    }
}

pub(crate) fn allocate_action_id(state: &mut WorldState) -> u64 {
    let id = state.creature.interaction_state.next_action_id.max(1);
    state.creature.interaction_state.next_action_id = id.saturating_add(1).max(1);
    id
}

fn allocate_toy_interaction_id(state: &mut WorldState) -> NonZeroU64 {
    let raw = state
        .creature
        .interaction_state
        .next_toy_interaction_id
        .max(1);
    state.creature.interaction_state.next_toy_interaction_id = raw.saturating_add(1).max(1);
    NonZeroU64::new(raw).expect("toy interaction IDs start at one")
}

pub(crate) fn set_travel_target(
    state: &mut WorldState,
    destination: SemanticDestination,
    purpose: crate::TravelPurpose,
    events: &mut Vec<GameEvent>,
) {
    let previous_destination = state.creature.aquarium.destination;
    let previous_purpose = state.creature.aquarium.travel_purpose;
    let replacing = previous_destination.is_some() || previous_purpose.is_some();
    let same = previous_destination == Some(destination) && previous_purpose == Some(purpose);
    if replacing && !same {
        let active_private_previous = state
            .creature
            .private_life
            .active
            .as_ref()
            .is_some_and(|activity| {
                matches!(
                    previous_purpose,
                    Some(crate::TravelPurpose::PrivateLife { activity_id }) if activity_id == activity.id
                )
            });
        let active_toy_owned_previous = state
            .creature
            .interaction_state
            .toy_interaction
            .as_ref()
            .is_some_and(|interaction| {
                matches!(
                    previous_purpose,
                    Some(crate::TravelPurpose::ToyInteraction { interaction_id }
                        | crate::TravelPurpose::RefusalStare { interaction_id })
                        if interaction_id == interaction.id
                )
            });
        if active_private_previous {
            interrupt_private_life(state, ActivityInterruptionOwner::Relationship, events);
        } else if active_toy_owned_previous {
            interrupt_toy_interaction(state, events);
        } else {
            clear_travel_target(state);
            if let Some(previous_destination) = previous_destination {
                events.push(GameEvent::ActionAborted {
                    destination: previous_destination,
                });
            }
        }
    }
    state.creature.aquarium.destination = Some(destination);
    state.creature.aquarium.travel_purpose = Some(purpose);
    state.creature.aquarium.steering = SteeringMode::Approach;
}

fn clear_travel_target(state: &mut WorldState) {
    state.creature.aquarium.destination = None;
    state.creature.aquarium.travel_purpose = None;
}

fn interrupt_travel(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let Some(destination) = state.creature.aquarium.destination else {
        state.creature.aquarium.travel_purpose = None;
        return;
    };
    clear_travel_target(state);
    state.creature.aquarium.steering = SteeringMode::Hover;
    events.push(GameEvent::ActionAborted { destination });
}

fn interrupt_toy_interaction(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let Some(mut interaction) = state.creature.interaction_state.toy_interaction.take() else {
        return;
    };
    if interaction.phase == crate::ToyInteractionPhase::Recovery {
        // Contact already committed. Language and other input must not erase the
        // physical response or strand a held sock before its recovery boundary.
        state.creature.interaction_state.toy_interaction = Some(interaction);
        return;
    }
    interaction.outcome = crate::ToyInteractionOutcome::Interrupted;
    interaction.phase = crate::ToyInteractionPhase::Interrupted;
    if matches!(
        state.creature.aquarium.travel_purpose,
        Some(crate::TravelPurpose::ToyInteraction { interaction_id }
            | crate::TravelPurpose::RefusalStare { interaction_id }) if interaction_id == interaction.id
    ) {
        clear_travel_target(state);
        state.creature.aquarium.steering = SteeringMode::Hover;
    }
    events.push(GameEvent::ToyInteractionInterrupted {
        toy: interaction.toy,
        interaction_id: interaction.id,
        origin: interaction.origin,
    });
}

fn interrupt_private_life(
    state: &mut WorldState,
    by: ActivityInterruptionOwner,
    events: &mut Vec<GameEvent>,
) {
    let Some(mut activity) = state.creature.private_life.active.take() else {
        return;
    };
    if activity.phase == ActivityPhase::Recover || activity.phase == ActivityPhase::Settle {
        state.creature.private_life.active = Some(activity);
        return;
    }
    let phase = activity.phase;
    activity.phase = ActivityPhase::Interrupted;
    push_recent_activity(state, &activity, Some(by));
    if matches!(
        state.creature.aquarium.travel_purpose,
        Some(crate::TravelPurpose::PrivateLife { activity_id }) if activity_id == activity.id
    ) {
        clear_travel_target(state);
        state.creature.aquarium.steering = SteeringMode::Hover;
    }
    events.push(GameEvent::PrivateLifeInterrupted {
        activity_id: activity.id,
        phase,
        by,
    });
}

fn private_phase_duration(kind: PrivateLifeKind, phase: ActivityPhase) -> u64 {
    match phase {
        ActivityPhase::Notice => PRIVATE_NOTICE_MS,
        ActivityPhase::Approach => PRIVATE_NOTICE_MS,
        // A snap at a bubble is quick; a long one reads as hovering with nothing there.
        ActivityPhase::Act if kind == PrivateLifeKind::OpenWaterDrift => BUBBLE_SNAP_MS,
        ActivityPhase::Act => PRIVATE_ACT_MS,
        ActivityPhase::Recover => PRIVATE_RECOVER_MS,
        ActivityPhase::Settle => PRIVATE_SETTLE_MS,
        ActivityPhase::Interrupted => 0,
    }
}

fn advance_private_life(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let Some(mut activity) = state.creature.private_life.active.clone() else {
        return;
    };
    if state
        .elapsed_ms
        .saturating_sub(activity.phase_started_at_ms)
        < private_phase_duration(activity.kind, activity.phase)
    {
        return;
    }
    let from = activity.phase;
    match from {
        ActivityPhase::Notice => {
            activity.phase = ActivityPhase::Approach;
            activity.phase_started_at_ms = state.elapsed_ms;
            if let Some(destination) = activity.kind.destination() {
                set_travel_target(
                    state,
                    destination,
                    crate::TravelPurpose::PrivateLife {
                        activity_id: activity.id,
                    },
                    events,
                );
                state.creature.aquarium.gaze = match activity.subject {
                    Some(ActivitySubject::Toy(toy)) => GazeTarget::Toy(toy),
                    Some(ActivitySubject::Cave) => GazeTarget::Cave,
                    Some(ActivitySubject::Plant) => GazeTarget::Plant,
                    _ => GazeTarget::None,
                };
            } else {
                // Open water is a bubble chase: a visible target high in the tank, so the swim
                // reads as wanting something rather than floating.
                let target = bubble_point(state, activity.id);
                set_travel_target(
                    state,
                    SemanticDestination::Position(target),
                    crate::TravelPurpose::PrivateLife {
                        activity_id: activity.id,
                    },
                    events,
                );
                state.creature.aquarium.gaze = GazeTarget::None;
            }
        }
        // A bubble that drifted out of reach is snapped at from wherever the chase got to.
        ActivityPhase::Approach
            if activity.kind.destination().is_none()
                && state
                    .elapsed_ms
                    .saturating_sub(activity.phase_started_at_ms)
                    >= BUBBLE_CHASE_GIVE_UP_MS =>
        {
            if matches!(
                state.creature.aquarium.travel_purpose,
                Some(crate::TravelPurpose::PrivateLife { activity_id }) if activity_id == activity.id
            ) {
                clear_travel_target(state);
            }
            activity.phase = ActivityPhase::Act;
            activity.phase_started_at_ms = state.elapsed_ms;
        }
        ActivityPhase::Act => {
            perform_private_life_payoff(state, &mut activity, events);
            activity.phase = match activity.kind {
                PrivateLifeKind::CaveSettle => ActivityPhase::Settle,
                _ => ActivityPhase::Recover,
            };
            activity.phase_started_at_ms = state.elapsed_ms;
        }
        ActivityPhase::Recover | ActivityPhase::Settle => {
            if activity.kind == PrivateLifeKind::ToyPlay(ToyId::Sock) {
                release_sock(state);
            }
            push_recent_activity(state, &activity, None);
            state.creature.private_life.active = None;
            // A newer owner (a toy offer, food, a relationship beat) may have started while
            // this activity settled; finishing must not take its body or intention away.
            let newer_owner = state.creature.interaction_state.toy_interaction.is_some()
                || state.creature.aquarium.action.is_some()
                || state.creature.relationship_expression.active.is_some()
                || state.creature.aquarium.destination.is_some();
            if !newer_owner {
                state.creature.aquarium.steering = SteeringMode::Hover;
                state.creature.current_intention = Intention::Idle;
            }
            schedule_next_idle_bout(state);
            events.push(GameEvent::PrivateLifeCompleted {
                activity_id: activity.id,
                kind: activity.kind,
                recipe: activity.recipe,
            });
            return;
        }
        ActivityPhase::Approach | ActivityPhase::Interrupted => return,
    }
    state.creature.private_life.active = Some(activity.clone());
    events.push(GameEvent::PrivateLifePhaseChanged {
        activity_id: activity.id,
        from,
        to: activity.phase,
    });
}

const BUBBLE_CHASE_GIVE_UP_MS: u64 = 4_000;

/// Where an open-water bubble chase heads: somewhere in the upper water, varied per activity.
#[must_use]
pub fn bubble_point(state: &WorldState, activity_id: NonZeroU64) -> NormalizedPosition {
    if let Some(bubble) = state
        .creature
        .private_life
        .active
        .as_ref()
        .filter(|activity| activity.id == activity_id)
        .and_then(|activity| activity.bubble)
    {
        return bubble;
    }
    let key = activity_id.get().wrapping_mul(7);
    let x = deterministic_unit(state.seed, RandomDomain::Motion, key + 3);
    let y = deterministic_unit(state.seed, RandomDomain::Motion, key + 5);
    NormalizedPosition::new(1_500 + (x * 7_000.0) as i32, 1_400 + (y * 2_200.0) as i32)
}

/// A bubble close in front of and above the creature, so the chase is a short, legible dart
/// rather than a trip across the tank.
fn nearby_bubble(state: &WorldState, activity_id: NonZeroU64) -> NormalizedPosition {
    let key = activity_id.get().wrapping_mul(11);
    let reach = deterministic_unit(state.seed, RandomDomain::Motion, key + 1);
    let rise = deterministic_unit(state.seed, RandomDomain::Motion, key + 2);
    let here = state.creature.aquarium.position;
    let ahead = 1_400 + (reach * 1_000.0) as i32;
    let mut x = match state.creature.aquarium.facing {
        crate::Facing::Right => here.x + ahead,
        crate::Facing::Left => here.x - ahead,
    };
    if !(1_200..=8_800).contains(&x) {
        x = 2 * here.x - x;
    }
    let y = (here.y - 1_000 - (rise * 1_000.0) as i32).clamp(1_300, 5_200);
    let mut bubble = NormalizedPosition::new(x.clamp(1_200, 8_800), y);
    // Keep it in clear water: a bubble over a toy reads as wanting the toy.
    let near_toy = |point: NormalizedPosition| {
        state.aquarium.toy_states.values().any(|toy| {
            (toy.position.x - point.x).abs() < 1_300 && (toy.position.y - point.y).abs() < 1_500
        })
    };
    if near_toy(bubble) {
        let mirrored = NormalizedPosition::new((2 * here.x - bubble.x).clamp(1_200, 8_800), y);
        bubble = if near_toy(mirrored) {
            NormalizedPosition::new(bubble.x, 1_300)
        } else {
            mirrored
        };
    }
    bubble
}

fn perform_private_life_payoff(
    state: &mut WorldState,
    activity: &mut PrivateLifeActivity,
    events: &mut Vec<GameEvent>,
) {
    if activity.payoff_reached {
        return;
    }
    activity.payoff_reached = true;
    match activity.kind {
        PrivateLifeKind::ToyPlay(toy) => {
            let Some(response) = respond_to_toy_contact(state, toy, activity.id) else {
                return;
            };
            events.push(GameEvent::ToyObjectResponded {
                toy,
                activity_id: activity.id,
                response,
            });
        }
        PrivateLifeKind::CaveSettle => {
            state.creature.needs.energy = (state.creature.needs.energy + 0.04).min(1.0);
        }
        PrivateLifeKind::PlantInspect => {
            state.creature.needs.curiosity = (state.creature.needs.curiosity - 0.08).max(0.0);
        }
        PrivateLifeKind::BottomForage => {
            state.creature.needs.curiosity = (state.creature.needs.curiosity - 0.12).max(0.0);
        }
        PrivateLifeKind::OpenWaterDrift => {
            state.creature.needs.comfort = (state.creature.needs.comfort + 0.02).min(1.0);
        }
    }
}

/// Both contact paths mutate the same physical toy. Their own phase/payoff guards
/// enforce exactly once: private activity and direct interaction IDs are separate
/// namespaces, so the saved last-contact number is never a deduplication key.
fn respond_to_toy_contact(
    state: &mut WorldState,
    toy: ToyId,
    contact_id: NonZeroU64,
) -> Option<ToyResponse> {
    let head = state.creature.aquarium.position;
    let object = state.aquarium.toy_states.get_mut(&toy)?;
    let response = match toy {
        ToyId::Ball => {
            let dx = object.position.x - head.x;
            let dy = object.position.y - head.y;
            let distance = dx.abs().max(dy.abs()).max(1);
            object.velocity = NormalizedVelocity {
                x: dx * BALL_NUDGE_SPEED / distance,
                y: dy * BALL_NUDGE_SPEED / distance,
            };
            ToyResponse::BallNudged
        }
        ToyId::Bell => {
            object.velocity = NormalizedVelocity::default();
            ToyResponse::BellStruck
        }
        ToyId::Sock => {
            object.carried = true;
            object.position = held_toy_position(head);
            object.velocity = NormalizedVelocity::default();
            ToyResponse::SockTugged
        }
    };
    object.last_response = response;
    object.last_contact_activity = Some(contact_id);
    Some(response)
}

fn finish_toy_recovery(state: &mut WorldState) {
    let Some(interaction) = state.creature.interaction_state.toy_interaction.as_mut() else {
        return;
    };
    if interaction.phase != crate::ToyInteractionPhase::Recovery
        || state.elapsed_ms < interaction.recovery_until_ms
    {
        return;
    }
    if interaction.rounds_left > 0
        && interaction.outcome == crate::ToyInteractionOutcome::Accepted
        && interaction.toy != ToyId::Sock
    {
        // Another round: chase the toy to wherever the last contact sent it.
        interaction.rounds_left -= 1;
        interaction.phase = crate::ToyInteractionPhase::Approach;
        let toy = interaction.toy;
        let interaction_id = interaction.id;
        state.creature.aquarium.destination = Some(SemanticDestination::Toy(toy));
        state.creature.aquarium.travel_purpose =
            Some(crate::TravelPurpose::ToyInteraction { interaction_id });
        state.creature.aquarium.steering = SteeringMode::Approach;
        state.creature.aquarium.gaze = GazeTarget::Toy(toy);
        state.creature.current_intention = Intention::Play;
        return;
    }
    end_toy_recovery(state);
}

/// End a committed toy recovery now, releasing a held sock. A newer offer uses this directly.
fn end_toy_recovery(state: &mut WorldState) {
    let Some(interaction) = state.creature.interaction_state.toy_interaction.as_ref() else {
        return;
    };
    if interaction.phase != crate::ToyInteractionPhase::Recovery {
        return;
    }
    if interaction.toy == ToyId::Sock
        && interaction.outcome == crate::ToyInteractionOutcome::Accepted
    {
        release_sock(state);
    }
    state.creature.interaction_state.toy_interaction = None;
}

fn push_recent_activity(
    state: &mut WorldState,
    activity: &PrivateLifeActivity,
    interrupted_by: Option<ActivityInterruptionOwner>,
) {
    if state
        .creature
        .private_life
        .recent
        .iter()
        .any(|entry| entry.id == activity.id)
    {
        return;
    }
    state.creature.private_life.recent.push(RecentActivity {
        id: activity.id,
        kind: activity.kind,
        subject: activity.subject,
        recipe: activity.recipe,
        selected_at_ms: activity.selected_at_ms,
        completed_at_ms: interrupted_by.is_none().then_some(state.elapsed_ms),
        interrupted_by,
        active_day: state.active_day(),
    });
    let excess = state
        .creature
        .private_life
        .recent
        .len()
        .saturating_sub(MAX_RECENT_ACTIVITIES);
    if excess > 0 {
        state.creature.private_life.recent.drain(..excess);
    }
}

fn resolve_toy_play(
    state: &mut WorldState,
    interaction_id: NonZeroU64,
    events: &mut Vec<GameEvent>,
) {
    let Some(mut interaction) = state.creature.interaction_state.toy_interaction.take() else {
        return;
    };
    if interaction.id != interaction_id
        || interaction.outcome != crate::ToyInteractionOutcome::Accepted
        || interaction.phase != crate::ToyInteractionPhase::Approach
    {
        state.creature.interaction_state.toy_interaction = Some(interaction);
        return;
    }
    interaction.phase = crate::ToyInteractionPhase::Contact;
    events.push(GameEvent::ToyContacted {
        toy: interaction.toy,
        interaction_id,
        origin: interaction.origin,
    });
    if let Some(response) = respond_to_toy_contact(state, interaction.toy, interaction_id) {
        events.push(GameEvent::ToyInteractionResponded {
            toy: interaction.toy,
            interaction_id,
            response,
        });
    }
    interaction.contacts = interaction.contacts.saturating_add(1);
    if interaction.contacts > 1 {
        // A later round of the same game: physical response only, no repeated reward.
        interaction.phase = crate::ToyInteractionPhase::Recovery;
        interaction.recovery_until_ms = state.elapsed_ms.saturating_add(toy_round_recovery_ms(
            interaction.toy,
            interaction.rounds_left,
        ));
        state.creature.interaction_state.toy_interaction = Some(interaction);
        return;
    }
    state.creature.needs.curiosity = (state.creature.needs.curiosity - 0.18).max(0.0);
    match interaction.origin {
        crate::ToyOrigin::Player => {
            let preference = state
                .creature
                .toy_preferences
                .get(&interaction.toy)
                .copied()
                .unwrap_or(0.0);
            improve_relationship(state, 0.01, 0.008, 0.012);
            state.remember(
                MemoryKind::PlayedWith {
                    toy: interaction.toy,
                },
                &[Concept::Good],
                preference,
                0.6,
            );
            if let Some(context) = interaction.relationship.take() {
                start_toy_relationship_moment(state, interaction_id, context, events);
            }
        }
        crate::ToyOrigin::Autonomous => {
            record_idle_arrival_state(state, SemanticDestination::Toy(interaction.toy));
        }
    }
    interaction.phase = crate::ToyInteractionPhase::Resolved;
    events.push(GameEvent::ToyPlayed {
        toy: interaction.toy,
        interaction_id,
        origin: interaction.origin,
    });
    state
        .creature
        .interaction_state
        .last_resolved_toy_interaction = Some(crate::ResolvedToyInteraction {
        id: interaction.id,
        toy: interaction.toy,
        origin: interaction.origin,
    });
    interaction.phase = crate::ToyInteractionPhase::Recovery;
    interaction.recovery_until_ms = state.elapsed_ms.saturating_add(toy_round_recovery_ms(
        interaction.toy,
        interaction.rounds_left,
    ));
    state.creature.interaction_state.toy_interaction = Some(interaction);
}

/// The pause after a contact: short between rounds of a game, longer after the last one. The
/// sock is towed around for a while before it is let go.
fn toy_round_recovery_ms(toy: ToyId, rounds_left: u8) -> u64 {
    match (toy, rounds_left) {
        (ToyId::Sock, _) => 2_600,
        (_, 0) => TOY_RECOVERY_MS,
        _ => 450,
    }
}

/// Rounds in a play session after the first contact.
fn toy_session_rounds(toy: ToyId, origin: crate::ToyOrigin) -> u8 {
    match (toy, origin) {
        (ToyId::Ball, crate::ToyOrigin::Player) => 3,
        (ToyId::Bell, crate::ToyOrigin::Player) => 2,
        (ToyId::Ball | ToyId::Bell, crate::ToyOrigin::Autonomous) => 1,
        (ToyId::Sock, _) => 0,
    }
}

fn dispatch_travel_arrival(
    state: &mut WorldState,
    destination: SemanticDestination,
    purpose: crate::TravelPurpose,
    events: &mut Vec<GameEvent>,
) {
    match purpose {
        crate::TravelPurpose::IdleVisit { .. } => {
            record_idle_arrival_state(state, destination);
            maybe_start_relationship_beat(
                state,
                RelationshipTrigger::PlaceArrived { destination },
                events,
            );
        }
        crate::TravelPurpose::ToyInteraction { interaction_id } => {
            resolve_toy_play(state, interaction_id, events);
        }
        crate::TravelPurpose::RefusalStare { interaction_id } => {
            if let Some(interaction) = state.creature.interaction_state.toy_interaction.as_mut()
                && interaction.id == interaction_id
                && interaction.phase == crate::ToyInteractionPhase::Approach
            {
                interaction.phase = crate::ToyInteractionPhase::Recovery;
                interaction.recovery_until_ms = state.elapsed_ms.saturating_add(TOY_RECOVERY_MS);
            }
        }
        crate::TravelPurpose::Relationship { .. } => {
            state.creature.idle_life.last_arrived_destination = Some(destination);
            state.creature.idle_life.settled_until_ms =
                state.elapsed_ms.saturating_add(IDLE_BOUT_MIN_MS);
        }
        crate::TravelPurpose::PrivateLife { activity_id } => {
            let Some(mut activity) = state.creature.private_life.active.clone() else {
                return;
            };
            if activity.id != activity_id || activity.phase != ActivityPhase::Approach {
                return;
            }
            let from = activity.phase;
            activity.phase = ActivityPhase::Act;
            activity.phase_started_at_ms = state.elapsed_ms;
            record_idle_arrival_state(state, destination);
            state.creature.private_life.active = Some(activity.clone());
            events.push(GameEvent::PrivateLifePhaseChanged {
                activity_id,
                from,
                to: ActivityPhase::Act,
            });
        }
        crate::TravelPurpose::CursorSocial { .. } | crate::TravelPurpose::Initiative { .. } => {}
    }
}

fn resolve_toy_interaction_offline(state: &mut WorldState) {
    let Some(mut interaction) = state.creature.interaction_state.toy_interaction.take() else {
        return;
    };
    if interaction.phase == crate::ToyInteractionPhase::Recovery {
        if interaction.toy == ToyId::Sock
            && interaction.outcome == crate::ToyInteractionOutcome::Accepted
        {
            release_sock(state);
        }
        clear_travel_target(state);
        return;
    }
    if interaction.phase == crate::ToyInteractionPhase::Approach
        && interaction.origin == crate::ToyOrigin::Player
        && matches!(
            state.creature.aquarium.travel_purpose,
            Some(crate::TravelPurpose::RefusalStare { interaction_id }) if interaction_id == interaction.id
        )
    {
        clear_travel_target(state);
        return;
    }
    if interaction.phase != crate::ToyInteractionPhase::Approach
        || interaction.outcome != crate::ToyInteractionOutcome::Accepted
        || !matches!(
            state.creature.aquarium.travel_purpose,
            Some(crate::TravelPurpose::ToyInteraction { interaction_id }) if interaction_id == interaction.id
        )
    {
        state.creature.interaction_state.toy_interaction = Some(interaction);
        return;
    }
    // Offline completion keeps the same durable physical outcome without
    // replaying old contact cues or leaving a sock carried across the absence.
    let _ = respond_to_toy_contact(state, interaction.toy, interaction.id);
    if interaction.toy == ToyId::Sock {
        release_sock(state);
    }
    state.creature.needs.curiosity = (state.creature.needs.curiosity - 0.18).max(0.0);
    match interaction.origin {
        crate::ToyOrigin::Player => {
            let preference = state
                .creature
                .toy_preferences
                .get(&interaction.toy)
                .copied()
                .unwrap_or(0.0);
            improve_relationship(state, 0.01, 0.008, 0.012);
            state.remember(
                MemoryKind::PlayedWith {
                    toy: interaction.toy,
                },
                &[Concept::Good],
                preference,
                0.6,
            );
            if let Some(context) = interaction.relationship.take() {
                record_relationship_expression(
                    state,
                    context.motif,
                    context.subject,
                    &context.evidence,
                    context.expression_kind,
                );
            }
        }
        crate::ToyOrigin::Autonomous => {
            record_idle_arrival_state(state, SemanticDestination::Toy(interaction.toy));
        }
    }
    state
        .creature
        .interaction_state
        .last_resolved_toy_interaction = Some(crate::ResolvedToyInteraction {
        id: interaction.id,
        toy: interaction.toy,
        origin: interaction.origin,
    });
    // Absence completes recovery too. Keeping its owner would suppress the
    // one-shot PlayerReturn trigger when advance_offline hands control back.
    clear_travel_target(state);
}

fn resolve_private_life_offline(state: &mut WorldState) {
    let Some(mut activity) = state.creature.private_life.active.take() else {
        return;
    };
    if !activity.payoff_reached {
        let mut discarded_events = Vec::new();
        perform_private_life_payoff(state, &mut activity, &mut discarded_events);
    }
    if activity.kind == PrivateLifeKind::ToyPlay(ToyId::Sock) {
        release_sock(state);
    }
    push_recent_activity(state, &activity, None);
    clear_travel_target(state);
    state.creature.aquarium.steering = SteeringMode::Hover;
    state.creature.current_intention = Intention::Idle;
}

fn start_action_relationship_moment(
    state: &mut WorldState,
    context: crate::ActionRelationshipContext,
    events: &mut Vec<GameEvent>,
) {
    let action_id = allocate_action_id(state);
    record_relationship_expression(
        state,
        context.motif,
        context.subject,
        &context.evidence,
        context.expression_kind,
    );
    events.push(GameEvent::ActionRelationshipStarted {
        action_id,
        motif: context.motif,
        expression: context.expression_kind,
        subject: context.subject,
        evidence: context.evidence.clone(),
    });
    state.creature.interaction_state.relationship_moment = Some(crate::ActionRelationshipMoment {
        action_id,
        context,
        started_at_ms: state.elapsed_ms,
        expires_at_ms: state
            .elapsed_ms
            .saturating_add(DIRECT_RELATIONSHIP_MOMENT_MS),
    });
}

fn start_toy_relationship_moment(
    state: &mut WorldState,
    interaction_id: NonZeroU64,
    context: crate::ActionRelationshipContext,
    events: &mut Vec<GameEvent>,
) {
    let action_id = interaction_id.get();
    state.creature.interaction_state.next_action_id = state
        .creature
        .interaction_state
        .next_action_id
        .max(action_id.saturating_add(1));
    record_relationship_expression(
        state,
        context.motif,
        context.subject,
        &context.evidence,
        context.expression_kind,
    );
    events.push(GameEvent::ActionRelationshipStarted {
        action_id,
        motif: context.motif,
        expression: context.expression_kind,
        subject: context.subject,
        evidence: context.evidence.clone(),
    });
    state.creature.interaction_state.relationship_moment = Some(crate::ActionRelationshipMoment {
        action_id,
        context,
        started_at_ms: state.elapsed_ms,
        expires_at_ms: state
            .elapsed_ms
            .saturating_add(DIRECT_RELATIONSHIP_MOMENT_MS),
    });
}

fn interrupt_action_relationship_moment(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let Some(moment) = state.creature.interaction_state.relationship_moment.take() else {
        return;
    };
    events.push(GameEvent::ActionRelationshipInterrupted {
        action_id: moment.action_id,
        motif: moment.context.motif,
        subject: moment.context.subject,
    });
}

fn apply_relationship_pose(
    state: &mut WorldState,
    beat: &crate::RelationshipBeat,
    events: &mut Vec<GameEvent>,
) {
    match beat.motif {
        crate::RelationshipMotifKey::SharedToy(toy) => {
            state.creature.aquarium.gaze = GazeTarget::Toy(toy);
            let restrained_approach = beat.expression_kind
                == crate::RelationshipExpressionKind::Notice
                && beat.phase == crate::RelationshipBeatPhase::Anticipate;
            if beat.phase != crate::RelationshipBeatPhase::Notice
                && (beat.expression_kind != crate::RelationshipExpressionKind::Notice
                    || restrained_approach)
            {
                set_relationship_travel(state, beat, events);
                state.creature.current_intention = Intention::Play;
            } else if beat.expression_kind == crate::RelationshipExpressionKind::Notice {
                clear_travel_target(state);
                state.creature.aquarium.steering = SteeringMode::Hover;
                state.creature.current_intention = Intention::Idle;
            }
        }
        crate::RelationshipMotifKey::ComfortRitual => {
            state.creature.aquarium.gaze = GazeTarget::Player;
            if beat.expression_kind != crate::RelationshipExpressionKind::Notice
                && beat.phase != crate::RelationshipBeatPhase::Notice
            {
                state.creature.current_intention = Intention::SeekComfort;
                set_relationship_travel(state, beat, events);
            }
        }
        crate::RelationshipMotifKey::TrustedFood(food) => {
            if let Some(id) = state.aquarium.objects.iter().find_map(|(id, object)| {
                matches!(object, WorldObject::Food(candidate) if candidate.food == food)
                    .then_some(*id)
            }) {
                state.creature.aquarium.gaze = GazeTarget::Food(id);
            }
            state.creature.current_intention = Intention::WaitAtBowl;
            if beat.phase != crate::RelationshipBeatPhase::Notice {
                set_relationship_travel(state, beat, events);
            }
        }
        crate::RelationshipMotifKey::FoodGrudge(food) => {
            if let Some(id) = state.aquarium.objects.iter().find_map(|(id, object)| {
                matches!(object, WorldObject::Food(candidate) if candidate.food == food)
                    .then_some(*id)
            }) {
                state.creature.aquarium.gaze = GazeTarget::Food(id);
            }
            if beat.expression_kind != crate::RelationshipExpressionKind::Notice
                && beat.phase != crate::RelationshipBeatPhase::Notice
            {
                state.creature.current_intention = Intention::RejectFood;
                set_relationship_travel(state, beat, events);
            }
        }
        crate::RelationshipMotifKey::PlayerReturns => {
            state.creature.aquarium.gaze = GazeTarget::Player;
            if beat.expression_kind != crate::RelationshipExpressionKind::Notice {
                state.creature.current_intention = Intention::ApproachPlayer;
                set_relationship_travel(state, beat, events);
            }
        }
        crate::RelationshipMotifKey::FamiliarPlace(destination) => {
            state.creature.current_intention = Intention::Idle;
            state.creature.aquarium.gaze = match destination {
                SemanticDestination::Cave => GazeTarget::Cave,
                SemanticDestination::Plant => GazeTarget::Plant,
                SemanticDestination::Toy(toy) => GazeTarget::Toy(toy),
                _ => GazeTarget::None,
            };
            if beat.phase == crate::RelationshipBeatPhase::Anticipate {
                set_relationship_travel(state, beat, events);
            } else {
                clear_travel_target(state);
                state.creature.aquarium.steering = SteeringMode::Hover;
            }
        }
    }
}

fn set_relationship_travel(
    state: &mut WorldState,
    beat: &crate::RelationshipBeat,
    events: &mut Vec<GameEvent>,
) {
    if let Some(destination) = beat.target {
        let beat_id = NonZeroU64::new(beat.started_at_ms.saturating_add(1))
            .expect("relationship beat IDs are offset from time");
        set_travel_target(
            state,
            destination,
            crate::TravelPurpose::Relationship { beat_id },
            events,
        );
    } else {
        clear_travel_target(state);
    }
}

fn interrupt_relationship_beat(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let Some(beat) = state.creature.relationship_expression.active.take() else {
        return;
    };
    if state.creature.aquarium.action.is_none() {
        clear_travel_target(state);
        state.creature.aquarium.steering = SteeringMode::Hover;
    }
    events.push(GameEvent::RelationshipBeatInterrupted(beat.motif));
}

fn advance_relationship_beat(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let Some(mut beat) = state.creature.relationship_expression.active.clone() else {
        return;
    };
    let elapsed = state.elapsed_ms.saturating_sub(beat.phase_started_at_ms);
    if elapsed < crate::relationship::phase_duration_ms(&beat) {
        apply_relationship_pose(state, &beat, events);
        return;
    }
    let old = beat.phase;
    let next = match old {
        crate::RelationshipBeatPhase::Notice => crate::RelationshipBeatPhase::Anticipate,
        crate::RelationshipBeatPhase::Anticipate => {
            if let crate::RelationshipMotifKey::FamiliarPlace(destination) = beat.motif
                && (state.creature.idle_life.last_arrived_destination != Some(destination)
                    || state.elapsed_ms >= state.creature.idle_life.settled_until_ms)
            {
                apply_relationship_pose(state, &beat, events);
                return;
            }
            crate::RelationshipBeatPhase::Act
        }
        crate::RelationshipBeatPhase::Act => crate::RelationshipBeatPhase::Recover,
        crate::RelationshipBeatPhase::Recover => {
            state.creature.relationship_expression.active = None;
            if state.creature.aquarium.action.is_none() {
                clear_travel_target(state);
                state.creature.aquarium.steering = SteeringMode::Hover;
                state.creature.current_intention = Intention::Idle;
            }
            events.push(GameEvent::RelationshipBeatCompleted(beat.motif));
            return;
        }
    };
    beat.phase = next;
    beat.phase_started_at_ms = state.elapsed_ms;
    apply_relationship_pose(state, &beat, events);
    state.creature.relationship_expression.active = Some(beat.clone());
    events.push(GameEvent::RelationshipBeatPhaseChanged {
        motif: beat.motif,
        from: Some(old),
        to: next,
    });
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
    if let Some(previous) = state.creature.aquarium.action.take() {
        if let Some(context) = previous.relationship.as_ref() {
            events.push(GameEvent::ActionRelationshipInterrupted {
                action_id: previous.action_id,
                motif: context.motif,
                subject: context.subject,
            });
        }
        events.push(GameEvent::ActionAborted {
            destination: previous.destination,
        });
    }
    let id = state.aquarium.next_object_id;
    state.aquarium.next_object_id = id.saturating_add(1);
    let action_id = allocate_action_id(state);
    if !state.creature.preferences.contains_key(&food) {
        let preference =
            deterministic_unit(state.seed, RandomDomain::Preferences, id).mul_add(2.0, -1.0);
        state.creature.preferences.insert(food, preference);
    }
    let relationship = crate::select_action_relationship_context(
        state,
        RelationshipTrigger::FoodPresented { food },
    );
    let buoyancy = match food {
        FoodId::Berry => FoodBuoyancy::Sink,
        FoodId::Mushroom => FoodBuoyancy::Float,
        FoodId::Pellet => FoodBuoyancy::Drift,
    };
    // Units per second: food visibly sinks, rises or drifts while Mop goes for it.
    let velocity = match buoyancy {
        FoodBuoyancy::Float => NormalizedVelocity { x: 0, y: -450 },
        FoodBuoyancy::Sink => NormalizedVelocity { x: 0, y: 650 },
        FoodBuoyancy::Drift => NormalizedVelocity { x: 160, y: 160 },
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
        action_id,
        phase: ActionPhase::Notice,
        elapsed_ms: 0,
        phase_duration_ms: food_phase_duration_ms(ActionPhase::Notice),
        destination: SemanticDestination::Food(id),
        food_id: Some(id),
        food: Some(food),
        food_outcome: None,
        relationship: relationship.clone(),
    });
    events.push(GameEvent::FoodDropped { id, food, position });
    events.push(GameEvent::ActionPhaseChanged {
        from: None,
        to: ActionPhase::Notice,
    });
    if let Some(context) = relationship {
        record_relationship_expression(
            state,
            context.motif,
            context.subject,
            &context.evidence,
            context.expression_kind,
        );
        events.push(GameEvent::ActionRelationshipStarted {
            action_id,
            motif: context.motif,
            expression: context.expression_kind,
            subject: context.subject,
            evidence: context.evidence,
        });
    }
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

fn advance_aquarium(state: &mut WorldState, sock_just_released: bool, events: &mut Vec<GameEvent>) {
    let elapsed = state.elapsed_ms;
    for (id, toy) in &mut state.aquarium.toy_states {
        // Release starts at the held anchor. Its velocity then drives continuous projection
        // and the following tick, rather than jumping a full fall step on the release frame.
        if toy.carried || (*id == ToyId::Sock && sock_just_released) {
            continue;
        }
        toy.position = NormalizedPosition::new(
            toy.position
                .x
                .saturating_add(tick_step(toy.velocity.x, elapsed)),
            toy.position
                .y
                .saturating_add(tick_step(toy.velocity.y, elapsed)),
        )
        .clamped();
        // Water drag halves a free toy's speed about every second.
        toy.velocity.x = toy.velocity.x * TOY_DRAG_PER_TICK / 1_000;
        toy.velocity.y = toy.velocity.y * TOY_DRAG_PER_TICK / 1_000;
    }
    if state.creature.current_intention != Intention::Sleep {
        advance_creature_motion(state, events);
    }
    let held_position = held_toy_position(state.creature.aquarium.position);
    for toy in state
        .aquarium
        .toy_states
        .values_mut()
        .filter(|toy| toy.carried)
    {
        toy.position = held_position;
    }
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
                food.position
                    .x
                    .saturating_add(tick_step(food.velocity.x, elapsed)),
                food.position
                    .y
                    .saturating_add(tick_step(food.velocity.y, elapsed)),
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
        events.push(GameEvent::FoodSettled(id));
    }
    if state.creature.current_intention == Intention::Sleep {
        return;
    }
    for id in expired {
        state.aquarium.objects.remove(&id);
        state.aquarium.object_names.remove(&id);
        events.push(GameEvent::FoodExpired(id));
    }
    let Some(mut timeline) = state.creature.aquarium.action.take() else {
        choose_idle_behavior(state, events);
        return;
    };
    if timeline.food_outcome.is_none()
        && timeline
            .food_id
            .is_none_or(|id| !matches!(state.aquarium.objects.get(&id), Some(WorldObject::Food(_))))
    {
        if let Some(context) = timeline.relationship.as_ref() {
            events.push(GameEvent::ActionRelationshipInterrupted {
                action_id: timeline.action_id,
                motif: context.motif,
                subject: context.subject,
            });
        }
        state.creature.aquarium.steering = SteeringMode::Hover;
        events.push(GameEvent::ActionAborted {
            destination: timeline.destination,
        });
        return;
    }
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
        let outcome = resolve_food(state, timeline.food_id, events);
        let Some(outcome) = outcome else {
            if let Some(context) = timeline.relationship.as_ref() {
                events.push(GameEvent::ActionRelationshipInterrupted {
                    action_id: timeline.action_id,
                    motif: context.motif,
                    subject: context.subject,
                });
            }
            state.creature.aquarium.steering = SteeringMode::Hover;
            events.push(GameEvent::ActionAborted {
                destination: timeline.destination,
            });
            return;
        };
        timeline.food_outcome = Some(outcome);
        if let Some(context) = timeline.relationship.as_ref() {
            events.push(GameEvent::ActionRelationshipResolved {
                action_id: timeline.action_id,
                motif: context.motif,
                subject: context.subject,
                outcome,
            });
        }
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
            state.creature.aquarium.steering = SteeringMode::Hover;
            if let Some(context) = timeline.relationship.as_ref() {
                events.push(GameEvent::ActionRelationshipCompleted {
                    action_id: timeline.action_id,
                    motif: context.motif,
                    subject: context.subject,
                });
            }
            return;
        }
    };
    timeline.phase = next;
    timeline.elapsed_ms = 0;
    timeline.phase_duration_ms = food_phase_duration_ms(next);
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

/// Feeding reads as one quick motion: a glance and turn of about half a second, the swim,
/// a short sniff, the bite and a satisfied settle.
const fn food_phase_duration_ms(phase: ActionPhase) -> u64 {
    match phase {
        ActionPhase::Notice => 120,
        ActionPhase::Brake | ActionPhase::Gaze => 60,
        ActionPhase::Turn => 100,
        ActionPhase::Approach => 0,
        ActionPhase::Inspect => 200,
        ActionPhase::Act => 350,
        ActionPhase::Recover => 800,
    }
}

/// A toy wedged so that no contact surface is clear gets nudged away from its nearest neighbor,
/// so the next attempt can reach it instead of failing again.
fn free_wedged_toy(state: &mut WorldState, toy: ToyId) {
    const PUSH: i32 = 1_200;
    let Some(position) = state
        .aquarium
        .toy_states
        .get(&toy)
        .map(|object| object.position)
    else {
        return;
    };
    let neighbor = state
        .aquarium
        .toy_states
        .iter()
        .filter(|(id, object)| **id != toy && !object.carried)
        .min_by_key(|(_, object)| manhattan_distance(object.position, position))
        .map(|(_, object)| object.position);
    let Some(neighbor) = neighbor else {
        return;
    };
    let away = if position.x == neighbor.x {
        if position.x < 5_000 { 1 } else { -1 }
    } else {
        (position.x - neighbor.x).signum()
    };
    if let Some(object) = state.aquarium.toy_states.get_mut(&toy) {
        object.velocity.x = away * PUSH;
    }
}

fn advance_creature_motion(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let action_target =
        state
            .creature
            .aquarium
            .action
            .as_ref()
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
    let target = action_target.or_else(|| steering_target(state, events));
    if let Some(target) = target {
        steer_toward(state, target);
    } else if state.creature.aquarium.action.is_none() {
        if holding_toy_contact(state) {
            state.creature.aquarium.velocity = NormalizedVelocity::default();
        } else if let Some(target) = toy_rest_target(state, state.creature.aquarium.position) {
            // A little backward buoyancy after contact, at the same world speed on both axes.
            // This also lets older overlapping saves leave the toy without a position jump.
            let position = state.creature.aquarium.position;
            let elapsed = state.elapsed_ms;
            let velocity = NormalizedVelocity { x: 1_200, y: 3_500 };
            let step = NormalizedVelocity {
                x: (target.x - position.x).clamp(
                    -tick_step(velocity.x, elapsed),
                    tick_step(velocity.x, elapsed),
                ),
                y: (target.y - position.y).clamp(
                    -tick_step(velocity.y, elapsed),
                    tick_step(velocity.y, elapsed),
                ),
            };
            state.creature.aquarium.position =
                NormalizedPosition::new(position.x + step.x, position.y + step.y).clamped();
            state.creature.aquarium.velocity = NormalizedVelocity {
                x: step.x * (1_000 / SIMULATION_TICK_MS as i32),
                y: step.y * (1_000 / SIMULATION_TICK_MS as i32),
            }
            .clamped();
        } else {
            drift_with_cause(state);
        }
    }
}

const ARRIVAL_DISTANCE: i32 = 220;
/// Cruise speed in fixed-point units per second; the tank is 10,000 units wide.
const APPROACH_SPEED: i32 = 4_200;
/// How quickly swimming speed can change, in units per second squared.
const SWIM_ACCELERATION: i32 = 16_000;
const FLEE_DISTANCE: i32 = 2_000;
/// `0.5^(tick / 1s)` in thousandths: free toys lose half their speed each second.
const TOY_DRAG_PER_TICK: i32 = 933;
/// How long a player's offer waits for a wedged toy to drift free before giving up.
const UNBLOCK_WAIT_MS: u64 = 2_000;
/// A nudged ball travels roughly a fifth of the tank before water drag stops it.
const BALL_NUDGE_SPEED: i32 = 1_500;

/// Displacement during the tick ending at `elapsed_after_ms` for a per-second velocity.
/// Differencing whole-second floors keeps slow motion exact instead of rounding it away.
fn tick_step(velocity: i32, elapsed_after_ms: u64) -> i32 {
    let after = i64::try_from(elapsed_after_ms).unwrap_or(i64::MAX / 16);
    let before = after - SIMULATION_TICK_MS as i64;
    let velocity = i64::from(velocity);
    ((velocity * after).div_euclid(1_000) - (velocity * before).div_euclid(1_000)) as i32
}

fn food_arrived_and_braked(state: &WorldState, food_id: Option<u64>) -> bool {
    let Some(id) = food_id else { return false };
    let Some(WorldObject::Food(food)) = state.aquarium.objects.get(&id) else {
        return false;
    };
    let position = state.creature.aquarium.position;
    // Stationary phases clear velocity, so reaching moving food is enough to begin inspecting.
    manhattan_distance(position, food.position) <= ARRIVAL_DISTANCE
}

fn steering_target(
    state: &mut WorldState,
    events: &mut Vec<GameEvent>,
) -> Option<NormalizedPosition> {
    if state.creature.aquarium.action.is_some() {
        return None;
    }
    if state.creature.aquarium.steering == SteeringMode::Flee
        && let Some(cursor) = state.aquarium.cursor
    {
        let position = state.creature.aquarium.position;
        if manhattan_distance(position, cursor) >= FLEE_DISTANCE {
            state.creature.aquarium.steering = SteeringMode::Hover;
            clear_travel_target(state);
            state.creature.aquarium.velocity = NormalizedVelocity::default();
            return None;
        }
        return Some(
            NormalizedPosition::new(
                position
                    .x
                    .saturating_add((position.x - cursor.x).signum() * FLEE_DISTANCE),
                position
                    .y
                    .saturating_add((position.y - cursor.y).signum() * FLEE_DISTANCE / 2),
            )
            .clamped(),
        );
    }
    let destination = state.creature.aquarium.destination?;
    let purpose = state.creature.aquarium.travel_purpose?;
    let Some(target) = approach_position(state, destination) else {
        if let SemanticDestination::Toy(toy) = destination {
            // A player's offer is worth a moment: nudge the wedged toy free and keep trying
            // briefly, so the click is never silently dropped.
            let now = state.elapsed_ms;
            if let crate::TravelPurpose::ToyInteraction { interaction_id } = purpose
                && let Some(interaction) = state
                    .creature
                    .interaction_state
                    .toy_interaction
                    .as_mut()
                    .filter(|interaction| {
                        interaction.id == interaction_id
                            && interaction.origin == crate::ToyOrigin::Player
                    })
            {
                if interaction.unblock_until_ms == 0 {
                    interaction.unblock_until_ms = now.saturating_add(UNBLOCK_WAIT_MS);
                    free_wedged_toy(state, toy);
                    return None;
                }
                if now < interaction.unblock_until_ms {
                    return None;
                }
            }
            free_wedged_toy(state, toy);
            // An obstructed contact is an interrupted approach, not a refusal or payoff.
            match purpose {
                crate::TravelPurpose::PrivateLife { .. } => {
                    interrupt_private_life(state, ActivityInterruptionOwner::Toy, events);
                }
                crate::TravelPurpose::ToyInteraction { .. }
                | crate::TravelPurpose::RefusalStare { .. } => {
                    interrupt_toy_interaction(state, events);
                }
                crate::TravelPurpose::Relationship { beat_id }
                    if state
                        .creature
                        .relationship_expression
                        .active
                        .as_ref()
                        .is_some_and(|beat| {
                            beat_id.get() == beat.started_at_ms.saturating_add(1)
                                && beat.target == Some(destination)
                        }) =>
                {
                    interrupt_relationship_beat(state, events);
                }
                _ => {}
            }
            interrupt_travel(state, events);
            set_intention(state, Intention::Idle, events);
            schedule_next_idle_bout(state);
        }
        return None;
    };
    // Contact and clear bottom resting points need precise arrival. The broad arrival
    // zone could otherwise begin a forage while the head is still inside a nearby toy.
    // Six fixed-point units only absorb integer ellipse projection rounding.
    let arrival_distance = if matches!(
        destination,
        SemanticDestination::Toy(_) | SemanticDestination::Bottom
    ) {
        6
    } else {
        ARRIVAL_DISTANCE
    };
    let neighbors_clear = match destination {
        SemanticDestination::Toy(toy) => {
            toy_contact_clears_neighbors(state, toy, state.creature.aquarium.position)
        }
        _ => true,
    };
    if manhattan_distance(state.creature.aquarium.position, target) <= arrival_distance
        && neighbors_clear
    {
        state.creature.aquarium.velocity = NormalizedVelocity::default();
        dispatch_travel_arrival(state, destination, purpose, events);
        clear_travel_target(state);
        state.creature.aquarium.steering = SteeringMode::Hover;
        return None;
    }
    Some(target)
}

/// Resolve a semantic destination against current authoritative objects and cursor state.
/// This is the object's anchor. Locomotion uses `approach_position` for physical contact.
pub fn destination_position(
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
        SemanticDestination::Bottom => Some(clear_bottom_position(state)),
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
        SemanticDestination::Toy(toy) => state
            .aquarium
            .toy_states
            .get(&toy)
            .map(|state| state.position)
            .or_else(|| {
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
            }),
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

/// The creature's head approaches a toy's surface, while gaze and object rendering retain
/// the real object anchor. The fixed-point envelope includes the head, fins and toy bounds
/// under the shared 13.2 by 4.55 world projection; the bell's float makes its top asymmetric.
pub fn approach_position(
    state: &WorldState,
    destination: SemanticDestination,
) -> Option<NormalizedPosition> {
    let anchor = destination_position(state, destination)?;
    let SemanticDestination::Toy(toy) = destination else {
        return Some(anchor);
    };
    let position = state.creature.aquarium.position;
    let preferred = toy_surface_position(
        state,
        position,
        anchor,
        toy_contact_radii(toy, position.y < anchor.y),
    );
    if toy_contact_clears_neighbors(state, toy, preferred) {
        return Some(preferred);
    }
    // Sample a fixed integer direction ring, not a path around obstacles. The maximum
    // angular gap is about 3.6 degrees; use each candidate's own asymmetric contact side.
    // Weighted Manhattan distance keeps the chosen alternative stable as axis-clamped
    // steering moves toward it. Once the ordinary surface becomes clear, preserve it.
    (0..128)
        .map(|index| {
            let along = (index % 32) * 2 - 32;
            let (x, y) = match index / 32 {
                0 => (32, along),
                1 => (-along, 32),
                2 => (-32, -along),
                _ => (along, -32),
            };
            let radii = toy_contact_radii(toy, y < 0);
            let ray = NormalizedPosition::new(anchor.x + x * radii.0, anchor.y + y * radii.1);
            let distance = toy_ellipse_distance(ray, anchor, radii);
            NormalizedPosition::new(
                anchor.x + (ray.x - anchor.x) * 1_000 / distance,
                anchor.y + (ray.y - anchor.y) * 1_000 / distance,
            )
        })
        .filter(|candidate| {
            *candidate == candidate.clamped()
                && toy_contact_clears_neighbors(state, toy, *candidate)
        })
        .min_by_key(|candidate| {
            (
                (candidate.x - position.x).abs() * 3 + (candidate.y - position.y).abs(),
                candidate.x,
                candidate.y,
            )
        })
}

fn toy_contact_clears_neighbors(
    state: &WorldState,
    intended: ToyId,
    position: NormalizedPosition,
) -> bool {
    state.aquarium.toy_states.iter().all(|(toy, object)| {
        *toy == intended
            || object.carried
            || toy_ellipse_distance(
                position,
                object.position,
                toy_rest_radii(*toy, position.y < object.position.y),
            ) >= 1_000
    })
}

/// Exact authoritative locomotion bounds for continuous presentation, including the small
/// retreat after toy contact. Reading this target never starts or resolves an interaction.
pub fn movement_target(state: &WorldState) -> Option<NormalizedPosition> {
    if holding_toy_contact(state) {
        return Some(state.creature.aquarium.position);
    }
    if let Some(destination) = state
        .creature
        .aquarium
        .action
        .as_ref()
        .map(|action| action.destination)
        .or(state.creature.aquarium.destination)
    {
        return approach_position(state, destination);
    }
    toy_rest_target(state, state.creature.aquarium.position)
}

fn toy_contact_radii(toy: ToyId, above: bool) -> (i32, i32) {
    let vertical = match (toy, above) {
        // The chin can meet the ball closely from above; the crest needs room below it.
        (ToyId::Ball, true) => 2_200,
        (ToyId::Ball, false) => 2_800,
        (ToyId::Bell, true) => 3_400,
        (ToyId::Bell, false) => 2_800,
        (ToyId::Sock, true) => 1_800,
        (ToyId::Sock, false) => 3_400,
    };
    (1_000, vertical)
}

fn toy_rest_radii(toy: ToyId, above: bool) -> (i32, i32) {
    let (x, y) = toy_contact_radii(toy, above);
    (x + 120, y + 250)
}

fn clear_bottom_position(state: &WorldState) -> NormalizedPosition {
    let preferred = NormalizedPosition::new(5_000, 9_200);
    let toy_sides = state
        .aquarium
        .toy_states
        .iter()
        .filter(|(_, object)| !object.carried)
        .flat_map(|(toy, object)| {
            let (horizontal, _) = toy_rest_radii(*toy, preferred.y < object.position.y);
            [
                object.position.x - horizontal,
                object.position.x + horizontal,
            ]
        });
    // Only the resting destination moves. Travel can still cross objects on its way there.
    // A toy's full horizontal radius supplies a clear candidate at any bottom height.
    [preferred.x, 0, 10_000]
        .into_iter()
        .chain(toy_sides)
        .filter(|x| (0..=10_000).contains(x))
        .map(|x| NormalizedPosition::new(x, preferred.y))
        .filter(|position| {
            state.aquarium.toy_states.iter().all(|(toy, object)| {
                object.carried
                    || toy_ellipse_distance(
                        *position,
                        object.position,
                        toy_rest_radii(*toy, position.y < object.position.y),
                    ) >= 1_000
            })
        })
        .min_by_key(|position| ((position.x - preferred.x).abs(), position.x))
        // Three toys can cover at most 3 * 2 * 1_120 of the 10_000-wide tank.
        .expect("the three toy envelopes leave a clear bottom column")
}

fn toy_ellipse_distance(
    position: NormalizedPosition,
    anchor: NormalizedPosition,
    radii: (i32, i32),
) -> i32 {
    let x = i64::from(position.x - anchor.x) * 1_000 / i64::from(radii.0);
    let y = i64::from(position.y - anchor.y) * 1_000 / i64::from(radii.1);
    ((x * x + y * y) as u64).isqrt() as i32
}

fn toy_surface_position(
    state: &WorldState,
    position: NormalizedPosition,
    anchor: NormalizedPosition,
    radii: (i32, i32),
) -> NormalizedPosition {
    let distance = toy_ellipse_distance(position, anchor, radii);
    let target = if distance == 0 {
        let side = if state.creature.aquarium.facing == crate::Facing::Left {
            1
        } else {
            -1
        };
        NormalizedPosition::new(anchor.x + side * radii.0, anchor.y)
    } else {
        NormalizedPosition::new(
            anchor.x + (position.x - anchor.x) * 1_000 / distance,
            anchor.y + (position.y - anchor.y) * 1_000 / distance,
        )
    };
    if target == target.clamped() {
        return target;
    }
    // Clamping a surface point at a tank edge would put the head inside the toy again.
    // Choose the nearest feasible cardinal surface instead; a horizontal side always fits.
    [
        NormalizedPosition::new(anchor.x - radii.0, anchor.y),
        NormalizedPosition::new(anchor.x + radii.0, anchor.y),
        NormalizedPosition::new(anchor.x, anchor.y - radii.1),
        NormalizedPosition::new(anchor.x, anchor.y + radii.1),
    ]
    .into_iter()
    .filter(|candidate| *candidate == candidate.clamped())
    .min_by_key(|candidate| (candidate.x - position.x).abs() * 3 + (candidate.y - position.y).abs())
    .expect("a toy always has an in-tank surface")
}

fn holding_toy_contact(state: &WorldState) -> bool {
    state
        .creature
        .private_life
        .active
        .as_ref()
        .is_some_and(|activity| {
            activity.phase == ActivityPhase::Act
                && matches!(activity.kind, PrivateLifeKind::ToyPlay(_))
        })
}

fn toy_rest_target(state: &WorldState, position: NormalizedPosition) -> Option<NormalizedPosition> {
    state
        .aquarium
        .toy_states
        .iter()
        .filter(|(_, object)| !object.carried)
        .filter_map(|(toy, object)| {
            let radii = toy_rest_radii(*toy, position.y < object.position.y);
            let distance = toy_ellipse_distance(position, object.position, radii);
            (distance < 1_000).then(|| {
                (
                    distance,
                    toy_surface_position(state, position, object.position, radii),
                )
            })
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, target)| target)
}

/// Where hand-fed food enters the water: a little above and ahead of the creature's face.
#[must_use]
pub fn feeding_position(state: &WorldState) -> NormalizedPosition {
    let head = state.creature.aquarium.position;
    // Lean toward open water so the creature turns into the tank rather than toward the glass.
    let inward = if head.x > 5_000 { -500 } else { 500 };
    NormalizedPosition::new(head.x + inward, head.y - 900).clamped()
}

/// Shared carried-object anchor, including the aquarium boundary at the floor.
pub fn held_toy_position(head: NormalizedPosition) -> NormalizedPosition {
    // The mouth hold is 0.34 world units below the head, matching the rendered carry pose.
    NormalizedPosition::new(head.x, head.y + 750).clamped()
}

fn release_sock(state: &mut WorldState) {
    let position = held_toy_position(state.creature.aquarium.position);
    if let Some(sock) = state.aquarium.toy_states.get_mut(&ToyId::Sock)
        && sock.carried
    {
        sock.carried = false;
        sock.position = position;
        sock.velocity = NormalizedVelocity {
            x: 0,
            y: SOCK_RELEASE_SPEED,
        };
    }
}

/// Swim toward `target` with a quick, bounded acceleration and an eased arrival.
/// Velocity is stored in units per second; the final step snaps exactly onto the target.
fn steer_toward(state: &mut WorldState, target: NormalizedPosition) {
    let position = state.creature.aquarium.position;
    let elapsed = state.elapsed_ms;
    let speed = if state.creature.aquarium.steering == SteeringMode::Flee {
        APPROACH_SPEED.saturating_mul(3) / 2
    } else {
        APPROACH_SPEED
    };
    let accel = tick_step(SWIM_ACCELERATION, elapsed).max(1);
    let axis = |delta: i32, current: i32, max: i32| -> (i32, i32) {
        if delta == 0 {
            return (0, 0);
        }
        // Ease in over the last stretch, but never so slowly that arrival stalls.
        let desired = delta.signum() * (delta.abs().saturating_mul(5)).clamp(900, max);
        let velocity = desired.clamp(current - accel, current + accel);
        // Never overshoot: a step that would pass the target lands on it.
        let step = tick_step(velocity, elapsed);
        if step.abs() >= delta.abs() || step == 0 && delta.abs() <= 12 {
            (delta, velocity)
        } else {
            (step, velocity)
        }
    };
    let current = state.creature.aquarium.velocity;
    let (step_x, velocity_x) = axis(target.x - position.x, current.x, speed);
    // The tank is shallower than it is wide, so vertical travel uses a lower cap.
    let (step_y, velocity_y) = axis(target.y - position.y, current.y, speed * 3 / 4);
    state.creature.aquarium.position = NormalizedPosition::new(
        position.x.saturating_add(step_x),
        position.y.saturating_add(step_y),
    )
    .clamped();
    state.creature.aquarium.velocity = NormalizedVelocity {
        x: if step_x == 0 { 0 } else { velocity_x },
        y: if step_y == 0 { 0 } else { velocity_y },
    }
    .clamped();
    if step_x.abs() > 2 {
        state.creature.aquarium.facing = if step_x < 0 {
            crate::Facing::Left
        } else {
            crate::Facing::Right
        };
    }
}

fn drift_with_cause(state: &mut WorldState) {
    // Drift velocity changes once per second; each tick moves a tenth of it.
    let key = state.elapsed_ms / 1_000;
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
    let elapsed = state.elapsed_ms;
    let proposed = NormalizedPosition::new(
        position.x.saturating_add(tick_step(dx, elapsed)),
        position.y.saturating_add(tick_step(dy, elapsed)),
    )
    .clamped();
    if toy_rest_target(state, proposed).is_some() {
        state.creature.aquarium.velocity = NormalizedVelocity::default();
    } else {
        state.creature.aquarium.position = proposed;
        state.creature.aquarium.velocity = NormalizedVelocity { x: dx, y: dy }.clamped();
    }
}

fn choose_idle_behavior(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    if state.creature.aquarium.action.is_some()
        || state.creature.interaction_state.toy_interaction.is_some()
        || state.creature.private_life.active.is_some()
        || state.creature.relationship_expression.active.is_some()
        || matches!(state.creature.aquarium.steering, SteeringMode::Flee)
    {
        return;
    }
    if state.creature.aquarium.destination.is_some() {
        return;
    }
    if state.creature.hidden_until_met {
        // Shy at first: it peeks from the cave, then comes to the glass to see who is there,
        // sooner if the player does anything at all.
        let counters = state.creature.development.interactions;
        let waited = state
            .creature
            .met_player_at_ms
            .is_some_and(|at| state.elapsed_ms.saturating_sub(at) >= FIRST_MEETING_PEEK_MS);
        let met = waited || counters.total() > 0 || counters.requests > 0 || counters.taps > 0;
        if !met {
            state.creature.aquarium.steering = SteeringMode::Hover;
            state.creature.aquarium.velocity = NormalizedVelocity::default();
            return;
        }
        state.creature.hidden_until_met = false;
        let action_id =
            NonZeroU64::new(allocate_action_id(state)).expect("action IDs start at one");
        set_travel_target(
            state,
            SemanticDestination::Position(NormalizedPosition::new(5_000, 4_200)),
            crate::TravelPurpose::CursorSocial { action_id },
            events,
        );
        state.creature.aquarium.gaze = GazeTarget::Player;
        set_intention(state, Intention::ApproachPlayer, events);
        // Stay at the glass a moment to look at the newcomer before going about its life.
        state.creature.idle_life.settled_until_ms =
            state.elapsed_ms.saturating_add(FIRST_MEETING_LINGER_MS);
        events.push(GameEvent::Emerged);
        return;
    }
    if state.elapsed_ms < state.creature.idle_life.settled_until_ms {
        state.creature.aquarium.steering = SteeringMode::Hover;
        glance_while_pausing(state);
        return;
    }
    let quiet_moment = state.aquarium.player_present
        && state.creature.initiated_behavior.is_none()
        && state.creature.current_intention == Intention::Idle
        && state.creature.needs.hunger < 0.78
        && state.creature.needs.energy >= 0.2
        && state.creature.needs.comfort >= 0.2;
    if quiet_moment {
        maybe_start_relationship_beat(state, RelationshipTrigger::QuietMoment, events);
        if state.creature.relationship_expression.active.is_some() {
            return;
        }
    }
    // Relationship callbacks get a bounded, explicitly grounded opportunity before the creature
    // falls back to private life. The relationship director's cooldown/ledger remains the source
    // of suppression, so a newly formed memory cannot create a receipt-like interruption.
    if start_private_life(state, events) {
        return;
    }
    let hour = active_day_hour(state);
    let routine = state
        .creature
        .routines
        .iter()
        .filter(|routine| routine.hour_start == hour)
        .max_by_key(|routine| routine.strength)
        .map(|routine| routine.destination);
    if let Some(destination) = routine {
        maybe_start_relationship_beat(
            state,
            RelationshipTrigger::RoutineWindow {
                hour_start: hour,
                destination,
            },
            events,
        );
        if state.creature.relationship_expression.active.is_some() {
            return;
        }
    }
    let favorite = state.favorite_destination();
    let (destination, avoid_repeat) = if state.creature.needs.energy < 0.2 {
        (Some(SemanticDestination::Cave), false)
    } else if state.creature.needs.hunger > 0.78 {
        (Some(SemanticDestination::Bottom), false)
    } else if let Some(routine) = routine {
        (Some(routine), false)
    } else if let Some(favorite) = favorite.filter(|_| {
        state.creature.current_intention == Intention::Idle
            && state.elapsed_ms % 8_000 < SIMULATION_TICK_MS
    }) {
        (Some(favorite), true)
    } else if state.creature.needs.curiosity > 0.65 {
        (
            Some(if state.creature.traits.fussiness > 0.6 {
                SemanticDestination::Plant
            } else {
                SemanticDestination::Toy(preferred_toy(state))
            }),
            true,
        )
    } else {
        (Some(identity_shaped_idle_destination(state)), true)
    };
    if let Some(destination) = destination.map(|destination| {
        if avoid_repeat {
            avoid_immediate_repeat(state, destination)
        } else {
            destination
        }
    }) {
        match destination {
            SemanticDestination::Toy(toy) => {
                let interaction_id = allocate_toy_interaction_id(state);
                state.creature.interaction_state.toy_interaction = Some(crate::ToyInteraction {
                    id: interaction_id,
                    toy,
                    origin: crate::ToyOrigin::Autonomous,
                    outcome: crate::ToyInteractionOutcome::Accepted,
                    phase: crate::ToyInteractionPhase::Approach,
                    relationship: None,
                    recovery_until_ms: 0,
                    rounds_left: toy_session_rounds(toy, crate::ToyOrigin::Autonomous),
                    contacts: 0,
                    unblock_until_ms: 0,
                });
                set_travel_target(
                    state,
                    destination,
                    crate::TravelPurpose::ToyInteraction { interaction_id },
                    events,
                );
                events.push(GameEvent::ToyPlayAccepted {
                    toy,
                    interaction_id,
                    origin: crate::ToyOrigin::Autonomous,
                });
            }
            _ => {
                let visit_id =
                    NonZeroU64::new(allocate_action_id(state)).expect("action IDs start at one");
                set_travel_target(
                    state,
                    destination,
                    crate::TravelPurpose::IdleVisit { visit_id },
                    events,
                );
            }
        }
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

/// A pausing creature is still looking at things: the player now and then, or a nearby toy.
/// Gaze only; it never starts travel, so the pause stays a pause.
fn glance_while_pausing(state: &mut WorldState) {
    if state.creature.aquarium.gaze == GazeTarget::Cursor {
        return;
    }
    let beat = state.elapsed_ms / 1_400;
    if state.elapsed_ms % 1_400 >= SIMULATION_TICK_MS {
        return;
    }
    let pick = deterministic_unit(state.seed, RandomDomain::Motion, beat.wrapping_mul(13));
    let head = state.creature.aquarium.position;
    let nearest_toy = state
        .aquarium
        .toy_states
        .iter()
        .filter(|(_, object)| !object.carried)
        .min_by_key(|(_, object)| manhattan_distance(head, object.position))
        .map(|(toy, _)| *toy);
    let social = state.aquarium.player_present && state.creature.relationship.bond > 0.05;
    state.creature.aquarium.gaze = match (pick, nearest_toy) {
        (pick, _) if social && pick < 0.45 => GazeTarget::Player,
        (pick, Some(toy)) if pick < 0.8 => GazeTarget::Toy(toy),
        _ => GazeTarget::None,
    };
}

fn start_private_life(state: &mut WorldState, events: &mut Vec<GameEvent>) -> bool {
    let candidates = [
        PrivateLifeKind::CaveSettle,
        PrivateLifeKind::PlantInspect,
        PrivateLifeKind::BottomForage,
        PrivateLifeKind::OpenWaterDrift,
        PrivateLifeKind::ToyPlay(ToyId::Ball),
        PrivateLifeKind::ToyPlay(ToyId::Bell),
        PrivateLifeKind::ToyPlay(ToyId::Sock),
    ];
    let recent = state.creature.private_life.recent.clone();
    let last = recent.last();
    let has_recent_kind = |kind| recent.iter().rev().take(3).any(|entry| entry.kind == kind);
    let has_recent_subject = |subject| {
        recent
            .iter()
            .rev()
            .take(2)
            .any(|entry| entry.subject == Some(subject))
    };
    let has_recent_recipe = |recipe| {
        recent
            .iter()
            .rev()
            .take(4)
            .any(|entry| entry.recipe == recipe)
    };
    let recipe_for = |kind| match kind {
        PrivateLifeKind::ToyPlay(ToyId::Ball) => ActivityRecipe::BallNudge,
        PrivateLifeKind::ToyPlay(ToyId::Bell) => ActivityRecipe::BellStrike,
        PrivateLifeKind::ToyPlay(ToyId::Sock) => ActivityRecipe::SockTug,
        PrivateLifeKind::CaveSettle => ActivityRecipe::CaveShelter,
        PrivateLifeKind::PlantInspect => ActivityRecipe::PlantOrbit,
        PrivateLifeKind::BottomForage => ActivityRecipe::BottomForage,
        PrivateLifeKind::OpenWaterDrift => ActivityRecipe::OpenWaterDrift,
    };
    let need_pressure = if state.creature.needs.energy < 0.28 {
        100
    } else if state.creature.needs.hunger > 0.75 {
        95
    } else {
        (state.creature.needs.curiosity * 70.0) as u8
    };
    let urgent_kind = if state.creature.needs.energy < 0.28 {
        Some(PrivateLifeKind::CaveSettle)
    } else if state.creature.needs.hunger > 0.75 {
        Some(PrivateLifeKind::BottomForage)
    } else {
        None
    };
    let mut excluded_families = Vec::new();
    let mut excluded_subjects = Vec::new();
    let mut excluded_recipes = Vec::new();
    let scored = candidates
        .into_iter()
        .map(|kind| {
            let subject = kind.subject();
            let recipe = recipe_for(kind);
            let mut score = match kind {
                PrivateLifeKind::CaveSettle => (1.0 - state.creature.needs.energy) * 90.0,
                PrivateLifeKind::PlantInspect => {
                    state.creature.needs.curiosity * 70.0 + state.creature.traits.fussiness * 20.0
                }
                PrivateLifeKind::BottomForage => state.creature.needs.hunger * 85.0,
                PrivateLifeKind::OpenWaterDrift => {
                    (1.0 - state.creature.needs.curiosity) * 48.0
                        + (1.0 - state.creature.needs.energy) * 15.0
                }
                PrivateLifeKind::ToyPlay(toy) => {
                    state.creature.needs.curiosity * 60.0
                        + state
                            .creature
                            .toy_preferences
                            .get(&toy)
                            .copied()
                            .unwrap_or(0.0)
                            * 55.0
                        + state.creature.traits.boldness * 14.0
                }
            } as i32;
            if has_recent_kind(kind) {
                excluded_families.push(kind);
                score -= 20;
            }
            if has_recent_subject(subject) {
                excluded_subjects.push(subject);
                score -= 12;
            }
            if has_recent_recipe(recipe) {
                excluded_recipes.push(recipe);
                score -= 10;
            }
            if last.is_some_and(|entry| entry.kind == kind) {
                score -= 75;
            }
            if urgent_kind == Some(kind) {
                score += 150;
            }
            if state.creature.routines.iter().any(|routine| {
                routine.hour_start == active_day_hour(state)
                    && kind.destination() == Some(routine.destination)
            }) {
                score += 170;
            }
            (kind, recipe, score)
        })
        .collect::<Vec<_>>();
    let dominant = scored
        .iter()
        .max_by_key(|(kind, _, score)| (*score, std::cmp::Reverse(*kind)))
        .copied()
        .filter(|(_, _, score)| *score >= 80);
    let selected = if let Some(urgent) = urgent_kind {
        scored
            .iter()
            .find(|candidate| candidate.0 == urgent)
            .copied()
    } else if dominant.is_some() {
        dominant
    } else {
        // Weighted deterministic choice makes state evidence shape likelihood without sorting
        // quiet life into a fixed highest-score tour. The exact previous activity remains the
        // sole hard exclusion; other recent evidence is a soft bias, so habits can recur without
        // becoming either a metronome or a seven-item checklist.
        let weighted = scored
            .iter()
            .map(|candidate| {
                let weight = if last.is_some_and(|entry| entry.kind == candidate.0) {
                    0
                } else {
                    u32::try_from(candidate.2.max(1)).unwrap_or(1)
                };
                (*candidate, weight)
            })
            .collect::<Vec<_>>();
        let total = weighted.iter().map(|(_, weight)| *weight).sum::<u32>();
        let mut draw = ((state.domain_draw(RandomDomain::Environment) * total as f32) as u32)
            .min(total.saturating_sub(1));
        weighted.into_iter().find_map(|(candidate, weight)| {
            if draw < weight {
                Some(candidate)
            } else {
                draw = draw.saturating_sub(weight);
                None
            }
        })
    };
    let Some((kind, recipe, _)) = selected else {
        return false;
    };
    let raw = state.creature.private_life.next_activity_id.max(1);
    state.creature.private_life.next_activity_id = raw.saturating_add(1).max(1);
    let id = NonZeroU64::new(raw).expect("private-life activity IDs start at one");
    let purpose = if urgent_kind == Some(kind) {
        ActivityPurpose::NeedUrgency
    } else if state.creature.routines.iter().any(|routine| {
        routine.destination
            == kind.destination().unwrap_or(SemanticDestination::Position(
                state.creature.aquarium.position,
            ))
    }) {
        ActivityPurpose::Routine
    } else if matches!(kind, PrivateLifeKind::ToyPlay(toy) if state.creature.toy_preferences.get(&toy).copied().unwrap_or_default() > 0.25)
    {
        ActivityPurpose::Preference
    } else {
        ActivityPurpose::Autonomous
    };
    let bubble = (kind == PrivateLifeKind::OpenWaterDrift).then(|| nearby_bubble(state, id));
    let activity = PrivateLifeActivity {
        bubble,
        id,
        kind,
        subject: Some(kind.subject()),
        purpose,
        recipe,
        phase: ActivityPhase::Notice,
        selected_at_ms: state.elapsed_ms,
        phase_started_at_ms: state.elapsed_ms,
        selected_from: ActivitySelectionEvidence {
            need_pressure,
            trait_bias: (state.creature.traits.boldness * 100.0) as u8,
            preference: match kind {
                PrivateLifeKind::ToyPlay(toy) => {
                    (state
                        .creature
                        .toy_preferences
                        .get(&toy)
                        .copied()
                        .unwrap_or_default()
                        * 100.0) as i8
                }
                _ => 0,
            },
            routine_hour: Some(active_day_hour(state)),
            relationship_evidence: Vec::new(),
            excluded_families,
            excluded_subjects,
            excluded_recipes,
            urgency_overrode_repetition: urgent_kind == Some(kind) && has_recent_kind(kind),
        },
        payoff_reached: false,
    };
    state.creature.private_life.active = Some(activity.clone());
    state.creature.aquarium.steering = SteeringMode::Hover;
    events.push(GameEvent::PrivateLifeStarted {
        activity_id: id,
        kind,
        recipe,
    });
    true
}

fn active_day_hour(state: &WorldState) -> u8 {
    ((state.elapsed_ms % crate::ACTIVE_DAY_MS) * ACTIVE_DAY_HOURS / crate::ACTIVE_DAY_MS) as u8
}

fn identity_shaped_idle_destination(state: &mut WorldState) -> SemanticDestination {
    let preferred = preferred_toy(state);
    let alternatives = [
        SemanticDestination::Cave,
        SemanticDestination::Plant,
        SemanticDestination::Toy(preferred),
        SemanticDestination::Toy(if state.creature.traits.fussiness > 0.55 {
            ToyId::Bell
        } else {
            ToyId::Sock
        }),
    ];
    let identity_bias = ((state.creature.traits.boldness * 3.0) as usize
        + (state.creature.traits.sociability * 2.0) as usize)
        % alternatives.len();
    let variation =
        (state.domain_draw(RandomDomain::Environment) * alternatives.len() as f32) as usize;
    alternatives[(identity_bias + variation) % alternatives.len()]
}

fn avoid_immediate_repeat(
    state: &mut WorldState,
    requested: SemanticDestination,
) -> SemanticDestination {
    if state.creature.idle_life.last_arrived_destination != Some(requested) {
        return requested;
    }
    let alternatives = [
        SemanticDestination::Cave,
        SemanticDestination::Plant,
        SemanticDestination::Toy(preferred_toy(state)),
        SemanticDestination::Toy(ToyId::Bell),
        SemanticDestination::Bottom,
    ];
    // Rotate the fallback from accumulated visits instead of consuming another random draw. The
    // requested destination was already selected randomly; advancing the environment stream again
    // here would make an immediate-repeat guard perturb unrelated future behavior.
    let completed_visits = state
        .creature
        .favorite_locations
        .values()
        .copied()
        .sum::<u32>() as usize;
    let identity_bias = ((state.creature.traits.boldness * 7.0) as usize
        + (state.creature.traits.sociability * 5.0) as usize)
        % alternatives.len();
    let start = (completed_visits + identity_bias) % alternatives.len();
    (0..alternatives.len())
        .map(|offset| alternatives[(start + offset) % alternatives.len()])
        .find(|candidate| *candidate != requested)
        .unwrap_or(requested)
}

fn record_idle_arrival_state(state: &mut WorldState, destination: SemanticDestination) {
    if !matches!(
        destination,
        SemanticDestination::Cave
            | SemanticDestination::Plant
            | SemanticDestination::Bottom
            | SemanticDestination::Toy(_)
    ) {
        return;
    }
    state.record_favorite(destination);
    state.creature.idle_life.last_arrived_destination = Some(destination);
    schedule_next_idle_bout(state);
    record_routine_visit(state, destination);
}

fn schedule_next_idle_bout(state: &mut WorldState) {
    let spans = IDLE_BOUT_MAX_MS / SIMULATION_TICK_MS - IDLE_BOUT_MIN_MS / SIMULATION_TICK_MS + 1;
    let duration = IDLE_BOUT_MIN_MS
        + (state.domain_draw(RandomDomain::Environment) * spans as f32) as u64 * SIMULATION_TICK_MS;
    state.creature.idle_life.settled_until_ms = state.elapsed_ms.saturating_add(duration);
}

fn record_routine_visit(state: &mut WorldState, destination: SemanticDestination) {
    let hour_start = active_day_hour(state);
    let active_day = state.active_day();
    let visits = if let Some(evidence) = state
        .creature
        .idle_life
        .visit_evidence
        .iter_mut()
        .find(|evidence| evidence.hour_start == hour_start && evidence.destination == destination)
    {
        if evidence.last_active_day != active_day {
            evidence.visits = evidence.visits.saturating_add(1);
            evidence.last_active_day = active_day;
        }
        evidence.visits
    } else {
        if state.creature.idle_life.visit_evidence.len() >= MAX_VISIT_EVIDENCE {
            state.creature.idle_life.visit_evidence.remove(0);
        }
        state
            .creature
            .idle_life
            .visit_evidence
            .push(crate::VisitEvidence {
                hour_start,
                destination,
                visits: 1,
                last_active_day: active_day,
            });
        state
            .creature
            .idle_life
            .visit_evidence
            .sort_by_key(|evidence| (evidence.hour_start, evidence.destination));
        1
    };
    if visits >= 2 {
        state.set_routine(crate::Routine {
            hour_start,
            destination,
            strength: visits.min(5),
        });
    }
}

pub(crate) fn play_with_toy(
    state: &mut WorldState,
    toy: ToyId,
    relationship: Option<crate::ActionRelationshipContext>,
    events: &mut Vec<GameEvent>,
) {
    if destination_position(state, SemanticDestination::Toy(toy)).is_none() {
        return;
    }
    // A new offer replaces the old owner, so finish any committed carry first.
    end_toy_recovery(state);
    let preference = *state
        .creature
        .toy_preferences
        .entry(toy)
        .or_insert_with(|| {
            deterministic_unit(state.seed, RandomDomain::Preferences, toy_key(toy))
                .mul_add(2.0, -1.0)
        });
    state.creature.aquarium.gaze = GazeTarget::Toy(toy);
    let interaction_id = allocate_toy_interaction_id(state);
    if preference < -0.35 {
        state.creature.interaction_state.toy_interaction = Some(crate::ToyInteraction {
            id: interaction_id,
            toy,
            origin: crate::ToyOrigin::Player,
            outcome: crate::ToyInteractionOutcome::Rejected,
            phase: crate::ToyInteractionPhase::Approach,
            relationship: None,
            recovery_until_ms: 0,
            rounds_left: 0,
            contacts: 0,
            unblock_until_ms: 0,
        });
        set_travel_target(
            state,
            SemanticDestination::Toy(toy),
            crate::TravelPurpose::RefusalStare { interaction_id },
            events,
        );
        set_intention(state, Intention::RefuseAndStare, events);
        let memory = state.remember(MemoryKind::DislikedToy { toy }, &[Concept::Bad], -0.5, 0.7);
        state.revise_belief(BeliefKind::ToyIsJealous, memory, true);
        events.push(GameEvent::ToyRejected {
            toy,
            interaction_id,
            origin: crate::ToyOrigin::Player,
        });
        events.push(GameEvent::NonverbalAct(NonverbalAct::TakeToyAway(toy)));
    } else {
        state.creature.interaction_state.toy_interaction = Some(crate::ToyInteraction {
            id: interaction_id,
            toy,
            origin: crate::ToyOrigin::Player,
            outcome: crate::ToyInteractionOutcome::Accepted,
            phase: crate::ToyInteractionPhase::Approach,
            relationship,
            recovery_until_ms: 0,
            rounds_left: toy_session_rounds(toy, crate::ToyOrigin::Player),
            contacts: 0,
            unblock_until_ms: 0,
        });
        set_travel_target(
            state,
            SemanticDestination::Toy(toy),
            crate::TravelPurpose::ToyInteraction { interaction_id },
            events,
        );
        set_intention(state, Intention::Play, events);
        events.push(GameEvent::ToyPlayAccepted {
            toy,
            interaction_id,
            origin: crate::ToyOrigin::Player,
        });
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

pub(crate) fn set_intention(
    state: &mut WorldState,
    intention: Intention,
    events: &mut Vec<GameEvent>,
) {
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

fn resolve_food(
    state: &mut WorldState,
    food_id: Option<u64>,
    events: &mut Vec<GameEvent>,
) -> Option<FoodOutcome> {
    let id = food_id?;
    let Some(WorldObject::Food(object)) = state.aquarium.objects.get(&id).cloned() else {
        return None;
    };
    let food = object.food;
    let seed = state.seed;
    let preference = *state.creature.preferences.entry(food).or_insert_with(|| {
        deterministic_unit(seed, RandomDomain::Preferences, id).mul_add(2.0, -1.0)
    });
    if preference < -0.35 {
        if let Some(WorldObject::Food(object)) = state.aquarium.objects.get_mut(&id) {
            object.disposition = FoodDisposition::Rejected;
            // Spat-out food dissolves soon rather than haunting the tank.
            object.lifetime_ms = object.lifetime_ms.min(object.age_ms.saturating_add(15_000));
        }
        let memory = state.remember(
            MemoryKind::RejectedFood { food },
            &[Concept::Food, Concept::Bad, Concept::Again],
            -0.75,
            0.85,
        );
        state.revise_belief(BeliefKind::FoodIsATrick, memory, true);
        state.creature.relationship.resentment += 0.035;
        events.push(GameEvent::FoodRejected(food));
        events.push(GameEvent::NonverbalAct(NonverbalAct::PushFoodAway(food)));
        Some(FoodOutcome::Rejected)
    } else {
        state.aquarium.objects.remove(&id);
        state.aquarium.object_names.remove(&id);
        state.creature.needs.hunger -= 0.45;
        improve_relationship(state, 0.012, 0.009, 0.018);
        let memory = state.remember(
            MemoryKind::WasFed { food },
            &[Concept::Food, Concept::You],
            preference,
            0.75,
        );
        state.revise_belief(BeliefKind::FoodIsATrick, memory, false);
        clear_satisfied_or_expired_initiative(state);
        events.push(GameEvent::FoodConsumed(food));
        Some(FoodOutcome::Consumed)
    }
}

/// Name a thing it is enjoying, using the player's word for it: the payoff of teaching. Rare
/// enough to stay a small surprise.
fn maybe_remark(state: &mut WorldState, first: usize, events: &mut Vec<GameEvent>) {
    if !state.aquarium.player_present {
        return;
    }
    let last = state.creature.conversation.last_remark_ms;
    if last != 0 && state.elapsed_ms.saturating_sub(last) < REMARK_INTERVAL_MS {
        return;
    }
    let subject = events[first..].iter().find_map(|event| match event {
        GameEvent::ToyPlayed { toy, .. } | GameEvent::ToyObjectResponded { toy, .. } => {
            Some(crate::Meaning::Toy(*toy))
        }
        GameEvent::FoodConsumed(food) => Some(crate::Meaning::Food(*food)),
        _ => None,
    });
    let Some((meaning, word)) = subject.and_then(|meaning| {
        state
            .creature
            .lexicon
            .word_for(meaning)
            .map(|word| (meaning, word.to_owned()))
    }) else {
        return;
    };
    // Novelty wears off: each time it has named a thing, the next naming of that thing waits
    // longer, so a fresh word is said with delight and an old one only now and then.
    let spoken = state
        .creature
        .lexicon
        .words
        .get(&word)
        .map_or(0, |knowledge| knowledge.spoken);
    let wait = REMARK_INTERVAL_MS.saturating_mul(1 + u64::from(spoken.min(6)));
    if last != 0 && state.elapsed_ms.saturating_sub(last) < wait {
        return;
    }
    state.creature.lexicon.note_spoken(&word);
    state.creature.conversation.last_remark_ms = state.elapsed_ms.max(1);
    events.push(GameEvent::Remarked(meaning));
}

/// Ask for what it wants, out loud, at a sociable pace: often enough to be heard, never while
/// busy, and sooner when it has learned a word for the thing it wants.
fn maybe_initiate(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    if state.creature.initiated_behavior.is_some()
        || !state.aquarium.player_present
        || creature_is_occupied(state)
    {
        return;
    }
    let Some(want) = crate::current_want(state) else {
        return;
    };
    let reason = match want {
        crate::Want::Food(_) => InitiativeReason::Hunger,
        crate::Want::Company => InitiativeReason::Loneliness,
        crate::Want::Toy(_) => InitiativeReason::Curiosity,
        crate::Want::Sleep => InitiativeReason::Ritual,
        crate::Want::NameOf(_) => return,
    };
    let interval = if state.creature.lexicon.word_for(want.meaning()).is_some() {
        ASK_WITH_WORD_INTERVAL_MS
    } else {
        ASK_WITHOUT_WORD_INTERVAL_MS
    };
    let last = state.creature.conversation.last_asked_ms;
    if last != 0 && state.elapsed_ms.saturating_sub(last) < interval {
        return;
    }
    state.creature.initiated_behavior = Some(crate::InitiatedBehavior {
        reason,
        nonverbal: (want == crate::Want::Company).then_some(NonverbalAct::LeanAgainstPlayer),
        requested_at_ms: state.elapsed_ms,
        expires_at_ms: state.elapsed_ms.saturating_add(INITIATIVE_DURATION_MS),
    });
    state.creature.conversation.last_asked_ms = state.elapsed_ms.max(1);
    events.push(GameEvent::InitiatedTalk(reason));
}

fn clear_satisfied_or_expired_initiative(state: &mut WorldState) {
    let Some(initiated) = state.creature.initiated_behavior.as_ref() else {
        return;
    };
    let expired_at = if initiated.expires_at_ms == 0 {
        initiated
            .requested_at_ms
            .saturating_add(INITIATIVE_DURATION_MS)
    } else {
        initiated.expires_at_ms
    };
    let satisfied = match initiated.reason {
        InitiativeReason::Hunger => state.creature.needs.hunger < 0.65,
        InitiativeReason::Loneliness => state.creature.needs.comfort > 0.35,
        InitiativeReason::Curiosity => state.creature.needs.curiosity < 0.45,
        InitiativeReason::Ritual | InitiativeReason::Request => false,
    };
    if satisfied || state.elapsed_ms >= expired_at {
        state.creature.initiated_behavior = None;
    }
}

pub(crate) fn improve_relationship(
    state: &mut WorldState,
    bond: f32,
    trust: f32,
    resentment_recovery: f32,
) {
    state.creature.relationship.bond += bond;
    state.creature.relationship.trust += trust;
    state.creature.relationship.resentment -= resentment_recovery;
    state.creature.relationship.clamp();
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

pub(crate) fn reinforce_act(state: &mut WorldState, act: SocialAct, delta: f32) {
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
    // Shared days give memory recall its time words. Language stage itself follows vocabulary
    // (see teaching), never the calendar.
    if days >= 2 && interactions.total() > 0 {
        learn(state, Concept::Again, events);
    }
    if days >= 3 && interactions.total() >= 3 {
        learn(state, Concept::Yesterday, events);
    }
}
fn learn(state: &mut WorldState, concept: Concept, events: &mut Vec<GameEvent>) {
    if state.creature.known_concepts.insert(concept) {
        events.push(GameEvent::ConceptLearned(concept));
    }
}
