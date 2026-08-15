//! UI-independent orchestration for playable and headless Beastie sessions.

use std::collections::BTreeSet;

use beastie_core::{
    Concept, FoodId, GameEvent, MemoryCue, MemoryQuery, Mood, OfflineProgress, PlayerEvent,
    Reaction, SeededRandom, ToyId, WorldState, advance_offline, step,
};
use beastie_protocol::{
    DialogueRequest, DialogueRequestContext, Gesture, build_dialogue_request, validate_request,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SESSION_PROTOCOL_VERSION: u32 = 1;
pub const SESSION_SAVE_VERSION: u32 = 2;
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
        let save = serde_json::from_str::<Self>(source).map_err(SessionError::Json)?;
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
        session.world.validate().map_err(SessionError::State)?;
        Ok((session, progress))
    }

    pub fn resume_json(
        source: &str,
        resumed_at_ms: u64,
    ) -> Result<(Self, OfflineProgress), SessionError> {
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
        let save_json = None;

        match envelope.command {
            SessionCommand::Feed { food } => {
                events = self.apply_player_event(PlayerEvent::Feed(food));
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
            }
            SessionCommand::Tick { milliseconds } => {
                events = step(&mut self.world, &[], milliseconds, &mut self.random);
            }
            SessionCommand::Talk { text } => {
                events = self.apply_player_event(PlayerEvent::Talk);
                let desired_social_act = events.iter().find_map(|event| match event {
                    GameEvent::SocialActExpressed(act) => Some(*act),
                    _ => None,
                });
                let request = build_dialogue_request(
                    &self.world,
                    &memory_query(&text),
                    DialogueRequestContext {
                        request_id: self.next_request_id,
                        mood: mood(&self.world),
                        player_said: &text,
                        desired_social_act,
                        max_words: 24,
                        allowed_gestures: BTreeSet::from([
                            Gesture::None,
                            Gesture::LookPlayer,
                            Gesture::LookWindow,
                            Gesture::Shiver,
                            Gesture::Sleepy,
                        ]),
                    },
                );
                validate_request(&request).map_err(SessionError::Dialogue)?;
                self.next_request_id = self.next_request_id.saturating_add(1);
                dialogue_request = Some(request);
            }
            SessionCommand::React { reaction } => {
                events = self.apply_player_event(PlayerEvent::React(reaction));
            }
            SessionCommand::Save => {
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
    fn checkpoint_load_restores_state_without_rewinding_transport_sequence() {
        let mut session = GameSession::new(9, "Sequence");
        let save_observation = session
            .apply(command(SessionCommand::Save))
            .expect("save command");
        assert_eq!(save_observation.sequence, 1);
        assert_eq!(save_observation.save_json, None);
        session
            .apply(command(SessionCommand::Load))
            .expect("load command");
        let inspect = session
            .apply(command(SessionCommand::Inspect))
            .expect("next command");
        assert_eq!(inspect.sequence, 3);
    }
}
