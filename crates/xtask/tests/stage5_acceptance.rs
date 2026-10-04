use beastie_core::{BeliefKind, FoodId, GameEvent, Intention, Meaning, MemoryKind, NonverbalAct};
use beastie_protocol::{
    SpeechIntent, constrained_fallback_reply, validate_reply, validate_request,
};
use beastie_session::{CommandEnvelope, GameSession, SessionCommand};

const SCENARIO: &str = include_str!("../../../fixtures/scenarios/stage5-three-day.jsonl");
const VISIBLE_SCENARIO: &str =
    include_str!("../../../fixtures/scenarios/stage5-three-day-visible.jsonl");
const AQUARIUM_SCENARIO: &str = include_str!("../../../fixtures/scenarios/aquarium-v1.jsonl");
const AQUARIUM_VISIBLE_SCENARIO: &str =
    include_str!("../../../fixtures/scenarios/aquarium-v1-visible.jsonl");
/// A seed whose ranked preferences make berry the disliked food.
const DISLIKED_BERRY_SEED: u64 = 1;

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

    // Every line the creature is asked to say is valid, and its no-model voice is too.
    for observation in &observations {
        if let Some(request) = &observation.dialogue_request {
            validate_request(request).expect("emitted request validates");
            assert!(request.speech_intent.is_some());
            validate_reply(request, constrained_fallback_reply(request))
                .expect("no-model line validates");
        }
    }

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

    // Day one: the first mention earns a curious echo, the second, in the same moment, a word.
    let first_mention = &observations[1];
    assert!(has_event(&first_mention.events, |event| {
        matches!(event, GameEvent::WordHeard { word: Some(word), echo: Some(_) } if word == "berry")
    }));
    assert!(matches!(
        first_mention
            .dialogue_request
            .as_ref()
            .and_then(|request| request.speech_intent.as_ref()),
        Some(SpeechIntent::Echo { .. })
    ));
    let lesson = &observations[3];
    assert!(lesson.events.contains(&GameEvent::WordLearned {
        word: "berry".to_owned(),
        meaning: Meaning::Food(FoodId::Berry),
    }));
    let lesson_request = lesson
        .dialogue_request
        .as_ref()
        .expect("a new word is said back");
    assert!(matches!(
        lesson_request.speech_intent,
        Some(SpeechIntent::NewWord { ref word, .. }) if word == "berry"
    ));
    assert!(
        constrained_fallback_reply(lesson_request)
            .say
            .contains("berry")
    );

    let saved_world = saved_world.expect("fixture should checkpoint before reload");
    let saved_json = saved_json.expect("fixture should encode a durable save");
    let (mut resumed, progress) =
        GameSession::resume_json(&saved_json, 1_000_000).expect("save should reload");
    assert_eq!(progress.applied_ms, 0);
    assert_eq!(resumed.world(), &saved_world);
    assert_eq!(
        resumed.world().creature.lexicon.meaning_of("berry"),
        Some(Meaning::Food(FoodId::Berry)),
        "a learned word survives the durable save"
    );

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

    // Days two and three: the taught word is understood and used back.
    for later in [&observations[11], &observations[13]] {
        assert!(has_event(&later.events, |event| matches!(
            event,
            GameEvent::Understood { word, meaning: Meaning::Food(FoodId::Berry), .. } if word == "berry"
        )));
        let request = later
            .dialogue_request
            .as_ref()
            .expect("an understood word is answered");
        assert!(matches!(
            request.speech_intent,
            Some(SpeechIntent::Answer { ref word, .. }) if word == "berry"
        ));
        assert!(request.vocabulary.iter().any(|entry| entry.word == "berry"));
    }
    assert_eq!(
        observations[13]
            .dialogue_request
            .as_ref()
            .map(|request| request.idiolect),
        Some(session.world().idiolect())
    );
    assert_eq!(
        session.world().creature.lexicon.learned_words(),
        vec![("berry".to_owned(), Meaning::Food(FoodId::Berry))]
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
