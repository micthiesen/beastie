//! UI-independent orchestration for playable and headless Beastie sessions.

use std::collections::BTreeSet;

use beastie_core::{
    Concept, FoodId, GameEvent, LanguageExposure, MemoryCue, MemoryQuery, Mood, NamingTarget,
    NormalizedPosition, OfflineProgress, PlayerEvent, Reaction, SaveGame, SeededRandom, ToyId,
    WorldState, advance_offline, step,
};
use beastie_protocol::{
    DialogueActionPhase, DialogueContext, DialogueRequest, DialogueRequestContext, DialogueTopic,
    Gesture, RecentTurn, build_dialogue_request, classify_content_boundary,
    normalize_dialogue_request, progression_max_words, validate_request,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SESSION_PROTOCOL_VERSION: u32 = 1;
pub const SESSION_SAVE_VERSION: u32 = 3;
const LEGACY_CORE_SAVE_VERSION: u32 = 1;
pub const MAX_COMMAND_BYTES: usize = 4_096;
pub const MAX_ADVANCE_MINUTES: u32 = 45;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub version: u32,
    #[serde(flatten)]
    pub command: SessionCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionCommand {
    Feed {
        food: FoodId,
    },
    DropFood {
        food: FoodId,
        position: NormalizedPosition,
    },
    Cursor {
        position: Option<NormalizedPosition>,
    },
    Name {
        target: NamingTarget,
        name: String,
    },
    Play {
        #[serde(default)]
        toy: ToyId,
    },
    Comfort,
    Tidy,
    Advance {
        minutes: u32,
    },
    Tick {
        milliseconds: u64,
    },
    Talk {
        text: String,
    },
    React {
        reaction: Reaction,
    },
    LanguageExposure {
        exposure: LanguageExposure,
    },
    Save,
    Load,
    Inspect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub version: u32,
    pub sequence: u64,
    pub events: Vec<GameEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialogue_request: Option<DialogueRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world: Option<WorldState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_json: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GameSession {
    world: WorldState,
    random: SeededRandom,
    sequence: u64,
    next_request_id: u64,
    checkpoint: Option<Checkpoint>,
}

#[derive(Debug, Clone)]
struct Checkpoint {
    world: WorldState,
    random: SeededRandom,
    next_request_id: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionSave {
    pub version: u32,
    pub saved_at_ms: u64,
    pub world: WorldState,
    pub random: SeededRandom,
    pub sequence: u64,
    pub next_request_id: u64,
}

impl SessionSave {
    pub fn to_json(&self) -> Result<String, SessionError> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(SessionError::Json)
    }

    pub fn from_json(source: &str) -> Result<Self, SessionError> {
        let mut value =
            serde_json::from_str::<serde_json::Value>(source).map_err(SessionError::Json)?;
        let version = value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if version == 2
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
            }
            if let Some(creature) = world
                .get_mut("creature")
                .and_then(serde_json::Value::as_object_mut)
            {
                creature.remove("position");
                creature.remove("movement");
            }
        }
        let mut save = serde_json::from_value::<Self>(value).map_err(SessionError::Json)?;
        if save.version == 2 {
            save.version = SESSION_SAVE_VERSION;
            save.world.save_version = beastie_core::SAVE_VERSION;
        }
        save.validate()?;
        Ok(save)
    }

    fn validate(&self) -> Result<(), SessionError> {
        if self.version != SESSION_SAVE_VERSION {
            return Err(SessionError::SaveVersion(self.version));
        }
        if self.next_request_id == 0 {
            return Err(SessionError::RequestId);
        }
        self.world.validate().map_err(SessionError::State)
    }
}

impl GameSession {
    #[must_use]
    pub fn new(seed: u64, name: impl Into<String>) -> Self {
        Self {
            world: WorldState::new(seed, name),
            random: SeededRandom::new(seed),
            sequence: 0,
            next_request_id: 1,
            checkpoint: None,
        }
    }

    #[must_use]
    pub fn world(&self) -> &WorldState {
        &self.world
    }

    #[must_use]
    pub fn capture(&self, saved_at_ms: u64) -> SessionSave {
        SessionSave {
            version: SESSION_SAVE_VERSION,
            saved_at_ms,
            world: self.world.clone(),
            random: self.random,
            sequence: self.sequence,
            next_request_id: self.next_request_id,
        }
    }

    pub fn resume(
        save: SessionSave,
        resumed_at_ms: u64,
    ) -> Result<(Self, OfflineProgress), SessionError> {
        save.validate()?;
        let offline_ms = resumed_at_ms.saturating_sub(save.saved_at_ms);
        let mut session = Self {
            world: save.world,
            random: save.random,
            sequence: save.sequence,
            next_request_id: save.next_request_id,
            checkpoint: None,
        };
        let progress = advance_offline(&mut session.world, offline_ms, &mut session.random);
        if progress.applied_ms > 0 {
            let conversation = &mut session.world.creature.conversation;
            conversation.next_talk_at_ms = conversation
                .next_talk_at_ms
                .saturating_sub(progress.applied_ms);
            conversation.contextual_follow_up_available = false;
        }
        session.world.validate().map_err(SessionError::State)?;
        Ok((session, progress))
    }

    pub fn resume_json(
        source: &str,
        resumed_at_ms: u64,
    ) -> Result<(Self, OfflineProgress), SessionError> {
        let header = serde_json::from_str::<SaveHeader>(source).map_err(SessionError::Json)?;
        match header.save_version {
            Some(LEGACY_CORE_SAVE_VERSION) => {
                let (world, random) = SaveGame::from_json(source)
                    .map_err(SessionError::LegacySave)?
                    .resume();
                return Ok((
                    Self {
                        world,
                        random,
                        sequence: 0,
                        next_request_id: 1,
                        checkpoint: None,
                    },
                    OfflineProgress {
                        requested_ms: 0,
                        applied_ms: 0,
                        events: Vec::new(),
                    },
                ));
            }
            Some(version) => {
                return Err(SessionError::LegacySave(beastie_core::SaveError::Version(
                    version,
                )));
            }
            None => {}
        }
        Self::resume(SessionSave::from_json(source)?, resumed_at_ms)
    }

    pub fn parse_command(line: &str) -> Result<CommandEnvelope, SessionError> {
        if line.len() > MAX_COMMAND_BYTES {
            return Err(SessionError::CommandTooLarge);
        }
        let envelope = serde_json::from_str::<CommandEnvelope>(line).map_err(SessionError::Json)?;
        validate_envelope(&envelope)?;
        Ok(envelope)
    }

    pub fn apply(&mut self, envelope: CommandEnvelope) -> Result<Observation, SessionError> {
        validate_envelope(&envelope)?;
        self.world.validate().map_err(SessionError::State)?;
        let next_sequence = self.sequence.saturating_add(1);
        let mut events = Vec::new();
        let mut dialogue_request = None;
        let mut inspected_world = None;
        let mut save_json = None;

        match envelope.command {
            SessionCommand::Feed { food } => {
                events = self.apply_player_event(PlayerEvent::Feed(food));
            }
            SessionCommand::DropFood { food, position } => {
                events = self.apply_player_event(PlayerEvent::DropFood { food, position });
            }
            SessionCommand::Cursor { position } => {
                events = self.apply_player_event(PlayerEvent::Cursor(position));
            }
            SessionCommand::Name { target, name } => {
                events = self.apply_player_event(PlayerEvent::Name { target, name });
            }
            SessionCommand::Play { toy } => {
                events = self.apply_player_event(PlayerEvent::Play(toy));
            }
            SessionCommand::Comfort => {
                events = self.apply_player_event(PlayerEvent::Comfort);
            }
            SessionCommand::Tidy => {
                events = self.apply_player_event(PlayerEvent::Tidy);
            }
            SessionCommand::Advance { minutes } => {
                for _ in 0..minutes {
                    events.extend(step(&mut self.world, &[], 60_000, &mut self.random));
                }
                dialogue_request = self.initiated_dialogue_request(&events)?;
                compact_advance_events(&mut events);
            }
            SessionCommand::Tick { milliseconds } => {
                events = step(&mut self.world, &[], milliseconds, &mut self.random);
                dialogue_request = self.initiated_dialogue_request(&events)?;
            }
            SessionCommand::Talk { text } => {
                let talk_events = self.apply_player_event(PlayerEvent::Talk);
                let accepted = talk_events
                    .iter()
                    .any(|event| matches!(event, GameEvent::TalkAccepted { .. }));
                events.extend(talk_events);
                if accepted && classify_content_boundary(&text).is_none() {
                    for exposure in language_exposures(&text) {
                        events.extend(
                            self.apply_player_event(PlayerEvent::LanguageExposure(exposure)),
                        );
                    }
                }
                if accepted {
                    let desired_social_act = events.iter().find_map(|event| match event {
                        GameEvent::SocialActExpressed(act) => Some(*act),
                        _ => None,
                    });
                    let mut request = build_dialogue_request(
                        &self.world,
                        &memory_query(&text),
                        DialogueRequestContext {
                            request_id: self.next_request_id,
                            mood: mood(&self.world),
                            player_said: &text,
                            desired_social_act,
                            max_words: progression_max_words(&self.world),
                            allowed_gestures: BTreeSet::from([
                                Gesture::None,
                                Gesture::LookPlayer,
                                Gesture::LookWindow,
                                Gesture::Shiver,
                                Gesture::Sleepy,
                            ]),
                        },
                    );
                    normalize_dialogue_request(&mut request);
                    validate_request(&request).map_err(SessionError::Dialogue)?;
                    self.next_request_id = self.next_request_id.saturating_add(1);
                    dialogue_request = Some(request);
                }
            }
            SessionCommand::React { reaction } => {
                events = self.apply_player_event(PlayerEvent::React(reaction));
            }
            SessionCommand::LanguageExposure { exposure } => {
                events = self.apply_player_event(PlayerEvent::LanguageExposure(exposure));
            }
            SessionCommand::Save => {
                let durable = SessionSave {
                    version: SESSION_SAVE_VERSION,
                    saved_at_ms: self.world.elapsed_ms,
                    world: self.world.clone(),
                    random: self.random,
                    sequence: next_sequence,
                    next_request_id: self.next_request_id,
                };
                save_json = Some(durable.to_json()?);
                self.checkpoint = Some(Checkpoint {
                    world: self.world.clone(),
                    random: self.random,
                    next_request_id: self.next_request_id,
                });
            }
            SessionCommand::Load => {
                let loaded = self.checkpoint.as_ref().ok_or(SessionError::NoCheckpoint)?;
                self.world = loaded.world.clone();
                self.random = loaded.random;
                self.next_request_id = loaded.next_request_id;
            }
            SessionCommand::Inspect => inspected_world = Some(self.world.clone()),
        }

        self.world.validate().map_err(SessionError::State)?;
        self.sequence = next_sequence;
        Ok(Observation {
            version: SESSION_PROTOCOL_VERSION,
            sequence: self.sequence,
            events,
            dialogue_request,
            world: inspected_world,
            save_json,
        })
    }

    fn apply_player_event(&mut self, event: PlayerEvent) -> Vec<GameEvent> {
        step(&mut self.world, &[event], 0, &mut self.random)
    }

    fn initiated_dialogue_request(
        &mut self,
        events: &[GameEvent],
    ) -> Result<Option<DialogueRequest>, SessionError> {
        let Some(reason) = events.iter().find_map(|event| match event {
            GameEvent::InitiatedTalk(reason) => Some(*reason),
            _ => None,
        }) else {
            return Ok(None);
        };
        let mut request = build_dialogue_request(
            &self.world,
            &MemoryQuery {
                cues: BTreeSet::new(),
                limit: 0,
            },
            DialogueRequestContext {
                request_id: self.next_request_id,
                mood: mood(&self.world),
                player_said: "",
                desired_social_act: None,
                max_words: progression_max_words(&self.world),
                allowed_gestures: BTreeSet::from([
                    Gesture::None,
                    Gesture::LookPlayer,
                    Gesture::LookWindow,
                    Gesture::Shiver,
                    Gesture::Sleepy,
                ]),
            },
        );
        request.context = DialogueContext {
            recent_turns: vec![RecentTurn {
                turn_id: self.next_request_id,
                topic: initiated_topic(reason),
                action_phase: DialogueActionPhase::Recover,
                selected_memory: None,
                selected_belief: None,
                selected_fact_ids: Vec::new(),
                fallback_lane: None,
            }],
            ..DialogueContext::default()
        };
        normalize_dialogue_request(&mut request);
        validate_request(&request).map_err(SessionError::Dialogue)?;
        self.next_request_id = self.next_request_id.saturating_add(1);
        Ok(Some(request))
    }
}

/// Collapses idempotent high-frequency notifications produced by accelerated time.
///
/// `NeedChanged` means "read the current authoritative needs" rather than describing a
/// historical delta, so one notification represents the final state just as well as hundreds.
/// All transition and identity-bearing events retain their original order and multiplicity.
fn compact_advance_events(events: &mut Vec<GameEvent>) {
    let mut emitted_need_change = false;
    events.retain(|event| {
        if matches!(event, GameEvent::NeedChanged) {
            let keep = !emitted_need_change;
            emitted_need_change = true;
            keep
        } else {
            true
        }
    });
}

fn initiated_topic(reason: beastie_core::InitiativeReason) -> DialogueTopic {
    match reason {
        beastie_core::InitiativeReason::Hunger => DialogueTopic::Food,
        beastie_core::InitiativeReason::Loneliness => DialogueTopic::Greeting,
        beastie_core::InitiativeReason::Curiosity => DialogueTopic::Other,
        beastie_core::InitiativeReason::Ritual => DialogueTopic::Ritual,
        beastie_core::InitiativeReason::Request => DialogueTopic::Other,
    }
}

fn language_exposures(text: &str) -> Vec<LanguageExposure> {
    let words = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let contains = |terms: &[&str]| {
        terms
            .iter()
            .any(|term| words.iter().any(|word| word == term))
    };
    let mut exposures = Vec::new();
    if contains(&["fuck", "fucking", "fucked", "damn", "bastard"]) {
        exposures.push(LanguageExposure::Profanity);
    }
    if contains(&["shit", "shitty", "piss", "fart", "ass", "arse", "butt"]) {
        exposures.push(LanguageExposure::Crudeness);
    }
    let normalized = words.join(" ");
    if [
        "that is what she said",
        "that s what she said",
        "nice package",
        "come to bed",
        "under the sheets",
    ]
    .iter()
    .any(|phrase| normalized.contains(phrase))
    {
        exposures.push(LanguageExposure::Innuendo);
    }
    exposures
}

#[derive(Deserialize)]
struct SaveHeader {
    save_version: Option<u32>,
}

fn validate_envelope(envelope: &CommandEnvelope) -> Result<(), SessionError> {
    if envelope.version != SESSION_PROTOCOL_VERSION {
        return Err(SessionError::Version(envelope.version));
    }
    match &envelope.command {
        SessionCommand::Advance { minutes } if *minutes > MAX_ADVANCE_MINUTES => {
            Err(SessionError::Advance(*minutes))
        }
        SessionCommand::Tick { milliseconds }
            if *milliseconds > u64::from(MAX_ADVANCE_MINUTES) * 60_000 =>
        {
            Err(SessionError::Tick(*milliseconds))
        }
        SessionCommand::Talk { text } if text.chars().count() > 512 => {
            Err(SessionError::TalkTooLong)
        }
        _ => Ok(()),
    }
}

fn memory_query(text: &str) -> MemoryQuery {
    let normalized = text.to_lowercase();
    let mut cues = BTreeSet::new();
    if normalized.contains("berry") || normalized.contains("berries") {
        cues.insert(MemoryCue::Food(FoodId::Berry));
        cues.insert(MemoryCue::Concept(Concept::Food));
    }
    if normalized.contains("mushroom") {
        cues.insert(MemoryCue::Food(FoodId::Mushroom));
        cues.insert(MemoryCue::Concept(Concept::Food));
    }
    if normalized.contains("pellet") {
        cues.insert(MemoryCue::Food(FoodId::Pellet));
        cues.insert(MemoryCue::Concept(Concept::Food));
    }
    if normalized.contains("play") || normalized.contains("toy") {
        cues.insert(MemoryCue::Concept(Concept::Toy));
    }
    if normalized.contains("remember") || normalized.contains("yesterday") {
        cues.insert(MemoryCue::Concept(Concept::Yesterday));
    }
    MemoryQuery { cues, limit: 8 }
}

fn mood(world: &WorldState) -> &'static str {
    match world.mood() {
        Mood::Content => "content",
        Mood::Curious => "curious",
        Mood::Hungry => "hungry",
        Mood::Sleepy => "sleepy",
        Mood::Lonely => "lonely",
        Mood::Resentful => "resentful",
    }
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("session protocol version {0} is unsupported")]
    Version(u32),
    #[error("command exceeds {MAX_COMMAND_BYTES} bytes")]
    CommandTooLarge,
    #[error("command JSON is malformed: {0}")]
    Json(serde_json::Error),
    #[error("cannot advance {0} minutes in one command")]
    Advance(u32),
    #[error("cannot tick {0} milliseconds in one command")]
    Tick(u64),
    #[error("talk text exceeds 512 characters")]
    TalkTooLong,
    #[error("no in-memory checkpoint exists")]
    NoCheckpoint,
    #[error("session save version {0} is unsupported")]
    SaveVersion(u32),
    #[error("next dialogue request ID must be nonzero")]
    RequestId,
    #[error("legacy save migration failed: {0}")]
    LegacySave(beastie_core::SaveError),
    #[error("session state became invalid: {0}")]
    State(beastie_core::StateValidationError),
    #[error("dialogue request is invalid: {0}")]
    Dialogue(beastie_protocol::ValidationError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(command: SessionCommand) -> CommandEnvelope {
        CommandEnvelope {
            version: SESSION_PROTOCOL_VERSION,
            command,
        }
    }

    #[test]
    fn invalid_command_does_not_mutate_the_session() {
        let mut session = GameSession::new(42, "Mop");
        let before = session.world().clone();
        let result = session.apply(command(SessionCommand::Advance {
            minutes: MAX_ADVANCE_MINUTES + 1,
        }));
        assert!(matches!(result, Err(SessionError::Advance(_))));
        assert_eq!(session.world(), &before);
    }

    #[test]
    fn long_advance_coalesces_need_notifications_without_changing_simulation() {
        let mut aggregated = GameSession::new(42, "Mop");
        aggregated.world.creature.needs.hunger = 1.0;
        aggregated
            .apply(command(SessionCommand::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(7_000, 2_000),
            }))
            .expect("food should drop");
        let mut raw = aggregated.clone();
        let mut raw_events = Vec::new();
        for _ in 0..15 {
            raw_events.extend(step(&mut raw.world, &[], 60_000, &mut raw.random));
        }
        assert_eq!(
            raw_events
                .iter()
                .filter(|event| matches!(event, GameEvent::NeedChanged))
                .count(),
            900,
            "reproduction should expose one redundant notification per fixed tick"
        );

        let observation = aggregated
            .apply(command(SessionCommand::Advance { minutes: 15 }))
            .expect("advance should apply");
        assert_eq!(aggregated.world, raw.world);
        assert_eq!(aggregated.random, raw.random);
        assert_eq!(
            observation
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::NeedChanged))
                .count(),
            1
        );

        let raw_semantic = raw_events
            .into_iter()
            .filter(|event| !matches!(event, GameEvent::NeedChanged))
            .collect::<Vec<_>>();
        let aggregated_semantic = observation
            .events
            .iter()
            .filter(|event| !matches!(event, GameEvent::NeedChanged))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(aggregated_semantic, raw_semantic);
        assert_eq!(observation.events.len(), aggregated_semantic.len() + 1);
        assert!(
            observation.events.len() <= 64,
            "accelerated output must stay bounded"
        );
        assert!(
            observation
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ActionPhaseChanged { .. }))
        );
        assert!(observation.events.iter().any(|event| matches!(
            event,
            GameEvent::FoodConsumed(_) | GameEvent::FoodRejected(_)
        )));
        assert!(
            observation
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::MemoryCreated(_)))
        );
    }

    #[test]
    fn event_compaction_preserves_non_idempotent_order_and_multiplicity() {
        let phase = GameEvent::ActionPhaseChanged {
            from: Some(beastie_core::ActionPhase::Notice),
            to: beastie_core::ActionPhase::Brake,
        };
        let memory = GameEvent::MemoryCreated(beastie_core::MemoryId(7));
        let mut events = vec![
            GameEvent::NeedChanged,
            phase.clone(),
            GameEvent::NeedChanged,
            memory.clone(),
            phase.clone(),
            GameEvent::NeedChanged,
        ];
        compact_advance_events(&mut events);
        assert_eq!(
            events,
            vec![GameEvent::NeedChanged, phase.clone(), memory, phase]
        );
    }

    #[test]
    fn every_primary_verb_is_accepted() {
        let mut session = GameSession::new(42, "Mop");
        for action in [
            SessionCommand::Feed {
                food: FoodId::Pellet,
            },
            SessionCommand::Play { toy: ToyId::Ball },
            SessionCommand::Comfort,
            SessionCommand::Tidy,
            SessionCommand::Talk {
                text: "hello".to_owned(),
            },
        ] {
            session.apply(command(action)).expect("verb should apply");
        }
    }

    #[test]
    fn save_and_load_restore_world_and_random_state() {
        let mut session = GameSession::new(99, "Mrrp");
        session
            .apply(command(SessionCommand::Play { toy: ToyId::Ball }))
            .expect("play should apply");
        let saved_world = session.world().clone();
        session
            .apply(command(SessionCommand::Save))
            .expect("save should apply");
        session
            .apply(command(SessionCommand::Comfort))
            .expect("comfort should apply");
        assert_ne!(session.world(), &saved_world);
        session
            .apply(command(SessionCommand::Load))
            .expect("load should apply");
        assert_eq!(session.world(), &saved_world);
    }

    #[test]
    fn durable_mid_food_approach_resumes_with_identical_events() {
        let mut original = GameSession::new(101, "Swim");
        original
            .apply(command(SessionCommand::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(8_000, 2_000),
            }))
            .expect("drop should apply");
        original
            .apply(command(SessionCommand::Tick {
                milliseconds: 6_000,
            }))
            .expect("approach ticks should apply");
        assert_eq!(
            original
                .world()
                .creature
                .aquarium
                .action
                .map(|action| action.phase),
            Some(beastie_core::ActionPhase::Approach)
        );
        let saved_at = original.world().elapsed_ms;
        let encoded = original
            .capture(saved_at)
            .to_json()
            .expect("mid-action save should encode");
        let (mut resumed, progress) =
            GameSession::resume_json(&encoded, saved_at).expect("resume should not add absence");
        assert_eq!(progress.applied_ms, 0);

        let expected = original
            .apply(command(SessionCommand::Tick {
                milliseconds: 10_000,
            }))
            .expect("original continuation");
        let actual = resumed
            .apply(command(SessionCommand::Tick {
                milliseconds: 10_000,
            }))
            .expect("resumed continuation");
        assert_eq!(actual.events, expected.events);
        assert_eq!(resumed.world(), original.world());
    }

    #[test]
    fn talk_projects_real_berry_memories() {
        let mut session = GameSession::new(99, "Mrrp");
        session
            .apply(command(SessionCommand::Feed {
                food: FoodId::Berry,
            }))
            .expect("feed should apply");
        for _ in 0..4 {
            session
                .apply(command(SessionCommand::Advance { minutes: 15 }))
                .expect("time should advance");
            if session
                .world()
                .creature
                .preferences
                .contains_key(&FoodId::Berry)
            {
                break;
            }
        }
        assert!(
            session
                .world()
                .creature
                .preferences
                .contains_key(&FoodId::Berry)
        );
        let observation = session
            .apply(command(SessionCommand::Talk {
                text: "Remember the berry?".to_owned(),
            }))
            .expect("talk should apply");
        let request = observation
            .dialogue_request
            .expect("talk should create a request");
        assert!(!request.candidate_memories.is_empty());
        assert!(
            request
                .candidate_memories
                .iter()
                .any(|memory| memory.fact.contains("berry"))
        );
    }

    #[test]
    fn legacy_play_json_defaults_to_the_ball() {
        let parsed = GameSession::parse_command(r#"{"version":1,"command":"play"}"#)
            .expect("play without a toy remains compatible");
        assert_eq!(parsed.command, SessionCommand::Play { toy: ToyId::Ball });
    }

    #[test]
    fn tick_preserves_subsecond_remainder() {
        let mut batched = GameSession::new(55, "Tock");
        let mut split = GameSession::new(55, "Tock");
        batched
            .apply(command(SessionCommand::Tick {
                milliseconds: 1_500,
            }))
            .expect("batched tick");
        split
            .apply(command(SessionCommand::Tick { milliseconds: 900 }))
            .expect("first partial tick");
        split
            .apply(command(SessionCommand::Tick { milliseconds: 600 }))
            .expect("second partial tick");
        assert_eq!(batched.world(), split.world());
        assert_eq!(batched.world().simulation_remainder_ms, 500);
    }

    #[test]
    fn durable_save_resumes_in_a_new_process_once_and_preserves_request_ids() {
        let mut original = GameSession::new(88, "Persist");
        let first_talk = original
            .apply(command(SessionCommand::Talk {
                text: "hello".to_owned(),
            }))
            .expect("first talk");
        assert_eq!(
            first_talk
                .dialogue_request
                .expect("dialogue request")
                .request_id,
            1
        );
        original
            .apply(command(SessionCommand::Tick {
                milliseconds: 1_500,
            }))
            .expect("tick before save");

        let saved_at = 1_000_000;
        let encoded = original
            .capture(saved_at)
            .to_json()
            .expect("durable session save");
        let resumed_at = saved_at + beastie_core::MAX_OFFLINE_MS + 99_000;
        let (mut resumed, progress) =
            GameSession::resume_json(&encoded, resumed_at).expect("new process should resume");
        assert_eq!(progress.applied_ms, beastie_core::MAX_OFFLINE_MS);
        assert_eq!(resumed.world().simulation_remainder_ms, 500);
        assert_eq!(
            resumed
                .world()
                .creature
                .memories
                .iter()
                .filter(|memory| {
                    memory.kind == beastie_core::MemoryKind::PlayerReturnedAfterAbsence
                })
                .count(),
            1
        );
        let second_talk = resumed
            .apply(command(SessionCommand::Talk {
                text: "again".to_owned(),
            }))
            .expect("second talk");
        assert_eq!(second_talk.sequence, 3);
        assert_eq!(
            second_talk
                .dialogue_request
                .expect("dialogue request")
                .request_id,
            2
        );

        let after_resume = resumed
            .capture(resumed_at)
            .to_json()
            .expect("second durable save");
        let (resumed_again, no_progress) = GameSession::resume_json(&after_resume, resumed_at)
            .expect("same timestamp should not reapply absence");
        assert_eq!(no_progress.applied_ms, 0);
        assert!(no_progress.events.is_empty());
        assert_eq!(resumed_again.world(), resumed.world());
    }

    #[test]
    fn durable_save_rejects_invalid_version_and_request_id() {
        let session = GameSession::new(1, "Strict");
        let mut save = session.capture(5);
        save.version = 1;
        assert!(matches!(save.to_json(), Err(SessionError::SaveVersion(_))));

        save.version = SESSION_SAVE_VERSION;
        save.next_request_id = 0;
        assert!(matches!(save.to_json(), Err(SessionError::RequestId)));
    }

    #[test]
    fn zero_offline_resume_preserves_future_rng_exactly() {
        let mut uninterrupted = GameSession::new(404, "Random");
        uninterrupted
            .apply(command(SessionCommand::Tick {
                milliseconds: 17_500,
            }))
            .expect("consume simulation randomness");
        let timestamp = 55_000;
        let encoded = uninterrupted
            .capture(timestamp)
            .to_json()
            .expect("session save");
        let (mut resumed, progress) =
            GameSession::resume_json(&encoded, timestamp).expect("zero-offline resume");
        assert_eq!(progress.applied_ms, 0);

        let future = command(SessionCommand::Play { toy: ToyId::Bell });
        let expected_play = uninterrupted
            .apply(future.clone())
            .expect("uninterrupted play");
        let actual_play = resumed.apply(future).expect("resumed play");
        assert_eq!(actual_play.events, expected_play.events);
        let tick = command(SessionCommand::Tick {
            milliseconds: 4_000,
        });
        let expected_tick = uninterrupted
            .apply(tick.clone())
            .expect("uninterrupted tick");
        let actual_tick = resumed.apply(tick).expect("resumed tick");
        assert_eq!(actual_tick.events, expected_tick.events);
        assert_eq!(resumed.world(), uninterrupted.world());
    }

    #[test]
    fn version_one_fixture_migrates_without_inventing_offline_time() {
        let legacy = include_str!("../../../fixtures/saves/v1-berry-ball.json");
        let (resumed, progress) =
            GameSession::resume_json(legacy, u64::MAX).expect("v1 fixture should migrate");

        assert_eq!(progress.requested_ms, 0);
        assert_eq!(progress.applied_ms, 0);
        assert!(progress.events.is_empty());
        assert_eq!(resumed.world().creature.name, "Keepsake");
        assert_eq!(resumed.world().creature.traits.boldness, 0.22);
        assert_eq!(resumed.world().creature.relationship.trust, 0.42);
        assert_eq!(resumed.world().creature.preferences[&FoodId::Berry], -0.75);
        assert_eq!(resumed.random, SeededRandom::new(123_456_789));
        assert_eq!(
            resumed.world().creature.aquarium.position,
            beastie_core::NormalizedPosition::new(5_000, 4_500)
        );
        assert_eq!(resumed.world().creature.aquarium.action, None);
        assert!(resumed.world().creature.memories.iter().any(|memory| {
            memory.kind == beastie_core::MemoryKind::PlayedWith { toy: ToyId::Ball }
        }));
        assert_eq!(resumed.world().creature.beliefs.len(), 1);
        assert!(
            !resumed.world().creature.memories.iter().any(|memory| {
                memory.kind == beastie_core::MemoryKind::PlayerReturnedAfterAbsence
            })
        );
    }

    #[test]
    fn migrated_v1_save_has_deterministic_future_continuation() {
        let legacy = include_str!("../../../fixtures/saves/v1-berry-ball.json");
        let timestamp = 8_000_000;
        let (mut migrated, _) =
            GameSession::resume_json(legacy, timestamp).expect("v1 fixture should migrate");
        let current = migrated
            .capture(timestamp)
            .to_json()
            .expect("migrated save should encode as v2");
        let (mut reloaded, progress) =
            GameSession::resume_json(&current, timestamp).expect("v2 save should reload");
        assert_eq!(progress.applied_ms, 0);

        for future in [
            command(SessionCommand::Tick {
                milliseconds: 8_000,
            }),
            command(SessionCommand::Play { toy: ToyId::Bell }),
            command(SessionCommand::Tick {
                milliseconds: 4_000,
            }),
        ] {
            let expected = migrated.apply(future.clone()).expect("migrated future");
            let actual = reloaded.apply(future).expect("reloaded future");
            assert_eq!(actual, expected);
        }
        assert_eq!(reloaded.world, migrated.world);
        assert_eq!(reloaded.random, migrated.random);
    }

    #[test]
    fn future_and_invalid_v1_saves_fail_without_mutating_a_live_session() {
        let legacy = include_str!("../../../fixtures/saves/v1-berry-ball.json");
        let future = legacy.replacen("\"save_version\": 1", "\"save_version\": 99", 1);
        assert!(matches!(
            GameSession::resume_json(&future, 0),
            Err(SessionError::LegacySave(beastie_core::SaveError::Version(
                99
            )))
        ));

        let invalid = legacy.replace("\"berry\": -0.75", "\"berry\": 2.0");
        assert!(matches!(
            GameSession::resume_json(&invalid, 0),
            Err(SessionError::LegacySave(beastie_core::SaveError::State(
                beastie_core::StateValidationError::Preference
            )))
        ));

        let live = GameSession::new(7, "Untouched");
        let before = live.world().clone();
        let _ = GameSession::resume_json(&future, 0);
        let _ = GameSession::resume_json(&invalid, 0);
        assert_eq!(live.world(), &before);
    }

    #[test]
    fn checkpoint_load_restores_state_without_rewinding_transport_sequence() {
        let mut session = GameSession::new(9, "Sequence");
        let save_observation = session
            .apply(command(SessionCommand::Save))
            .expect("save command");
        assert_eq!(save_observation.sequence, 1);
        let durable_json = save_observation
            .save_json
            .expect("save command should return durable JSON");
        let durable = SessionSave::from_json(&durable_json).expect("durable session save");
        assert_eq!(durable.sequence, 1);
        assert_eq!(durable.world, *session.world());
        session
            .apply(command(SessionCommand::Load))
            .expect("load command");
        let inspect = session
            .apply(command(SessionCommand::Inspect))
            .expect("next command");
        assert_eq!(inspect.sequence, 3);
    }

    #[test]
    fn save_observation_round_trips_in_a_fresh_session() {
        let mut session = GameSession::new(64, "Portable");
        session
            .apply(command(SessionCommand::Tick {
                milliseconds: 1_500,
            }))
            .expect("tick before save");
        let saved_world = session.world().clone();
        let observation = session
            .apply(command(SessionCommand::Save))
            .expect("save command");
        let source = observation.save_json.expect("durable JSON");
        let decoded = SessionSave::from_json(&source).expect("decode save observation");
        assert_eq!(decoded.sequence, observation.sequence);
        assert_eq!(decoded.world, saved_world);

        let (resumed, progress) = GameSession::resume_json(&source, decoded.saved_at_ms)
            .expect("fresh session should resume");
        assert_eq!(progress.applied_ms, 0);
        assert_eq!(resumed.world(), &saved_world);
    }

    #[test]
    fn ignored_talk_is_silent_and_one_reaction_follow_up_is_emitted() {
        let mut session = GameSession::new(70, "Sparse");
        let first = session
            .apply(command(SessionCommand::Talk {
                text: "hello".to_owned(),
            }))
            .expect("first talk");
        assert!(first.dialogue_request.is_some());
        assert!(first.events.iter().any(|event| matches!(
            event,
            GameEvent::TalkAccepted {
                contextual_follow_up: false
            }
        )));

        let ignored = session
            .apply(command(SessionCommand::Talk {
                text: "again already".to_owned(),
            }))
            .expect("ignored talk command");
        assert_eq!(ignored.dialogue_request, None);
        assert!(ignored.events.contains(&GameEvent::TalkIgnored));

        session
            .apply(command(SessionCommand::React {
                reaction: Reaction::Laugh,
            }))
            .expect("reaction");
        let follow_up = session
            .apply(command(SessionCommand::Talk {
                text: "well?".to_owned(),
            }))
            .expect("contextual follow-up");
        assert_eq!(
            follow_up
                .dialogue_request
                .expect("follow-up request")
                .request_id,
            2
        );
        assert!(follow_up.events.iter().any(|event| matches!(
            event,
            GameEvent::TalkAccepted {
                contextual_follow_up: true
            }
        )));

        session
            .apply(command(SessionCommand::React {
                reaction: Reaction::Laugh,
            }))
            .expect("second reaction");
        let exhausted = session
            .apply(command(SessionCommand::Talk {
                text: "and again?".to_owned(),
            }))
            .expect("exhausted follow-up");
        assert_eq!(exhausted.dialogue_request, None);
        assert!(exhausted.events.contains(&GameEvent::TalkIgnored));
    }

    #[test]
    fn language_exposure_command_is_typed_and_carries_no_raw_content() {
        let parsed = GameSession::parse_command(
            r#"{"version":1,"command":"language_exposure","exposure":"profanity"}"#,
        )
        .expect("typed exposure command");
        assert_eq!(
            parsed.command,
            SessionCommand::LanguageExposure {
                exposure: LanguageExposure::Profanity,
            }
        );
        assert!(
            GameSession::parse_command(
                r#"{"version":1,"command":"language_exposure","exposure":"profanity","text":"raw"}"#
            )
            .is_err()
        );

        let mut session = GameSession::new(71, "Learner");
        let observation = session.apply(parsed).expect("apply exposure");
        assert!(
            observation
                .events
                .contains(&GameEvent::LanguageExposureRegistered(
                    LanguageExposure::Profanity
                ))
        );
        assert!(session.world().creature.social_habits.profanity > 0.02);
        assert!(session.world().creature.memories.is_empty());
    }

    #[test]
    fn drop_food_uses_normalized_position_and_emits_aquarium_events() {
        let mut session = GameSession::new(12, "Aquarium");
        let parsed = GameSession::parse_command(
            r#"{"version":1,"command":"drop_food","food":"pellet","position":{"x":12000,"y":-5}}"#,
        )
        .expect("drop food command");
        let observation = session.apply(parsed).expect("drop food should apply");
        assert!(
            observation
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::FoodDropped { .. }))
        );
        let object = session
            .world()
            .aquarium
            .objects
            .values()
            .find_map(|object| match object {
                beastie_core::WorldObject::Food(food) => Some(food),
                _ => None,
            })
            .expect("food object");
        assert_eq!(object.position, NormalizedPosition::new(10_000, 0));
    }

    #[test]
    fn motivated_tick_emits_one_typed_initiated_dialogue_request() {
        let mut seed = GameSession::new(222, "Initiator").capture(0);
        seed.world.creature.needs.hunger = 0.9;
        let (mut session, _) = GameSession::resume(seed, 0).expect("resume mutated fixture");
        let first = session
            .apply(command(SessionCommand::Tick {
                milliseconds: 1_000,
            }))
            .expect("motivated tick");
        let request = first.dialogue_request.expect("initiated request");
        assert!(request.player_said.is_empty());
        assert_eq!(
            request.context.recent_turns[0].topic,
            beastie_protocol::DialogueTopic::Food
        );
        validate_request(&request).expect("initiated request validates");
        let second = session
            .apply(command(SessionCommand::Tick {
                milliseconds: 1_000,
            }))
            .expect("quiet follow-up tick");
        assert!(second.dialogue_request.is_none());
    }

    #[test]
    fn ordinary_player_language_teaches_permitted_lanes_without_storing_raw_text() {
        let mut session = GameSession::new(75, "Bad Influence");
        let observation = session
            .apply(command(SessionCommand::Talk {
                text: "Fuck, that shitty bell has a nice package.".to_owned(),
            }))
            .expect("permitted rough language should be accepted");

        for exposure in [
            LanguageExposure::Profanity,
            LanguageExposure::Crudeness,
            LanguageExposure::Innuendo,
        ] {
            assert!(
                observation
                    .events
                    .contains(&GameEvent::LanguageExposureRegistered(exposure))
            );
        }
        assert!(session.world().creature.social_habits.profanity > 0.02);
        assert!(session.world().creature.social_habits.crudeness > 0.02);
        assert!(session.world().creature.social_habits.sexual_innuendo > 0.01);
        assert!(session.world().creature.memories.is_empty());
        let save = session.capture(session.world().elapsed_ms);
        assert!(!save.to_json().expect("save JSON").contains("nice package"));
    }

    #[test]
    fn ignored_talk_cannot_farm_language_exposure_during_cooldown() {
        let mut session = GameSession::new(76, "Selective Hearing");
        session
            .apply(command(SessionCommand::Talk {
                text: "hello".to_owned(),
            }))
            .expect("first talk");
        let before = session.world().creature.social_habits;
        let ignored = session
            .apply(command(SessionCommand::Talk {
                text: "fuck that shitty nice package".to_owned(),
            }))
            .expect("cooldown talk");

        assert!(ignored.events.contains(&GameEvent::TalkIgnored));
        assert!(
            !ignored
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::LanguageExposureRegistered(_)))
        );
        assert_eq!(session.world().creature.social_habits, before);
    }

    #[test]
    fn prohibited_talk_input_is_normalized_before_request_serialization() {
        let mut session = GameSession::new(72, "Boundary");
        let observation = session
            .apply(command(SessionCommand::Talk {
                text: "go kill yourself".to_owned(),
            }))
            .expect("boundary input produces a constrained request");
        let request = observation.dialogue_request.expect("accepted talk request");
        assert!(request.player_said.is_empty());
        assert_eq!(
            request.input_rejection,
            Some(beastie_protocol::ContentBoundaryViolation::SelfHarmEncouragement)
        );
        let serialized = serde_json::to_string(&request).expect("serialize request");
        assert!(!serialized.contains("go kill yourself"));
        assert!(session.world().creature.memories.is_empty());
    }
}
