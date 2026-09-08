//! UI-independent orchestration for playable and headless Beastie sessions.

use std::collections::BTreeSet;

#[cfg(test)]
use beastie_core::SIMULATION_TICK_MS;
use beastie_core::{
    DialogueActionOwner, DialogueHandoffState, FoodId, GameEvent, LanguageExposure, MemoryCue,
    MemoryQuery, Mood, NamingTarget, NonverbalAct, NormalizedPosition, OfflineProgress,
    PlayerEvent, Reaction, SaveGame, SeededRandom, SpeechAttention, ToyId, ToyOrigin,
    UtteranceInterpretation, UtteranceReference, WorldState, advance_offline, dialogue_handoff,
    ground_utterance, speech_attention, step,
};
use beastie_protocol::{
    AcousticConfidence, DialogueActionPhase, DialogueContext, DialogueReply, DialogueRequest,
    DialogueRequestContext, DialogueTopic, FallbackLane, Gesture, RecentTurn,
    RelationshipDialogueContext, SpeechInputFailure, build_dialogue_request,
    classify_content_boundary, normalize_dialogue_request, progression_max_words,
    project_candidate_memory, reply_fingerprint, validate_reply, validate_request,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SESSION_PROTOCOL_VERSION: u32 = 1;
pub const SESSION_SAVE_VERSION: u32 = 5;
const LEGACY_CORE_SAVE_VERSION: u32 = 1;
pub const MAX_COMMAND_BYTES: usize = 4_096;
pub const MAX_ADVANCE_MINUTES: u32 = 45;
pub const MAX_SCENARIO_ABSENCE_MS: u64 = beastie_core::MAX_OFFLINE_MS;
const DIALOGUE_HISTORY_VERSION: u32 = 1;
const MAX_DIALOGUE_HISTORY: usize = 6;
const DEFERRED_UTTERANCE_MAX_MS: u64 = 45_000;

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
    /// Deterministic scripted absence/return. This is intentionally bounded and has no UI key.
    Resume {
        elapsed_ms: u64,
    },
    Tick {
        milliseconds: u64,
    },
    Talk {
        text: String,
    },
    SpeechStarted,
    SpeechCandidate {
        text: String,
        confidence: AcousticConfidence,
    },
    SpeechEnded,
    SpeechFailed {
        failure: SpeechInputFailure,
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
    pub spoken_input: Option<SpokenInputStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world: Option<WorldState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_json: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpokenInputStatus {
    Listening,
    CandidateUpdated {
        confidence: AcousticConfidence,
    },
    Submitted,
    AcousticUncertainty {
        confidence: AcousticConfidence,
    },
    NoCandidate,
    Deferred,
    Expired,
    Refused,
    NotEngaged {
        attention: beastie_core::SpeechAttention,
    },
    InfrastructureFailure {
        failure: SpeechInputFailure,
    },
}

#[derive(Debug, Clone)]
pub struct GameSession {
    world: WorldState,
    random: SeededRandom,
    sequence: u64,
    next_request_id: u64,
    checkpoint: Option<Checkpoint>,
    spoken_input: SpokenInputState,
    dialogue_history: DialogueHistory,
    pending_dialogue: Option<PendingDialogue>,
}

#[derive(Debug, Clone, Copy)]
struct PendingDialogue {
    request_id: u64,
    motif: beastie_protocol::RelationshipMotifKey,
    subject: beastie_protocol::RelationshipSubject,
    mode: beastie_protocol::RelationshipExpressionMode,
    expression_kind: beastie_protocol::RelationshipExpressionKind,
    action_id: Option<u64>,
    invalidated: bool,
}

#[derive(Debug, Clone)]
struct RelationshipDialogueSelection {
    motif: beastie_protocol::RelationshipMotifKey,
    subject: beastie_protocol::RelationshipSubject,
    mode: beastie_protocol::RelationshipExpressionMode,
    expression_kind: beastie_protocol::RelationshipExpressionKind,
    phase: Option<beastie_protocol::RelationshipBeatPhase>,
    evidence: Vec<beastie_protocol::RelationshipEvidence>,
    target: Option<beastie_core::SemanticDestination>,
    action_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SpokenInputState {
    Idle,
    Listening {
        attention: beastie_core::SpeechAttention,
        candidate: Option<SpokenCandidate>,
    },
    Deferred {
        candidate: SpokenCandidate,
        ready_at_ms: u64,
        expires_at_ms: u64,
        owner: Option<DialogueActionOwner>,
        channel: InputChannel,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputChannel {
    Typed,
    Spoken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpokenCandidate {
    text: String,
    confidence: AcousticConfidence,
}

#[derive(Debug, Clone)]
struct Checkpoint {
    world: WorldState,
    random: SeededRandom,
    next_request_id: u64,
    dialogue_history: DialogueHistory,
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
    #[serde(default)]
    pub dialogue_history: DialogueHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DialogueHistory {
    #[serde(default = "dialogue_history_version")]
    pub version: u32,
    #[serde(default)]
    pub recent: Vec<SemanticDialogueTurn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDialogueTurn {
    pub turn_id: u64,
    pub topic: DialogueTopic,
    pub action_phase: DialogueActionPhase,
    #[serde(default)]
    pub selected_memory: Option<beastie_core::MemoryId>,
    #[serde(default)]
    pub selected_belief: Option<beastie_protocol::BeliefId>,
    #[serde(default)]
    pub motif: Option<beastie_protocol::RelationshipMotifKey>,
    #[serde(default)]
    pub expression_kind: Option<beastie_protocol::RelationshipExpressionKind>,
    #[serde(default)]
    pub fallback_lane: Option<FallbackLane>,
    pub expressed_at_ms: u64,
    #[serde(default)]
    pub reply_fingerprint: Option<String>,
}

fn dialogue_history_version() -> u32 {
    DIALOGUE_HISTORY_VERSION
}

impl DialogueHistory {
    fn validate(&self) -> Result<(), SessionError> {
        if self.version != DIALOGUE_HISTORY_VERSION || self.recent.len() > MAX_DIALOGUE_HISTORY {
            return Err(SessionError::DialogueHistory);
        }
        if self.recent.iter().any(|turn| {
            turn.turn_id == 0
                || turn
                    .reply_fingerprint
                    .as_deref()
                    .is_some_and(|fingerprint| !beastie_protocol::is_reply_fingerprint(fingerprint))
        }) {
            return Err(SessionError::DialogueHistory);
        }
        Ok(())
    }

    fn push(&mut self, turn: SemanticDialogueTurn) {
        self.recent.push(turn);
        let keep_from = self.recent.len().saturating_sub(MAX_DIALOGUE_HISTORY);
        if keep_from > 0 {
            self.recent.drain(..keep_from);
        }
    }
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
        let version = u32::try_from(version).unwrap_or(u32::MAX);
        if !matches!(version, 2 | 3 | 4 | SESSION_SAVE_VERSION) {
            return Err(SessionError::SaveVersion(version));
        }
        migrate_embedded_core_save(&mut value)?;
        value["version"] = serde_json::Value::from(SESSION_SAVE_VERSION);
        let mut save = serde_json::from_value::<Self>(value).map_err(SessionError::Json)?;
        if save.dialogue_history.version == 0 {
            save.dialogue_history.version = DIALOGUE_HISTORY_VERSION;
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
        self.world.validate().map_err(SessionError::State)?;
        self.dialogue_history.validate()?;
        Ok(())
    }
}

fn migrate_embedded_core_save(value: &mut serde_json::Value) -> Result<(), SessionError> {
    let core_version = value
        .get("world")
        .and_then(|world| world.get("save_version"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .unwrap_or(0);
    if core_version == beastie_core::SAVE_VERSION {
        return Ok(());
    }
    if core_version == 2
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
            aquarium.remove("creature_position");
            aquarium.remove("creature_velocity");
            aquarium.remove("facing");
            aquarium.remove("gaze");
            aquarium.remove("depth_lane");
            aquarium.remove("steering");
            aquarium.remove("destination");
        }
        if let Some(creature) = world
            .get_mut("creature")
            .and_then(serde_json::Value::as_object_mut)
        {
            creature.remove("position");
            creature.remove("movement");
        }
    }
    let world = serde_json::from_value::<WorldState>(
        value
            .get("world")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .map_err(SessionError::Json)?;
    let migrated = beastie_core::migrate_world(world).map_err(SessionError::LegacySave)?;
    value["world"] = serde_json::to_value(migrated).map_err(SessionError::Json)?;
    Ok(())
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
            spoken_input: SpokenInputState::Idle,
            dialogue_history: DialogueHistory {
                version: DIALOGUE_HISTORY_VERSION,
                recent: Vec::new(),
            },
            pending_dialogue: None,
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
            dialogue_history: self.dialogue_history.clone(),
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
            spoken_input: SpokenInputState::Idle,
            dialogue_history: save.dialogue_history,
            pending_dialogue: None,
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
                        spoken_input: SpokenInputState::Idle,
                        dialogue_history: DialogueHistory {
                            version: DIALOGUE_HISTORY_VERSION,
                            recent: Vec::new(),
                        },
                        pending_dialogue: None,
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
        validate_spoken_input_order(&self.spoken_input, &envelope.command)?;
        self.world.validate().map_err(SessionError::State)?;
        if matches!(self.spoken_input, SpokenInputState::Deferred { .. })
            && matches!(
                &envelope.command,
                SessionCommand::Talk { .. } | SessionCommand::SpeechStarted
            )
        {
            self.spoken_input = SpokenInputState::Idle;
        }
        let next_sequence = self.sequence.saturating_add(1);
        let mut events = Vec::new();
        let mut dialogue_request = None;
        let mut spoken_input = None;
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
                self.apply_deferred_speech(&mut dialogue_request, &mut events, &mut spoken_input)?;
            }
            SessionCommand::Resume { elapsed_ms } => {
                let progress = advance_offline(&mut self.world, elapsed_ms, &mut self.random);
                events = progress.events;
                if progress.applied_ms > 0 {
                    let conversation = &mut self.world.creature.conversation;
                    conversation.next_talk_at_ms = conversation
                        .next_talk_at_ms
                        .saturating_sub(progress.applied_ms);
                    conversation.contextual_follow_up_available = false;
                }
                dialogue_request = self.initiated_dialogue_request(&events)?;
                compact_advance_events(&mut events);
                self.apply_deferred_speech(&mut dialogue_request, &mut events, &mut spoken_input)?;
            }
            SessionCommand::Tick { milliseconds } => {
                events = step(&mut self.world, &[], milliseconds, &mut self.random);
                dialogue_request = self.initiated_dialogue_request(&events)?;
                self.apply_deferred_speech(&mut dialogue_request, &mut events, &mut spoken_input)?;
            }
            SessionCommand::Talk { text } => {
                dialogue_request = self.submit_utterance(
                    SpokenCandidate {
                        text,
                        confidence: AcousticConfidence::new(1_000)
                            .expect("typed input has certain acoustic confidence"),
                    },
                    speech_attention(&self.world),
                    InputChannel::Typed,
                    &mut events,
                    &mut spoken_input,
                )?;
            }
            SessionCommand::SpeechStarted => {
                events = self.apply_player_event(PlayerEvent::SpeechStarted);
                let attention = events
                    .iter()
                    .find_map(|event| match event {
                        GameEvent::SpeechPerceived(attention) => Some(*attention),
                        _ => None,
                    })
                    .expect("speech start always emits a perception result");
                self.spoken_input = SpokenInputState::Listening {
                    attention,
                    candidate: None,
                };
                spoken_input = Some(SpokenInputStatus::Listening);
            }
            SessionCommand::SpeechCandidate { text, confidence } => {
                let SpokenInputState::Listening { attention, .. } = self.spoken_input else {
                    unreachable!("spoken input ordering is validated before mutation");
                };
                self.spoken_input = SpokenInputState::Listening {
                    attention,
                    candidate: Some(SpokenCandidate { text, confidence }),
                };
                spoken_input = Some(SpokenInputStatus::CandidateUpdated { confidence });
            }
            SessionCommand::SpeechEnded => {
                let SpokenInputState::Listening {
                    attention,
                    candidate,
                } = &self.spoken_input
                else {
                    unreachable!("spoken input ordering is validated before mutation");
                };
                let attention = *attention;
                let candidate = candidate.clone();
                self.spoken_input = SpokenInputState::Idle;
                match candidate {
                    Some(candidate) if candidate.confidence.is_usable() => match attention {
                        beastie_core::SpeechAttention::Attended
                        | beastie_core::SpeechAttention::Glanced
                        | beastie_core::SpeechAttention::Ignored => {
                            dialogue_request = self.submit_utterance(
                                candidate,
                                attention,
                                InputChannel::Spoken,
                                &mut events,
                                &mut spoken_input,
                            )?;
                        }
                    },
                    Some(candidate) => {
                        spoken_input = Some(SpokenInputStatus::AcousticUncertainty {
                            confidence: candidate.confidence,
                        });
                    }
                    None => spoken_input = Some(SpokenInputStatus::NoCandidate),
                }
            }
            SessionCommand::SpeechFailed { failure } => {
                self.spoken_input = SpokenInputState::Idle;
                spoken_input = Some(SpokenInputStatus::InfrastructureFailure { failure });
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
                    dialogue_history: self.dialogue_history.clone(),
                };
                save_json = Some(durable.to_json()?);
                self.checkpoint = Some(Checkpoint {
                    world: self.world.clone(),
                    random: self.random,
                    next_request_id: self.next_request_id,
                    dialogue_history: self.dialogue_history.clone(),
                });
            }
            SessionCommand::Load => {
                let loaded = self.checkpoint.as_ref().ok_or(SessionError::NoCheckpoint)?;
                self.world = loaded.world.clone();
                self.random = loaded.random;
                self.next_request_id = loaded.next_request_id;
                self.dialogue_history = loaded.dialogue_history.clone();
                self.spoken_input = SpokenInputState::Idle;
                self.pending_dialogue = None;
            }
            SessionCommand::Inspect => inspected_world = Some(self.world.clone()),
        }

        self.observe_dialogue_events(
            &events,
            dialogue_request.as_ref().map(|request| request.request_id),
        );
        self.world.validate().map_err(SessionError::State)?;
        self.sequence = next_sequence;
        Ok(Observation {
            version: SESSION_PROTOCOL_VERSION,
            sequence: self.sequence,
            events,
            dialogue_request,
            spoken_input,
            world: inspected_world,
            save_json,
        })
    }

    fn apply_player_event(&mut self, event: PlayerEvent) -> Vec<GameEvent> {
        step(&mut self.world, &[event], 0, &mut self.random)
    }

    fn apply_talk(
        &mut self,
        text: &str,
        events: &mut Vec<GameEvent>,
    ) -> Result<Option<DialogueRequest>, SessionError> {
        let talk_events = self.apply_player_event(PlayerEvent::Talk);
        let accepted = talk_events
            .iter()
            .any(|event| matches!(event, GameEvent::TalkAccepted { .. }));
        events.extend(talk_events);
        if accepted && classify_content_boundary(text).is_none() {
            for exposure in language_exposures(text) {
                events.extend(self.apply_player_event(PlayerEvent::LanguageExposure(exposure)));
            }
        }
        if !accepted {
            return Ok(None);
        }

        let desired_social_act = events.iter().find_map(|event| match event {
            GameEvent::SocialActExpressed(act) => Some(*act),
            _ => None,
        });
        let grounded = ground_utterance(
            text,
            &self.world.creature.name,
            &self.world.creature.known_concepts,
        );
        let interpretation = grounded.interpretation;
        if classify_content_boundary(text).is_none()
            && !interpretation.ambiguous
            && (!interpretation.references.is_empty()
                || !interpretation.understood_concepts.is_empty())
        {
            events.extend(beastie_core::apply_grounded_utterance(
                &mut self.world,
                &interpretation,
            ));
        }
        let mut request = build_dialogue_request(
            &self.world,
            &memory_query(&interpretation),
            DialogueRequestContext {
                request_id: self.next_request_id,
                mood: mood(&self.world),
                player_said: text,
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
        request.interpretation = interpretation.into();
        normalize_dialogue_request(&mut request);
        if request.input_rejection.is_none() {
            request.player_said = grounded.grounded_text;
        }
        let request_topic = initiated_topic_for_request(&request);
        self.ensure_relationship_evidence_offered(&mut request);
        self.attach_dialogue_context(&mut request, request_topic);
        validate_request(&request).map_err(SessionError::Dialogue)?;
        self.next_request_id = self.next_request_id.saturating_add(1);
        Ok(Some(request))
    }

    fn submit_utterance(
        &mut self,
        candidate: SpokenCandidate,
        attention: SpeechAttention,
        channel: InputChannel,
        events: &mut Vec<GameEvent>,
        spoken_input: &mut Option<SpokenInputStatus>,
    ) -> Result<Option<DialogueRequest>, SessionError> {
        if matches!(attention, SpeechAttention::Ignored) {
            if matches!(channel, InputChannel::Spoken) {
                *spoken_input = Some(SpokenInputStatus::NotEngaged { attention });
            } else {
                events.push(GameEvent::TalkIgnored);
            }
            return Ok(None);
        }

        if self.world.mood() == Mood::Resentful {
            events.push(GameEvent::UtteranceRefused);
            events.push(GameEvent::NonverbalAct(NonverbalAct::RefuseAndStare));
            if matches!(channel, InputChannel::Spoken) {
                *spoken_input = Some(SpokenInputStatus::Refused);
            }
            return Ok(None);
        }

        let waiting_for_cooldown = self.world.elapsed_ms < self.next_talk_ready_at_ms();
        if matches!(attention, SpeechAttention::Glanced)
            || (matches!(channel, InputChannel::Spoken) && waiting_for_cooldown)
        {
            let handoff = dialogue_handoff(&self.world);
            let ready_at_ms = self.world.elapsed_ms.max(self.next_talk_ready_at_ms());
            self.spoken_input = SpokenInputState::Deferred {
                candidate,
                ready_at_ms,
                expires_at_ms: self
                    .world
                    .elapsed_ms
                    .saturating_add(DEFERRED_UTTERANCE_MAX_MS),
                owner: handoff.owner,
                channel,
            };
            events.push(GameEvent::UtteranceDeferred);
            if matches!(channel, InputChannel::Spoken) {
                *spoken_input = Some(SpokenInputStatus::Deferred);
            }
            return Ok(None);
        }

        let request = self.apply_talk(&candidate.text, events)?;
        if matches!(channel, InputChannel::Spoken) {
            *spoken_input = Some(if request.is_some() {
                SpokenInputStatus::Submitted
            } else {
                SpokenInputStatus::NotEngaged { attention }
            });
        }
        Ok(request)
    }

    fn next_talk_ready_at_ms(&self) -> u64 {
        if self
            .world
            .creature
            .conversation
            .contextual_follow_up_available
        {
            self.world.elapsed_ms
        } else {
            self.world.creature.conversation.next_talk_at_ms
        }
    }

    fn apply_deferred_speech(
        &mut self,
        dialogue_request: &mut Option<DialogueRequest>,
        events: &mut Vec<GameEvent>,
        spoken_input: &mut Option<SpokenInputStatus>,
    ) -> Result<(), SessionError> {
        let SpokenInputState::Deferred {
            candidate,
            ready_at_ms,
            expires_at_ms,
            owner,
            channel,
        } = &self.spoken_input
        else {
            return Ok(());
        };
        if self.world.elapsed_ms >= *expires_at_ms {
            let channel = *channel;
            self.spoken_input = SpokenInputState::Idle;
            if matches!(channel, InputChannel::Spoken) {
                *spoken_input = Some(SpokenInputStatus::Expired);
            } else {
                events.push(GameEvent::TalkIgnored);
            }
            return Ok(());
        }
        if self.world.elapsed_ms < *ready_at_ms
            || self.world.elapsed_ms < self.next_talk_ready_at_ms()
            || dialogue_request.is_some()
        {
            return Ok(());
        }
        let handoff = dialogue_handoff(&self.world);
        let current_direct_action = match handoff.owner {
            Some(DialogueActionOwner::Food(_)) => true,
            Some(DialogueActionOwner::Toy(_) | DialogueActionOwner::Refusal(_)) => self
                .world
                .creature
                .interaction_state
                .toy_interaction
                .as_ref()
                .is_some_and(|interaction| interaction.origin == ToyOrigin::Player),
            _ => false,
        };
        // Ending the original owner does not finish a newer direct interaction. Its own
        // contact/recovery boundary still matters, even for cooldown-only deferral. Unrelated
        // private life may not repeatedly capture words that were already waiting.
        let boundary_ready = matches!(
            handoff.state,
            DialogueHandoffState::Ready | DialogueHandoffState::SafeBoundary
        ) || (!current_direct_action
            && (owner.is_none() || handoff.owner != *owner));
        if !boundary_ready {
            return Ok(());
        }
        let candidate = candidate.clone();
        let channel = *channel;
        let attention = match speech_attention(&self.world) {
            SpeechAttention::Glanced => SpeechAttention::Attended,
            attention => attention,
        };
        self.spoken_input = SpokenInputState::Idle;
        *dialogue_request =
            self.submit_utterance(candidate, attention, channel, events, spoken_input)?;
        Ok(())
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
                motif: None,
                expression_kind: None,
                expressed_at_ms: None,
                reply_fingerprint: None,
            }],
            ..DialogueContext::default()
        };
        self.ensure_relationship_evidence_offered(&mut request);
        self.attach_dialogue_context(&mut request, initiated_topic(reason));
        normalize_dialogue_request(&mut request);
        validate_request(&request).map_err(SessionError::Dialogue)?;
        self.next_request_id = self.next_request_id.saturating_add(1);
        Ok(Some(request))
    }

    fn attach_dialogue_context(&mut self, request: &mut DialogueRequest, topic: DialogueTopic) {
        let current_turns = request.context.recent_turns.clone();
        let mut recent_turns = self
            .dialogue_history
            .recent
            .iter()
            .map(|turn| RecentTurn {
                turn_id: turn.turn_id,
                topic: turn.topic,
                action_phase: turn.action_phase,
                selected_memory: turn.selected_memory,
                selected_belief: turn.selected_belief,
                selected_fact_ids: turn.selected_memory.into_iter().map(|id| id.0).collect(),
                fallback_lane: turn.fallback_lane,
                motif: turn.motif,
                expression_kind: turn.expression_kind,
                expressed_at_ms: Some(turn.expressed_at_ms),
                reply_fingerprint: turn.reply_fingerprint.clone(),
            })
            .collect::<Vec<_>>();
        recent_turns.extend(current_turns);
        let keep_from = recent_turns
            .len()
            .saturating_sub(beastie_protocol::MAX_RECENT_TURNS);
        if keep_from > 0 {
            recent_turns.drain(..keep_from);
        }
        request.context.recent_turns = recent_turns;
        request.context.repetition_count = repetition_count(&request.context.recent_turns, topic);
        request.context.avoid_reply_fingerprints = self
            .dialogue_history
            .recent
            .iter()
            .filter_map(|turn| turn.reply_fingerprint.clone())
            .rev()
            .take(beastie_protocol::MAX_REPLY_PROHIBITIONS)
            .collect();
        if let Some(selected) = self.relationship_dialogue_selection() {
            let evidence = selected
                .evidence
                .iter()
                .filter(|evidence| match evidence {
                    beastie_core::RelationshipEvidence::Memory { id } => request
                        .candidate_memories
                        .iter()
                        .any(|memory| memory.id == *id),
                    beastie_core::RelationshipEvidence::Belief { id, kind } => request
                        .candidate_beliefs
                        .iter()
                        .any(|candidate| candidate.id == *id && candidate.proposition == *kind),
                    beastie_core::RelationshipEvidence::Visit { .. } => true,
                })
                .cloned()
                .collect();
            request.context.relationship = Some(RelationshipDialogueContext {
                motif: selected.motif,
                subject: selected.subject,
                mode: selected.mode,
                expression_kind: selected.expression_kind,
                recently_expressed: self
                    .dialogue_history
                    .recent
                    .iter()
                    .any(|turn| turn.motif == Some(selected.motif)),
                phase: selected.phase,
                evidence,
                target: selected.target,
            });
            if request.context.recent_turns.last().is_none() {
                request.context.recent_turns.push(RecentTurn {
                    turn_id: request.request_id,
                    topic,
                    action_phase: DialogueActionPhase::Notice,
                    selected_memory: None,
                    selected_belief: None,
                    selected_fact_ids: Vec::new(),
                    fallback_lane: Some(FallbackLane::Relationship),
                    motif: request.context.relationship.as_ref().map(|r| r.motif),
                    expression_kind: request
                        .context
                        .relationship
                        .as_ref()
                        .map(|r| r.expression_kind),
                    expressed_at_ms: Some(self.world.elapsed_ms),
                    reply_fingerprint: None,
                });
            }
            self.pending_dialogue = Some(PendingDialogue {
                request_id: request.request_id,
                motif: selected.motif,
                subject: selected.subject,
                mode: selected.mode,
                expression_kind: selected.expression_kind,
                action_id: selected.action_id,
                invalidated: false,
            });
        } else {
            self.pending_dialogue = None;
        }
    }

    fn relationship_dialogue_selection(&self) -> Option<RelationshipDialogueSelection> {
        if let Some(action) = self.world.creature.aquarium.action.as_ref()
            && let Some(context) = action.relationship.as_ref()
        {
            return Some(RelationshipDialogueSelection {
                motif: context.motif,
                subject: context.subject,
                mode: beastie_protocol::RelationshipExpressionMode::ActionBound,
                expression_kind: context.expression_kind,
                phase: Some(action_relationship_phase(action.phase)),
                evidence: context.evidence.clone(),
                target: relationship_dialogue_target(context.motif),
                action_id: Some(action.action_id),
            });
        }
        if let Some(moment) = self
            .world
            .creature
            .interaction_state
            .relationship_moment
            .as_ref()
        {
            return Some(RelationshipDialogueSelection {
                motif: moment.context.motif,
                subject: moment.context.subject,
                mode: beastie_protocol::RelationshipExpressionMode::ActionBound,
                expression_kind: moment.context.expression_kind,
                phase: Some(beastie_protocol::RelationshipBeatPhase::Act),
                evidence: moment.context.evidence.clone(),
                target: relationship_dialogue_target(moment.context.motif),
                action_id: Some(moment.action_id),
            });
        }
        let beat = self
            .world
            .creature
            .relationship_expression
            .active
            .as_ref()?;
        Some(RelationshipDialogueSelection {
            motif: beat.motif,
            subject: beat
                .subject
                .expect("validated relationship beat has a subject"),
            mode: beastie_protocol::RelationshipExpressionMode::Standalone,
            expression_kind: beat.expression_kind,
            phase: Some(beat.phase),
            evidence: beat.evidence.clone(),
            target: beat.target,
            action_id: None,
        })
    }

    fn observe_dialogue_events(&mut self, events: &[GameEvent], created_request_id: Option<u64>) {
        let Some(pending) = self.pending_dialogue.as_mut() else {
            return;
        };
        let request_created_here = created_request_id == Some(pending.request_id);
        if events.iter().any(|event| match event {
            GameEvent::RelationshipBeatInterrupted(motif) => {
                *motif == pending.motif && !request_created_here
            }
            GameEvent::RelationshipBeatStarted { .. } => !request_created_here,
            GameEvent::ActionRelationshipInterrupted { action_id, .. } => {
                pending.action_id == Some(*action_id) && !request_created_here
            }
            GameEvent::ActionRelationshipStarted { action_id, .. } => {
                pending.action_id != Some(*action_id) && !request_created_here
            }
            _ => false,
        }) {
            pending.invalidated = true;
        }
    }

    fn ensure_relationship_evidence_offered(&self, request: &mut DialogueRequest) {
        let Some(selected) = self.relationship_dialogue_selection() else {
            return;
        };
        for id in selected
            .evidence
            .iter()
            .filter_map(|evidence| match evidence {
                beastie_core::RelationshipEvidence::Memory { id } => Some(*id),
                beastie_core::RelationshipEvidence::Belief { .. } => None,
                beastie_core::RelationshipEvidence::Visit { .. } => None,
            })
        {
            if request
                .candidate_memories
                .iter()
                .any(|memory| memory.id == id)
            {
                continue;
            }
            let Some(memory) = self
                .world
                .creature
                .memories
                .iter()
                .find(|memory| memory.id == id)
            else {
                continue;
            };
            if request.candidate_memories.len() >= beastie_protocol::MAX_CANDIDATE_MEMORIES {
                request.candidate_memories.pop();
            }
            request
                .candidate_memories
                .push(project_candidate_memory(memory, self.world.active_day()));
        }
    }

    /// Accepts a completed worker turn only while its authoritative relationship beat is current.
    /// The persisted record contains IDs, enums, time, fallback metadata, and a one-way digest,
    /// never player or generated prose.
    pub fn accept_dialogue_turn(
        &mut self,
        request: &DialogueRequest,
        reply: &DialogueReply,
        retry_count: u8,
        fallback: bool,
    ) -> bool {
        if validate_reply(request, reply.clone()).is_err() || retry_count > 1 {
            return false;
        }
        if let Some(selected) = &request.context.relationship {
            let Some(pending) = self.pending_dialogue else {
                return false;
            };
            if pending.invalidated
                || pending.request_id != request.request_id
                || pending.motif != selected.motif
                || pending.subject != selected.subject
                || pending.mode != selected.mode
                || pending.expression_kind != selected.expression_kind
            {
                return false;
            }
            if let Some(current) = self.relationship_dialogue_selection()
                && (current.motif != selected.motif
                    || current.subject != selected.subject
                    || current.mode != selected.mode
                    || current.expression_kind != selected.expression_kind
                    || current.action_id != pending.action_id)
            {
                return false;
            }
        }
        let relationship = request.context.relationship.as_ref();
        let fallback_lane = request
            .context
            .recent_turns
            .last()
            .and_then(|turn| turn.fallback_lane)
            .or_else(|| relationship.map(|_| FallbackLane::Relationship));
        let turn = SemanticDialogueTurn {
            turn_id: request.request_id,
            topic: request
                .context
                .recent_turns
                .last()
                .map_or(DialogueTopic::Other, |turn| turn.topic),
            action_phase: request
                .context
                .recent_turns
                .last()
                .map_or(DialogueActionPhase::Idle, |turn| turn.action_phase),
            selected_memory: reply.recalled_memory,
            selected_belief: reply.recalled_belief,
            motif: relationship.map(|relationship| relationship.motif),
            expression_kind: relationship.map(|relationship| relationship.expression_kind),
            fallback_lane: fallback.then_some(fallback_lane).flatten(),
            expressed_at_ms: self.world.elapsed_ms,
            reply_fingerprint: Some(reply_fingerprint(&reply.say)),
        };
        self.dialogue_history.push(turn);
        self.pending_dialogue = None;
        true
    }

    #[must_use]
    pub fn dialogue_history(&self) -> &DialogueHistory {
        &self.dialogue_history
    }
}

const fn action_relationship_phase(
    phase: beastie_core::ActionPhase,
) -> beastie_protocol::RelationshipBeatPhase {
    match phase {
        beastie_core::ActionPhase::Notice | beastie_core::ActionPhase::Gaze => {
            beastie_protocol::RelationshipBeatPhase::Notice
        }
        beastie_core::ActionPhase::Brake
        | beastie_core::ActionPhase::Turn
        | beastie_core::ActionPhase::Approach
        | beastie_core::ActionPhase::Inspect => beastie_protocol::RelationshipBeatPhase::Anticipate,
        beastie_core::ActionPhase::Act => beastie_protocol::RelationshipBeatPhase::Act,
        beastie_core::ActionPhase::Recover => beastie_protocol::RelationshipBeatPhase::Recover,
    }
}

const fn relationship_dialogue_target(
    motif: beastie_protocol::RelationshipMotifKey,
) -> Option<beastie_core::SemanticDestination> {
    match motif {
        beastie_protocol::RelationshipMotifKey::SharedToy(toy) => {
            Some(beastie_core::SemanticDestination::Toy(toy))
        }
        beastie_protocol::RelationshipMotifKey::ComfortRitual
        | beastie_protocol::RelationshipMotifKey::PlayerReturns => {
            Some(beastie_core::SemanticDestination::Player)
        }
        beastie_protocol::RelationshipMotifKey::TrustedFood(_)
        | beastie_protocol::RelationshipMotifKey::FoodGrudge(_) => {
            Some(beastie_core::SemanticDestination::Bottom)
        }
        beastie_protocol::RelationshipMotifKey::FamiliarPlace(destination) => Some(destination),
    }
}

/// Collapses idempotent high-frequency notifications produced by accelerated time.
///
/// `NeedChanged` means "read the current authoritative needs" rather than describing a
/// historical delta, so one notification represents the final state just as well as hundreds.
/// Autonomous `ToyPlayed` events are presentation beats rather than history; an accelerated span
/// retains the first arrival for each toy instead of replaying minutes of obsolete impacts.
/// Private-life performance events are likewise stale after an accelerated span. Their durable
/// outcome is already present in the world and toy-object state, so replaying their notices,
/// contacts, and recoveries on resume would fictionalize hours-old action.
fn compact_advance_events(events: &mut Vec<GameEvent>) {
    let mut emitted_need_change = false;
    let mut emitted_toy_arrivals = BTreeSet::new();
    let mut emitted_relationship_starts = BTreeSet::new();
    events.retain(|event| {
        if matches!(event, GameEvent::NeedChanged) {
            let keep = !emitted_need_change;
            emitted_need_change = true;
            keep
        } else if let GameEvent::ToyPlayed {
            toy,
            origin: ToyOrigin::Autonomous,
            ..
        } = event
        {
            emitted_toy_arrivals.insert(*toy)
        } else if matches!(
            event,
            GameEvent::PrivateLifeStarted { .. }
                | GameEvent::PrivateLifePhaseChanged { .. }
                | GameEvent::PrivateLifeCompleted { .. }
                | GameEvent::PrivateLifeInterrupted { .. }
                | GameEvent::ToyObjectResponded { .. }
        ) {
            false
        } else if let GameEvent::RelationshipBeatStarted { motif, .. } = event {
            emitted_relationship_starts.insert(*motif)
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

fn initiated_topic_for_request(request: &DialogueRequest) -> DialogueTopic {
    request
        .context
        .recent_turns
        .last()
        .map_or(DialogueTopic::Other, |turn| turn.topic)
}

fn repetition_count(turns: &[RecentTurn], topic: DialogueTopic) -> u8 {
    turns
        .iter()
        .rev()
        .take_while(|turn| turn.topic == topic)
        .count()
        .min(8) as u8
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
        SessionCommand::Resume { elapsed_ms } if *elapsed_ms > MAX_SCENARIO_ABSENCE_MS => {
            Err(SessionError::Resume(*elapsed_ms))
        }
        SessionCommand::Tick { milliseconds }
            if *milliseconds > u64::from(MAX_ADVANCE_MINUTES) * 60_000 =>
        {
            Err(SessionError::Tick(*milliseconds))
        }
        SessionCommand::Talk { text } | SessionCommand::SpeechCandidate { text, .. }
            if text.chars().count() > 512 =>
        {
            Err(SessionError::TalkTooLong)
        }
        SessionCommand::SpeechCandidate { text, .. } if text.trim().is_empty() => {
            Err(SessionError::EmptySpeechCandidate)
        }
        _ => Ok(()),
    }
}

fn validate_spoken_input_order(
    state: &SpokenInputState,
    command: &SessionCommand,
) -> Result<(), SessionError> {
    match (state, command) {
        (SpokenInputState::Idle, SessionCommand::SpeechStarted)
        | (SpokenInputState::Listening { .. }, SessionCommand::SpeechCandidate { .. })
        | (SpokenInputState::Listening { .. }, SessionCommand::SpeechEnded)
        | (SpokenInputState::Listening { .. }, SessionCommand::SpeechFailed { .. }) => Ok(()),
        (SpokenInputState::Deferred { .. }, SessionCommand::SpeechStarted) => Ok(()),
        (SpokenInputState::Listening { .. }, SessionCommand::SpeechStarted) => {
            Err(SessionError::SpeechAlreadyStarted)
        }
        (
            SpokenInputState::Idle | SpokenInputState::Deferred { .. },
            SessionCommand::SpeechCandidate { .. }
            | SessionCommand::SpeechEnded
            | SessionCommand::SpeechFailed { .. },
        ) => Err(SessionError::SpeechNotStarted),
        (SpokenInputState::Deferred { .. }, SessionCommand::Talk { .. }) => Ok(()),
        (SpokenInputState::Listening { .. }, SessionCommand::Talk { .. }) => {
            Err(SessionError::UtteranceBusy)
        }
        _ => Ok(()),
    }
}

fn memory_query(interpretation: &UtteranceInterpretation) -> MemoryQuery {
    let mut cues = BTreeSet::new();
    for concept in &interpretation.understood_concepts {
        cues.insert(MemoryCue::Concept(*concept));
    }
    for reference in &interpretation.references {
        if let UtteranceReference::Food(food) = reference {
            cues.insert(MemoryCue::Food(*food));
        }
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
    #[error("cannot simulate {0} milliseconds of absence in one command")]
    Resume(u64),
    #[error("talk text exceeds 512 characters")]
    TalkTooLong,
    #[error("spoken candidate must contain non-whitespace text")]
    EmptySpeechCandidate,
    #[error("speech input has already started")]
    SpeechAlreadyStarted,
    #[error("speech input has not started")]
    SpeechNotStarted,
    #[error("another utterance is already being heard or held for a delayed response")]
    UtteranceBusy,
    #[error("no in-memory checkpoint exists")]
    NoCheckpoint,
    #[error("session save version {0} is unsupported")]
    SaveVersion(u32),
    #[error("next dialogue request ID must be nonzero")]
    RequestId,
    #[error("dialogue history is malformed or exceeds its bound")]
    DialogueHistory,
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
    use beastie_core::{ToyInteractionOutcome, ToyInteractionPhase};

    fn command(command: SessionCommand) -> CommandEnvelope {
        CommandEnvelope {
            version: SESSION_PROTOCOL_VERSION,
            command,
        }
    }

    fn toy_position(session: &GameSession, toy: ToyId) -> NormalizedPosition {
        session
            .world()
            .aquarium
            .objects
            .values()
            .find_map(|object| match object {
                beastie_core::WorldObject::Toy {
                    toy: candidate,
                    position,
                } if *candidate == toy => Some(*position),
                _ => None,
            })
            .expect("default toy exists")
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

        compact_advance_events(&mut raw_events);
        assert_eq!(observation.events, raw_events);
        assert!(
            observation
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::ToyPlayed { .. }))
                .count()
                <= 3
        );
        assert!(
            observation.events.len() <= 96,
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
            autonomous_toy_played(ToyId::Sock, 1),
            phase.clone(),
            GameEvent::NeedChanged,
            autonomous_toy_played(ToyId::Sock, 2),
            autonomous_toy_played(ToyId::Ball, 3),
            memory.clone(),
            phase.clone(),
            GameEvent::NeedChanged,
        ];
        compact_advance_events(&mut events);
        assert_eq!(
            events,
            vec![
                GameEvent::NeedChanged,
                autonomous_toy_played(ToyId::Sock, 1),
                phase.clone(),
                autonomous_toy_played(ToyId::Ball, 3),
                memory,
                phase,
            ]
        );
    }

    fn autonomous_toy_played(toy: ToyId, interaction_id: u64) -> GameEvent {
        GameEvent::ToyPlayed {
            toy,
            interaction_id: std::num::NonZeroU64::new(interaction_id)
                .expect("test interaction ID is nonzero"),
            origin: ToyOrigin::Autonomous,
        }
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
                .as_ref()
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
    fn dialogue_projects_the_current_action_bound_subject_and_evidence() {
        let source = include_str!("../../../fixtures/saves/feel/trusted-berry.json");
        let save = SessionSave::from_json(source).expect("trusted fixture");
        let resumed_at = save.saved_at_ms;
        let (mut session, progress) =
            GameSession::resume(save, resumed_at).expect("resume fixture");
        assert_eq!(progress.applied_ms, 0);
        session
            .apply(command(SessionCommand::DropFood {
                food: FoodId::Berry,
                position: NormalizedPosition::new(5_000, 3_000),
            }))
            .expect("drop trusted berry");

        let mut request = build_dialogue_request(
            session.world(),
            &MemoryQuery {
                cues: BTreeSet::from([MemoryCue::Food(FoodId::Berry)]),
                limit: 8,
            },
            DialogueRequestContext {
                request_id: 77,
                mood: "content",
                player_said: "berry",
                desired_social_act: None,
                max_words: 8,
                allowed_gestures: BTreeSet::from([Gesture::None]),
            },
        );
        session.ensure_relationship_evidence_offered(&mut request);
        session.attach_dialogue_context(&mut request, DialogueTopic::Food);
        let relationship = request
            .context
            .relationship
            .as_ref()
            .expect("action-bound relationship dialogue context");
        assert_eq!(
            relationship.motif,
            beastie_protocol::RelationshipMotifKey::TrustedFood(FoodId::Berry)
        );
        assert_eq!(
            relationship.subject,
            beastie_protocol::RelationshipSubject::Food(FoodId::Berry)
        );
        assert_eq!(
            relationship.mode,
            beastie_protocol::RelationshipExpressionMode::ActionBound
        );
        assert!(relationship.evidence.len() >= 2);
        assert!(validate_request(&request).is_ok());
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
        // This test isolates memory grounding from the separate V2 willingness policy.
        session.world.creature.relationship.resentment = 0.0;
        session.world.creature.current_intention = beastie_core::Intention::Idle;
        session.world.creature.needs.energy = 1.0;
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
    fn session_v3_migrates_embedded_core_v5_without_inventing_toy_payoff() {
        let session = GameSession::new(9, "Legacy");
        let mut value = serde_json::to_value(session.capture(500)).expect("current save value");
        value["version"] = serde_json::Value::from(3);
        value["world"]["save_version"] = serde_json::Value::from(5);
        value["world"]["creature"]["current_intention"] =
            serde_json::to_value(beastie_core::Intention::Play).expect("intention value");
        value["world"]["creature"]["aquarium"]["destination"] =
            serde_json::to_value(beastie_core::SemanticDestination::Toy(ToyId::Ball))
                .expect("destination value");
        let aquarium = value["world"]["creature"]["aquarium"]
            .as_object_mut()
            .expect("aquarium object");
        aquarium.remove("travel_purpose");
        let interaction = value["world"]["creature"]["interaction_state"]
            .as_object_mut()
            .expect("interaction object");
        interaction.remove("next_toy_interaction_id");
        interaction.remove("toy_interaction");
        interaction.remove("last_resolved_toy_interaction");

        let encoded = serde_json::to_string(&value).expect("legacy session JSON");
        let migrated = SessionSave::from_json(&encoded).expect("v3 session should migrate");
        assert_eq!(migrated.version, SESSION_SAVE_VERSION);
        assert_eq!(migrated.world.save_version, beastie_core::SAVE_VERSION);
        assert_eq!(migrated.world.creature.aquarium.destination, None);
        assert_eq!(migrated.world.creature.aquarium.travel_purpose, None);
        assert!(
            migrated
                .world
                .creature
                .interaction_state
                .toy_interaction
                .is_none()
        );
    }

    #[test]
    fn current_session_save_resumes_accepted_toy_with_exact_outcome_and_id() {
        let mut session = GameSession::new(405, "AcceptedResume");
        session
            .world
            .creature
            .toy_preferences
            .insert(ToyId::Ball, 0.8);
        session.world.creature.aquarium.position = toy_position(&session, ToyId::Ball);
        let receipt = session
            .apply(command(SessionCommand::Play { toy: ToyId::Ball }))
            .expect("accepted toy receipt");
        let interaction_id = receipt
            .events
            .iter()
            .find_map(|event| match event {
                GameEvent::ToyPlayAccepted { interaction_id, .. } => Some(*interaction_id),
                _ => None,
            })
            .expect("accepted interaction ID");
        let saved_at = 50_000;
        let json = session.capture(saved_at).to_json().unwrap();
        let (mut resumed, progress) = GameSession::resume_json(&json, saved_at).unwrap();
        assert_eq!(progress.applied_ms, 0);
        assert!(progress.events.is_empty());
        assert!(
            resumed
                .world()
                .creature
                .interaction_state
                .toy_interaction
                .as_ref()
                .is_some_and(|interaction| interaction.id == interaction_id
                    && interaction.toy == ToyId::Ball
                    && interaction.origin == ToyOrigin::Player
                    && interaction.outcome == ToyInteractionOutcome::Accepted
                    && interaction.phase == ToyInteractionPhase::Approach)
        );

        let contact = resumed
            .apply(command(SessionCommand::Tick {
                milliseconds: SIMULATION_TICK_MS,
            }))
            .unwrap();
        assert!(contact.events.contains(&GameEvent::ToyContacted {
            toy: ToyId::Ball,
            interaction_id,
            origin: ToyOrigin::Player,
        }));
        assert!(contact.events.contains(&GameEvent::ToyPlayed {
            toy: ToyId::Ball,
            interaction_id,
            origin: ToyOrigin::Player,
        }));
        assert_eq!(
            resumed
                .world()
                .creature
                .interaction_state
                .last_resolved_toy_interaction
                .map(|resolved| (resolved.id, resolved.toy, resolved.origin)),
            Some((interaction_id, ToyId::Ball, ToyOrigin::Player))
        );
    }

    #[test]
    fn current_session_save_resumes_refusal_stare_without_positive_outcome() {
        let mut session = GameSession::new(406, "RefusalResume");
        session
            .world
            .creature
            .toy_preferences
            .insert(ToyId::Bell, -1.0);
        session.world.creature.aquarium.position = toy_position(&session, ToyId::Bell);
        let relationship = session.world().creature.relationship;
        let receipt = session
            .apply(command(SessionCommand::Play { toy: ToyId::Bell }))
            .expect("rejected toy receipt");
        let interaction_id = receipt
            .events
            .iter()
            .find_map(|event| match event {
                GameEvent::ToyRejected { interaction_id, .. } => Some(*interaction_id),
                _ => None,
            })
            .expect("rejected interaction ID");
        let saved_at = 60_000;
        let json = session.capture(saved_at).to_json().unwrap();
        let (mut resumed, progress) = GameSession::resume_json(&json, saved_at).unwrap();
        assert_eq!(progress.applied_ms, 0);
        assert!(progress.events.is_empty());
        assert!(
            resumed
                .world()
                .creature
                .interaction_state
                .toy_interaction
                .as_ref()
                .is_some_and(|interaction| interaction.id == interaction_id
                    && interaction.toy == ToyId::Bell
                    && interaction.origin == ToyOrigin::Player
                    && interaction.outcome == ToyInteractionOutcome::Rejected
                    && interaction.phase == ToyInteractionPhase::Approach)
        );

        let arrival = resumed
            .apply(command(SessionCommand::Tick {
                milliseconds: SIMULATION_TICK_MS,
            }))
            .unwrap();
        assert!(!arrival.events.iter().any(|event| matches!(
            event,
            GameEvent::ToyContacted { .. } | GameEvent::ToyPlayed { .. }
        )));
        assert_eq!(resumed.world().creature.relationship, relationship);
        assert!(
            resumed
                .world()
                .creature
                .interaction_state
                .toy_interaction
                .as_ref()
                .is_some_and(|interaction| interaction.id == interaction_id
                    && interaction.outcome == ToyInteractionOutcome::Rejected
                    && interaction.phase == ToyInteractionPhase::Recovery)
        );
        assert!(
            resumed
                .world()
                .creature
                .interaction_state
                .last_resolved_toy_interaction
                .is_none()
        );
        assert!(
            !resumed
                .world()
                .creature
                .memories
                .iter()
                .any(|memory| matches!(
                    memory.kind,
                    beastie_core::MemoryKind::PlayedWith { toy: ToyId::Bell }
                ))
        );
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

    #[test]
    fn spoken_notice_precedes_words_and_identical_text_has_identical_semantics() {
        let mut spoken = GameSession::new(51, "Same Beastie");
        let mut typed = GameSession::new(51, "Same Beastie");
        let started = spoken
            .apply(command(SessionCommand::SpeechStarted))
            .expect("speech starts");
        assert!(
            started
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::SpeechPerceived(_)))
        );
        assert!(started.dialogue_request.is_none());
        spoken
            .apply(command(SessionCommand::SpeechCandidate {
                text: "Remember the berry?".to_owned(),
                confidence: AcousticConfidence::new(900).expect("valid confidence"),
            }))
            .expect("candidate arrives");
        let spoken_end = spoken
            .apply(command(SessionCommand::SpeechEnded))
            .expect("speech ends");
        let typed_talk = typed
            .apply(command(SessionCommand::Talk {
                text: "Remember the berry?".to_owned(),
            }))
            .expect("typed talk");

        assert_eq!(spoken_end.events, typed_talk.events);
        assert_eq!(spoken_end.dialogue_request, typed_talk.dialogue_request);
        assert_eq!(spoken_end.spoken_input, Some(SpokenInputStatus::Submitted));
    }

    #[test]
    fn uncertain_or_missing_words_never_reach_language_or_dialogue() {
        let mut session = GameSession::new(52, "Uncertain");
        session
            .apply(command(SessionCommand::SpeechStarted))
            .expect("speech starts");
        session
            .apply(command(SessionCommand::SpeechCandidate {
                text: "fuck that shitty nice package".to_owned(),
                confidence: AcousticConfidence::new(649).expect("valid confidence"),
            }))
            .expect("candidate arrives");
        let uncertain = session
            .apply(command(SessionCommand::SpeechEnded))
            .expect("speech ends");
        assert_eq!(uncertain.dialogue_request, None);
        assert_eq!(
            uncertain.spoken_input,
            Some(SpokenInputStatus::AcousticUncertainty {
                confidence: AcousticConfidence::new(649).expect("valid confidence")
            })
        );
        assert!(!uncertain.events.contains(&GameEvent::TalkIgnored));
        assert!(
            !uncertain
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::LanguageExposureRegistered(_)))
        );
        assert_eq!(session.world().creature.social_habits.profanity, 0.02);

        session
            .apply(command(SessionCommand::SpeechStarted))
            .expect("another speech starts");
        let missing = session
            .apply(command(SessionCommand::SpeechEnded))
            .expect("speech without words ends");
        assert_eq!(missing.spoken_input, Some(SpokenInputStatus::NoCandidate));
        assert!(missing.dialogue_request.is_none());
    }

    #[test]
    fn infrastructure_failure_is_distinct_from_creature_refusal() {
        let mut session = GameSession::new(53, "Technical");
        session
            .apply(command(SessionCommand::SpeechStarted))
            .expect("speech starts");
        let failed = session
            .apply(command(SessionCommand::SpeechFailed {
                failure: SpeechInputFailure::RecognizerUnavailable,
            }))
            .expect("recognition fails");
        assert_eq!(failed.events, Vec::new());
        assert_eq!(failed.dialogue_request, None);
        assert_eq!(
            failed.spoken_input,
            Some(SpokenInputStatus::InfrastructureFailure {
                failure: SpeechInputFailure::RecognizerUnavailable
            })
        );
        assert!(!failed.events.contains(&GameEvent::TalkIgnored));
    }

    #[test]
    fn ignored_speech_never_turns_recognition_into_a_talk() {
        let mut session = GameSession::new(531, "Sleeping");
        session.world.creature.current_intention = beastie_core::Intention::Sleep;
        session
            .apply(command(SessionCommand::SpeechStarted))
            .expect("sleeping creature perceives the sound");
        session
            .apply(command(SessionCommand::SpeechCandidate {
                text: "remember the berry".to_owned(),
                confidence: AcousticConfidence::new(900).expect("valid confidence"),
            }))
            .expect("recognition completes");
        let ended = session
            .apply(command(SessionCommand::SpeechEnded))
            .expect("speech ends");

        assert!(ended.dialogue_request.is_none());
        assert!(ended.events.is_empty());
        assert_eq!(
            ended.spoken_input,
            Some(SpokenInputStatus::NotEngaged {
                attention: beastie_core::SpeechAttention::Ignored
            })
        );
    }

    #[test]
    fn sleeping_creature_ignores_typed_language_without_side_effects() {
        let mut session = GameSession::new(5311, "Sleeping");
        session.world.creature.current_intention = beastie_core::Intention::Sleep;
        let before = session.world.clone();
        let ignored = session
            .apply(command(SessionCommand::Talk {
                text: "fuck remember the berry".to_owned(),
            }))
            .expect("sleeping creature ignores typed language");

        assert_eq!(ignored.events, vec![GameEvent::TalkIgnored]);
        assert!(ignored.dialogue_request.is_none());
        assert_eq!(session.world, before);
    }

    #[test]
    fn occupied_glance_waits_for_the_exact_toy_boundary_without_interrupting() {
        let mut session = GameSession::new(532, "Busy");
        session.world.creature.traits.sociability = 1.0;
        session.world.creature.relationship.bond = 1.0;
        session.world.creature.relationship.resentment = 0.0;
        session
            .world
            .creature
            .toy_preferences
            .insert(ToyId::Ball, 0.8);
        session
            .apply(command(SessionCommand::Play { toy: ToyId::Ball }))
            .expect("accepted toy establishes an exact owner");
        session
            .apply(command(SessionCommand::SpeechStarted))
            .expect("occupied creature glances");
        session
            .apply(command(SessionCommand::SpeechCandidate {
                text: "remember the berry".to_owned(),
                confidence: AcousticConfidence::new(900).expect("valid confidence"),
            }))
            .expect("recognition completes");
        let ended = session
            .apply(command(SessionCommand::SpeechEnded))
            .expect("speech ends");
        assert_eq!(ended.spoken_input, Some(SpokenInputStatus::Deferred));
        assert!(ended.events.contains(&GameEvent::UtteranceDeferred));
        assert!(ended.dialogue_request.is_none());

        let early = session
            .apply(command(SessionCommand::Tick {
                milliseconds: 2_000,
            }))
            .expect("time advances");
        assert!(early.dialogue_request.is_none());
        assert!(matches!(
            session.spoken_input,
            SpokenInputState::Deferred { .. }
        ));

        let mut saw_contact = false;
        let mut submitted = None;
        for _ in 0..20 {
            let observation = session
                .apply(command(SessionCommand::Tick {
                    milliseconds: 1_000,
                }))
                .expect("activity progresses toward its semantic boundary");
            saw_contact |= observation
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ToyContacted { .. }));
            if observation.dialogue_request.is_some() {
                submitted = Some(observation);
                break;
            }
        }
        let ready = submitted.expect("deferred response becomes ready after toy contact");
        assert!(saw_contact);
        assert_eq!(ready.spoken_input, Some(SpokenInputStatus::Submitted));
    }

    fn waiting_language(channel: InputChannel, behind_toy: bool) -> GameSession {
        let mut session = GameSession::new(532, "Patient");
        // Keep the pre-pass habitat that exposed the owner race, independent of new-save art.
        for (toy, x) in [
            (ToyId::Ball, 5000),
            (ToyId::Bell, 6500),
            (ToyId::Sock, 8000),
        ] {
            let legacy_position = NormalizedPosition::new(x, 8900);
            session
                .world
                .aquarium
                .toy_states
                .get_mut(&toy)
                .unwrap()
                .position = legacy_position;
            for object in session.world.aquarium.objects.values_mut() {
                if let beastie_core::WorldObject::Toy {
                    toy: candidate,
                    position,
                } = object
                    && *candidate == toy
                {
                    *position = legacy_position;
                }
            }
        }
        session.world.creature.traits.sociability = 1.0;
        session.world.creature.relationship.bond = 1.0;
        session.world.creature.relationship.resentment = 0.0;
        session
            .world
            .creature
            .toy_preferences
            .insert(ToyId::Ball, 0.8);
        if behind_toy {
            session
                .apply(command(SessionCommand::Play { toy: ToyId::Ball }))
                .unwrap();
        } else {
            session.world.creature.conversation.next_talk_at_ms = SIMULATION_TICK_MS;
        }
        let text = "remember the berry".to_owned();
        let queued = match channel {
            InputChannel::Typed => session
                .apply(command(SessionCommand::Talk { text }))
                .unwrap(),
            InputChannel::Spoken => {
                session
                    .apply(command(SessionCommand::SpeechStarted))
                    .unwrap();
                session
                    .apply(command(SessionCommand::SpeechCandidate {
                        text,
                        confidence: AcousticConfidence::new(900).unwrap(),
                    }))
                    .unwrap();
                session.apply(command(SessionCommand::SpeechEnded)).unwrap()
            }
        };
        assert!(queued.events.contains(&GameEvent::UtteranceDeferred));
        assert!(queued.dialogue_request.is_none());
        session
    }

    #[test]
    fn deferred_language_waits_for_replacement_play_refusal_and_food() {
        for channel in [InputChannel::Typed, InputChannel::Spoken] {
            for replacement in 0..3 {
                let mut session = waiting_language(channel, true);
                session
                    .world
                    .creature
                    .toy_preferences
                    .insert(ToyId::Bell, if replacement == 1 { -1.0 } else { 0.8 });
                let command_to_replace = if replacement == 2 {
                    SessionCommand::DropFood {
                        food: FoodId::Berry,
                        position: NormalizedPosition::new(7200, 1800),
                    }
                } else {
                    SessionCommand::Play { toy: ToyId::Bell }
                };
                let receipt = session.apply(command(command_to_replace)).unwrap();
                let replacement_owner = dialogue_handoff(&session.world).owner;
                assert!(replacement_owner.is_some());
                let body_before = session.world.creature.aquarium.clone();
                let interaction_before = session
                    .world
                    .creature
                    .interaction_state
                    .toy_interaction
                    .clone();
                let early = session
                    .apply(command(SessionCommand::Tick { milliseconds: 0 }))
                    .unwrap();
                assert!(
                    early.dialogue_request.is_none(),
                    "replacement {replacement} must keep its body"
                );
                assert_eq!(session.world.creature.aquarium, body_before);
                assert_eq!(
                    session.world.creature.interaction_state.toy_interaction,
                    interaction_before
                );
                assert_eq!(dialogue_handoff(&session.world).owner, replacement_owner);

                // The body survives persistence; the private waiting words deliberately do not.
                let save = session.capture(0).to_json().unwrap();
                let (resumed, _) = GameSession::resume_json(&save, 0).unwrap();
                assert_eq!(dialogue_handoff(&resumed.world).owner, replacement_owner);
                assert_eq!(
                    resumed.world.creature.aquarium.destination,
                    body_before.destination
                );
                assert!(matches!(resumed.spoken_input, SpokenInputState::Idle));

                let mut events = receipt.events;
                let mut delivered = false;
                for _ in 0..60 {
                    let next = session
                        .apply(command(SessionCommand::Tick {
                            milliseconds: SIMULATION_TICK_MS,
                        }))
                        .unwrap();
                    events.extend(next.events);
                    if next.dialogue_request.is_some() {
                        delivered = true;
                        break;
                    }
                }
                assert!(
                    delivered,
                    "replacement {replacement} must eventually release words"
                );
                assert!(!events.iter().any(|event| matches!(
                    event,
                    GameEvent::ToyInteractionInterrupted {
                        toy: ToyId::Bell,
                        ..
                    }
                )));
                match replacement {
                    0 => assert_eq!(
                        events
                            .iter()
                            .filter(|event| matches!(
                                event,
                                GameEvent::ToyPlayed {
                                    toy: ToyId::Bell,
                                    ..
                                }
                            ))
                            .count(),
                        1
                    ),
                    1 => {
                        assert_eq!(
                            events
                                .iter()
                                .filter(|event| matches!(
                                    event,
                                    GameEvent::ToyRejected {
                                        toy: ToyId::Bell,
                                        ..
                                    }
                                ))
                                .count(),
                            1
                        );
                        assert!(!events.iter().any(|event| matches!(
                            event,
                            GameEvent::ToyPlayed { .. } | GameEvent::ToyContacted { .. }
                        )));
                        assert!(
                            session
                                .world
                                .creature
                                .interaction_state
                                .last_resolved_toy_interaction
                                .is_none()
                        );
                    }
                    _ => assert_eq!(
                        events
                            .iter()
                            .filter(|event| matches!(event, GameEvent::FoodConsumed(FoodId::Berry)))
                            .count(),
                        1
                    ),
                }
                assert_eq!(session.world.creature.development.interactions.talks, 1);
            }
        }
    }

    #[test]
    fn cooldown_only_speech_also_respects_new_direct_play() {
        let mut session = waiting_language(InputChannel::Spoken, false);
        session
            .apply(command(SessionCommand::Play { toy: ToyId::Ball }))
            .unwrap();
        let owner = dialogue_handoff(&session.world).owner;
        let early = session
            .apply(command(SessionCommand::Tick {
                milliseconds: SIMULATION_TICK_MS,
            }))
            .unwrap();
        assert!(early.dialogue_request.is_none());
        assert_eq!(dialogue_handoff(&session.world).owner, owner);
        assert!(matches!(
            session.spoken_input,
            SpokenInputState::Deferred { .. }
        ));
    }

    #[test]
    fn completed_owner_does_not_let_unrelated_private_life_capture_waiting_words() {
        let mut session = waiting_language(InputChannel::Typed, true);
        // Advance the body independently, as when a single session tick crosses both the
        // original toy's recovery and the start of a new private activity.
        for _ in 0..30 {
            step(
                &mut session.world,
                &[],
                SIMULATION_TICK_MS,
                &mut session.random,
            );
            if matches!(
                dialogue_handoff(&session.world).owner,
                Some(DialogueActionOwner::PrivateLife(_))
            ) {
                break;
            }
        }
        let handoff = dialogue_handoff(&session.world);
        assert!(matches!(
            handoff.owner,
            Some(DialogueActionOwner::PrivateLife(_))
        ));
        assert_eq!(handoff.state, DialogueHandoffState::WaitingForContact);
        let released = session
            .apply(command(SessionCommand::Tick { milliseconds: 0 }))
            .unwrap();
        assert!(released.dialogue_request.is_some());
        assert!(matches!(session.spoken_input, SpokenInputState::Idle));
    }

    #[test]
    fn autonomous_toy_interest_does_not_capture_cooldown_only_words() {
        let mut session = waiting_language(InputChannel::Spoken, false);
        session
            .apply(command(SessionCommand::Play { toy: ToyId::Ball }))
            .unwrap();
        // Autonomous toy interactions are also valid canonical owners (including in saves),
        // but carry no newer player action that should take precedence over waiting language.
        session
            .world
            .creature
            .interaction_state
            .toy_interaction
            .as_mut()
            .unwrap()
            .origin = ToyOrigin::Autonomous;
        let released = session
            .apply(command(SessionCommand::Tick {
                milliseconds: SIMULATION_TICK_MS,
            }))
            .unwrap();
        assert!(released.dialogue_request.is_some());
        assert_eq!(released.spoken_input, Some(SpokenInputStatus::Submitted));
    }

    #[test]
    fn repeated_direct_replacement_keeps_original_language_expiry_and_care_immediate() {
        for channel in [InputChannel::Typed, InputChannel::Spoken] {
            let mut session = waiting_language(channel, true);
            session
                .world
                .creature
                .toy_preferences
                .insert(ToyId::Bell, -1.0);
            // Start near the existing deadline so repeated replacements can be exercised
            // before this distant refusal finishes its approach.
            if let SpokenInputState::Deferred { expires_at_ms, .. } = &session.spoken_input {
                session.world.elapsed_ms = expires_at_ms - 2 * SIMULATION_TICK_MS;
            }
            let mut expired = false;
            for _ in 0..DEFERRED_UTTERANCE_MAX_MS / SIMULATION_TICK_MS {
                let care = session.apply(command(SessionCommand::Comfort)).unwrap();
                assert!(care.events.contains(&GameEvent::Comforted));
                // A new refusal replaces comfort immediately, even while old words wait.
                let rejection = session
                    .apply(command(SessionCommand::Play { toy: ToyId::Bell }))
                    .unwrap();
                assert!(
                    rejection
                        .events
                        .iter()
                        .any(|event| matches!(event, GameEvent::ToyRejected { .. }))
                );
                let owner = dialogue_handoff(&session.world).owner;
                let next = session
                    .apply(command(SessionCommand::Tick {
                        milliseconds: SIMULATION_TICK_MS,
                    }))
                    .unwrap();
                assert!(next.dialogue_request.is_none());
                assert_eq!(dialogue_handoff(&session.world).owner, owner);
                if matches!(session.spoken_input, SpokenInputState::Idle) {
                    expired = true;
                    match channel {
                        InputChannel::Typed => {
                            assert!(next.events.contains(&GameEvent::TalkIgnored))
                        }
                        InputChannel::Spoken => {
                            assert_eq!(next.spoken_input, Some(SpokenInputStatus::Expired))
                        }
                    }
                    break;
                }
            }
            assert!(expired);
            assert_eq!(session.world.creature.development.interactions.talks, 0);
        }
    }

    #[test]
    fn deferred_utterance_rechecks_sleep_and_resentment_before_release() {
        fn deferred_session(seed: u64) -> GameSession {
            let mut session = GameSession::new(seed, "Busy");
            session.world.creature.current_intention = beastie_core::Intention::Play;
            session.world.creature.traits.sociability = 1.0;
            session.world.creature.relationship.bond = 1.0;
            session
                .apply(command(SessionCommand::Talk {
                    text: "remember the berry".to_owned(),
                }))
                .expect("occupied creature defers");
            assert!(matches!(
                session.spoken_input,
                SpokenInputState::Deferred { .. }
            ));
            session
        }

        let mut sleeping = deferred_session(5_321);
        sleeping.world.creature.current_intention = beastie_core::Intention::Sleep;
        sleeping.world.creature.needs.energy = 0.05;
        let ignored = sleeping
            .apply(command(SessionCommand::Tick {
                milliseconds: SIMULATION_TICK_MS.saturating_mul(2),
            }))
            .expect("sleeping release is handled");
        assert!(ignored.dialogue_request.is_none());
        assert!(ignored.events.contains(&GameEvent::TalkIgnored));
        assert!(matches!(sleeping.spoken_input, SpokenInputState::Idle));

        let mut resentful = deferred_session(5_322);
        resentful.world.creature.relationship.resentment = 0.9;
        let refused = resentful
            .apply(command(SessionCommand::Tick {
                milliseconds: SIMULATION_TICK_MS.saturating_mul(2),
            }))
            .expect("resentful release is handled");
        assert!(refused.dialogue_request.is_none());
        assert!(refused.events.contains(&GameEvent::UtteranceRefused));
        assert!(matches!(resentful.spoken_input, SpokenInputState::Idle));
    }

    #[test]
    fn deferred_utterance_expires_honestly_instead_of_seizing_the_body() {
        let mut session = GameSession::new(5_323, "Patient");
        session.spoken_input = SpokenInputState::Deferred {
            candidate: SpokenCandidate {
                text: "remember the berry".to_owned(),
                confidence: AcousticConfidence::new(900).expect("valid confidence"),
            },
            ready_at_ms: session
                .world
                .elapsed_ms
                .saturating_add(DEFERRED_UTTERANCE_MAX_MS + SIMULATION_TICK_MS),
            expires_at_ms: session
                .world
                .elapsed_ms
                .saturating_add(DEFERRED_UTTERANCE_MAX_MS),
            owner: None,
            channel: InputChannel::Spoken,
        };

        session.world.elapsed_ms = session
            .world
            .elapsed_ms
            .saturating_add(DEFERRED_UTTERANCE_MAX_MS);
        let mut dialogue_request = None;
        let mut events = Vec::new();
        let mut status = None;
        session
            .apply_deferred_speech(&mut dialogue_request, &mut events, &mut status)
            .expect("bounded deferred utterance expires");

        assert_eq!(status, Some(SpokenInputStatus::Expired));
        assert!(dialogue_request.is_none());
        assert!(events.is_empty());
        assert!(matches!(session.spoken_input, SpokenInputState::Idle));
    }

    #[test]
    fn unknown_language_is_removed_before_typed_or_spoken_prompt_grounding() {
        let text = "Muck bring the red gizmo from beside the bed";
        let mut typed = GameSession::new(533, "Muck");
        let mut spoken = typed.clone();

        let typed_request = typed
            .apply(command(SessionCommand::Talk {
                text: text.to_owned(),
            }))
            .expect("typed utterance")
            .dialogue_request
            .expect("idle creature attends");
        spoken
            .apply(command(SessionCommand::SpeechStarted))
            .expect("speech starts");
        spoken
            .apply(command(SessionCommand::SpeechCandidate {
                text: text.to_owned(),
                confidence: AcousticConfidence::new(900).expect("confidence"),
            }))
            .expect("candidate");
        let spoken_request = spoken
            .apply(command(SessionCommand::SpeechEnded))
            .expect("speech ends")
            .dialogue_request
            .expect("idle creature attends");

        assert_eq!(typed_request, spoken_request);
        assert_eq!(typed_request.player_said, "muck");
        assert!(!typed_request.player_said.contains("gizmo"));
        assert!(!typed_request.player_said.contains("beside"));
        assert!(typed_request.interpretation.unknown_words >= 5);
        assert!(typed_request.candidate_memories.is_empty());
    }

    #[test]
    fn resentment_is_a_legible_refusal_for_typed_and_spoken_without_mutation() {
        let mut typed = GameSession::new(534, "Grudge");
        typed.world.creature.relationship.resentment = 0.9;
        typed.world.creature.current_intention = beastie_core::Intention::Play;
        let before = typed.world.clone();
        let refused = typed
            .apply(command(SessionCommand::Talk {
                text: "fuck that toy".to_owned(),
            }))
            .expect("typed refusal");
        assert!(refused.events.contains(&GameEvent::UtteranceRefused));
        assert!(
            refused
                .events
                .contains(&GameEvent::NonverbalAct(NonverbalAct::RefuseAndStare))
        );
        assert!(refused.dialogue_request.is_none());
        assert_eq!(
            typed.world.creature.current_intention,
            before.creature.current_intention
        );
        assert_eq!(
            typed.world.creature.conversation,
            before.creature.conversation
        );
        assert_eq!(
            typed.world.creature.social_habits,
            before.creature.social_habits
        );

        let mut spoken = GameSession::new(534, "Grudge");
        spoken.world = before;
        spoken
            .apply(command(SessionCommand::SpeechStarted))
            .expect("resentful creature visibly hears");
        spoken
            .apply(command(SessionCommand::SpeechCandidate {
                text: "fuck that toy".to_owned(),
                confidence: AcousticConfidence::new(900).expect("confidence"),
            }))
            .expect("candidate");
        let refused = spoken
            .apply(command(SessionCommand::SpeechEnded))
            .expect("spoken refusal");
        assert_eq!(refused.spoken_input, Some(SpokenInputStatus::Refused));
        assert!(refused.events.contains(&GameEvent::UtteranceRefused));
        assert_eq!(
            spoken.world.creature.conversation,
            typed.world.creature.conversation
        );
        assert_eq!(
            spoken.world.creature.social_habits,
            typed.world.creature.social_habits
        );
    }

    #[test]
    fn newer_utterance_supersedes_deferred_text_without_cooldown_or_habit_mutation() {
        let mut session = GameSession::new(535, "Busy");
        session.world.creature.current_intention = beastie_core::Intention::Play;
        session.world.creature.traits.sociability = 1.0;
        session.world.creature.relationship.bond = 1.0;
        session
            .apply(command(SessionCommand::Talk {
                text: "play toy".to_owned(),
            }))
            .expect("first utterance is deferred");
        let world_before_overlap = session.world.clone();

        let replacement = session
            .apply(command(SessionCommand::Talk {
                text: "fuck shit".to_owned(),
            }))
            .expect("newer utterance replaces the deferred candidate");
        assert!(replacement.dialogue_request.is_none());
        assert_eq!(session.world, world_before_overlap);
        assert!(matches!(
            &session.spoken_input,
            SpokenInputState::Deferred { candidate, .. } if candidate.text == "fuck shit"
        ));
    }

    #[test]
    fn recognized_speech_waits_out_cooldown_instead_of_being_lost() {
        let mut session = GameSession::new(5351, "Patient");
        session.world.creature.conversation.next_talk_at_ms = SIMULATION_TICK_MS.saturating_mul(2);
        session
            .apply(command(SessionCommand::SpeechStarted))
            .expect("speech starts");
        session
            .apply(command(SessionCommand::SpeechCandidate {
                text: "remember food".to_owned(),
                confidence: AcousticConfidence::new(900).expect("confidence"),
            }))
            .expect("candidate");
        let before = session.world.creature.conversation;
        let ended = session
            .apply(command(SessionCommand::SpeechEnded))
            .expect("recognized speech is held");
        assert_eq!(ended.spoken_input, Some(SpokenInputStatus::Deferred));
        assert_eq!(session.world.creature.conversation, before);
        assert_eq!(session.world.creature.development.interactions.talks, 0);

        let early = session
            .apply(command(SessionCommand::Tick {
                milliseconds: SIMULATION_TICK_MS,
            }))
            .expect("first cooldown tick");
        assert!(early.dialogue_request.is_none());
        let ready = session
            .apply(command(SessionCommand::Tick {
                milliseconds: SIMULATION_TICK_MS,
            }))
            .expect("cooldown expires");
        assert!(ready.dialogue_request.is_some());
        assert_eq!(ready.spoken_input, Some(SpokenInputStatus::Submitted));
    }

    #[test]
    fn deferred_transcript_is_episode_transient_and_fixed_tick_deterministic() {
        let mut first = GameSession::new(536, "Delay");
        first.world.creature.traits.sociability = 1.0;
        first.world.creature.relationship.bond = 1.0;
        first
            .world
            .creature
            .toy_preferences
            .insert(ToyId::Ball, 0.8);
        first
            .apply(command(SessionCommand::Play { toy: ToyId::Ball }))
            .expect("accepted toy establishes an exact deferred owner");
        let mut second = first.clone();
        for session in [&mut first, &mut second] {
            session
                .apply(command(SessionCommand::SpeechStarted))
                .expect("speech starts");
            session
                .apply(command(SessionCommand::SpeechCandidate {
                    text: "private gizmo toy".to_owned(),
                    confidence: AcousticConfidence::new(900).expect("confidence"),
                }))
                .expect("candidate");
            let ended = session
                .apply(command(SessionCommand::SpeechEnded))
                .expect("speech is deferred");
            assert_eq!(ended.spoken_input, Some(SpokenInputStatus::Deferred));
        }

        let save = first.capture(0).to_json().expect("save");
        assert!(!save.contains("private gizmo toy"));
        let (resumed, _) = GameSession::resume_json(&save, 0).expect("resume");
        assert!(matches!(resumed.spoken_input, SpokenInputState::Idle));

        let mut submitted = false;
        for _ in 0..20 {
            let next_a = first
                .apply(command(SessionCommand::Tick {
                    milliseconds: SIMULATION_TICK_MS,
                }))
                .expect("first deterministic tick");
            let next_b = second
                .apply(command(SessionCommand::Tick {
                    milliseconds: SIMULATION_TICK_MS,
                }))
                .expect("matching deterministic tick");
            assert_eq!(next_a, next_b);
            if next_a.dialogue_request.is_some() {
                submitted = true;
                break;
            }
        }
        assert!(
            submitted,
            "the exact toy boundary eventually releases dialogue"
        );
    }

    #[test]
    fn spoken_lifecycle_replays_deterministically_and_raw_words_are_not_saved() {
        let commands = [
            command(SessionCommand::SpeechStarted),
            command(SessionCommand::SpeechCandidate {
                text: "private spoken words".to_owned(),
                confidence: AcousticConfidence::new(800).expect("valid confidence"),
            }),
            command(SessionCommand::SpeechEnded),
        ];
        let mut first = GameSession::new(54, "Replay");
        let mut second = GameSession::new(54, "Replay");
        for input in commands {
            assert_eq!(
                first.apply(input.clone()).expect("first replay"),
                second.apply(input).expect("second replay")
            );
        }
        assert_eq!(first.world(), second.world());

        let mut mid_speech = GameSession::new(55, "Private");
        mid_speech
            .apply(command(SessionCommand::SpeechStarted))
            .expect("speech starts");
        mid_speech
            .apply(command(SessionCommand::SpeechCandidate {
                text: "never persist this transcript".to_owned(),
                confidence: AcousticConfidence::new(900).expect("valid confidence"),
            }))
            .expect("candidate arrives");
        let saved = mid_speech
            .capture(0)
            .to_json()
            .expect("mid-speech save remains valid");
        assert!(!saved.contains("never persist this transcript"));
        let (mut resumed, _) = GameSession::resume_json(&saved, 0).expect("resume save");
        assert!(matches!(
            resumed.apply(command(SessionCommand::SpeechEnded)),
            Err(SessionError::SpeechNotStarted)
        ));
    }

    #[test]
    fn malformed_oversized_and_out_of_order_speech_reject_without_mutation() {
        assert!(
            GameSession::parse_command(
                r#"{"version":1,"command":"speech_candidate","text":"hello","confidence":1001}"#
            )
            .is_err()
        );
        assert!(GameSession::parse_command(
            r#"{"version":1,"command":"speech_candidate","text":"hello","confidence":900,"extra":true}"#
        )
        .is_err());

        let mut session = GameSession::new(56, "Strict Speech");
        let before = session.capture(0).to_json().expect("snapshot");
        assert!(matches!(
            session.apply(command(SessionCommand::SpeechEnded)),
            Err(SessionError::SpeechNotStarted)
        ));
        assert_eq!(session.capture(0).to_json().expect("unchanged"), before);

        session
            .apply(command(SessionCommand::SpeechStarted))
            .expect("speech starts");
        let listening = session.capture(0).to_json().expect("listening snapshot");
        assert!(matches!(
            session.apply(command(SessionCommand::SpeechCandidate {
                text: "x".repeat(513),
                confidence: AcousticConfidence::new(900).expect("valid confidence"),
            })),
            Err(SessionError::TalkTooLong)
        ));
        assert_eq!(
            session.capture(0).to_json().expect("still unchanged"),
            listening
        );
        assert!(matches!(
            session.apply(command(SessionCommand::SpeechStarted)),
            Err(SessionError::SpeechAlreadyStarted)
        ));
    }

    #[test]
    fn completed_dialogue_persists_only_semantic_history_and_fingerprint() {
        let mut session = GameSession::new(57, "History");
        let observation = session
            .apply(command(SessionCommand::Talk {
                text: "hello there".to_owned(),
            }))
            .expect("talk request");
        let request = observation.dialogue_request.expect("dialogue request");
        let reply = beastie_protocol::constrained_fallback_reply(&request);
        assert!(session.accept_dialogue_turn(&request, &reply, 0, true));
        let saved = session.capture(0).to_json().expect("history save");
        assert!(saved.contains("dialogue_history"));
        assert!(saved.contains("reply_fingerprint"));
        assert!(!saved.contains("hello there"));
        let (resumed, _) = GameSession::resume_json(&saved, 0).expect("history migration");
        assert_eq!(resumed.dialogue_history().recent.len(), 1);
    }
}
