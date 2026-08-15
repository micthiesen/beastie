use serde::{Deserialize, Serialize};

use crate::{
    ACTIVE_DAY_MS, BeliefKind, Concept, FoodId, Intention, LanguageExposure, LanguageStage,
    MemoryId, MemoryKind, Movement, NonverbalAct, RandomSource, Reaction, RoomSpot, SocialAct,
    ToyId, WorldState,
};

pub const SIMULATION_TICK_MS: u64 = 1_000;
pub const MOVEMENT_DURATION_MS: u64 = 3_000;
pub const MAX_OFFLINE_MS: u64 = ACTIVE_DAY_MS * 8;
pub const TALK_COOLDOWN_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PlayerEvent {
    Feed(FoodId),
    Play(ToyId),
    Comfort,
    Tidy,
    ReturnedAfterAbsence,
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
    TalkAccepted { contextual_follow_up: bool },
    TalkIgnored,
    LanguageExposureRegistered(LanguageExposure),
    NonverbalAct(NonverbalAct),
    MovementStarted { from: RoomSpot, to: RoomSpot },
    Arrived(RoomSpot),
    ConceptLearned(Concept),
    LanguageAdvanced(LanguageStage),
    IntentionChanged { from: Intention, to: Intention },
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
        let previous_memory_count = state.creature.memories.len();
        apply_player_event(state, event, &mut events);
        append_created_memories(state, previous_memory_count, &mut events);
    }

    let accumulated = state.simulation_remainder_ms.saturating_add(dt_ms);
    let tick_count = accumulated / SIMULATION_TICK_MS;
    state.simulation_remainder_ms = accumulated % SIMULATION_TICK_MS;
    for _ in 0..tick_count {
        let previous_memory_count = state.creature.memories.len();
        fixed_tick(state, rng, &mut events);
        append_created_memories(state, previous_memory_count, &mut events);
    }
    events
}

fn append_created_memories(
    state: &WorldState,
    previous_memory_count: usize,
    events: &mut Vec<GameEvent>,
) {
    events.extend(
        state.creature.memories[previous_memory_count..]
            .iter()
            .map(|memory| GameEvent::MemoryCreated(memory.id)),
    );
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
    state.room.player_present = false;
    let minutes = applied_ms as f32 / 60_000.0;
    state.creature.needs.hunger += 0.025 * minutes;
    state.creature.needs.energy += 0.03 * minutes;
    state.creature.needs.comfort -= 0.004 * minutes;
    state.creature.needs.curiosity += 0.006 * minutes;
    state.creature.needs.clamp();
    let mut events = vec![GameEvent::NeedChanged];

    // Offline time changes needs without advancing the active-play development clock.
    // Absence can make the creature needy, but never permanently ruins a save.
    state.creature.needs.energy = state.creature.needs.energy.max(0.2);
    state.creature.needs.comfort = state.creature.needs.comfort.max(0.2);
    state.creature.relationship.trust = state.creature.relationship.trust.max(0.05);
    state.creature.relationship.bond = state.creature.relationship.bond.max(0.05);
    events.extend(step(state, &[PlayerEvent::ReturnedAfterAbsence], 0, rng));
    OfflineProgress {
        requested_ms,
        applied_ms,
        events,
    }
}

fn fixed_tick(state: &mut WorldState, rng: &mut impl RandomSource, events: &mut Vec<GameEvent>) {
    state.elapsed_ms = state.elapsed_ms.saturating_add(SIMULATION_TICK_MS);
    let minutes = SIMULATION_TICK_MS as f32 / 60_000.0;
    state.creature.needs.hunger += 0.025 * minutes;
    state.creature.needs.energy -= 0.018 * minutes;
    state.creature.needs.comfort -= 0.008 * minutes;
    state.creature.needs.curiosity += 0.012 * minutes;

    if state.creature.needs.energy < 0.1 && state.creature.current_intention != Intention::Sleep {
        state.creature.movement = None;
    }
    let arrived = advance_movement(state, events);
    if state.creature.movement.is_none() && (arrived || at_intention_target(state)) {
        enact_current_intention(state, minutes, rng, events);
    }

    state.creature.needs.clamp();
    state.creature.relationship.clamp();
    state.creature.social_habits.clamp();
    events.push(GameEvent::NeedChanged);
    update_development(state, events);

    if state.creature.movement.is_none() {
        change_intention(state, choose_intention(state, rng), events);
    }
}

fn apply_player_event(state: &mut WorldState, event: &PlayerEvent, events: &mut Vec<GameEvent>) {
    match event {
        PlayerEvent::Feed(food) => {
            state.creature.movement = None;
            state.room.last_nonverbal_act = None;
            state.room.food_in_bowl = Some(*food);
            state.creature.development.interactions.feeds = state
                .creature
                .development
                .interactions
                .feeds
                .saturating_add(1);
            if state
                .creature
                .preferences
                .get(food)
                .is_some_and(|preference| *preference < -0.35)
            {
                state.creature.relationship.resentment += 0.025;
            }
        }
        PlayerEvent::Play(toy) => {
            state.creature.movement = None;
            state.room.toy = *toy;
            state.room.toy_available = true;
            state.room.play_requested = true;
            state.room.last_nonverbal_act = None;
            state.creature.development.interactions.plays = state
                .creature
                .development
                .interactions
                .plays
                .saturating_add(1);
        }
        PlayerEvent::Comfort => {
            state.creature.movement = None;
            state.room.comfort_requested = true;
            state.room.last_nonverbal_act = None;
            state.creature.development.interactions.comforts = state
                .creature
                .development
                .interactions
                .comforts
                .saturating_add(1);
        }
        PlayerEvent::Tidy => {
            state.room.tidy = true;
            state.room.last_nonverbal_act = None;
        }
        PlayerEvent::ReturnedAfterAbsence => {
            state.room.player_present = true;
            state.creature.development.interactions.returns = state
                .creature
                .development
                .interactions
                .returns
                .saturating_add(1);
            state.remember(
                MemoryKind::PlayerReturnedAfterAbsence,
                &[Concept::You, Concept::Again],
                state.creature.relationship.bond,
                0.8,
            );
        }
        PlayerEvent::Talk => {
            let contextual_follow_up = state.creature.conversation.contextual_follow_up_available;
            if state.elapsed_ms < state.creature.conversation.next_talk_at_ms
                && !contextual_follow_up
            {
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
            state.creature.conversation.contextual_follow_up_used = contextual_follow_up;
            let act = choose_social_act(state);
            state.creature.last_social_act = Some(act);
            events.push(GameEvent::TalkAccepted {
                contextual_follow_up,
            });
            events.push(GameEvent::SocialActExpressed(act));
        }
        PlayerEvent::React(reaction) => apply_reaction(state, *reaction),
        PlayerEvent::LanguageExposure(exposure) => {
            match exposure {
                LanguageExposure::Profanity => state.creature.social_habits.profanity += 0.08,
                LanguageExposure::Crudeness => state.creature.social_habits.crudeness += 0.07,
                LanguageExposure::Innuendo => {
                    state.creature.social_habits.sexual_innuendo += 0.06;
                }
            }
            state.creature.social_habits.clamp();
            events.push(GameEvent::LanguageExposureRegistered(*exposure));
        }
    }
}

fn advance_movement(state: &mut WorldState, events: &mut Vec<GameEvent>) -> bool {
    let Some(mut movement) = state.creature.movement else {
        return false;
    };
    movement.elapsed_ms = movement.elapsed_ms.saturating_add(SIMULATION_TICK_MS);
    if movement.elapsed_ms >= movement.duration_ms {
        state.creature.position = movement.to;
        state.creature.movement = None;
        events.push(GameEvent::Arrived(movement.to));
        true
    } else {
        state.creature.movement = Some(movement);
        false
    }
}

fn change_intention(state: &mut WorldState, next: Intention, events: &mut Vec<GameEvent>) {
    let previous = state.creature.current_intention;
    if next != previous {
        state.creature.current_intention = next;
        if previous == Intention::Sleep {
            events.push(GameEvent::SleepEnded);
        }
        if next == Intention::Sleep {
            events.push(GameEvent::SleepStarted);
        }
        events.push(GameEvent::IntentionChanged {
            from: previous,
            to: next,
        });
    }
    let target = intention_target(next);
    if state.creature.position != target {
        state.creature.movement = Some(Movement {
            from: state.creature.position,
            to: target,
            elapsed_ms: 0,
            duration_ms: MOVEMENT_DURATION_MS,
        });
        events.push(GameEvent::MovementStarted {
            from: state.creature.position,
            to: target,
        });
    }
}

#[must_use]
pub fn intention_target(intention: Intention) -> RoomSpot {
    match intention {
        Intention::Eat | Intention::WaitAtBowl | Intention::RejectFood => RoomSpot::Bowl,
        Intention::Sleep => RoomSpot::Bed,
        Intention::Play => RoomSpot::Toy,
        Intention::ApproachPlayer
        | Intention::SeekComfort
        | Intention::RefuseAndStare
        | Intention::ShowAffection => RoomSpot::Player,
        Intention::UndoTidy => RoomSpot::Center,
        Intention::Idle => RoomSpot::Center,
    }
}

fn at_intention_target(state: &WorldState) -> bool {
    state.creature.position == intention_target(state.creature.current_intention)
}

fn enact_current_intention(
    state: &mut WorldState,
    minutes: f32,
    rng: &mut impl RandomSource,
    events: &mut Vec<GameEvent>,
) {
    match state.creature.current_intention {
        Intention::Eat => eat(state, rng, events),
        Intention::WaitAtBowl => {}
        Intention::RejectFood => reject_food(state, events),
        Intention::Sleep => state.creature.needs.energy += 0.12 * minutes,
        Intention::Play => play(state, rng, minutes, events),
        Intention::ApproachPlayer | Intention::SeekComfort => comfort(state, events),
        Intention::UndoTidy => undo_tidy(state, events),
        Intention::RefuseAndStare => {
            express_nonverbal_once(state, NonverbalAct::RefuseAndStare, events)
        }
        Intention::ShowAffection => {
            express_nonverbal_once(state, NonverbalAct::LeanAgainstPlayer, events)
        }
        Intention::Idle => {}
    }
}

fn express_nonverbal_once(state: &mut WorldState, act: NonverbalAct, events: &mut Vec<GameEvent>) {
    if state.room.last_nonverbal_act != Some(act) {
        state.room.last_nonverbal_act = Some(act);
        events.push(GameEvent::NonverbalAct(act));
    }
}

fn undo_tidy(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    if state.room.tidy {
        state.room.tidy = false;
        express_nonverbal_once(state, NonverbalAct::UndoTidy, events);
    }
}

fn eat(state: &mut WorldState, rng: &mut impl RandomSource, events: &mut Vec<GameEvent>) {
    let Some(food) = state.room.food_in_bowl else {
        return;
    };
    if state
        .creature
        .preferences
        .get(&food)
        .is_some_and(|preference| *preference < -0.35)
    {
        return;
    }
    state.room.food_in_bowl = None;
    let inherited = rng.next_unit() * 2.0 - 1.0;
    let preference = *state.creature.preferences.entry(food).or_insert(inherited);
    state.creature.needs.hunger -= 0.45;
    state.remember(
        MemoryKind::WasFed { food },
        &[Concept::Food, Concept::You],
        preference,
        0.75,
    );
    if preference < -0.35 {
        let dislike_memory = state.remember(
            MemoryKind::DislikedFood { food },
            &[Concept::Food, Concept::Bad],
            preference,
            0.95,
        );
        state.creature.relationship.resentment += 0.08;
        state.creature.social_habits.spite += 0.03;
        state.creature.social_habits.provocation += 0.02;
        if food == FoodId::Berry {
            state.reinforce_belief(
                BeliefKind::RedFoodIsATrick,
                dislike_memory,
                0.45 + state.creature.traits.stubbornness * 0.25,
            );
        }
    } else {
        state.creature.relationship.trust += 0.025;
        recover_resentment(state, 0.006);
    }
    events.push(GameEvent::FoodConsumed(food));
}

fn reject_food(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let Some(food) = state.room.food_in_bowl else {
        return;
    };
    if !state
        .creature
        .preferences
        .get(&food)
        .is_some_and(|preference| *preference < -0.35)
    {
        return;
    }
    state.room.food_in_bowl = None;
    let act = NonverbalAct::PushFoodAway(food);
    state.room.last_nonverbal_act = Some(act);
    state.creature.relationship.resentment += 0.035;
    state.creature.social_habits.spite += 0.02;
    state.remember(
        MemoryKind::RejectedFood { food },
        &[Concept::Food, Concept::Bad, Concept::Again],
        -0.75,
        0.85,
    );
    events.push(GameEvent::FoodRejected(food));
    events.push(GameEvent::NonverbalAct(act));
}

fn play(
    state: &mut WorldState,
    rng: &mut impl RandomSource,
    minutes: f32,
    events: &mut Vec<GameEvent>,
) {
    if !state.room.play_requested {
        state.creature.needs.curiosity -= 0.06 * minutes;
        return;
    }
    state.room.play_requested = false;
    let toy = state.room.toy;
    let inherited = rng.next_unit() * 2.0 - 1.0;
    let preference = *state
        .creature
        .toy_preferences
        .entry(toy)
        .or_insert(inherited);
    if preference < -0.35 {
        let act = NonverbalAct::TakeToyAway(toy);
        state.room.last_nonverbal_act = Some(act);
        state.creature.relationship.resentment += 0.04;
        state.remember(
            MemoryKind::DislikedToy { toy },
            &[Concept::Toy, Concept::Bad, Concept::You],
            preference,
            0.85,
        );
        events.push(GameEvent::ToyRejected(toy));
        events.push(GameEvent::NonverbalAct(act));
    } else {
        state.creature.needs.curiosity -= 0.5;
        state.creature.relationship.bond += 0.06;
        state.creature.relationship.trust += 0.03;
        recover_resentment(state, 0.008);
        state.remember(
            MemoryKind::PlayedWith { toy },
            &[Concept::Toy, Concept::You, Concept::Good],
            preference.max(0.2),
            0.7,
        );
    }
}

fn comfort(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    if !state.room.comfort_requested {
        return;
    }
    state.room.comfort_requested = false;
    state.creature.needs.comfort += 0.35;
    state.creature.relationship.bond += 0.08;
    state.creature.relationship.trust += 0.05;
    recover_resentment(state, 0.012);
    state.remember(
        MemoryKind::WasComforted,
        &[Concept::You, Concept::Good, Concept::Trust],
        0.8,
        0.8,
    );
    events.push(GameEvent::Comforted);
}

fn apply_reaction(state: &mut WorldState, reaction: Reaction) {
    let Some(act) = state.creature.last_social_act.take() else {
        return;
    };
    if !state.creature.conversation.contextual_follow_up_used {
        state.creature.conversation.contextual_follow_up_available = true;
    }
    let stubborn = state.creature.traits.stubbornness > 0.75;
    match reaction {
        Reaction::Laugh => {
            reinforce_act(state, act, 0.12);
            state.creature.social_habits.provocation += 0.1;
            state.creature.relationship.respect -= 0.025;
        }
        Reaction::Disapprove if stubborn => {
            reinforce_act(state, act, 0.035);
            state.creature.social_habits.provocation += 0.025;
        }
        Reaction::Disapprove => {
            reinforce_act(state, act, -0.08);
            state.creature.relationship.respect += 0.035;
        }
        Reaction::Comfort => {
            state.creature.relationship.bond += 0.04;
            state.creature.social_habits.provocation += 0.05;
        }
    }
    state.remember(
        MemoryKind::PlayerReacted { reaction, to: act },
        &[Concept::You, Concept::Again],
        match reaction {
            Reaction::Laugh | Reaction::Comfort => 0.55,
            Reaction::Disapprove => -0.3,
        },
        0.75,
    );
}

fn recover_resentment(state: &mut WorldState, amount: f32) {
    state.creature.relationship.resentment =
        (state.creature.relationship.resentment - amount).max(0.0);
}

fn reinforce_act(state: &mut WorldState, act: SocialAct, delta: f32) {
    let habits = &mut state.creature.social_habits;
    match act {
        SocialAct::Profanity => habits.profanity += delta,
        SocialAct::Crudeness => habits.crudeness += delta,
        SocialAct::Insult => habits.spite += delta,
        SocialAct::Provocation => habits.provocation += delta,
        SocialAct::Innuendo => habits.sexual_innuendo += delta,
        SocialAct::Neutral => {}
    }
}

fn choose_social_act(state: &WorldState) -> SocialAct {
    let creature = &state.creature;
    let habits = creature.social_habits;
    if habits.provocation >= 0.1 {
        SocialAct::Provocation
    } else if creature.relationship.resentment + habits.spite >= 0.1 {
        SocialAct::Insult
    } else if creature.development.language_stage >= LanguageStage::Words
        && habits.profanity >= 0.25
    {
        SocialAct::Profanity
    } else if creature.development.language_stage >= LanguageStage::Phrases
        && habits.crudeness >= 0.25
    {
        SocialAct::Crudeness
    } else if creature.development.language_stage >= LanguageStage::Phrases
        && habits.sexual_innuendo >= 0.25
    {
        SocialAct::Innuendo
    } else {
        SocialAct::Neutral
    }
}

fn update_development(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let active_days = u32::try_from(state.active_day()).unwrap_or(u32::MAX);
    state.creature.development.active_days_reached = state
        .creature
        .development
        .active_days_reached
        .max(active_days);
    let interactions = state.creature.development.interactions;

    if active_days >= 2 && interactions.total() > 0 {
        learn(state, Concept::Again, events);
        if interactions.feeds > 0 {
            learn(state, Concept::Give, events);
        }
        if interactions.plays > 0 {
            learn(state, Concept::Toy, events);
        }
        advance_language(state, LanguageStage::Words, events);
    }
    if active_days >= 3 && interactions.total() >= 3 {
        learn(state, Concept::Yesterday, events);
        if interactions.comforts > 0 {
            learn(state, Concept::Trust, events);
        }
        if interactions.returns > 0 || interactions.talks > 0 {
            learn(state, Concept::Friend, events);
        }
        if interactions.talks > 0 {
            learn(state, Concept::Why, events);
        }
        advance_language(state, LanguageStage::Phrases, events);
    }
}

fn learn(state: &mut WorldState, concept: Concept, events: &mut Vec<GameEvent>) {
    if state.creature.known_concepts.insert(concept) {
        events.push(GameEvent::ConceptLearned(concept));
    }
}

fn advance_language(
    state: &mut WorldState,
    language_stage: LanguageStage,
    events: &mut Vec<GameEvent>,
) {
    if language_stage > state.creature.development.language_stage {
        state.creature.development.language_stage = language_stage;
        events.push(GameEvent::LanguageAdvanced(language_stage));
    }
}

fn choose_intention(state: &WorldState, rng: &mut impl RandomSource) -> Intention {
    let creature = &state.creature;
    let food_available = state.room.food_in_bowl.is_some();
    let known_preference = state
        .room
        .food_in_bowl
        .and_then(|food| creature.preferences.get(&food).copied());
    let food_preference = known_preference.map_or(1.0, |value| (value + 1.0) / 2.0);
    let rejection = known_preference.map_or(0.0, |value| {
        (-value).max(0.0) * (0.65 + creature.traits.stubbornness * 0.35)
    });
    let toy_preference = creature
        .toy_preferences
        .get(&state.room.toy)
        .copied()
        .map_or(0.5, |value| (value + 1.0) / 2.0);
    let candidates = [
        (Intention::RejectFood, f32::from(food_available) * rejection),
        (
            Intention::Eat,
            (0.4 + creature.needs.hunger * 0.6) * f32::from(food_available) * food_preference,
        ),
        (Intention::Sleep, (1.0 - creature.needs.energy) * 0.9),
        (
            Intention::Play,
            if state.room.play_requested {
                1.5
            } else {
                creature.needs.curiosity
                    * f32::from(state.room.toy_available)
                    * toy_preference
                    * (0.6 + creature.traits.sociability * 0.4)
            },
        ),
        (Intention::ApproachPlayer, 0.0),
        (Intention::Idle, 0.12),
        (
            Intention::WaitAtBowl,
            creature.needs.hunger * 0.72 * f32::from(!food_available),
        ),
        (
            Intention::SeekComfort,
            if state.room.comfort_requested {
                1.6
            } else {
                f32::from(state.room.player_present)
                    * (1.0 - creature.needs.comfort)
                    * (0.75 + creature.traits.sociability * 0.25)
            },
        ),
        (
            Intention::UndoTidy,
            f32::from(state.room.tidy)
                * (creature.relationship.resentment + creature.social_habits.spite)
                * (0.25 + creature.traits.stubbornness * 0.55),
        ),
        (
            Intention::RefuseAndStare,
            f32::from(state.room.player_present)
                * (creature.relationship.resentment + creature.social_habits.spite)
                * (0.25 + creature.traits.stubbornness * 0.5),
        ),
        (
            Intention::ShowAffection,
            f32::from(state.room.player_present)
                * creature.relationship.bond
                * (1.0 - creature.relationship.resentment)
                * (0.35 + creature.traits.sociability * 0.35),
        ),
    ];
    let current = creature.current_intention;
    candidates
        .into_iter()
        .map(|(intention, score)| {
            let hysteresis = if intention == current { 0.08 } else { 0.0 };
            // Keep the random stream compatible with saves created before the extra explicit
            // intentions existed. New candidates are deterministic projections of state.
            let noise = match intention {
                Intention::RejectFood
                | Intention::Eat
                | Intention::Sleep
                | Intention::Play
                | Intention::ApproachPlayer
                | Intention::Idle => rng.next_unit() * 0.025,
                Intention::WaitAtBowl
                | Intention::SeekComfort
                | Intention::UndoTidy
                | Intention::RefuseAndStare
                | Intention::ShowAffection => 0.0,
            };
            (intention, score + hysteresis + noise)
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map_or(Intention::Idle, |(intention, _)| intention)
}
