use std::collections::BTreeSet;

use beastie_core::{
    BeliefKind, Concept, FoodId, GameEvent, Intention, LanguageStage, MemoryKind, NonverbalAct,
    Reaction, SocialAct,
};
use beastie_session::{CommandEnvelope, GameSession, SessionCommand};

const SCENARIO: &str = include_str!("../../../fixtures/scenarios/stage5-three-day.jsonl");
const VISIBLE_SCENARIO: &str =
    include_str!("../../../fixtures/scenarios/stage5-three-day-visible.jsonl");
const AQUARIUM_SCENARIO: &str = include_str!("../../../fixtures/scenarios/aquarium-v1.jsonl");
const AQUARIUM_VISIBLE_SCENARIO: &str =
    include_str!("../../../fixtures/scenarios/aquarium-v1-visible.jsonl");
const DISLIKED_BERRY_SEED: u64 = 8;

fn commands() -> Vec<CommandEnvelope> {
    SCENARIO
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| GameSession::parse_command(line).expect("acceptance fixture command is valid"))
        .collect()
}

fn has_event(events: &[GameEvent], expected: impl Fn(&GameEvent) -> bool) -> bool {
    events.iter().any(expected)
}

#[test]
fn visible_scenario_wraps_the_same_semantic_arc() {
    let semantic_lines = semantic_lines;
    assert_eq!(semantic_lines(VISIBLE_SCENARIO), semantic_lines(SCENARIO));
}

#[test]
fn aquarium_visible_scenario_wraps_the_same_semantic_arc() {
    assert_eq!(
        semantic_lines(AQUARIUM_VISIBLE_SCENARIO),
        semantic_lines(AQUARIUM_SCENARIO)
    );
}

fn semantic_lines(source: &str) -> Vec<serde_json::Value> {
    let mut commands = Vec::<serde_json::Value>::new();
    for line in source.lines().filter(|line| !line.trim().is_empty()) {
        let mut value: serde_json::Value =
            serde_json::from_str(line).expect("scenario line is valid JSON");
        match value.get("command").and_then(serde_json::Value::as_str) {
            Some("capture" | "inspect") => continue,
            Some("tick") => {
                let milliseconds = value["milliseconds"]
                    .as_u64()
                    .expect("tick has milliseconds");
                if let Some(previous) = commands.last_mut().filter(|previous| {
                    previous.get("command").and_then(serde_json::Value::as_str) == Some("tick")
                }) {
                    previous["milliseconds"] = serde_json::Value::from(
                        previous["milliseconds"]
                            .as_u64()
                            .expect("prior tick has milliseconds")
                            + milliseconds,
                    );
                } else {
                    commands.push(value);
                }
            }
            _ => commands.push(std::mem::take(&mut value)),
        }
    }
    commands
}

#[test]
fn three_day_acceptance_arc_is_deterministic_and_grounded() {
    let mut session = GameSession::new(DISLIKED_BERRY_SEED, "Mop");
    let mut observations = Vec::new();
    let mut saved_world = None;
    let mut saved_json = None;

    for (index, command) in commands().into_iter().enumerate() {
        let observation = session
            .apply(command)
            .expect("acceptance command should apply");
        if index == 7 {
            saved_world = Some(session.world().clone());
            saved_json = Some(
                session
                    .capture(1_000_000)
                    .to_json()
                    .expect("durable acceptance save should encode"),
            );
        }
        observations.push(observation);
    }

    assert_eq!(observations.len(), 17);

    let berry_preference = session
        .world()
        .creature
        .preferences
        .get(&FoodId::Berry)
        .copied()
        .expect("berry preference should be learned");
    assert!(
        berry_preference < -0.35,
        "berry preference: {berry_preference}"
    );
    assert!(session.world().creature.memories.iter().any(|memory| {
        memory.kind
            == (MemoryKind::RejectedFood {
                food: FoodId::Berry,
            })
    }));
    assert!(
        session
            .world()
            .creature
            .beliefs
            .iter()
            .any(|belief| belief.kind == BeliefKind::FoodIsATrick)
    );

    let first_talk = &observations[4];
    assert!(has_event(&first_talk.events, |event| {
        matches!(event, GameEvent::SocialActExpressed(SocialAct::Neutral))
    }));
    let first_request = first_talk
        .dialogue_request
        .as_ref()
        .expect("first talk should create a dialogue request");
    assert!(
        first_request
            .candidate_memories
            .iter()
            .any(|memory| { memory.id.0 == 1 && memory.fact.contains("pushed away the berry") })
    );

    assert!(session.world().creature.memories.iter().any(|memory| {
        memory.kind
            == (MemoryKind::PlayerReacted {
                reaction: Reaction::Laugh,
                to: SocialAct::Neutral,
            })
    }));
    assert!(has_event(&observations[6].events, |event| {
        matches!(event, GameEvent::SocialActExpressed(SocialAct::Provocation))
    }));

    let saved_world = saved_world.expect("fixture should checkpoint before reload");
    let saved_json = saved_json.expect("fixture should encode a durable save");
    let (mut resumed, progress) =
        GameSession::resume_json(&saved_json, 1_000_000).expect("save should reload");
    assert_eq!(progress.applied_ms, 0);
    assert_eq!(resumed.world(), &saved_world);

    let mut uninterrupted = GameSession::new(DISLIKED_BERRY_SEED, "Mop");
    for command in commands().into_iter().take(8) {
        uninterrupted
            .apply(command)
            .expect("prefix should apply before durable reload comparison");
    }
    let advance = CommandEnvelope {
        version: 1,
        command: SessionCommand::Advance { minutes: 15 },
    };
    let resumed_events = resumed
        .apply(advance.clone())
        .expect("reloaded day two advance should apply")
        .events;
    let uninterrupted_events = uninterrupted
        .apply(advance)
        .expect("uninterrupted day two advance should apply")
        .events;
    assert_eq!(resumed_events, uninterrupted_events);
    assert_eq!(resumed.world(), uninterrupted.world());

    // The command boundary's in-memory Load and the durable JSON reload both preserve state.
    assert_eq!(observations[9].events, Vec::<GameEvent>::new());
    assert_eq!(observations[10].sequence, 11);
    assert_eq!(observations[10].events, {
        let mut expected = GameSession::new(DISLIKED_BERRY_SEED, "Mop");
        for command in commands().into_iter().take(8) {
            expected.apply(command).expect("prefix should apply");
        }
        expected
            .apply(CommandEnvelope {
                version: 1,
                command: SessionCommand::Advance { minutes: 15 },
            })
            .expect("day two advance should apply")
            .events
    });

    assert_eq!(session.world().active_day(), 3);
    assert_eq!(session.world().creature.development.active_days_reached, 3);
    assert_eq!(
        session.world().creature.development.language_stage,
        LanguageStage::Phrases
    );
    assert!(
        session
            .world()
            .creature
            .known_concepts
            .is_superset(&BTreeSet::from([Concept::Yesterday]))
    );

    let later_talk = &observations[13];
    assert!(has_event(&later_talk.events, |event| {
        matches!(event, GameEvent::SocialActExpressed(SocialAct::Provocation))
    }));
    let later_request = later_talk
        .dialogue_request
        .as_ref()
        .expect("later talk should create a dialogue request");
    assert_eq!(later_request.idiolect, session.world().idiolect());
    assert!(
        later_request
            .candidate_memories
            .iter()
            .any(|memory| memory.id.0 == 1)
    );

    let rejection = &observations[15];
    assert!(has_event(&rejection.events, |event| {
        matches!(event, GameEvent::FoodRejected(FoodId::Berry))
    }));
    assert!(has_event(&rejection.events, |event| {
        matches!(
            event,
            GameEvent::NonverbalAct(NonverbalAct::PushFoodAway(FoodId::Berry))
        )
    }));
    assert_ne!(
        session.world().creature.current_intention,
        Intention::Eat,
        "the rejected berry must not become a delayed eating intention"
    );
}
