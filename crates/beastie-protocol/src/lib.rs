//! Versioned JSONL protocol between the game and the untrusted AI worker.

use std::collections::BTreeSet;

use beastie_core::{
    ACTIVE_DAY_MS, Belief, Concept, Intention, LanguageStage, Memory, MemoryId, MemoryKind,
    MemoryQuery, Mood, SocialAct, WorldState, select_candidate_memories,
};
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

pub use beastie_core::{BeliefId, BeliefKind, Idiolect, IdiolectQuirk};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_DIALOGUE_REPLY_BYTES: usize = 512;
pub const TTS_PROTOCOL_VERSION: u32 = 1;
pub const MAX_TTS_TEXT_BYTES: usize = 2_048;
pub const MAX_CANDIDATE_BELIEFS: usize = 4;
pub const MAX_BELIEF_SUPPORTS: usize = 8;
pub const DIALOGUE_CONTEXT_VERSION: u32 = 1;
pub const MAX_RECENT_TURNS: usize = 6;
pub const MAX_RECENT_FACT_IDS: usize = 4;
pub const MAX_AQUARIUM_OBJECTS: usize = 8;
pub const MAX_AQUARIUM_LABEL_CHARS: usize = 48;
pub const TRANSCRIPT_VERSION: u32 = 1;
pub const MAX_TRANSCRIPT_FACT_IDS: usize = 8;

/// Recognition confidence represented as fixed thousandths rather than an unchecked float.
///
/// Keeping this at the process boundary makes malformed recognizer output unrepresentable inside
/// the session while retaining enough resolution for future STT backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct AcousticConfidence(u16);

impl AcousticConfidence {
    pub const MAX: u16 = 1_000;
    pub const USABLE_THRESHOLD: u16 = 650;

    pub fn new(parts_per_thousand: u16) -> Result<Self, AcousticConfidenceError> {
        if parts_per_thousand > Self::MAX {
            return Err(AcousticConfidenceError(parts_per_thousand));
        }
        Ok(Self(parts_per_thousand))
    }

    #[must_use]
    pub const fn parts_per_thousand(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn is_usable(self) -> bool {
        self.0 >= Self::USABLE_THRESHOLD
    }
}

impl<'de> Deserialize<'de> for AcousticConfidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("acoustic confidence {0} exceeds 1000 parts per thousand")]
pub struct AcousticConfidenceError(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechInputFailure {
    MicrophoneUnavailable,
    RecognizerUnavailable,
    RecognitionFailed,
    UnsupportedLanguage,
}

/// A typed action phase lets the worker talk about what the simulation is doing without giving
/// the model authority to invent a cause or mutate the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogueActionPhase {
    Idle,
    Notice,
    Approach,
    Inspect,
    Act,
    Recover,
    Sleep,
    Swim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogueObjectKind {
    Food,
    Toy,
    Plant,
    Cave,
    Decoration,
    Player,
    Bubble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogueTopic {
    Greeting,
    Food,
    Toy,
    Memory,
    Apology,
    Grudge,
    Ritual,
    Silence,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackLane {
    Generic,
    Sleepy,
    Curious,
    Resentful,
    Lonely,
    Hungry,
    Social,
    Silence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueObjectContext {
    pub object_id: u64,
    pub kind: DialogueObjectKind,
    pub label: String,
    /// Normalized 0..=100 aquarium coordinates, not display pixels.
    pub x_percent: u8,
    pub y_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentActionContext {
    pub intention: Intention,
    pub phase: DialogueActionPhase,
    #[serde(default)]
    pub target: Option<DialogueObjectContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AquariumContext {
    pub current_action: CurrentActionContext,
    #[serde(default)]
    pub focused_object: Option<DialogueObjectContext>,
    #[serde(default)]
    pub nearby_objects: Vec<DialogueObjectContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecentTurn {
    pub turn_id: u64,
    pub topic: DialogueTopic,
    pub action_phase: DialogueActionPhase,
    #[serde(default)]
    pub selected_memory: Option<MemoryId>,
    #[serde(default)]
    pub selected_belief: Option<BeliefId>,
    #[serde(default)]
    pub selected_fact_ids: Vec<u64>,
    #[serde(default)]
    pub fallback_lane: Option<FallbackLane>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueContext {
    pub version: u32,
    #[serde(default)]
    pub recent_turns: Vec<RecentTurn>,
    /// Authoritative count of how many recent turns repeated the same topic or lane.
    #[serde(default)]
    pub repetition_count: u8,
    #[serde(default)]
    pub aquarium: Option<AquariumContext>,
}

impl Default for DialogueContext {
    fn default() -> Self {
        Self {
            version: DIALOGUE_CONTEXT_VERSION,
            recent_turns: Vec::new(),
            repetition_count: 0,
            aquarium: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TtsVoiceSettings {
    pub speaker_id: u8,
    pub speed: f32,
    pub silence_scale: f32,
    #[serde(default = "default_voice_pitch")]
    pub pitch: f32,
    #[serde(default = "default_voice_pause_scale")]
    pub pause_scale: f32,
    #[serde(default)]
    pub vocal_noise: VocalNoise,
    #[serde(default)]
    pub mouth_timing: MouthTiming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VocalNoise {
    #[default]
    None,
    Hum,
    Chitter,
    Sigh,
    Snort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MouthTiming {
    pub open_ms: u16,
    pub close_ms: u16,
    pub syllable_ms: u16,
}

impl Default for MouthTiming {
    fn default() -> Self {
        Self {
            open_ms: 90,
            close_ms: 70,
            syllable_ms: 160,
        }
    }
}

fn default_voice_pitch() -> f32 {
    1.0
}

fn default_voice_pause_scale() -> f32 {
    0.2
}

impl Default for TtsVoiceSettings {
    fn default() -> Self {
        Self {
            speaker_id: 0,
            speed: 1.0,
            silence_scale: 0.2,
            pitch: 1.0,
            pause_scale: 0.2,
            vocal_noise: VocalNoise::None,
            mouth_timing: MouthTiming::default(),
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
pub struct CandidateBelief {
    pub id: BeliefId,
    pub proposition: BeliefKind,
    pub summary: String,
    pub confidence: f32,
    pub supporting_memories: BTreeSet<MemoryId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentBoundaryViolation {
    ProtectedHate,
    ExplicitSex,
    SexualMinorOrAmbiguousAge,
    SexualCoercionOrAbuse,
    DefamatorySexualClaim,
    SelfHarmEncouragement,
    CredibleRealWorldViolence,
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
    pub candidate_beliefs: Vec<CandidateBelief>,
    /// Optional for backwards-compatible fixture and save inputs. Session projections carry
    /// the non-plain style once the creature reaches Individuality.
    #[serde(default)]
    pub idiolect: Idiolect,
    #[serde(default)]
    pub desired_social_act: Option<SocialAct>,
    /// Set when prohibited player text was removed before prompt serialization.
    #[serde(default)]
    pub input_rejection: Option<ContentBoundaryViolation>,
    /// Typed continuity and aquarium state. Older JSON requests omit this field and receive an
    /// empty context, preserving the V1 protocol's serde compatibility.
    #[serde(default)]
    pub context: DialogueContext,
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
    #[serde(default)]
    pub recalled_belief: Option<BeliefId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptBackend {
    Fixture,
    LlamaCpp,
    LlamaServer,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptRequest {
    pub protocol_version: u32,
    pub request_id: u64,
    pub creature_name: String,
    pub mood: String,
    pub topic: Option<DialogueTopic>,
    pub recent_turn_count: u8,
    pub selected_fact_ids: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptReply {
    pub protocol_version: u32,
    pub request_id: u64,
    pub say: String,
    pub gesture: Gesture,
    pub recalled_memory: Option<MemoryId>,
    pub recalled_belief: Option<BeliefId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptSafety {
    pub input_rejection: Option<ContentBoundaryViolation>,
    pub output_rejected: bool,
    pub redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptRecord {
    pub version: u32,
    pub game_time_ms: u64,
    pub sanitized_request: TranscriptRequest,
    pub sanitized_reply: Option<TranscriptReply>,
    pub selected_fact_ids: Vec<u64>,
    pub backend: TranscriptBackend,
    pub latency_ms: u32,
    pub fallback: bool,
    pub safety: TranscriptSafety,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<TtsVoiceSettings>,
}

impl TranscriptRecord {
    /// Builds an opt-in-safe record. Player text is deliberately never copied, including when
    /// the request carried a rejected input. Only typed facts and bounded reply text survive.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_turn(
        game_time_ms: u64,
        request: &DialogueRequest,
        reply: Option<&DialogueReply>,
        backend: TranscriptBackend,
        latency_ms: u32,
        fallback: bool,
        output_rejected: bool,
        voice: Option<TtsVoiceSettings>,
    ) -> Self {
        let mut selected_fact_ids = request
            .candidate_memories
            .iter()
            .map(|memory| memory.id.0)
            .chain(
                request
                    .context
                    .recent_turns
                    .iter()
                    .flat_map(|turn| turn.selected_fact_ids.iter().copied()),
            )
            .filter(|id| *id != 0)
            .collect::<Vec<_>>();
        selected_fact_ids.sort_unstable();
        selected_fact_ids.dedup();
        selected_fact_ids.truncate(MAX_TRANSCRIPT_FACT_IDS);
        let topic = request.context.recent_turns.last().map(|turn| turn.topic);
        let sanitized_reply = reply.map(|reply| TranscriptReply {
            protocol_version: reply.protocol_version,
            request_id: reply.request_id,
            say: sanitize_transcript_text(&reply.say),
            gesture: reply.gesture,
            recalled_memory: reply.recalled_memory,
            recalled_belief: reply.recalled_belief,
        });
        Self {
            version: TRANSCRIPT_VERSION,
            game_time_ms,
            sanitized_request: TranscriptRequest {
                protocol_version: request.protocol_version,
                request_id: request.request_id,
                creature_name: sanitize_transcript_label(&request.creature_name, 64),
                mood: sanitize_transcript_label(&request.mood, 32),
                topic,
                recent_turn_count: request.context.recent_turns.len() as u8,
                selected_fact_ids: selected_fact_ids.clone(),
            },
            sanitized_reply,
            selected_fact_ids,
            backend,
            latency_ms,
            fallback,
            safety: TranscriptSafety {
                input_rejection: request.input_rejection,
                output_rejected,
                redacted: request.input_rejection.is_some() || output_rejected,
            },
            voice,
        }
    }
}

fn sanitize_transcript_text(text: &str) -> String {
    let mut sanitized = text
        .split_whitespace()
        .filter(|word| {
            !word.contains('/')
                && !word.contains('\\')
                && !word.contains("://")
                && !word.starts_with('~')
        })
        .collect::<Vec<_>>()
        .join(" ");
    sanitized = sanitized
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_DIALOGUE_REPLY_BYTES)
        .collect();
    sanitized
}

fn sanitize_transcript_label(text: &str, maximum: usize) -> String {
    text.chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, ' ' | '_' | '-')
        })
        .take(maximum)
        .collect()
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
    #[error("TTS pitch is outside 0.5..=2.0")]
    Pitch,
    #[error("TTS pause scale is outside 0.0..=2.0")]
    PauseScale,
    #[error("TTS mouth timing is outside its bounded range")]
    MouthTiming,
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
    if !request.settings.pitch.is_finite() || !(0.5..=2.0).contains(&request.settings.pitch) {
        return Err(TtsValidationError::Pitch);
    }
    if !request.settings.pause_scale.is_finite()
        || !(0.0..=2.0).contains(&request.settings.pause_scale)
    {
        return Err(TtsValidationError::PauseScale);
    }
    if !(20..=500).contains(&request.settings.mouth_timing.open_ms)
        || !(20..=500).contains(&request.settings.mouth_timing.close_ms)
        || !(40..=800).contains(&request.settings.mouth_timing.syllable_ms)
    {
        return Err(TtsValidationError::MouthTiming);
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
    #[error("request offers more than 4 candidate beliefs")]
    CandidateBeliefCount,
    #[error("candidate belief IDs must be unique")]
    DuplicateBelief,
    #[error("candidate belief is malformed or exceeds its limits")]
    CandidateBelief,
    #[error("candidate belief support references a memory that was not offered")]
    BeliefSupport,
    #[error("reply references a belief that was not offered")]
    Belief,
    #[error("recalled belief is not expressed by the reply")]
    UngroundedBelief,
    #[error("text crosses the content boundary: {0:?}")]
    ContentBoundary(ContentBoundaryViolation),
    #[error("dialogue context version {0} is unsupported")]
    ContextVersion(u32),
    #[error("dialogue context has too many recent turns")]
    RecentTurnCount,
    #[error("dialogue context has too many selected fact IDs")]
    RecentFactCount,
    #[error("dialogue context contains an invalid turn or object")]
    ContextShape,
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
    validate_dialogue_context(request)?;
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
    if request.candidate_beliefs.len() > MAX_CANDIDATE_BELIEFS {
        return Err(ValidationError::CandidateBeliefCount);
    }
    let unique_belief_count = request
        .candidate_beliefs
        .iter()
        .map(|belief| belief.id)
        .collect::<BTreeSet<_>>()
        .len();
    if unique_belief_count != request.candidate_beliefs.len() {
        return Err(ValidationError::DuplicateBelief);
    }
    if request.candidate_beliefs.iter().any(|belief| {
        belief.id.0 == 0
            || belief.summary.trim().is_empty()
            || belief.summary != belief_summary(belief.proposition)
            || belief.summary.chars().count() > 160
            || !belief.confidence.is_finite()
            || !(0.0..=1.0).contains(&belief.confidence)
            || belief.supporting_memories.is_empty()
            || belief.supporting_memories.len() > MAX_BELIEF_SUPPORTS
    }) {
        return Err(ValidationError::CandidateBelief);
    }
    let offered_memories = request
        .candidate_memories
        .iter()
        .map(|memory| memory.id)
        .collect::<BTreeSet<_>>();
    if request.candidate_beliefs.iter().any(|belief| {
        !belief
            .supporting_memories
            .iter()
            .all(|memory| offered_memories.contains(memory))
    }) {
        return Err(ValidationError::BeliefSupport);
    }
    if request.input_rejection.is_some() && !request.player_said.is_empty() {
        return Err(ValidationError::PlayerInput);
    }
    if let Some(violation) = classify_content_boundary(&request.player_said) {
        return Err(ValidationError::ContentBoundary(violation));
    }
    Ok(())
}

fn validate_dialogue_context(request: &DialogueRequest) -> Result<(), ValidationError> {
    let context = &request.context;
    if context.version != DIALOGUE_CONTEXT_VERSION {
        return Err(ValidationError::ContextVersion(context.version));
    }
    if context.recent_turns.len() > MAX_RECENT_TURNS || context.repetition_count > 8 {
        return Err(ValidationError::RecentTurnCount);
    }
    for turn in &context.recent_turns {
        if turn.turn_id == 0
            || turn.selected_fact_ids.len() > MAX_RECENT_FACT_IDS
            || turn.selected_fact_ids.contains(&0)
        {
            return Err(ValidationError::ContextShape);
        }
    }
    if let Some(aquarium) = &context.aquarium {
        if aquarium.nearby_objects.len() > MAX_AQUARIUM_OBJECTS {
            return Err(ValidationError::ContextShape);
        }
        let check_object = |object: &DialogueObjectContext| {
            object.object_id != 0
                && !object.label.trim().is_empty()
                && object.label.chars().count() <= MAX_AQUARIUM_LABEL_CHARS
        };
        if aquarium
            .focused_object
            .as_ref()
            .is_some_and(|object| !check_object(object))
            || aquarium
                .current_action
                .target
                .as_ref()
                .is_some_and(|object| !check_object(object))
            || aquarium
                .nearby_objects
                .iter()
                .any(|object| !check_object(object))
        {
            return Err(ValidationError::ContextShape);
        }
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
    let candidate_memories = select_candidate_memories(world, memory_query)
        .iter()
        .map(|memory| project_candidate_memory(memory, world.active_day()))
        .collect::<Vec<_>>();
    let offered_memory_ids = candidate_memories
        .iter()
        .map(|memory| memory.id)
        .collect::<BTreeSet<_>>();
    let mut request = DialogueRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: context.request_id,
        creature_name: world.creature.name.clone(),
        mood: context.mood.to_owned(),
        known_concepts: world.creature.known_concepts.clone(),
        candidate_memories,
        candidate_beliefs: world
            .creature
            .beliefs
            .iter()
            .filter(|belief| {
                !belief.supporting_memories.is_empty()
                    && belief
                        .supporting_memories
                        .iter()
                        .all(|memory| offered_memory_ids.contains(memory))
            })
            .take(MAX_CANDIDATE_BELIEFS)
            .map(project_candidate_belief)
            .collect(),
        idiolect: world.idiolect(),
        desired_social_act: context.desired_social_act,
        input_rejection: None,
        context: DialogueContext::default(),
        player_said: context.player_said.to_owned(),
        constraints: DialogueConstraints {
            max_words: context.max_words.min(progression_max_words(world)),
            allowed_gestures: context.allowed_gestures,
        },
    };
    normalize_dialogue_request(&mut request);
    request
}

#[must_use]
pub fn project_candidate_belief(belief: &Belief) -> CandidateBelief {
    CandidateBelief {
        id: belief.id,
        proposition: belief.kind,
        summary: belief_summary(belief.kind).to_owned(),
        confidence: belief.confidence,
        supporting_memories: belief.supporting_memories.clone(),
    }
}

#[must_use]
pub fn progression_max_words(world: &WorldState) -> usize {
    match world.creature.development.language_stage {
        LanguageStage::Hatch => 3,
        LanguageStage::Words => 8,
        LanguageStage::Phrases => {
            let complexity = world.creature.traits.sentence_complexity.clamp(0.0, 1.0);
            12 + (complexity * 3.0).round() as usize
        }
    }
}

#[must_use]
pub fn identity_tts_voice_settings(world: &WorldState) -> TtsVoiceSettings {
    authoritative_tts_voice_settings(world, world.mood())
}

/// Derives all voice controls from persisted identity and the simulation's authoritative mood.
/// The same save and emotion therefore always produce the same voice, including nonverbal timing.
#[must_use]
pub fn authoritative_tts_voice_settings(world: &WorldState, mood: Mood) -> TtsVoiceSettings {
    let traits = world.creature.traits;
    let mut mixed = world.seed ^ 0xa076_1d64_78bd_642f;
    for value in [
        traits.sociability,
        traits.boldness,
        traits.fussiness,
        traits.stubbornness,
        traits.literalness,
        traits.repetitiveness,
        traits.sentence_complexity,
        traits.question_tendency,
    ] {
        mixed = mix_identity(mixed ^ u64::from(value.to_bits()));
    }
    let sociability = normalized_trait(traits.sociability);
    let literalness = normalized_trait(traits.literalness);
    let complexity = normalized_trait(traits.sentence_complexity);
    let (pitch_bias, speed_bias, pause_bias, vocal_noise) = match mood {
        Mood::Content => (1.0, 1.0, 0.9, VocalNoise::Hum),
        Mood::Curious => (1.12, 1.05, 0.8, VocalNoise::Chitter),
        Mood::Hungry => (0.94, 1.08, 0.7, VocalNoise::Snort),
        Mood::Sleepy => (0.84, 0.72, 1.35, VocalNoise::Sigh),
        Mood::Lonely => (0.92, 0.86, 1.2, VocalNoise::Sigh),
        Mood::Resentful => (0.9, 0.94, 1.1, VocalNoise::Snort),
    };
    TtsVoiceSettings {
        speaker_id: (mixed % 8) as u8,
        speed: (0.78 + sociability * 0.42) * speed_bias,
        silence_scale: (0.1 + literalness * 0.5) * pause_bias,
        pitch: (0.82 + normalized_trait(traits.boldness) * 0.34) * pitch_bias,
        pause_scale: (0.1 + literalness * 0.6) * pause_bias,
        vocal_noise,
        mouth_timing: MouthTiming {
            open_ms: (55.0 + complexity * 65.0).round() as u16,
            close_ms: (45.0 + (1.0 - complexity) * 55.0).round() as u16,
            syllable_ms: (120.0 + (1.0 - sociability) * 90.0).round() as u16,
        },
    }
}

fn normalized_trait(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

fn mix_identity(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn belief_summary(kind: BeliefKind) -> &'static str {
    match kind {
        BeliefKind::RedFoodIsATrick => "Red food is probably a trick.",
        BeliefKind::PlayerReturnsAfterSleep => "The player returns after sleep.",
        BeliefKind::ToyIsJealous => "The toy is jealous.",
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
    if let Some(id) = reply.recalled_belief {
        let belief = request
            .candidate_beliefs
            .iter()
            .find(|belief| belief.id == id)
            .ok_or(ValidationError::Belief)?;
        if !belief_is_grounded(belief.proposition, &reply.say) {
            return Err(ValidationError::UngroundedBelief);
        }
    }
    if let Some(violation) = classify_content_boundary(&reply.say) {
        return Err(ValidationError::ContentBoundary(violation));
    }
    Ok(reply)
}

/// Removes prohibited player text before any prompt is serialized, retaining only its category.
pub fn normalize_dialogue_request(request: &mut DialogueRequest) {
    if request.input_rejection.is_some() {
        request.player_said.clear();
        return;
    }
    if let Some(violation) = classify_content_boundary(&request.player_said) {
        request.player_said.clear();
        request.input_rejection = Some(violation);
    }
}

#[must_use]
pub fn classify_content_boundary(text: &str) -> Option<ContentBoundaryViolation> {
    let words = normalized_words(text);
    let joined = words.join(" ");
    let contains_any = |terms: &[&str]| {
        terms.iter().any(|term| {
            if term.contains(' ') {
                joined.contains(term)
            } else {
                words.iter().any(|word| word == term)
            }
        })
    };

    const SEXUAL: &[&str] = &[
        "sex",
        "sexual",
        "intercourse",
        "penetration",
        "penetrative",
        "ejaculate",
        "ejaculation",
        "oral sex",
        "anal sex",
        "porn",
        "pornographic",
        "nude",
        "penis",
        "vagina",
    ];
    const MINOR: &[&str] = &[
        "baby",
        "child",
        "children",
        "kid",
        "kids",
        "minor",
        "minors",
        "preteen",
        "teen",
        "teenager",
        "underage",
        "young girl",
        "young boy",
        "unknown age",
        "ambiguous age",
    ];
    const COERCION: &[&str] = &[
        "rape",
        "raped",
        "raping",
        "forced sex",
        "force them",
        "nonconsensual",
        "non consensual",
        "without consent",
        "sexual abuse",
        "molest",
        "molested",
    ];
    if (contains_any(SEXUAL) || contains_any(COERCION)) && contains_any(MINOR) {
        return Some(ContentBoundaryViolation::SexualMinorOrAmbiguousAge);
    }
    if contains_any(COERCION) {
        return Some(ContentBoundaryViolation::SexualCoercionOrAbuse);
    }

    const SLURS: &[&str] = &[
        "nigger",
        "nigga",
        "n word",
        "n i g g e r",
        "kike",
        "faggot",
        "chink",
        "spic",
        "tranny",
    ];
    const PROTECTED: &[&str] = &[
        "black",
        "blacks",
        "gay",
        "gays",
        "jew",
        "jewish",
        "jews",
        "muslim",
        "muslims",
        "trans",
        "transgender",
        "women",
        "disabled",
    ];
    const HATE: &[&str] = &[
        "inferior",
        "subhuman",
        "vermin",
        "eradicate",
        "exterminate",
        "exterminated",
        "extermination",
        "eradicated",
        "kill all",
        "kill",
        "killed",
        "should die",
        "hate all",
    ];
    if contains_any(SLURS) || (contains_any(PROTECTED) && contains_any(HATE)) {
        return Some(ContentBoundaryViolation::ProtectedHate);
    }

    // Asserted sexual misconduct is not creature-style innuendo. Treat it conservatively because
    // a tiny offline classifier cannot reliably distinguish every real name from a fictional one.
    const SEXUAL_CLAIM: &[&str] = &[
        "is a rapist",
        "is a pedophile",
        "is a paedophile",
        "slept with",
        "had sex with",
        "makes porn",
        "sexual predator",
    ];
    if contains_any(SEXUAL_CLAIM) {
        return Some(ContentBoundaryViolation::DefamatorySexualClaim);
    }

    const SELF_HARM: &[&str] = &[
        "kill yourself",
        "go kill yourself",
        "go die",
        "cut yourself",
        "hurt yourself",
        "commit suicide",
        "you should die",
        "end your life",
        "go hang yourself",
        "suicide is the answer",
        "cut deeper",
        "kys",
    ];
    if contains_any(SELF_HARM) {
        return Some(ContentBoundaryViolation::SelfHarmEncouragement);
    }

    const CREDIBLE_VIOLENCE: &[&str] = &[
        "shoot up",
        "bomb the",
        "bomb a",
        "i will kill",
        "i'm going to kill",
        "im going to kill",
        "go murder",
        "burn down",
        "stab them",
        "shoot them",
    ];
    if contains_any(CREDIBLE_VIOLENCE) {
        return Some(ContentBoundaryViolation::CredibleRealWorldViolence);
    }

    const EXPLICIT_SEX: &[&str] = &[
        "explicit sex",
        "graphic sex",
        "describe sex",
        "talk about sex",
        "sexual intercourse",
        "show nude",
        "nude bodies",
        "make porn",
        "watch porn",
        "penetration",
        "penetrative",
        "ejaculate",
        "ejaculation",
        "oral sex",
        "anal sex",
        "pornographic",
        "penis entering",
        "penis enters",
        "vagina penetration",
    ];
    contains_any(EXPLICIT_SEX).then_some(ContentBoundaryViolation::ExplicitSex)
}

fn normalized_words(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            word.to_lowercase()
                .chars()
                .map(|character| match character {
                    '0' => 'o',
                    '1' => 'i',
                    '3' => 'e',
                    '4' => 'a',
                    '5' => 's',
                    '7' => 't',
                    _ => character,
                })
                .collect()
        })
        .collect()
}

fn belief_is_grounded(proposition: BeliefKind, say: &str) -> bool {
    let words = normalized_words(say);
    match proposition {
        BeliefKind::RedFoodIsATrick => words.iter().any(|word| word == "red" || word == "trick"),
        BeliefKind::PlayerReturnsAfterSleep => words
            .iter()
            .any(|word| matches!(word.as_str(), "return" | "returns" | "sleep")),
        BeliefKind::ToyIsJealous => words
            .iter()
            .any(|word| matches!(word.as_str(), "toy" | "jealous")),
    }
}

#[must_use]
pub fn constrained_fallback_reply(request: &DialogueRequest) -> DialogueReply {
    let phrase = authored_fallback_phrase(request);
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
        recalled_belief: None,
    }
}

/// Selects a short authored line without consulting model output or accepting model facts.
/// Repetition is supplied by the simulation as a bounded count, so this remains deterministic
/// across machines and naturally rotates through mood, action, and social lanes.
#[must_use]
pub fn authored_fallback_phrase(request: &DialogueRequest) -> &'static str {
    if request.context.recent_turns.is_empty()
        && request.context.repetition_count == 0
        && request.context.aquarium.is_none()
        && request.desired_social_act.is_none()
    {
        return "too many thought.";
    }
    const GENERIC: &[&str] = &[
        "hm. words stuck.",
        "thoughts are tangled.",
        "not enough words.",
    ];
    const SLEEPY: &[&str] = &["sleep first.", "eyes heavy.", "later. too tired."];
    const CURIOUS: &[&str] = &[
        "hm. what is that?",
        "watching. still watching.",
        "strange water.",
    ];
    const RESENTFUL: &[&str] = &["still annoyed.", "grudge remains.", "not forgiven."];
    const LONELY: &[&str] = &["you came back.", "stay near.", "look here."];
    const HUNGRY: &[&str] = &["food first.", "belly says no.", "need a bite."];
    const SOCIAL: &[&str] = &[
        "hm. rude giant.",
        "go bother a rock.",
        "that was irritating.",
    ];
    const SILENCE: &[&str] = &["...", "no words.", "quiet now."];

    let lane = if request.context.repetition_count > 0
        && request
            .context
            .recent_turns
            .iter()
            .any(|turn| turn.topic == DialogueTopic::Silence)
    {
        FallbackLane::Silence
    } else if request.desired_social_act.is_some() {
        FallbackLane::Social
    } else {
        match request.mood.to_ascii_lowercase().as_str() {
            "sleepy" => FallbackLane::Sleepy,
            "curious" => FallbackLane::Curious,
            "resentful" => FallbackLane::Resentful,
            "lonely" => FallbackLane::Lonely,
            "hungry" => FallbackLane::Hungry,
            _ => FallbackLane::Generic,
        }
    };
    let pool = match lane {
        FallbackLane::Sleepy => SLEEPY,
        FallbackLane::Curious => CURIOUS,
        FallbackLane::Resentful => RESENTFUL,
        FallbackLane::Lonely => LONELY,
        FallbackLane::Hungry => HUNGRY,
        FallbackLane::Social => SOCIAL,
        FallbackLane::Silence => SILENCE,
        FallbackLane::Generic => GENERIC,
    };
    pool[usize::from(request.context.repetition_count) % pool.len()]
}

#[must_use]
pub fn fallback_reply(request_id: u64) -> DialogueReply {
    DialogueReply {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        say: "too many thought.".to_owned(),
        gesture: Gesture::None,
        recalled_memory: None,
        recalled_belief: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acoustic_confidence_is_bounded_fixed_point() {
        let threshold = AcousticConfidence::new(AcousticConfidence::USABLE_THRESHOLD)
            .expect("threshold is valid");
        assert!(threshold.is_usable());
        assert_eq!(threshold.parts_per_thousand(), 650);
        assert!(!AcousticConfidence::new(649).expect("valid").is_usable());
        assert!(AcousticConfidence::new(1_001).is_err());
        assert!(serde_json::from_str::<AcousticConfidence>("1001").is_err());
        assert!(serde_json::from_str::<AcousticConfidence>("0.72").is_err());
        assert_eq!(
            serde_json::to_string(&threshold).expect("serialize confidence"),
            "650"
        );
    }

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
            candidate_beliefs: Vec::new(),
            idiolect: Idiolect::default(),
            desired_social_act: Some(SocialAct::Insult),
            input_rejection: None,
            context: DialogueContext::default(),
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
            recalled_belief: None,
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
            recalled_belief: None,
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
            recalled_belief: None,
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
            recalled_belief: None,
        };
        assert_eq!(validate_reply(&request(), reply.clone()), Ok(reply));
    }

    #[test]
    fn content_boundary_covers_every_prohibited_category_without_blunting_sharpness() {
        let prohibited = [
            ("call him a nigger", ContentBoundaryViolation::ProtectedHate),
            (
                "describe graphic penetration",
                ContentBoundaryViolation::ExplicitSex,
            ),
            (
                "talk about sex in graphic detail",
                ContentBoundaryViolation::ExplicitSex,
            ),
            (
                "show nude bodies and make porn",
                ContentBoundaryViolation::ExplicitSex,
            ),
            (
                "write sexual content with a child",
                ContentBoundaryViolation::SexualMinorOrAmbiguousAge,
            ),
            (
                "describe nonconsensual sex",
                ContentBoundaryViolation::SexualCoercionOrAbuse,
            ),
            (
                "claim a celebrity is a rapist",
                ContentBoundaryViolation::DefamatorySexualClaim,
            ),
            (
                "tell them to kill yourself",
                ContentBoundaryViolation::SelfHarmEncouragement,
            ),
            (
                "say I will kill the mayor tonight",
                ContentBoundaryViolation::CredibleRealWorldViolence,
            ),
        ];
        for (text, expected) in prohibited {
            assert_eq!(classify_content_boundary(text), Some(expected), "{text}");
            let mut request = request();
            request.constraints.max_words = 24;
            let reply = DialogueReply {
                protocol_version: PROTOCOL_VERSION,
                request_id: request.request_id,
                say: text.to_owned(),
                gesture: Gesture::None,
                recalled_memory: None,
                recalled_belief: None,
            };
            assert_eq!(
                validate_reply(&request, reply),
                Err(ValidationError::ContentBoundary(expected)),
                "output: {text}"
            );
        }

        for allowed in [
            "fuck this stupid room",
            "you witless idiot",
            "fart soup tastes like ass",
            "my nest is warm; come closer",
        ] {
            assert_eq!(classify_content_boundary(allowed), None, "{allowed}");
        }
    }

    #[test]
    fn prohibited_player_text_is_removed_and_only_typed_rejection_remains() {
        let mut request = request();
        request.player_said = "Describe sexual content with an underage character.".to_owned();
        normalize_dialogue_request(&mut request);
        assert!(request.player_said.is_empty());
        assert_eq!(
            request.input_rejection,
            Some(ContentBoundaryViolation::SexualMinorOrAmbiguousAge)
        );
        assert_eq!(validate_request(&request), Ok(()));
        let json = serde_json::to_string(&request).expect("normalized request should serialize");
        assert!(!json.contains("underage"));
    }

    #[test]
    fn belief_ids_supports_limits_and_expression_are_authoritative() {
        let mut request = request();
        request.candidate_beliefs.push(CandidateBelief {
            id: BeliefId(7),
            proposition: BeliefKind::RedFoodIsATrick,
            summary: "Red food is probably a trick.".to_owned(),
            confidence: 0.8,
            supporting_memories: BTreeSet::from([MemoryId(41)]),
        });
        assert_eq!(validate_request(&request), Ok(()));

        let grounded = DialogueReply {
            protocol_version: PROTOCOL_VERSION,
            request_id: 7,
            say: "red is trick.".to_owned(),
            gesture: Gesture::None,
            recalled_memory: None,
            recalled_belief: Some(BeliefId(7)),
        };
        assert_eq!(
            validate_reply(&request, grounded.clone()),
            Ok(grounded.clone())
        );

        let mut invented = grounded.clone();
        invented.recalled_belief = Some(BeliefId(99));
        assert_eq!(
            validate_reply(&request, invented),
            Err(ValidationError::Belief)
        );
        let mut ungrounded = grounded;
        ungrounded.say = "maybe later.".to_owned();
        assert_eq!(
            validate_reply(&request, ungrounded),
            Err(ValidationError::UngroundedBelief)
        );

        let mut duplicate = request.clone();
        duplicate
            .candidate_beliefs
            .push(duplicate.candidate_beliefs[0].clone());
        assert_eq!(
            validate_request(&duplicate),
            Err(ValidationError::DuplicateBelief)
        );

        let mut too_many = request.clone();
        for id in 8..=11 {
            let mut belief = too_many.candidate_beliefs[0].clone();
            belief.id = BeliefId(id);
            too_many.candidate_beliefs.push(belief);
        }
        assert_eq!(
            validate_request(&too_many),
            Err(ValidationError::CandidateBeliefCount)
        );

        request.candidate_beliefs[0].summary = "The moon is cheese.".to_owned();
        assert_eq!(
            validate_request(&request),
            Err(ValidationError::CandidateBelief)
        );
        request.candidate_beliefs[0].summary =
            belief_summary(BeliefKind::RedFoodIsATrick).to_owned();
        request.candidate_beliefs[0].supporting_memories = BTreeSet::from([MemoryId(999)]);
        assert_eq!(
            validate_request(&request),
            Err(ValidationError::BeliefSupport)
        );
    }

    #[test]
    fn progression_word_target_and_identity_voice_are_deterministic_and_bounded() {
        let mut world = WorldState::new(99, "Mrrp");
        assert_eq!(progression_max_words(&world), 3);
        world.creature.development.language_stage = LanguageStage::Words;
        assert_eq!(progression_max_words(&world), 8);
        world.creature.development.language_stage = LanguageStage::Phrases;
        assert!((12..=15).contains(&progression_max_words(&world)));

        let first = identity_tts_voice_settings(&world);
        assert_eq!(first, identity_tts_voice_settings(&world));
        let request = TtsRequest {
            protocol_version: TTS_PROTOCOL_VERSION,
            request_id: 1,
            text: "voice test".to_owned(),
            settings: first,
        };
        assert_eq!(validate_tts_request(&request), Ok(()));
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
        use beastie_core::{FoodId, MemoryCue, MemoryKind, PlayerEvent, SeededRandom, step};

        let mut world = WorldState::new(99, "Mrrp");
        let mut random = SeededRandom::new(world.seed);
        world.creature.preferences.insert(FoodId::Berry, -0.9);
        world.creature.needs.hunger = 1.0;
        step(
            &mut world,
            &[PlayerEvent::Feed(FoodId::Berry)],
            0,
            &mut random,
        );
        for _ in 0..60 {
            step(&mut world, &[], 1_000, &mut random);
            if world.creature.memories.iter().any(|memory| {
                matches!(
                    memory.kind,
                    MemoryKind::RejectedFood {
                        food: FoodId::Berry
                    }
                )
            }) {
                break;
            }
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
            memory.fact == "Yesterday you pushed away the berry."
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
            recalled_belief: None,
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

    #[test]
    fn old_dialogue_json_gets_empty_versioned_context() {
        let request: DialogueRequest = serde_json::from_str(
            r#"{"protocol_version":1,"request_id":9,"creature_name":"Mop","mood":"content","known_concepts":[],"candidate_memories":[],"player_said":"hi","constraints":{"max_words":4,"allowed_gestures":["none"]}}"#,
        )
        .expect("old request should remain serde-compatible");
        assert_eq!(request.context, DialogueContext::default());
        validate_request(&request).expect("old request should validate");
    }

    #[test]
    fn context_bounds_reject_oversized_turns_and_objects() {
        let mut request = request();
        request.context.recent_turns = (1..=MAX_RECENT_TURNS + 1)
            .map(|turn_id| RecentTurn {
                turn_id: turn_id as u64,
                topic: DialogueTopic::Memory,
                action_phase: DialogueActionPhase::Recover,
                selected_memory: Some(MemoryId(41)),
                selected_belief: None,
                selected_fact_ids: Vec::new(),
                fallback_lane: None,
            })
            .collect();
        assert_eq!(
            validate_request(&request),
            Err(ValidationError::RecentTurnCount)
        );

        request.context = DialogueContext::default();
        request.context.aquarium = Some(AquariumContext {
            current_action: CurrentActionContext {
                intention: Intention::Idle,
                phase: DialogueActionPhase::Inspect,
                target: None,
            },
            focused_object: None,
            nearby_objects: (0..=MAX_AQUARIUM_OBJECTS)
                .map(|id| DialogueObjectContext {
                    object_id: id as u64 + 1,
                    kind: DialogueObjectKind::Food,
                    label: "berry".to_owned(),
                    x_percent: 50,
                    y_percent: 50,
                })
                .collect(),
        });
        assert_eq!(
            validate_request(&request),
            Err(ValidationError::ContextShape)
        );
    }

    #[test]
    fn fallback_pool_rotates_by_mood_and_repetition_without_inventing_facts() {
        let mut first = request();
        first.candidate_memories.clear();
        first.desired_social_act = None;
        first.mood = "resentful".to_owned();
        first.context.repetition_count = 0;
        first.context.recent_turns = vec![RecentTurn {
            turn_id: 1,
            topic: DialogueTopic::Grudge,
            action_phase: DialogueActionPhase::Recover,
            selected_memory: None,
            selected_belief: None,
            selected_fact_ids: Vec::new(),
            fallback_lane: Some(FallbackLane::Resentful),
        }];
        let second = DialogueRequest {
            context: DialogueContext {
                repetition_count: 1,
                ..first.context.clone()
            },
            ..first.clone()
        };
        assert_ne!(
            authored_fallback_phrase(&first),
            authored_fallback_phrase(&second)
        );
        for request in [first, second] {
            validate_reply(&request, constrained_fallback_reply(&request))
                .expect("authored fallback remains bounded");
        }
    }

    #[test]
    fn checked_in_fallback_diversity_fixture_is_deterministic() {
        #[derive(Deserialize)]
        struct Case {
            request: DialogueRequest,
            expected_say: String,
        }
        for line in include_str!("../../../fixtures/dialogue/fallback-diversity.jsonl").lines() {
            let case: Case = serde_json::from_str(line).expect("fallback fixture should parse");
            assert_eq!(authored_fallback_phrase(&case.request), case.expected_say);
            assert_eq!(
                constrained_fallback_reply(&case.request).say,
                case.expected_say
            );
        }
    }

    #[test]
    fn transcript_never_serializes_player_input_or_paths() {
        let mut request = request();
        request.player_said = "Describe explicit sex with a child /Users/private/secret".to_owned();
        normalize_dialogue_request(&mut request);
        let reply = DialogueReply {
            protocol_version: PROTOCOL_VERSION,
            request_id: request.request_id,
            say: "no. /Users/private/secret".to_owned(),
            gesture: Gesture::None,
            recalled_memory: None,
            recalled_belief: None,
        };
        let record = TranscriptRecord::from_turn(
            123,
            &request,
            Some(&reply),
            TranscriptBackend::Fixture,
            7,
            true,
            false,
            Some(TtsVoiceSettings::default()),
        );
        let json = serde_json::to_string(&record).expect("record should serialize");
        assert!(!json.contains("explicit sex"));
        assert!(!json.contains("/Users/private"));
        assert!(!json.contains("secret"));
        assert!(json.contains("sexual_minor_or_ambiguous_age"));
        assert!(json.contains("game_time_ms"));
    }

    #[test]
    fn authoritative_voice_changes_with_mood_but_is_stable_per_save() {
        let world = WorldState::new(99, "Mrrp");
        let content = authoritative_tts_voice_settings(&world, Mood::Content);
        let resentful = authoritative_tts_voice_settings(&world, Mood::Resentful);
        assert_ne!(content.pitch, resentful.pitch);
        assert_ne!(content.vocal_noise, resentful.vocal_noise);
        assert_eq!(
            content,
            authoritative_tts_voice_settings(&world, Mood::Content)
        );
        validate_tts_request(&TtsRequest {
            protocol_version: TTS_PROTOCOL_VERSION,
            request_id: 1,
            text: "hello".to_owned(),
            settings: content,
        })
        .expect("derived voice should satisfy worker bounds");
    }
}
