//! Versioned JSONL protocol between the game and the untrusted AI worker.

use std::collections::BTreeSet;

use beastie_core::{
    ACTIVE_DAY_MS, Concept, Memory, MemoryId, MemoryKind, MemoryQuery, SocialAct, WorldState,
    select_candidate_memories,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_DIALOGUE_REPLY_BYTES: usize = 512;
pub const TTS_PROTOCOL_VERSION: u32 = 1;
pub const MAX_TTS_TEXT_BYTES: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TtsVoiceSettings {
    pub speaker_id: u8,
    pub speed: f32,
    pub silence_scale: f32,
}

impl Default for TtsVoiceSettings {
    fn default() -> Self {
        Self {
            speaker_id: 0,
            speed: 1.0,
            silence_scale: 0.2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TtsRequest {
    pub protocol_version: u32,
    pub request_id: u64,
    pub text: String,
    pub settings: TtsVoiceSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsErrorCode {
    InvalidRequest,
    SynthesisFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum TtsOutcome {
    Ready { cache_key: String },
    Error { code: TtsErrorCode },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TtsReply {
    pub protocol_version: u32,
    pub request_id: u64,
    #[serde(flatten)]
    pub outcome: TtsOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gesture {
    None,
    LookPlayer,
    LookWindow,
    Shiver,
    Sleepy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateMemory {
    pub id: MemoryId,
    pub fact: String,
    pub feeling: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueConstraints {
    pub max_words: usize,
    pub allowed_gestures: BTreeSet<Gesture>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueRequest {
    pub protocol_version: u32,
    pub request_id: u64,
    pub creature_name: String,
    pub mood: String,
    pub known_concepts: BTreeSet<Concept>,
    pub candidate_memories: Vec<CandidateMemory>,
    #[serde(default)]
    pub desired_social_act: Option<SocialAct>,
    pub player_said: String,
    pub constraints: DialogueConstraints,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueReply {
    pub protocol_version: u32,
    pub request_id: u64,
    pub say: String,
    pub gesture: Gesture,
    pub recalled_memory: Option<MemoryId>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TtsValidationError {
    #[error("TTS protocol version {0} is unsupported")]
    ProtocolVersion(u32),
    #[error("TTS reply request id does not match")]
    RequestId,
    #[error("TTS text is empty or exceeds its byte limit")]
    Text,
    #[error("TTS speaker ID is outside 0..=7")]
    Voice,
    #[error("TTS speed is outside 0.5..=2.0")]
    Speed,
    #[error("TTS silence scale is outside 0.0..=2.0")]
    SilenceScale,
    #[error("TTS cache key is not a lowercase SHA-256 digest")]
    CacheKey,
}

pub fn validate_tts_request(request: &TtsRequest) -> Result<(), TtsValidationError> {
    if request.protocol_version != TTS_PROTOCOL_VERSION {
        return Err(TtsValidationError::ProtocolVersion(
            request.protocol_version,
        ));
    }
    if request.text.trim().is_empty() || request.text.len() > MAX_TTS_TEXT_BYTES {
        return Err(TtsValidationError::Text);
    }
    if request.text.contains('\0') {
        return Err(TtsValidationError::Text);
    }
    if request.settings.speaker_id > 7 {
        return Err(TtsValidationError::Voice);
    }
    if !request.settings.speed.is_finite() || !(0.5..=2.0).contains(&request.settings.speed) {
        return Err(TtsValidationError::Speed);
    }
    if !request.settings.silence_scale.is_finite()
        || !(0.0..=2.0).contains(&request.settings.silence_scale)
    {
        return Err(TtsValidationError::SilenceScale);
    }
    Ok(())
}

pub fn validate_tts_reply(
    request: &TtsRequest,
    reply: TtsReply,
) -> Result<TtsReply, TtsValidationError> {
    if reply.protocol_version != TTS_PROTOCOL_VERSION {
        return Err(TtsValidationError::ProtocolVersion(reply.protocol_version));
    }
    if reply.request_id != request.request_id {
        return Err(TtsValidationError::RequestId);
    }
    if let TtsOutcome::Ready { cache_key } = &reply.outcome
        && (cache_key.len() != 64
            || !cache_key
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    {
        return Err(TtsValidationError::CacheKey);
    }
    Ok(reply)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("protocol version {0} is unsupported")]
    ProtocolVersion(u32),
    #[error("reply request id does not match")]
    RequestId,
    #[error("reply exceeds the {maximum}-word limit with {actual} words")]
    TooManyWords { maximum: usize, actual: usize },
    #[error("reply text must not be empty")]
    EmptyReply,
    #[error("reply text exceeds the {maximum}-byte limit with {actual} bytes")]
    ReplyTooLong { maximum: usize, actual: usize },
    #[error("reply text contains an ASCII control character")]
    ReplyControl,
    #[error("reply gesture is not allowed")]
    Gesture,
    #[error("reply references a memory that was not offered")]
    Memory,
    #[error("request must allow between 1 and 24 words")]
    WordLimit,
    #[error("request must allow at least one gesture")]
    NoGestures,
    #[error("player input exceeds 512 characters")]
    PlayerInput,
    #[error("creature name exceeds 64 characters")]
    CreatureName,
    #[error("request offers more than 8 candidate memories")]
    CandidateCount,
    #[error("candidate memory text exceeds its limit")]
    CandidateText,
    #[error("candidate memory IDs must be unique")]
    DuplicateMemory,
}

pub fn validate_request(request: &DialogueRequest) -> Result<(), ValidationError> {
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(ValidationError::ProtocolVersion(request.protocol_version));
    }
    if !(1..=24).contains(&request.constraints.max_words) {
        return Err(ValidationError::WordLimit);
    }
    if request.constraints.allowed_gestures.is_empty() {
        return Err(ValidationError::NoGestures);
    }
    if request.player_said.chars().count() > 512 {
        return Err(ValidationError::PlayerInput);
    }
    if request.creature_name.chars().count() > 64 {
        return Err(ValidationError::CreatureName);
    }
    if request.candidate_memories.len() > 8 {
        return Err(ValidationError::CandidateCount);
    }
    if request
        .candidate_memories
        .iter()
        .any(|memory| memory.fact.chars().count() > 256 || memory.feeling.chars().count() > 64)
    {
        return Err(ValidationError::CandidateText);
    }
    let unique_memory_count = request
        .candidate_memories
        .iter()
        .map(|memory| memory.id)
        .collect::<BTreeSet<_>>()
        .len();
    if unique_memory_count != request.candidate_memories.len() {
        return Err(ValidationError::DuplicateMemory);
    }
    Ok(())
}

pub struct DialogueRequestContext<'a> {
    pub request_id: u64,
    pub mood: &'a str,
    pub player_said: &'a str,
    pub desired_social_act: Option<SocialAct>,
    pub max_words: usize,
    pub allowed_gestures: BTreeSet<Gesture>,
}

#[must_use]
pub fn build_dialogue_request(
    world: &WorldState,
    memory_query: &MemoryQuery,
    context: DialogueRequestContext<'_>,
) -> DialogueRequest {
    DialogueRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: context.request_id,
        creature_name: world.creature.name.clone(),
        mood: context.mood.to_owned(),
        known_concepts: world.creature.known_concepts.clone(),
        candidate_memories: select_candidate_memories(world, memory_query)
            .iter()
            .map(|memory| project_candidate_memory(memory, world.active_day()))
            .collect(),
        desired_social_act: context.desired_social_act,
        player_said: context.player_said.to_owned(),
        constraints: DialogueConstraints {
            max_words: context.max_words,
            allowed_gestures: context.allowed_gestures,
        },
    }
}

#[must_use]
pub fn project_candidate_memory(memory: &Memory, current_day: u64) -> CandidateMemory {
    let happened_day = memory.happened_at_ms / ACTIVE_DAY_MS + 1;
    let when = match current_day.saturating_sub(happened_day) {
        0 => "Today",
        1 => "Yesterday",
        _ => "Earlier",
    };
    let fact = match memory.kind {
        MemoryKind::WasFed { food } => {
            format!("{when} the player gave you a {}.", food_name(food))
        }
        MemoryKind::DislikedFood { food } => {
            format!(
                "{when} you discovered that you dislike {}.",
                food_name(food)
            )
        }
        MemoryKind::RejectedFood { food } => {
            format!("{when} you pushed away the {}.", food_name(food))
        }
        MemoryKind::PlayedWith { toy } => {
            format!(
                "{when} the player played with you using the {}.",
                toy_name(toy)
            )
        }
        MemoryKind::DislikedToy { toy } => {
            format!(
                "{when} you discovered that you dislike the {}.",
                toy_name(toy)
            )
        }
        MemoryKind::WasComforted => format!("{when} the player comforted you."),
        MemoryKind::PlayerReturnedAfterAbsence => {
            format!("{when} the player returned after being away.")
        }
        MemoryKind::PlayerReacted { reaction, to } => {
            format!("{when} the player reacted with {reaction:?} to your {to:?}.")
        }
    };
    let feeling = match memory.valence {
        value if value <= -0.65 => "strong dislike",
        value if value <= -0.2 => "dislike",
        value if value < 0.2 => "uncertain",
        value if value < 0.65 => "liked",
        _ => "strongly liked",
    };
    CandidateMemory {
        id: memory.id,
        fact,
        feeling: feeling.to_owned(),
    }
}

fn food_name(food: beastie_core::FoodId) -> &'static str {
    match food {
        beastie_core::FoodId::Berry => "berry",
        beastie_core::FoodId::Mushroom => "mushroom",
        beastie_core::FoodId::Pellet => "pellet",
    }
}

fn toy_name(toy: beastie_core::ToyId) -> &'static str {
    match toy {
        beastie_core::ToyId::Ball => "ball",
        beastie_core::ToyId::Bell => "bell",
        beastie_core::ToyId::Sock => "sock",
    }
}

pub fn validate_reply(
    request: &DialogueRequest,
    reply: DialogueReply,
) -> Result<DialogueReply, ValidationError> {
    if reply.protocol_version != PROTOCOL_VERSION {
        return Err(ValidationError::ProtocolVersion(reply.protocol_version));
    }
    if reply.request_id != request.request_id {
        return Err(ValidationError::RequestId);
    }
    if reply.say.trim().is_empty() {
        return Err(ValidationError::EmptyReply);
    }
    if reply.say.len() > MAX_DIALOGUE_REPLY_BYTES {
        return Err(ValidationError::ReplyTooLong {
            maximum: MAX_DIALOGUE_REPLY_BYTES,
            actual: reply.say.len(),
        });
    }
    if reply.say.bytes().any(|byte| byte <= 0x1f || byte == 0x7f) {
        return Err(ValidationError::ReplyControl);
    }
    let actual = reply.say.split_whitespace().count();
    if actual > request.constraints.max_words {
        return Err(ValidationError::TooManyWords {
            maximum: request.constraints.max_words,
            actual,
        });
    }
    if !request
        .constraints
        .allowed_gestures
        .contains(&reply.gesture)
    {
        return Err(ValidationError::Gesture);
    }
    if let Some(id) = reply.recalled_memory
        && !request
            .candidate_memories
            .iter()
            .any(|memory| memory.id == id)
    {
        return Err(ValidationError::Memory);
    }
    Ok(reply)
}

#[must_use]
pub fn constrained_fallback_reply(request: &DialogueRequest) -> DialogueReply {
    let phrase = "too many thought.";
    let say = phrase
        .split_whitespace()
        .take(request.constraints.max_words)
        .collect::<Vec<_>>()
        .join(" ");
    DialogueReply {
        protocol_version: PROTOCOL_VERSION,
        request_id: request.request_id,
        say,
        gesture: request
            .constraints
            .allowed_gestures
            .iter()
            .next()
            .copied()
            .unwrap_or(Gesture::None),
        recalled_memory: None,
    }
}

#[must_use]
pub fn fallback_reply(request_id: u64) -> DialogueReply {
    DialogueReply {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        say: "too many thought.".to_owned(),
        gesture: Gesture::None,
        recalled_memory: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> DialogueRequest {
        DialogueRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: 7,
            creature_name: "Mop".to_owned(),
            mood: "sleepy".to_owned(),
            known_concepts: BTreeSet::new(),
            candidate_memories: vec![CandidateMemory {
                id: MemoryId(41),
                fact: "The player gave you a berry.".to_owned(),
                feeling: "strong dislike".to_owned(),
            }],
            desired_social_act: Some(SocialAct::Insult),
            player_said: "Remember the berry?".to_owned(),
            constraints: DialogueConstraints {
                max_words: 6,
                allowed_gestures: BTreeSet::from([Gesture::None, Gesture::LookPlayer]),
            },
        }
    }

    #[test]
    fn accepts_a_bounded_reply_with_an_offered_memory() {
        let reply = DialogueReply {
            protocol_version: PROTOCOL_VERSION,
            request_id: 7,
            say: "red food remains crime.".to_owned(),
            gesture: Gesture::LookPlayer,
            recalled_memory: Some(MemoryId(41)),
        };
        assert_eq!(validate_reply(&request(), reply.clone()), Ok(reply));
    }

    #[test]
    fn rejects_invented_memory_references() {
        let reply = DialogueReply {
            protocol_version: PROTOCOL_VERSION,
            request_id: 7,
            say: "yes.".to_owned(),
            gesture: Gesture::None,
            recalled_memory: Some(MemoryId(999)),
        };
        assert_eq!(
            validate_reply(&request(), reply),
            Err(ValidationError::Memory)
        );
    }

    #[test]
    fn rejects_empty_oversized_and_control_character_replies() {
        let mut reply = DialogueReply {
            protocol_version: PROTOCOL_VERSION,
            request_id: 7,
            say: "   ".to_owned(),
            gesture: Gesture::None,
            recalled_memory: None,
        };
        assert_eq!(
            validate_reply(&request(), reply.clone()),
            Err(ValidationError::EmptyReply)
        );

        reply.say = "x".repeat(MAX_DIALOGUE_REPLY_BYTES + 1);
        assert_eq!(
            validate_reply(&request(), reply.clone()),
            Err(ValidationError::ReplyTooLong {
                maximum: MAX_DIALOGUE_REPLY_BYTES,
                actual: MAX_DIALOGUE_REPLY_BYTES + 1,
            })
        );

        for text in ["two\nlines", "tab\ttext", "nul\0text", "delete\u{7f}"] {
            reply.say = text.to_owned();
            assert_eq!(
                validate_reply(&request(), reply.clone()),
                Err(ValidationError::ReplyControl),
                "{text:?}"
            );
        }
    }

    #[test]
    fn accepts_ordinary_unicode_reply_text() {
        let reply = DialogueReply {
            protocol_version: PROTOCOL_VERSION,
            request_id: 7,
            say: "møp says こんにちは.".to_owned(),
            gesture: Gesture::None,
            recalled_memory: None,
        };
        assert_eq!(validate_reply(&request(), reply.clone()), Ok(reply));
    }

    #[test]
    fn rejects_an_unsupported_request_version() {
        let mut request = request();
        request.protocol_version = 999;
        assert_eq!(
            validate_request(&request),
            Err(ValidationError::ProtocolVersion(999))
        );
    }

    #[test]
    fn constrained_fallback_obeys_reply_bounds() {
        let mut request = request();
        request.constraints.max_words = 1;
        request.constraints.allowed_gestures = BTreeSet::from([Gesture::Shiver]);
        let reply = constrained_fallback_reply(&request);
        assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
    }

    #[test]
    fn builds_a_grounded_request_from_real_world_memory() {
        use beastie_core::{FoodId, MemoryCue, PlayerEvent, SeededRandom, step};

        let mut world = WorldState::new(99, "Mrrp");
        let mut random = SeededRandom::new(world.seed);
        world.creature.needs.hunger = 1.0;
        step(
            &mut world,
            &[PlayerEvent::Feed(FoodId::Berry)],
            1_000,
            &mut random,
        );
        for _ in 0..4 {
            step(&mut world, &[], 1_000, &mut random);
        }
        world.elapsed_ms = ACTIVE_DAY_MS;

        let request = build_dialogue_request(
            &world,
            &MemoryQuery {
                cues: BTreeSet::from([MemoryCue::Food(FoodId::Berry)]),
                limit: 8,
            },
            DialogueRequestContext {
                request_id: 12,
                mood: "wary",
                player_said: "Remember the berry?",
                desired_social_act: Some(SocialAct::Insult),
                max_words: 12,
                allowed_gestures: BTreeSet::from([Gesture::None, Gesture::LookPlayer]),
            },
        );

        validate_request(&request).expect("projected request should be valid");
        assert!(request.candidate_memories.iter().any(|memory| {
            memory.fact == "Yesterday you discovered that you dislike berry."
                && memory.feeling == "strong dislike"
        }));
        let recalled_memory = request
            .candidate_memories
            .first()
            .expect("berry memory should be offered")
            .id;
        let reply = DialogueReply {
            protocol_version: PROTOCOL_VERSION,
            request_id: 12,
            say: "red betrayal remains.".to_owned(),
            gesture: Gesture::LookPlayer,
            recalled_memory: Some(recalled_memory),
        };
        validate_reply(&request, reply).expect("offered memory should validate");
    }

    #[test]
    fn tts_ready_and_error_replies_round_trip_with_flattened_status() {
        let replies = [
            TtsReply {
                protocol_version: TTS_PROTOCOL_VERSION,
                request_id: 4,
                outcome: TtsOutcome::Ready {
                    cache_key: "a".repeat(64),
                },
            },
            TtsReply {
                protocol_version: TTS_PROTOCOL_VERSION,
                request_id: 5,
                outcome: TtsOutcome::Error {
                    code: TtsErrorCode::SynthesisFailed,
                },
            },
        ];
        for reply in replies {
            let json = serde_json::to_string(&reply).expect("reply should encode");
            let decoded = serde_json::from_str::<TtsReply>(&json).expect("reply should decode");
            assert_eq!(decoded, reply);
        }
        let extra = format!(
            r#"{{"protocol_version":1,"request_id":4,"status":"ready","cache_key":"{}","evil":1}}"#,
            "a".repeat(64)
        );
        assert!(serde_json::from_str::<TtsReply>(&extra).is_err());
    }

    #[test]
    fn tts_reply_rejects_worker_paths_and_mismatched_ids() {
        let request = TtsRequest {
            protocol_version: TTS_PROTOCOL_VERSION,
            request_id: 8,
            text: "hello".to_owned(),
            settings: TtsVoiceSettings::default(),
        };
        let path_reply = TtsReply {
            protocol_version: TTS_PROTOCOL_VERSION,
            request_id: 8,
            outcome: TtsOutcome::Ready {
                cache_key: "../../outside.wav".to_owned(),
            },
        };
        assert_eq!(
            validate_tts_reply(&request, path_reply),
            Err(TtsValidationError::CacheKey)
        );
        let wrong_id = TtsReply {
            protocol_version: TTS_PROTOCOL_VERSION,
            request_id: 9,
            outcome: TtsOutcome::Error {
                code: TtsErrorCode::SynthesisFailed,
            },
        };
        assert_eq!(
            validate_tts_reply(&request, wrong_id),
            Err(TtsValidationError::RequestId)
        );
    }
}
