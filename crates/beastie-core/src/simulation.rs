use crate::{
    ActionPhase, ActionTimeline, BeliefKind, Concept, DevelopmentMilestone, FoodBuoyancy,
    FoodDisposition, FoodDropRejectionReason, FoodId, FoodObject, FoodOutcome, GazeTarget,
    InitiativeReason, Intention, LanguageExposure, LanguageStage, MemoryId, MemoryKind,
    NamingTarget, NonverbalAct, NormalizedPosition, NormalizedVelocity, RandomDomain, RandomSource,
    Reaction, RelationshipSubject, RelationshipTrigger, SemanticDestination, SocialAct,
    SteeringMode, ToyId, UtteranceInterpretation, UtteranceReference, WorldObject, WorldState,
    deterministic_unit,
};
use serde::{Deserialize, Serialize};

pub const SIMULATION_TICK_MS: u64 = 1_000;
pub const MAX_OFFLINE_MS: u64 = crate::ACTIVE_DAY_MS * 8;
pub const TALK_COOLDOWN_MS: u64 = 30_000;
const IDLE_BOUT_MIN_MS: u64 = 4_000;
const IDLE_BOUT_MAX_MS: u64 = 10_000;
const AFFECTION_DURATION_MS: u64 = 7_000;
const MAX_SLEEP_MS: u64 = 30_000;
const INITIATIVE_DURATION_MS: u64 = 45_000;
const ACTIVE_DAY_HOURS: u64 = 24;
const MAX_VISIT_EVIDENCE: usize = 32;
const DIRECT_RELATIONSHIP_MOMENT_MS: u64 = 3_000;

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum GameEvent {
    NeedChanged,
    MemoryCreated(MemoryId),
    FoodConsumed(FoodId),
    FoodRejected(FoodId),
    ToyPlayed(ToyId),
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
        if should_interrupt_for_player_event(event) {
            interrupt_action_relationship_moment(state, &mut events);
            interrupt_relationship_beat(state, &mut events);
        }
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
    advance_embodied_state(state, events);
    advance_relationship_beat(state, events);
    let minutes = SIMULATION_TICK_MS as f32 / 60_000.0;
    state.creature.needs.hunger += 0.025 * minutes;
    state.creature.needs.energy -= 0.018 * minutes;
    state.creature.needs.comfort -= 0.008 * minutes;
    state.creature.needs.curiosity += 0.012 * minutes;
    advance_aquarium(state, events);
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
    if state.creature.needs.energy < 0.1
        && state.creature.current_intention != crate::Intention::Sleep
    {
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
}

fn advance_embodied_state(state: &mut WorldState, events: &mut Vec<GameEvent>) {
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
        state.creature.needs.energy = (state.creature.needs.energy + 0.08).min(1.0);
        state.creature.needs.comfort = (state.creature.needs.comfort + 0.01).min(1.0);
        if state.creature.needs.energy >= 0.68
            || state.elapsed_ms.saturating_sub(*started_at) >= MAX_SLEEP_MS
        {
            state.creature.interaction_state.sleep_started_at_ms = None;
            set_intention(state, Intention::Idle, events);
            state.creature.aquarium.destination = None;
            state.creature.aquarium.steering = SteeringMode::Hover;
            events.push(GameEvent::SleepEnded);
        }
    }

    if state.creature.current_intention == Intention::ShowAffection
        && state.elapsed_ms >= state.creature.interaction_state.affectionate_until_ms
    {
        set_intention(state, Intention::Idle, events);
        state.creature.aquarium.destination = None;
        state.creature.aquarium.steering = SteeringMode::Hover;
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
            if play_with_toy(state, *toy, events)
                && let Some(context) = relationship
            {
                start_action_relationship_moment(state, context, events);
            }
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
            state.creature.aquarium.destination = Some(SemanticDestination::Player);
            state.creature.aquarium.steering = SteeringMode::Approach;
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
    let travelling_to_toy = matches!(
        creature.aquarium.destination,
        Some(SemanticDestination::Toy(_))
    );
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
    !matches!(
        event,
        PlayerEvent::SpeechStarted
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
    record_relationship_expression(state, motif, expression);
    apply_relationship_pose(state, &beat);
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
}

fn allocate_action_id(state: &mut WorldState) -> u64 {
    let id = state.creature.interaction_state.next_action_id.max(1);
    state.creature.interaction_state.next_action_id = id.saturating_add(1).max(1);
    id
}

fn start_action_relationship_moment(
    state: &mut WorldState,
    context: crate::ActionRelationshipContext,
    events: &mut Vec<GameEvent>,
) {
    let action_id = allocate_action_id(state);
    record_relationship_expression(state, context.motif, context.expression_kind);
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

fn apply_relationship_pose(state: &mut WorldState, beat: &crate::RelationshipBeat) {
    match beat.motif {
        crate::RelationshipMotifKey::SharedToy(toy) => {
            state.creature.aquarium.gaze = GazeTarget::Toy(toy);
            if beat.expression_kind != crate::RelationshipExpressionKind::Notice
                && beat.phase != crate::RelationshipBeatPhase::Notice
            {
                state.creature.aquarium.destination = beat.target;
                state.creature.aquarium.steering = SteeringMode::Approach;
                state.creature.current_intention = Intention::Play;
            }
        }
        crate::RelationshipMotifKey::ComfortRitual => {
            state.creature.aquarium.gaze = GazeTarget::Player;
            if beat.expression_kind != crate::RelationshipExpressionKind::Notice
                && beat.phase != crate::RelationshipBeatPhase::Notice
            {
                state.creature.current_intention = Intention::SeekComfort;
                state.creature.aquarium.destination = beat.target;
                state.creature.aquarium.steering = SteeringMode::Approach;
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
                state.creature.aquarium.destination = beat.target;
                state.creature.aquarium.steering = SteeringMode::Approach;
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
                state.creature.aquarium.destination = beat.target;
                state.creature.aquarium.steering = SteeringMode::Approach;
            }
        }
        crate::RelationshipMotifKey::PlayerReturns => {
            state.creature.aquarium.gaze = GazeTarget::Player;
            if beat.expression_kind != crate::RelationshipExpressionKind::Notice {
                state.creature.current_intention = Intention::ApproachPlayer;
                state.creature.aquarium.destination = beat.target;
                state.creature.aquarium.steering = SteeringMode::Approach;
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
                state.creature.aquarium.destination = Some(destination);
                state.creature.aquarium.steering = SteeringMode::Approach;
            } else {
                state.creature.aquarium.destination = None;
                state.creature.aquarium.steering = SteeringMode::Hover;
            }
        }
    }
}

fn interrupt_relationship_beat(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let Some(beat) = state.creature.relationship_expression.active.take() else {
        return;
    };
    if state.creature.aquarium.action.is_none() {
        state.creature.aquarium.destination = None;
        state.creature.aquarium.steering = SteeringMode::Hover;
    }
    events.push(GameEvent::RelationshipBeatInterrupted(beat.motif));
}

fn advance_relationship_beat(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let Some(mut beat) = state.creature.relationship_expression.active.clone() else {
        return;
    };
    let elapsed = state.elapsed_ms.saturating_sub(beat.phase_started_at_ms);
    if elapsed < crate::relationship::phase_duration_ms(beat.phase) {
        apply_relationship_pose(state, &beat);
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
                apply_relationship_pose(state, &beat);
                return;
            }
            crate::RelationshipBeatPhase::Act
        }
        crate::RelationshipBeatPhase::Act => crate::RelationshipBeatPhase::Recover,
        crate::RelationshipBeatPhase::Recover => {
            state.creature.relationship_expression.active = None;
            if state.creature.aquarium.action.is_none() {
                state.creature.aquarium.destination = None;
                state.creature.aquarium.steering = SteeringMode::Hover;
                state.creature.current_intention = Intention::Idle;
            }
            events.push(GameEvent::RelationshipBeatCompleted(beat.motif));
            return;
        }
    };
    beat.phase = next;
    beat.phase_started_at_ms = state.elapsed_ms;
    apply_relationship_pose(state, &beat);
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
        action_id,
        phase: ActionPhase::Notice,
        elapsed_ms: 0,
        phase_duration_ms: 1_000,
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
        record_relationship_expression(state, context.motif, context.expression_kind);
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

fn advance_aquarium(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    if state.creature.current_intention != Intention::Sleep {
        advance_creature_motion(state, events);
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
        record_genuine_arrival(state, destination, events);
        state.creature.aquarium.destination = None;
        state.creature.aquarium.steering = SteeringMode::Hover;
        maybe_start_relationship_beat(
            state,
            RelationshipTrigger::PlaceArrived { destination },
            events,
        );
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
        || state.creature.relationship_expression.active.is_some()
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
    if state.elapsed_ms < state.creature.idle_life.settled_until_ms {
        state.creature.aquarium.steering = SteeringMode::Hover;
        return;
    }
    let quiet_moment = state.aquarium.player_present
        && state.aquarium.cursor.is_none()
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
            && (state.elapsed_ms / SIMULATION_TICK_MS).is_multiple_of(8)
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

fn record_genuine_arrival(
    state: &mut WorldState,
    destination: SemanticDestination,
    events: &mut Vec<GameEvent>,
) {
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
    let spans = IDLE_BOUT_MAX_MS / SIMULATION_TICK_MS - IDLE_BOUT_MIN_MS / SIMULATION_TICK_MS + 1;
    let duration = IDLE_BOUT_MIN_MS
        + (state.domain_draw(RandomDomain::Environment) * spans as f32) as u64 * SIMULATION_TICK_MS;
    state.creature.idle_life.settled_until_ms = state.elapsed_ms.saturating_add(duration);
    record_routine_visit(state, destination);
    if let SemanticDestination::Toy(toy) = destination {
        events.push(GameEvent::ToyPlayed(toy));
    }
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

fn play_with_toy(state: &mut WorldState, toy: ToyId, events: &mut Vec<GameEvent>) -> bool {
    if destination_position(state, SemanticDestination::Toy(toy)).is_none() {
        return false;
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
        false
    } else {
        set_intention(state, Intention::Play, events);
        state.creature.needs.curiosity = (state.creature.needs.curiosity - 0.18).max(0.0);
        improve_relationship(state, 0.01, 0.008, 0.012);
        state.remember(
            MemoryKind::PlayedWith { toy },
            &[Concept::Good],
            preference,
            0.6,
        );
        events.push(GameEvent::ToyPlayed(toy));
        true
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
        expires_at_ms: state.elapsed_ms.saturating_add(INITIATIVE_DURATION_MS),
    });
    if nonverbal.is_some() {
        events.push(GameEvent::NonverbalRequest(reason));
    } else {
        events.push(GameEvent::InitiatedTalk(reason));
    }
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

fn improve_relationship(state: &mut WorldState, bond: f32, trust: f32, resentment_recovery: f32) {
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
