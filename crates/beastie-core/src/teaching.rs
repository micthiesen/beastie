//! Hearing words, learning them from shared focus, and answering words the creature knows.
//!
//! Every utterance is perceived at once, whether or not the creature then speaks. Learning uses
//! only simulation evidence: what the creature and player were attending to when the word was
//! heard. Understood words become social requests that the creature may honor or refuse.

use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use crate::simulation::{
    GameEvent, allocate_action_id, creature_is_occupied, improve_relationship,
    interrupt_for_player, play_with_toy, reinforce_act, set_intention, set_travel_target,
    start_sleep,
};
use crate::{
    ActWord, ActivityPhase, DevelopmentMilestone, FocusMark, FoodId, GazeTarget, Intention,
    LanguageStage, Meaning, Mood, NormalizedPosition, PrivateLifeKind, RandomDomain, Salience,
    SemanticDestination, ToyId, WorldState, echo_attempt, mark_focus,
};

/// How the creature answered a word it understood.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestResponse {
    /// It does what was asked.
    Comply,
    /// It understood and visibly declines.
    Refuse,
    /// It is pleased (praise, greeting, its name).
    Delight,
    /// It shrinks from disapproval.
    Sulk,
    /// It looks at the player.
    Look,
}

const FOCUS_SHORT_MS: u64 = 6_000;
const FOCUS_LONG_MS: u64 = 10_000;
const GREETING_WINDOW_MS: u64 = 20_000;
/// How long a refusal stays visible: long enough to read, short enough not to sulk.
const REFUSAL_SHOWN_MS: u64 = 2_500;

/// Record shared focus from authoritative events, so a word said right after a moment can
/// refer to it.
pub(crate) fn mark_attention_from_events(state: &mut WorldState, events: &[GameEvent]) {
    let now = state.elapsed_ms;
    let mut refusal = None;
    let mut mark = |meaning: Meaning, weight: u16, duration: u64| {
        mark_focus(
            &mut state.creature.attention,
            FocusMark {
                meaning,
                weight,
                until_ms: now.saturating_add(duration),
            },
            now,
        );
    };
    for event in events {
        match event {
            // What the player points at (offers, drops) is the strongest teaching context.
            GameEvent::FoodDropped { food, .. } => {
                mark(Meaning::Food(*food), 4, FOCUS_LONG_MS);
                mark(Meaning::Act(ActWord::Eat), 1, FOCUS_LONG_MS);
            }
            GameEvent::FoodConsumed(food) => {
                mark(Meaning::Food(*food), 3, FOCUS_SHORT_MS);
                mark(Meaning::Act(ActWord::Eat), 2, FOCUS_SHORT_MS);
                mark(Meaning::Praise, 2, FOCUS_SHORT_MS);
            }
            GameEvent::FoodRejected(food) => {
                refusal = Some((Meaning::Food(*food), now + REFUSAL_SHOWN_MS));
                mark(Meaning::Food(*food), 3, FOCUS_SHORT_MS);
                mark(Meaning::Scold, 2, FOCUS_SHORT_MS);
            }
            GameEvent::ToyPlayAccepted {
                toy,
                origin: crate::ToyOrigin::Player,
                ..
            } => {
                mark(Meaning::Toy(*toy), 4, FOCUS_LONG_MS);
                mark(Meaning::Act(ActWord::Play), 1, FOCUS_LONG_MS);
            }
            GameEvent::ToyPlayAccepted { toy, .. } | GameEvent::ToyContacted { toy, .. } => {
                mark(Meaning::Toy(*toy), 2, FOCUS_LONG_MS);
                mark(Meaning::Act(ActWord::Play), 1, FOCUS_LONG_MS);
            }
            GameEvent::ToyPlayed { toy, .. } => {
                mark(Meaning::Toy(*toy), 3, FOCUS_SHORT_MS);
                mark(Meaning::Act(ActWord::Play), 1, FOCUS_SHORT_MS);
                mark(Meaning::Praise, 2, FOCUS_SHORT_MS);
            }
            GameEvent::ToyObjectResponded { toy, .. } => {
                mark(Meaning::Toy(*toy), 2, FOCUS_SHORT_MS);
                mark(Meaning::Act(ActWord::Play), 1, FOCUS_SHORT_MS);
            }
            GameEvent::ToyRejected { toy, .. } => {
                refusal = Some((Meaning::Toy(*toy), now + REFUSAL_SHOWN_MS));
                // Refused or not, the player pointed at this toy.
                mark(Meaning::Toy(*toy), 4, FOCUS_LONG_MS);
                mark(Meaning::Scold, 1, FOCUS_SHORT_MS);
            }
            GameEvent::Comforted => {
                mark(Meaning::Creature, 2, FOCUS_SHORT_MS);
                mark(Meaning::Praise, 1, FOCUS_SHORT_MS);
                mark(Meaning::Act(ActWord::Come), 1, FOCUS_SHORT_MS);
            }
            GameEvent::SleepStarted => mark(Meaning::Act(ActWord::Sleep), 3, FOCUS_LONG_MS),
            GameEvent::PrivateLifeStarted {
                kind: PrivateLifeKind::CaveSettle,
                ..
            } => mark(Meaning::Act(ActWord::Sleep), 1, FOCUS_LONG_MS),
            GameEvent::PrivateLifeStarted {
                kind: PrivateLifeKind::ToyPlay(toy),
                ..
            } => {
                mark(Meaning::Toy(*toy), 1, FOCUS_SHORT_MS);
                mark(Meaning::Act(ActWord::Play), 1, FOCUS_SHORT_MS);
            }
            GameEvent::Understood {
                response: RequestResponse::Comply,
                ..
            } => mark(Meaning::Praise, 1, FOCUS_SHORT_MS),
            _ => {}
        }
    }
    if refusal.is_some() {
        state.creature.refusing = refusal;
    }
}

/// What a word heard right now would most plausibly refer to.
#[must_use]
pub fn current_salience(state: &WorldState) -> Vec<Salience> {
    let now = state.elapsed_ms;
    let mut salient: Vec<Salience> = Vec::new();
    let add = |salient: &mut Vec<Salience>, meaning: Meaning, weight: u16| {
        if let Some(existing) = salient.iter_mut().find(|(known, _)| *known == meaning) {
            existing.1 = existing.1.max(weight);
        } else {
            salient.push((meaning, weight));
        }
    };
    for mark in &state.creature.attention {
        if mark.until_ms > now {
            add(&mut salient, mark.meaning, mark.weight);
        }
    }
    let creature = &state.creature;
    if let Some(interaction) = creature.interaction_state.toy_interaction.as_ref() {
        add(&mut salient, Meaning::Toy(interaction.toy), 3);
    }
    if let Some(activity) = creature.private_life.active.as_ref()
        && let PrivateLifeKind::ToyPlay(toy) = activity.kind
        && matches!(activity.phase, ActivityPhase::Approach | ActivityPhase::Act)
    {
        add(&mut salient, Meaning::Toy(toy), 1);
    }
    if let Some(food) = creature
        .aquarium
        .action
        .as_ref()
        .and_then(|action| action.food)
    {
        add(&mut salient, Meaning::Food(food), 2);
        add(&mut salient, Meaning::Act(ActWord::Eat), 1);
    }
    if creature.current_intention == Intention::Sleep {
        add(&mut salient, Meaning::Act(ActWord::Sleep), 3);
    }
    if let GazeTarget::Toy(toy) = creature.aquarium.gaze {
        add(&mut salient, Meaning::Toy(toy), 1);
    }
    // With nothing else shared, words are about the participants: a greeting when the player
    // has just arrived, otherwise the creature's own name if it is looking at the speaker.
    if salient.is_empty() {
        let greeted = creature
            .lexicon
            .words
            .values()
            .any(|word| word.learned == Some(Meaning::Greeting));
        let looking_at_player = matches!(
            creature.aquarium.gaze,
            GazeTarget::Player | GazeTarget::Cursor
        );
        if now < GREETING_WINDOW_MS && !greeted {
            add(&mut salient, Meaning::Greeting, 2);
            add(&mut salient, Meaning::Creature, 1);
        } else {
            add(
                &mut salient,
                Meaning::Creature,
                if looking_at_player { 2 } else { 1 },
            );
        }
    }
    salient.sort();
    salient
}

/// Hear an utterance: learn from it, react to it, and answer what was understood.
pub(crate) fn hear_utterance(state: &mut WorldState, text: &str, events: &mut Vec<GameEvent>) {
    // A sleeping creature does not hear words, so it cannot learn from them either.
    if state.creature.current_intention == Intention::Sleep {
        events.push(GameEvent::TalkIgnored);
        return;
    }
    let mut salient = current_salience(state);
    // Saying the creature's own name is addressing it: the name the player gave is strong
    // evidence that this word means the creature itself.
    let name = state.creature.name.to_lowercase();
    if crate::content_words(text).contains(&name) {
        match salient
            .iter_mut()
            .find(|(meaning, _)| *meaning == Meaning::Creature)
        {
            Some(entry) => entry.1 = entry.1.max(3),
            None => salient.push((Meaning::Creature, 3)),
        }
    }
    let now = state.elapsed_ms;
    let hearing = state.creature.lexicon.hear(text, &salient, now);
    if hearing.content_words.is_empty() {
        return;
    }
    let counters = &mut state.creature.development.interactions;
    counters.talks = counters.talks.saturating_add(1);
    // Being talked to is company, whatever is understood.
    state.creature.needs.comfort = (state.creature.needs.comfort + 0.02).min(1.0);
    // Hearing turns an unoccupied creature toward the speaker. A busy one keeps looking at
    // what it is doing, so its intent stays readable.
    if !creature_is_occupied(state) {
        state.creature.aquarium.gaze = GazeTarget::Player;
    }
    for (word, meaning) in &hearing.learned {
        events.push(GameEvent::WordLearned {
            word: word.clone(),
            meaning: *meaning,
        });
        if *meaning == Meaning::Creature {
            state
                .creature
                .development
                .milestones
                .insert(DevelopmentMilestone::NameRecognized);
        }
    }
    advance_language_from_vocabulary(state, events);
    if !hearing.learned.is_empty() {
        // A new word is its own answer.
        return;
    }
    let actionable = hearing
        .understood
        .iter()
        .max_by_key(|(_, meaning)| request_priority(*meaning))
        .cloned();
    let Some((word, meaning)) = actionable else {
        events.push(GameEvent::WordHeard {
            word: hearing.curious_about.clone(),
            echo: hearing.curious_about.as_deref().map(echo_attempt),
        });
        return;
    };
    let response = answer_request(state, meaning, events);
    let counters = &mut state.creature.development.interactions;
    counters.requests = counters.requests.saturating_add(1);
    events.push(GameEvent::Understood {
        word,
        meaning,
        response,
    });
}

const fn request_priority(meaning: Meaning) -> u8 {
    match meaning {
        Meaning::Toy(_) | Meaning::Food(_) => 5,
        Meaning::Act(_) => 4,
        Meaning::Scold => 3,
        Meaning::Praise => 2,
        Meaning::Creature | Meaning::Greeting => 1,
        Meaning::Player => 0,
    }
}

fn answer_request(
    state: &mut WorldState,
    meaning: Meaning,
    events: &mut Vec<GameEvent>,
) -> RequestResponse {
    let resentful = state.mood() == Mood::Resentful;
    match meaning {
        Meaning::Praise => {
            improve_relationship(state, 0.01, 0.006, 0.01);
            reinforce_recent_choice(state, 0.06);
            state.creature.aquarium.gaze = GazeTarget::Player;
            RequestResponse::Delight
        }
        Meaning::Scold => {
            reinforce_recent_choice(state, -0.06);
            if let Some(act) = state.creature.last_social_act {
                reinforce_act(state, act, -0.05);
            }
            RequestResponse::Sulk
        }
        Meaning::Player => {
            state.creature.aquarium.gaze = GazeTarget::Player;
            RequestResponse::Look
        }
        _ if resentful || playful_refusal(state) => {
            state.creature.aquarium.gaze = GazeTarget::Player;
            RequestResponse::Refuse
        }
        Meaning::Toy(toy) => {
            interrupt_for_player(state, events);
            play_with_toy(state, toy, None, events);
            if state
                .creature
                .interaction_state
                .toy_interaction
                .as_ref()
                .is_some_and(|interaction| {
                    interaction.outcome == crate::ToyInteractionOutcome::Rejected
                })
            {
                RequestResponse::Refuse
            } else {
                RequestResponse::Comply
            }
        }
        Meaning::Act(ActWord::Play) => {
            interrupt_for_player(state, events);
            let toy = favorite_toy(state);
            play_with_toy(state, toy, None, events);
            RequestResponse::Comply
        }
        Meaning::Food(food)
            if state
                .creature
                .preferences
                .get(&food)
                .copied()
                .unwrap_or(0.0)
                < -0.35 =>
        {
            // It knows exactly which food that is, and it does not want it.
            RequestResponse::Refuse
        }
        Meaning::Food(_) | Meaning::Act(ActWord::Eat) => {
            if state.creature.needs.hunger < 0.3 {
                RequestResponse::Refuse
            } else {
                interrupt_for_player(state, events);
                // It cannot feed itself: it comes to the glass and asks.
                come_to_player(state, events);
                RequestResponse::Comply
            }
        }
        Meaning::Act(ActWord::Sleep) => {
            if state.creature.needs.energy > 0.6 {
                RequestResponse::Refuse
            } else {
                interrupt_for_player(state, events);
                start_sleep(state, events);
                RequestResponse::Comply
            }
        }
        Meaning::Act(ActWord::Come) => {
            interrupt_for_player(state, events);
            come_to_player(state, events);
            RequestResponse::Comply
        }
        Meaning::Creature | Meaning::Greeting => {
            state.creature.aquarium.gaze = GazeTarget::Player;
            if state.creature.aquarium.action.is_none()
                && state.creature.interaction_state.toy_interaction.is_none()
            {
                interrupt_for_player(state, events);
                come_to_player(state, events);
            }
            RequestResponse::Delight
        }
    }
}

fn come_to_player(state: &mut WorldState, events: &mut Vec<GameEvent>) {
    let target = state
        .aquarium
        .cursor
        .unwrap_or(NormalizedPosition::new(5_000, 4_000));
    // Stop just short of the glass point so the face, not the tail, arrives.
    let target = NormalizedPosition::new(target.x, target.y.clamp(1_500, 7_500));
    let action_id = NonZeroU64::new(allocate_action_id(state)).expect("action IDs start at one");
    set_travel_target(
        state,
        SemanticDestination::Position(target),
        crate::TravelPurpose::CursorSocial { action_id },
        events,
    );
    state.creature.aquarium.gaze = GazeTarget::Player;
    set_intention(state, Intention::ApproachPlayer, events);
}

/// A stubborn creature occasionally says no just to see what happens. Never twice in a row.
fn playful_refusal(state: &mut WorldState) -> bool {
    let stubborn = state.creature.traits.stubbornness;
    if stubborn < 0.55 || state.creature.conversation.refused_last_request {
        state.creature.conversation.refused_last_request = false;
        return false;
    }
    let refuse = state.domain_draw(RandomDomain::Social) < (stubborn - 0.45) * 0.6;
    state.creature.conversation.refused_last_request = refuse;
    refuse
}

fn favorite_toy(state: &WorldState) -> ToyId {
    [ToyId::Ball, ToyId::Bell, ToyId::Sock]
        .into_iter()
        .max_by(|left, right| {
            let preference = |toy| {
                state
                    .creature
                    .toy_preferences
                    .get(toy)
                    .copied()
                    .unwrap_or(0.0)
            };
            preference(left).total_cmp(&preference(right))
        })
        .unwrap_or(ToyId::Ball)
}

/// Praise or disapproval nudges the preference behind whatever the creature just chose.
fn reinforce_recent_choice(state: &mut WorldState, delta: f32) {
    let now = state.elapsed_ms;
    let recent = |meaning: Meaning| {
        state
            .creature
            .attention
            .iter()
            .any(|mark| mark.meaning == meaning && mark.until_ms > now)
    };
    for toy in [ToyId::Ball, ToyId::Bell, ToyId::Sock] {
        if recent(Meaning::Toy(toy)) {
            let preference = state.creature.toy_preferences.entry(toy).or_insert(0.0);
            *preference = (*preference + delta).clamp(-1.0, 1.0);
        }
    }
    for food in [FoodId::Berry, FoodId::Mushroom, FoodId::Pellet] {
        if recent(Meaning::Food(food)) {
            let preference = state.creature.preferences.entry(food).or_insert(0.0);
            *preference = (*preference + delta * 0.5).clamp(-1.0, 1.0);
        }
    }
}

/// Language stage follows vocabulary, not the calendar.
pub(crate) fn advance_language_from_vocabulary(
    state: &mut WorldState,
    events: &mut Vec<GameEvent>,
) {
    let learned = state.creature.lexicon.learned_count();
    let stage = if learned >= 8 {
        LanguageStage::Phrases
    } else if learned >= 3 {
        LanguageStage::Words
    } else {
        LanguageStage::Hatch
    };
    if stage > state.creature.development.language_stage {
        state.creature.development.language_stage = stage;
        events.push(GameEvent::LanguageAdvanced(stage));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlayerEvent, SIMULATION_TICK_MS, SeededRandom, step};

    fn run_until(
        world: &mut WorldState,
        rng: &mut SeededRandom,
        limit_ms: u64,
        mut done: impl FnMut(&[GameEvent], &WorldState) -> bool,
    ) -> bool {
        let mut elapsed = 0;
        while elapsed < limit_ms {
            let events = step(world, &[], SIMULATION_TICK_MS, rng);
            elapsed += SIMULATION_TICK_MS;
            if done(&events, world) {
                return true;
            }
        }
        false
    }

    fn say(world: &mut WorldState, rng: &mut SeededRandom, text: &str) -> Vec<GameEvent> {
        step(world, &[PlayerEvent::Utterance(text.to_owned())], 0, rng)
    }

    #[test]
    fn naming_a_toy_during_play_teaches_it_and_the_word_becomes_a_request() {
        let mut world = WorldState::new(12, "Mop");
        let mut rng = SeededRandom::new(12);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        let first = say(&mut world, &mut rng, "ball!");
        assert!(first.iter().any(|event| matches!(
            event,
            GameEvent::WordHeard { word: Some(word), echo: Some(echo) } if word == "ball" && echo == "baw?"
        )));
        assert!(run_until(&mut world, &mut rng, 10_000, |events, _| {
            events
                .iter()
                .any(|event| matches!(event, GameEvent::ToyPlayed { .. }))
        }));
        let second = say(&mut world, &mut rng, "good ball");
        assert!(second.contains(&GameEvent::WordLearned {
            word: "ball".to_owned(),
            meaning: Meaning::Toy(ToyId::Ball),
        }));
        // Let the play finish, then ask for the ball by name from elsewhere.
        run_until(&mut world, &mut rng, 30_000, |_, world| {
            world.creature.interaction_state.toy_interaction.is_none()
                && world.creature.aquarium.action.is_none()
        });
        world.creature.aquarium.position = NormalizedPosition::new(1_000, 2_000);
        let asked = say(&mut world, &mut rng, "ball");
        assert!(asked.iter().any(|event| matches!(
            event,
            GameEvent::Understood {
                meaning: Meaning::Toy(ToyId::Ball),
                response: RequestResponse::Comply | RequestResponse::Refuse,
                ..
            }
        )));
    }

    #[test]
    fn a_name_is_learned_from_being_addressed_while_looking_at_the_player() {
        let mut world = WorldState::new(5, "Mop");
        let mut rng = SeededRandom::new(5);
        world.elapsed_ms = 60_000;
        world.creature.aquarium.gaze = GazeTarget::Player;
        say(&mut world, &mut rng, "mop");
        world.creature.aquarium.gaze = GazeTarget::Player;
        let events = say(&mut world, &mut rng, "hey mop");
        assert!(events.contains(&GameEvent::WordLearned {
            word: "mop".to_owned(),
            meaning: Meaning::Creature,
        }));
        assert!(
            world
                .creature
                .development
                .milestones
                .contains(&DevelopmentMilestone::NameRecognized)
        );
        // Calling a known name brings the creature over.
        let called = say(&mut world, &mut rng, "mop");
        assert!(called.iter().any(|event| matches!(
            event,
            GameEvent::Understood {
                meaning: Meaning::Creature,
                response: RequestResponse::Delight,
                ..
            }
        )));
        assert_eq!(world.creature.current_intention, Intention::ApproachPlayer);
        // Vocabulary and focus survive a save exactly.
        let json = crate::SaveGame::capture(&world, &rng).to_json().unwrap();
        let (loaded, _) = crate::SaveGame::from_json(&json).unwrap().resume();
        assert_eq!(loaded, world);
    }

    #[test]
    fn hearing_is_immediate_and_never_interrupts_an_action() {
        let mut world = WorldState::new(9, "Mop");
        let mut rng = SeededRandom::new(9);
        step(
            &mut world,
            &[PlayerEvent::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(4_000, 3_000),
            }],
            0,
            &mut rng,
        );
        let action = world.creature.aquarium.action.clone();
        let events = say(&mut world, &mut rng, "pellet");
        assert_eq!(world.creature.aquarium.action, action);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, GameEvent::WordHeard { .. }))
        );
    }

    #[test]
    fn a_direct_ball_offer_is_a_chase_game_with_one_reward() {
        let mut world = WorldState::new(12, "Mop");
        let mut rng = SeededRandom::new(12);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        world.creature.idle_life.settled_until_ms = 120_000;
        step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        let mut contacts = 0;
        let mut rewards = 0;
        let memories = world.creature.memories.len();
        run_until(&mut world, &mut rng, 30_000, |events, world| {
            contacts += events
                .iter()
                .filter(|event| matches!(event, GameEvent::ToyContacted { .. }))
                .count();
            rewards += events
                .iter()
                .filter(|event| matches!(event, GameEvent::ToyPlayed { .. }))
                .count();
            world.creature.interaction_state.toy_interaction.is_none()
        });
        assert_eq!(contacts, 4, "first contact plus three chase rounds");
        world.validate().unwrap();
        assert_eq!(rewards, 1);
        assert_eq!(world.creature.memories.len(), memories + 1);
    }

    #[test]
    fn every_tick_of_a_session_with_taps_play_and_talk_stays_valid() {
        let mut world = WorldState::first_meeting(31, "Mop");
        let mut rng = SeededRandom::new(31);
        let script: Vec<Vec<PlayerEvent>> = vec![
            vec![PlayerEvent::Tap(NormalizedPosition::new(7_000, 3_000))],
            vec![PlayerEvent::Play(ToyId::Ball)],
            vec![PlayerEvent::Utterance("ball".to_owned())],
            vec![PlayerEvent::Tap(NormalizedPosition::new(2_000, 6_000))],
            vec![PlayerEvent::Utterance("ball".to_owned())],
            vec![PlayerEvent::Play(ToyId::Sock)],
            vec![PlayerEvent::Comfort],
            vec![PlayerEvent::Utterance("mop".to_owned())],
        ];
        for events in script {
            step(&mut world, &events, 0, &mut rng);
            world.validate().unwrap();
            for _ in 0..25 {
                step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
                world.validate().unwrap();
            }
        }
        assert!(!world.creature.hidden_until_met);
    }

    /// Random play across many creatures must never produce a state the save validator rejects:
    /// an invalid state stops the game.
    #[test]
    fn random_play_never_produces_invalid_state() {
        use crate::{FoodId, RandomSource};
        let words = [
            "ball", "bell", "sock", "berry", "mop", "good", "no", "zorp", "come", "play", "sleep",
            "pellet", "eat",
        ];
        for seed in 0..48_u64 {
            let mut world = if seed % 2 == 0 {
                WorldState::first_meeting(seed, "Mop")
            } else {
                WorldState::new(seed, "Mop")
            };
            // Half the creatures already know words, so requests and remarks fire often.
            if seed % 4 < 2 {
                let taught = [
                    ("ball", Meaning::Toy(ToyId::Ball)),
                    ("bell", Meaning::Toy(ToyId::Bell)),
                    ("sock", Meaning::Toy(ToyId::Sock)),
                    ("berry", Meaning::Food(FoodId::Berry)),
                    ("pellet", Meaning::Food(FoodId::Pellet)),
                    ("mop", Meaning::Creature),
                    ("come", Meaning::Act(ActWord::Come)),
                    ("play", Meaning::Act(ActWord::Play)),
                    ("sleep", Meaning::Act(ActWord::Sleep)),
                    ("eat", Meaning::Act(ActWord::Eat)),
                    ("good", Meaning::Praise),
                    ("no", Meaning::Scold),
                ];
                for (word, meaning) in taught {
                    world.creature.lexicon.hear(word, &[(meaning, 3)], 0);
                    world.creature.lexicon.hear(word, &[(meaning, 3)], 1);
                }
            }
            let mut rng = SeededRandom::new(seed);
            let mut chooser = SeededRandom::new(seed ^ 0x5eed);
            for _ in 0..400 {
                let pick = (chooser.next_unit() * 10.0) as u32;
                let x = (chooser.next_unit() * 10_000.0) as i32;
                let y = (chooser.next_unit() * 10_000.0) as i32;
                let toy = [ToyId::Ball, ToyId::Bell, ToyId::Sock]
                    [(chooser.next_unit() * 3.0) as usize % 3];
                let food = [FoodId::Berry, FoodId::Mushroom, FoodId::Pellet]
                    [(chooser.next_unit() * 3.0) as usize % 3];
                let word = words[(chooser.next_unit() * words.len() as f32) as usize % words.len()];
                let event = match pick {
                    0 => Some(PlayerEvent::Play(toy)),
                    1 if chooser.next_unit() < 0.2 => Some(PlayerEvent::Arrived),
                    1 => Some(PlayerEvent::Comfort),
                    2 => Some(PlayerEvent::DropFood {
                        food,
                        position: NormalizedPosition::new(x, y),
                    }),
                    3 => Some(PlayerEvent::Tap(NormalizedPosition::new(x, y))),
                    4 | 5 => Some(PlayerEvent::Utterance(word.to_owned())),
                    6 => Some(PlayerEvent::Cursor(Some(NormalizedPosition::new(x, y)))),
                    7 => {
                        // Relationship moments overlap with requests in real play.
                        crate::trigger_relationship_beat(
                            &mut world,
                            crate::RelationshipTrigger::QuietMoment,
                        );
                        world
                            .validate()
                            .unwrap_or_else(|error| panic!("seed {seed} beat: {error}"));
                        None
                    }
                    _ => None,
                };
                if let Some(event) = event {
                    step(&mut world, &[event], 0, &mut rng);
                    world
                        .validate()
                        .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
                }
                let ticks = 1 + (chooser.next_unit() * 30.0) as u64;
                for _ in 0..ticks {
                    let before = world.clone();
                    let events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
                    if let Err(error) = world.validate() {
                        panic!(
                            "seed {seed} after tick: {error}\nevents {events:?}\nbefore toy {:?} dest {:?} purpose {:?} intent {:?}\nafter toy {:?} dest {:?} purpose {:?} intent {:?} private {:?}",
                            before.creature.interaction_state.toy_interaction,
                            before.creature.aquarium.destination,
                            before.creature.aquarium.travel_purpose,
                            before.creature.current_intention,
                            world.creature.interaction_state.toy_interaction,
                            world.creature.aquarium.destination,
                            world.creature.aquarium.travel_purpose,
                            world.creature.current_intention,
                            world
                                .creature
                                .private_life
                                .active
                                .as_ref()
                                .map(|a| (a.kind, a.phase)),
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_taught_toy_is_named_during_play_but_not_every_time() {
        let mut world = WorldState::new(12, "Mop");
        let mut rng = SeededRandom::new(12);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        world
            .creature
            .lexicon
            .hear("ball", &[(Meaning::Toy(ToyId::Ball), 3)], 0);
        world
            .creature
            .lexicon
            .hear("ball", &[(Meaning::Toy(ToyId::Ball), 3)], 1);
        let mut remarks = 0;
        for _ in 0..2 {
            step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
            run_until(&mut world, &mut rng, 12_000, |events, _| {
                remarks += events
                    .iter()
                    .filter(|event| **event == GameEvent::Remarked(Meaning::Toy(ToyId::Ball)))
                    .count();
                false
            });
        }
        assert!(remarks >= 1, "names the ball it was taught");
        assert!(remarks <= 2, "remarks are rate limited");
    }

    #[test]
    fn a_refused_toy_is_still_named_by_the_players_pointing() {
        let mut world = WorldState::new(7, "Mop");
        let mut rng = SeededRandom::new(7);
        world.creature.toy_preferences.insert(ToyId::Ball, -0.6);
        world.creature.toy_preferences.insert(ToyId::Sock, 0.8);
        step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        say(&mut world, &mut rng, "ball");
        // Meanwhile it wanders off to its own favorite.
        mark_focus(
            &mut world.creature.attention,
            FocusMark {
                meaning: Meaning::Toy(ToyId::Sock),
                weight: 1,
                until_ms: world.elapsed_ms + 6_000,
            },
            world.elapsed_ms,
        );
        run_until(&mut world, &mut rng, 2_500, |_, _| false);
        let events = say(&mut world, &mut rng, "ball");
        assert!(events.contains(&GameEvent::WordLearned {
            word: "ball".to_owned(),
            meaning: Meaning::Toy(ToyId::Ball),
        }));
    }

    #[test]
    fn a_wedged_toy_is_freed_so_the_next_offer_reaches_it() {
        let mut world = WorldState::new(42, "Mop");
        let mut rng = SeededRandom::new(42);
        world
            .aquarium
            .toy_states
            .retain(|toy, _| *toy != ToyId::Sock);
        for object in world.aquarium.toy_states.values_mut() {
            object.position = NormalizedPosition::new(5_000, 10_000);
        }
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        world.creature.idle_life.settled_until_ms = 600_000;
        step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        run_until(&mut world, &mut rng, 4_000, |_, _| false);
        step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        assert!(
            run_until(&mut world, &mut rng, 10_000, |events, _| events
                .iter()
                .any(|event| matches!(event, GameEvent::ToyContacted { .. }))),
            "the second offer reaches the freed ball"
        );
    }

    #[test]
    fn vocabulary_size_drives_language_stage() {
        let mut world = WorldState::new(3, "Mop");
        let mut events = Vec::new();
        for (index, toy) in [ToyId::Ball, ToyId::Bell, ToyId::Sock]
            .into_iter()
            .enumerate()
        {
            let word = ["ball", "bell", "sock"][index];
            world
                .creature
                .lexicon
                .hear(word, &[(Meaning::Toy(toy), 3)], 0);
            world
                .creature
                .lexicon
                .hear(word, &[(Meaning::Toy(toy), 3)], 1);
        }
        advance_language_from_vocabulary(&mut world, &mut events);
        assert_eq!(
            world.creature.development.language_stage,
            LanguageStage::Words
        );
        assert_eq!(
            events,
            vec![GameEvent::LanguageAdvanced(LanguageStage::Words)]
        );
    }
}
