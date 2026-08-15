use serde::{Deserialize, Serialize};

use crate::{
    ACTIVE_DAY_MS, BeliefKind, Concept, FoodId, Intention, MemoryId, MemoryKind, NonverbalAct,
    RandomSource, Reaction, SocialAct, WorldState,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PlayerEvent {
    Feed(FoodId),
    Play,
    Comfort,
    Tidy,
    ReturnedAfterAbsence,
    Talk,
    React(Reaction),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum GameEvent {
    NeedChanged,
    MemoryCreated(MemoryId),
    FoodConsumed(FoodId),
    FoodRejected(FoodId),
    SleepStarted,
    SleepEnded,
    SocialActExpressed(SocialAct),
    NonverbalAct(NonverbalAct),
    IntentionChanged { from: Intention, to: Intention },
}

pub fn step(
    state: &mut WorldState,
    input: &[PlayerEvent],
    dt_ms: u64,
    rng: &mut impl RandomSource,
) -> Vec<GameEvent> {
    let mut events = Vec::new();
    let previous_memory_count = state.creature.memories.len();
    state.elapsed_ms = state.elapsed_ms.saturating_add(dt_ms);
    let minutes = dt_ms as f32 / 60_000.0;
    state.creature.needs.hunger += 0.025 * minutes;
    state.creature.needs.energy -= 0.018 * minutes;
    state.creature.needs.comfort -= 0.008 * minutes;
    state.creature.needs.curiosity += 0.012 * minutes;
    enact_current_intention(state, minutes, rng, &mut events);
    for event in input {
        apply_player_event(state, event, &mut events);
    }

    state.creature.needs.clamp();
    state.creature.relationship.clamp();
    state.creature.social_habits.clamp();
    events.push(GameEvent::NeedChanged);

    let previous = state.creature.current_intention;
    let next = choose_intention(state, rng);
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
    events.extend(
        state.creature.memories[previous_memory_count..]
            .iter()
            .map(|memory| GameEvent::MemoryCreated(memory.id)),
    );
    events
}

fn apply_player_event(state: &mut WorldState, event: &PlayerEvent, events: &mut Vec<GameEvent>) {
    match event {
        PlayerEvent::Feed(food) => {
            state.room.last_nonverbal_act = None;
            state.room.food_in_bowl = Some(*food);
            if state
                .creature
                .preferences
                .get(food)
                .is_some_and(|preference| *preference < -0.35)
            {
                state.creature.relationship.resentment += 0.025;
            }
        }
        PlayerEvent::Play => {
            state.creature.needs.curiosity -= 0.5;
            state.creature.relationship.bond += 0.06;
            state.creature.relationship.trust += 0.03;
            state.creature.known_concepts.insert(Concept::Toy);
            state.remember(
                MemoryKind::Played,
                &[Concept::Toy, Concept::You, Concept::Good],
                0.65,
                0.7,
            );
        }
        PlayerEvent::Comfort => {
            state.creature.needs.comfort += 0.35;
            state.creature.relationship.bond += 0.08;
            state.creature.relationship.trust += 0.05;
            state.remember(
                MemoryKind::WasComforted,
                &[Concept::You, Concept::Good, Concept::Trust],
                0.8,
                0.8,
            );
        }
        PlayerEvent::Tidy => state.room.tidy = true,
        PlayerEvent::ReturnedAfterAbsence => {
            state.room.player_present = true;
            state.creature.known_concepts.insert(Concept::Again);
            state.remember(
                MemoryKind::PlayerReturnedAfterAbsence,
                &[Concept::You, Concept::Again],
                state.creature.relationship.bond,
                0.8,
            );
        }
        PlayerEvent::Talk => {
            let act = choose_social_act(state);
            state.creature.last_social_act = Some(act);
            events.push(GameEvent::SocialActExpressed(act));
        }
        PlayerEvent::React(reaction) => apply_reaction(state, *reaction),
    }
}

fn apply_reaction(state: &mut WorldState, reaction: Reaction) {
    let Some(act) = state.creature.last_social_act.take() else {
        return;
    };
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
    } else if habits.profanity >= 0.25 {
        SocialAct::Profanity
    } else if habits.crudeness >= 0.25 {
        SocialAct::Crudeness
    } else if habits.sexual_innuendo >= 0.25 {
        SocialAct::Innuendo
    } else {
        SocialAct::Neutral
    }
}

fn enact_current_intention(
    state: &mut WorldState,
    minutes: f32,
    rng: &mut impl RandomSource,
    events: &mut Vec<GameEvent>,
) {
    match state.creature.current_intention {
        Intention::Eat => {
            if let Some(food) = state.room.food_in_bowl.take() {
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
                }
                events.push(GameEvent::FoodConsumed(food));
            }
        }
        Intention::RejectFood => {
            if let Some(food) = state.room.food_in_bowl.take() {
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
        }
        Intention::Sleep => {
            state.creature.needs.energy += 0.12 * minutes;
            if state.elapsed_ms >= ACTIVE_DAY_MS {
                state.creature.known_concepts.insert(Concept::Yesterday);
            }
        }
        Intention::Play => state.creature.needs.curiosity -= 0.06 * minutes,
        Intention::ApproachPlayer | Intention::Idle => {}
    }
}

fn choose_intention(state: &WorldState, rng: &mut impl RandomSource) -> Intention {
    let creature = &state.creature;
    let known_preference = state
        .room
        .food_in_bowl
        .and_then(|food| creature.preferences.get(&food).copied());
    let food_preference = known_preference.map_or(0.5, |value| (value + 1.0) / 2.0);
    let rejection = known_preference.map_or(0.0, |value| {
        (-value).max(0.0) * (0.65 + creature.traits.stubbornness * 0.35)
    });
    let candidates = [
        (
            Intention::RejectFood,
            f32::from(state.room.food_in_bowl.is_some()) * rejection,
        ),
        (
            Intention::Eat,
            creature.needs.hunger * f32::from(state.room.food_in_bowl.is_some()) * food_preference,
        ),
        (Intention::Sleep, (1.0 - creature.needs.energy) * 0.9),
        (
            Intention::Play,
            creature.needs.curiosity
                * f32::from(state.room.toy_available)
                * (0.6 + creature.traits.sociability * 0.4),
        ),
        (
            Intention::ApproachPlayer,
            f32::from(state.room.player_present)
                * creature.relationship.bond
                * (0.5 + creature.traits.sociability * 0.5),
        ),
        (Intention::Idle, 0.12),
    ];
    let current = creature.current_intention;
    candidates
        .into_iter()
        .map(|(intention, score)| {
            let hysteresis = if intention == current { 0.08 } else { 0.0 };
            let noise = rng.next_unit() * 0.025;
            (intention, score + hysteresis + noise)
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map_or(Intention::Idle, |(intention, _)| intention)
}
