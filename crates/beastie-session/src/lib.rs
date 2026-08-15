//! UI-independent orchestration for playable and headless Beastie sessions.

use std::collections::BTreeSet;

use beastie_core::{
    Concept, FoodId, GameEvent, MemoryCue, MemoryQuery, PlayerEvent, Reaction, SaveGame,
    SeededRandom, WorldState, step,
};
use beastie_protocol::{
    DialogueRequest, DialogueRequestContext, Gesture, build_dialogue_request, validate_request,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SESSION_PROTOCOL_VERSION: u32 = 1;
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
    Feed { food: FoodId },
    Play,
    Comfort,
    Tidy,
    Advance { minutes: u32 },
    Talk { text: String },
    React { reaction: Reaction },
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
    checkpoint: Option<String>,
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
        let next_sequence = self.sequence.saturating_add(1);
        let mut events = Vec::new();
        let mut dialogue_request = None;
        let mut inspected_world = None;
        let mut save_json = None;

        match envelope.command {
            SessionCommand::Feed { food } => {
                events = self.apply_player_event(PlayerEvent::Feed(food));
            }
            SessionCommand::Play => {
                events = self.apply_player_event(PlayerEvent::Play);
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
                let encoded = SaveGame::capture(&self.world, &self.random)
                    .to_json()
                    .map_err(SessionError::Save)?;
                self.checkpoint = Some(encoded.clone());
                save_json = Some(encoded);
            }
            SessionCommand::Load => {
                let encoded = self
                    .checkpoint
                    .as_deref()
                    .ok_or(SessionError::NoCheckpoint)?;
                let (world, random) = SaveGame::from_json(encoded)
                    .map_err(SessionError::Save)?
                    .resume();
                self.world = world;
                self.random = random;
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
    if world.creature.current_intention == beastie_core::Intention::Sleep {
        "sleepy"
    } else if world.creature.relationship.resentment > 0.25 {
        "resentful"
    } else if world.creature.needs.hunger > 0.75 {
        "hungry"
    } else {
        "wary"
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
    #[error("talk text exceeds 512 characters")]
    TalkTooLong,
    #[error("no in-memory checkpoint exists")]
    NoCheckpoint,
    #[error("save operation failed: {0}")]
    Save(beastie_core::SaveError),
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
            SessionCommand::Play,
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
            .apply(command(SessionCommand::Play))
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
}
