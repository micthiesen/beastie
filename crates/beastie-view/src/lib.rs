//! Display-free semantic projection of the authoritative aquarium into a 3D scene.
//!
//! This crate owns UI layout units, presentation timing, semantic hit regions, and
//! presentation-only effects. It never mutates simulation state.

use beastie_core::{
    ActionPhase, ActivityPhase, ActivityRecipe, FoodDisposition, FoodDropRejectionReason, FoodId,
    GameEvent, GazeTarget, Intention, Mood, NonverbalAct, NormalizedPosition, PrivateLifeKind,
    Reaction, RelationshipBeatPhase, RelationshipExpressionKind, RelationshipMotifKey,
    RelationshipPerformanceRecipe, SemanticDestination, SpeechAttention, SteeringMode, ToyId,
    ToyResponse, WorldObject, WorldState, performance_recipe_for,
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

pub const LOGICAL_WIDTH: i32 = 320;
pub const LOGICAL_HEIGHT: i32 = 180;
pub const AQUARIUM_BOTTOM: i32 = 129;
pub const COMPOSE_BAR_TOP: i32 = 130;
pub const CREATURE_HIT_WIDTH: i32 = 52;
pub const CREATURE_HIT_HEIGHT: i32 = 40;
pub const SPEECH_LIFETIME_MS: u64 = 8_000;
pub const SPEECH_RELEASE_MS: u64 = 500;
pub const CUE_QUEUE_LIMIT: usize = 8;

mod ui_art;
pub use ui_art::TextRole;
use ui_art::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    #[must_use]
    pub const fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTarget {
    OpenWater,
    Creature,
    Cave,
    Plant(u64),
    FoodObject(u64),
    Toy(ToyId),
    Reaction(Reaction),
    ComposeField,
    Actions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiAction {
    OpenContext(UiTarget),
    CloseContext,
    OpenFoodChoice,
    OpenToyChoice,
    SelectFood(FoodId),
    /// The game shell combines this with the pointer's normalized aquarium position.
    DropFood(FoodId),
    Play(ToyId),
    Comfort,
    Inspect,
    Talk,
    React(Reaction),
    FocusCompose,
    TypeCharacter(char),
    Backspace,
    SubmitText,
    ClearText,
    CancelMode,
    OpenSettings,
    SelectSettingsPage(u8),
    SetTextScale(u8),
    ToggleReducedMotion,
    ToggleReducedFlashes,
    ToggleReducedShake,
    CycleWindowScale,
    ToggleFullscreen,
    CycleEffectsVolume,
    CycleSpeechVolume,
    ToggleVoice,
    ToggleSubtitles,
    ToggleMicrophone,
    /// Starts push-to-talk on press. The shell ends capture on release or focus loss.
    PushToTalk,
    CycleTextSpeed,
    OpenBindings,
    BeginRebind(BindableAction),
    ResetBindings,
    Rename,
    SubmitName,
    OpenDataManagement,
    RecoverBackup,
    RequestReset,
    ConfirmReset,
    ToggleTranscript,
    ExportTranscript,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindableAction {
    PushToTalk,
    Food,
    Play,
    Comfort,
    Settings,
    Cancel,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MicrophoneState {
    Disabled,
    #[default]
    Idle,
    Listening,
    Recognizing,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UiMode {
    /// Normal play. The compose field remains focused and accepts printable text.
    #[default]
    Compose,
    Context(UiTarget),
    FoodChoice,
    FoodDrop(FoodId),
    ToyChoice,
    OnScreenKeyboard,
    Settings,
    Bindings,
    Rebinding(BindableAction),
    Rename,
    DataManagement,
    ConfirmReset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorKind {
    Default,
    Pointer,
    FoodDrop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationCueKind {
    Notice,
    PositiveNotice,
    FoodSuspicion,
    PlaceNotice,
    Recoil,
    Delight,
    Suspicion,
    Affection,
    Comfort,
    Spit,
    Crumbs,
    SandPuff,
    Wake,
    Sleep,
    AquariumFull,
    BallNudge,
    BellStrike,
    SockTug,
    CaveShelter,
    PlantOrbit,
    BottomForage,
    OpenWaterDrift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SemanticOwner {
    Ordinary,
    StandaloneRelationship(RelationshipMotifKey),
    ActionRelationship(u64),
    PrivateLife(NonZeroU64),
    DirectOutcome,
}

impl SemanticOwner {
    const fn priority(self) -> u8 {
        match self {
            Self::Ordinary => 0,
            Self::StandaloneRelationship(_) => 1,
            Self::ActionRelationship(_) => 2,
            Self::PrivateLife(_) => 2,
            Self::DirectOutcome => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationChannel {
    CreatureExpression,
    CreatureVoice,
    Physical,
    Ui,
    Ambience,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationCue {
    pub owner: SemanticOwner,
    pub channel: PresentationChannel,
    pub kind: PresentationCueKind,
    pub starts_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewState {
    pub mode: UiMode,
    #[serde(default)]
    pub settings_page: u8,
    /// Stable [`HitRegion::id`] selected by keyboard or controller navigation.
    pub focused_region: Option<String>,
    /// Stable [`HitRegion::id`] beneath the pointer.
    pub hovered_region: Option<String>,
    pub text_buffer: String,
    pub pending: bool,
    pub speech: Option<String>,
    /// Full utterance keeps the caption bounds steady during progressive text reveal.
    #[serde(default)]
    pub speech_layout_text: Option<String>,
    pub speech_expires_at_ms: Option<u64>,
    #[serde(default)]
    pub cue_queue: Vec<PresentationCue>,
    #[serde(default = "default_text_scale")]
    pub text_scale: u8,
    #[serde(default)]
    pub reduced_motion: bool,
    #[serde(default)]
    pub reduced_flashes: bool,
    #[serde(default)]
    pub reduced_shake: bool,
    #[serde(default = "default_window_scale")]
    pub window_scale: u8,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default = "default_volume")]
    pub effects_volume: u8,
    #[serde(default = "default_volume")]
    pub speech_volume: u8,
    #[serde(default = "default_true")]
    pub voice_enabled: bool,
    #[serde(default = "default_true")]
    pub subtitles: bool,
    #[serde(default)]
    pub microphone_enabled: bool,
    #[serde(default)]
    pub microphone_state: MicrophoneState,
    /// 0 = instant, 1 = normal, 2 = slow.
    #[serde(default = "default_text_speed")]
    pub text_speed: u8,
    #[serde(default)]
    pub binding_labels: BindingLabels,
    #[serde(default)]
    pub controller_active: bool,
    #[serde(default)]
    pub speaking: bool,
    /// Protocol/game-owned mouth phase: 0 closed, 1 resting, 2 open.
    #[serde(default)]
    pub mouth_phase: u8,
    #[serde(default)]
    pub transcript_enabled: bool,
    #[serde(default)]
    pub status_message: Option<String>,
    #[serde(default)]
    pub status_expires_at_ms: Option<u64>,
    #[serde(default)]
    pub transcript_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingLabels {
    pub push_to_talk: String,
    pub food: String,
    pub play: String,
    pub comfort: String,
    pub settings: String,
    pub cancel: String,
}

impl Default for BindingLabels {
    fn default() -> Self {
        Self {
            push_to_talk: "F1".to_owned(),
            food: "F2".to_owned(),
            play: "F3".to_owned(),
            comfort: "F4".to_owned(),
            settings: "F5".to_owned(),
            cancel: "Escape".to_owned(),
        }
    }
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            mode: UiMode::Compose,
            settings_page: 0,
            focused_region: Some("compose/input".to_owned()),
            hovered_region: None,
            text_buffer: String::new(),
            pending: false,
            speech: None,
            speech_layout_text: None,
            speech_expires_at_ms: None,
            cue_queue: Vec::new(),
            text_scale: default_text_scale(),
            reduced_motion: false,
            reduced_flashes: false,
            reduced_shake: false,
            window_scale: default_window_scale(),
            fullscreen: false,
            effects_volume: default_volume(),
            speech_volume: default_volume(),
            voice_enabled: true,
            subtitles: true,
            microphone_enabled: false,
            microphone_state: MicrophoneState::Disabled,
            text_speed: default_text_speed(),
            binding_labels: BindingLabels::default(),
            controller_active: false,
            speaking: false,
            mouth_phase: 0,
            transcript_enabled: false,
            status_message: None,
            status_expires_at_ms: None,
            transcript_status: None,
        }
    }
}

impl ViewState {
    pub fn show_speech(&mut self, speech: String, now_ms: u64) {
        self.speech_layout_text = Some(speech.clone());
        self.speech = Some(speech);
        self.speech_expires_at_ms = Some(now_ms.saturating_add(SPEECH_LIFETIME_MS));
    }

    pub fn clear_speech(&mut self) {
        self.speech = None;
        self.speech_layout_text = None;
        self.speech_expires_at_ms = None;
        if self
            .focused_region
            .as_deref()
            .is_some_and(|region| region.starts_with("reaction/"))
        {
            self.focused_region = Some("compose/input".to_owned());
        }
    }

    pub fn show_status(&mut self, status: impl Into<String>, now_ms: u64, duration_ms: u64) {
        self.status_message = Some(status.into());
        self.status_expires_at_ms = Some(now_ms.saturating_add(duration_ms.max(1)));
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
        self.status_expires_at_ms = None;
    }

    /// Projects an authoritative event batch and returns its owned audio commands.
    pub fn observe_events(&mut self, events: &[GameEvent], now_ms: u64) -> AudioPlan {
        self.expire(now_ms);
        for event in events {
            match event {
                GameEvent::RelationshipBeatInterrupted(motif)
                | GameEvent::RelationshipBeatCompleted(motif) => {
                    self.cancel_owner(SemanticOwner::StandaloneRelationship(*motif));
                }
                GameEvent::ActionRelationshipInterrupted { action_id, .. } => {
                    self.cancel_owner(SemanticOwner::ActionRelationship(*action_id));
                }
                GameEvent::ActionRelationshipCompleted { action_id, .. } => {
                    self.cancel_owner(SemanticOwner::ActionRelationship(*action_id));
                    self.cancel_owner(SemanticOwner::DirectOutcome);
                }
                GameEvent::PrivateLifeCompleted { activity_id, .. }
                | GameEvent::PrivateLifeInterrupted { activity_id, .. } => {
                    self.cancel_owner(SemanticOwner::PrivateLife(*activity_id));
                }
                _ => {}
            }
            let cue = cue_for_event(event);
            if let Some((owner, kind, duration_ms)) = cue {
                self.enqueue_owned_cue(owner, kind, duration_ms, now_ms);
            }
        }
        audio_plan_for_events(events)
    }

    pub fn expire(&mut self, now_ms: u64) {
        if self
            .speech_expires_at_ms
            .is_some_and(|expires| now_ms >= expires)
        {
            if self.speaking && self.speech.is_some() {
                self.speech_expires_at_ms = Some(now_ms.saturating_add(SPEECH_RELEASE_MS));
            } else {
                self.clear_speech();
            }
        }
        if self
            .status_expires_at_ms
            .is_some_and(|expires| now_ms >= expires)
        {
            self.clear_status();
        }
        self.cue_queue.retain(|cue| now_ms < cue.expires_at_ms);
    }

    pub fn enqueue_cue(&mut self, kind: PresentationCueKind, duration_ms: u64, now_ms: u64) {
        self.enqueue_owned_cue(SemanticOwner::Ordinary, kind, duration_ms, now_ms);
    }

    pub fn enqueue_owned_cue(
        &mut self,
        owner: SemanticOwner,
        kind: PresentationCueKind,
        duration_ms: u64,
        now_ms: u64,
    ) {
        self.cue_queue.retain(|cue| cue.expires_at_ms > now_ms);
        if let Some(last) = self
            .cue_queue
            .last_mut()
            .filter(|cue| cue.owner == owner && cue.kind == kind && cue.expires_at_ms > now_ms)
        {
            last.expires_at_ms = last
                .expires_at_ms
                .max(now_ms.saturating_add(duration_ms.max(1)));
            return;
        }
        self.cue_queue.retain(|cue| {
            cue.channel != PresentationChannel::CreatureExpression
                || cue.owner.priority() > owner.priority()
        });
        if self.cue_queue.iter().any(|cue| {
            cue.channel == PresentationChannel::CreatureExpression
                && cue.owner.priority() > owner.priority()
        }) {
            return;
        }
        if self.cue_queue.len() >= CUE_QUEUE_LIMIT {
            return;
        }
        let starts_at_ms = self
            .cue_queue
            .last()
            .map_or(now_ms, |cue| cue.expires_at_ms.max(now_ms));
        self.cue_queue.push(PresentationCue {
            owner,
            channel: PresentationChannel::CreatureExpression,
            kind,
            starts_at_ms,
            expires_at_ms: starts_at_ms.saturating_add(duration_ms.max(1)),
        });
    }

    fn cancel_owner(&mut self, owner: SemanticOwner) {
        self.cue_queue.retain(|cue| cue.owner != owner);
    }

    #[must_use]
    pub fn active_cue(&self, now_ms: u64) -> Option<PresentationCueKind> {
        self.active_cue_timing(now_ms).map(|(kind, _)| kind)
    }

    fn active_cue_timing(&self, now_ms: u64) -> Option<(PresentationCueKind, u64)> {
        self.cue_queue
            .iter()
            .find(|cue| now_ms >= cue.starts_at_ms && now_ms < cue.expires_at_ms)
            .map(|cue| (cue.kind, now_ms.saturating_sub(cue.starts_at_ms)))
    }
}

const fn default_text_scale() -> u8 {
    1
}

const fn default_window_scale() -> u8 {
    3
}

const fn default_volume() -> u8 {
    100
}

const fn default_true() -> bool {
    true
}

const fn default_text_speed() -> u8 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RectCommand {
    pub id: String,
    pub rect: Rect,
    pub color: [u8; 4],
    pub layer: i16,
    /// Outlines are one UI layout unit wide.
    #[serde(default)]
    pub outline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextCommand {
    pub bounds: Option<Rect>,
    pub muted: bool,
    pub role: TextRole,
    pub id: String,
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub layer: i16,
    pub scale: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitRegion {
    pub id: String,
    pub target: Option<UiTarget>,
    pub action: UiAction,
    pub rect: Rect,
    pub enabled: bool,
    pub label: String,
    pub cursor: CursorKind,
    /// World geometry is picked by the shell; rectangles provide keyboard anchors only.
    #[serde(default)]
    pub shape: HitShape,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HitShape {
    #[default]
    Rect,
    World(UiTarget),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatureSummary {
    pub name: String,
    pub mood: Mood,
    pub mood_label: String,
    pub behavior: String,
    pub stage: String,
    pub discovered_fact: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Highlight {
    None,
    Hover,
    Focus,
    HoverFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconKind {
    Microphone,
    Food,
    Settings,
    Send,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconCommand {
    pub id: String,
    pub kind: IconKind,
    pub x: i32,
    pub y: i32,
    pub layer: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreaturePose {
    Hover,
    Swim,
    Turn,
    Inspect,
    Eat,
    Sleep,
    Play,
    React,
    Recover,
    Settle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpressionScene {
    pub owner: SemanticOwner,
    pub cue: PresentationCueKind,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateLifeScene {
    pub id: NonZeroU64,
    pub kind: PrivateLifeKind,
    pub recipe: ActivityRecipe,
    pub phase: ActivityPhase,
    pub elapsed_ms: u64,
    pub payoff_reached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipScene {
    pub motif: RelationshipMotifKey,
    pub recipe: RelationshipPerformanceRecipe,
    pub phase: RelationshipBeatPhase,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatureScene {
    pub position: NormalizedPosition,
    pub velocity: beastie_core::NormalizedVelocity,
    pub steering: SteeringMode,
    pub facing: beastie_core::Facing,
    pub gaze: GazeTarget,
    pub gaze_position: Option<NormalizedPosition>,
    pub mood: Mood,
    pub action: Option<beastie_core::ActionTimeline>,
    pub pose: CreaturePose,
    pub action_phase: Option<ActionPhase>,
    pub phase_elapsed_ms: u64,
    pub expression: Option<ExpressionScene>,
    pub speaking: bool,
    pub mouth_phase: u8,
    pub private_life: Option<PrivateLifeScene>,
    pub relationship: Option<RelationshipScene>,
    pub highlight: Highlight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectKind {
    Food(FoodId),
    Toy(ToyId),
    Plant,
    Cave,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectScene {
    pub id: u64,
    pub kind: ObjectKind,
    pub position: NormalizedPosition,
    pub velocity: beastie_core::NormalizedVelocity,
    pub carried: bool,
    pub response: ToyResponse,
    pub highlight: Highlight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectScene {
    pub owner: SemanticOwner,
    pub cue: PresentationCueKind,
    pub position: NormalizedPosition,
    pub target: UiTarget,
    pub elapsed_ms: u64,
}

/// Renderer-independent scene semantics. UI coordinates are layout units, never raster pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenePlan {
    pub creature: CreatureScene,
    pub objects: Vec<ObjectScene>,
    pub effects: Vec<EffectScene>,
    pub icons: Vec<IconCommand>,
    pub rects: Vec<RectCommand>,
    pub text: Vec<TextCommand>,
    pub hit_regions: Vec<HitRegion>,
    pub summary: CreatureSummary,
    pub elapsed_ms: u64,
    pub simulation_remainder_ms: u64,
    pub reduced_motion: bool,
    pub reduced_flashes: bool,
    pub reduced_shake: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioCue {
    AquariumHum,
    Bubble,
    SwimWake,
    Wake,
    FoodDrop,
    FoodEat,
    FoodReject,
    Sand,
    ToyImpact,
    BallNudge,
    BellRing,
    SockRustle,
    Affection,
    Curious,
    Mrr,
    Annoyed,
    Sleep,
    UiReject,
    UiConfirm,
    BubbleAlternate,
    /// Recovery sound for a creature completing a retreat into its cave.
    CaveSettle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioCommand {
    Play {
        owner: SemanticOwner,
        channel: PresentationChannel,
        cue: AudioCue,
        /// Per-command gain in thousandths, before the user's effects setting.
        gain_milli: u16,
    },
    CancelOwner {
        owner: SemanticOwner,
    },
    CancelLowerPriority {
        owner: SemanticOwner,
        channel: PresentationChannel,
    },
}

impl AudioCommand {
    #[must_use]
    pub const fn play(
        owner: SemanticOwner,
        channel: PresentationChannel,
        cue: AudioCue,
        gain_milli: u16,
    ) -> Self {
        Self::Play {
            owner,
            channel,
            cue,
            gain_milli,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioPlan {
    pub ambience: Vec<AudioCue>,
    pub events: Vec<AudioCommand>,
}

#[must_use]
pub fn audio_plan_for_events(events: &[GameEvent]) -> AudioPlan {
    let food_rejected = events
        .iter()
        .any(|event| matches!(event, GameEvent::FoodRejected(_)));
    let toy_rejected = events
        .iter()
        .any(|event| matches!(event, GameEvent::ToyRejected { .. }));
    let comforted = events.contains(&GameEvent::Comforted);
    let mut commands = Vec::new();
    for event in events {
        let ordinary = SemanticOwner::Ordinary;
        let physical = PresentationChannel::Physical;
        let voice = PresentationChannel::CreatureVoice;
        let play = match event {
            GameEvent::FoodDropped { .. } => Some((ordinary, physical, AudioCue::FoodDrop, 700)),
            GameEvent::FoodDropRejected(_) => {
                Some((ordinary, PresentationChannel::Ui, AudioCue::UiReject, 450))
            }
            GameEvent::FoodConsumed(_) => Some((
                SemanticOwner::DirectOutcome,
                physical,
                AudioCue::FoodEat,
                700,
            )),
            GameEvent::FoodRejected(_) => Some((
                SemanticOwner::DirectOutcome,
                physical,
                AudioCue::FoodReject,
                700,
            )),
            GameEvent::FoodSettled(_) => Some((ordinary, physical, AudioCue::Sand, 700)),
            GameEvent::FoodExpired(_) => Some((
                ordinary,
                PresentationChannel::Ambience,
                AudioCue::Bubble,
                700,
            )),
            GameEvent::ToyPlayed { .. } => Some((
                SemanticOwner::DirectOutcome,
                physical,
                AudioCue::ToyImpact,
                700,
            )),
            GameEvent::ToyObjectResponded {
                activity_id,
                response,
                ..
            } => match response {
                ToyResponse::None => None,
                ToyResponse::BallNudged => Some((
                    SemanticOwner::PrivateLife(*activity_id),
                    physical,
                    AudioCue::BallNudge,
                    680,
                )),
                ToyResponse::BellStruck => Some((
                    SemanticOwner::PrivateLife(*activity_id),
                    physical,
                    AudioCue::BellRing,
                    650,
                )),
                ToyResponse::SockTugged => Some((
                    SemanticOwner::PrivateLife(*activity_id),
                    physical,
                    AudioCue::SockRustle,
                    620,
                )),
            },
            GameEvent::ToyRejected { .. } | GameEvent::UtteranceRefused => {
                Some((SemanticOwner::DirectOutcome, voice, AudioCue::Annoyed, 700))
            }
            GameEvent::Comforted => Some((
                SemanticOwner::DirectOutcome,
                voice,
                AudioCue::Affection,
                700,
            )),
            GameEvent::SleepStarted => {
                Some((SemanticOwner::DirectOutcome, voice, AudioCue::Sleep, 700))
            }
            GameEvent::SleepEnded => {
                Some((SemanticOwner::DirectOutcome, voice, AudioCue::Wake, 700))
            }
            GameEvent::ActionPhaseChanged {
                to: ActionPhase::Approach,
                ..
            } => Some((ordinary, physical, AudioCue::SwimWake, 540)),
            GameEvent::SpeechPerceived(SpeechAttention::Glanced | SpeechAttention::Attended) => {
                Some((ordinary, voice, AudioCue::Curious, 700))
            }
            GameEvent::NonverbalAct(NonverbalAct::LeanAgainstPlayer) if !comforted => {
                Some((ordinary, voice, AudioCue::Mrr, 700))
            }
            GameEvent::NonverbalAct(NonverbalAct::RefuseAndStare) => {
                Some((ordinary, voice, AudioCue::Annoyed, 700))
            }
            GameEvent::NonverbalAct(NonverbalAct::PushFoodAway(_) | NonverbalAct::RefuseToEat)
                if !food_rejected =>
            {
                Some((
                    SemanticOwner::DirectOutcome,
                    physical,
                    AudioCue::FoodReject,
                    700,
                ))
            }
            GameEvent::NonverbalAct(NonverbalAct::TakeToyAway(_)) if !toy_rejected => {
                Some((SemanticOwner::DirectOutcome, voice, AudioCue::Annoyed, 700))
            }
            GameEvent::NonverbalAct(NonverbalAct::UndoTidy) => {
                Some((ordinary, physical, AudioCue::Sand, 700))
            }
            GameEvent::ActionRelationshipStarted {
                action_id, motif, ..
            } => match motif {
                RelationshipMotifKey::TrustedFood(_) => Some((
                    SemanticOwner::ActionRelationship(*action_id),
                    voice,
                    AudioCue::Mrr,
                    450,
                )),
                RelationshipMotifKey::FoodGrudge(_) => Some((
                    SemanticOwner::ActionRelationship(*action_id),
                    voice,
                    AudioCue::Annoyed,
                    400,
                )),
                _ => None,
            },
            GameEvent::RelationshipBeatStarted {
                motif, expression, ..
            } if !matches!(motif, RelationshipMotifKey::FamiliarPlace(_)) => Some((
                SemanticOwner::StandaloneRelationship(*motif),
                voice,
                relationship_audio_cue(*motif, *expression),
                600,
            )),
            GameEvent::RelationshipBeatPhaseChanged {
                motif: RelationshipMotifKey::FamiliarPlace(SemanticDestination::Cave),
                to: RelationshipBeatPhase::Act,
                ..
            } => Some((
                SemanticOwner::StandaloneRelationship(RelationshipMotifKey::FamiliarPlace(
                    SemanticDestination::Cave,
                )),
                physical,
                AudioCue::CaveSettle,
                700,
            )),
            _ => None,
        };
        match event {
            GameEvent::RelationshipBeatInterrupted(motif)
            | GameEvent::RelationshipBeatCompleted(motif) => {
                commands.push(AudioCommand::CancelOwner {
                    owner: SemanticOwner::StandaloneRelationship(*motif),
                })
            }
            GameEvent::ActionRelationshipInterrupted { action_id, .. }
            | GameEvent::ActionRelationshipCompleted { action_id, .. } => {
                commands.push(AudioCommand::CancelOwner {
                    owner: SemanticOwner::ActionRelationship(*action_id),
                })
            }
            GameEvent::ActionRelationshipResolved { action_id, .. } => {
                commands.push(AudioCommand::CancelOwner {
                    owner: SemanticOwner::ActionRelationship(*action_id),
                })
            }
            GameEvent::PrivateLifeCompleted { activity_id, .. }
            | GameEvent::PrivateLifeInterrupted { activity_id, .. } => {
                commands.push(AudioCommand::CancelOwner {
                    owner: SemanticOwner::PrivateLife(*activity_id),
                })
            }
            GameEvent::FoodConsumed(_)
            | GameEvent::FoodRejected(_)
            | GameEvent::ToyPlayed { .. }
            | GameEvent::Comforted => commands.push(AudioCommand::CancelLowerPriority {
                owner: SemanticOwner::DirectOutcome,
                channel: PresentationChannel::CreatureVoice,
            }),
            _ => {}
        }
        if let Some((owner, channel, cue, gain_milli)) = play {
            let command = AudioCommand::play(owner, channel, cue, gain_milli);
            if !commands.contains(&command) {
                commands.push(command);
            }
        }
    }
    AudioPlan {
        ambience: Vec::new(),
        events: commands,
    }
}

fn relationship_audio_cue(
    motif: RelationshipMotifKey,
    expression: RelationshipExpressionKind,
) -> AudioCue {
    match (motif, expression) {
        // Recognition/anticipation is not a toy collision. The authored impact sound is reserved
        // for the authoritative ToyPlayed event.
        (RelationshipMotifKey::SharedToy(_), _) => AudioCue::Curious,
        (RelationshipMotifKey::ComfortRitual, RelationshipExpressionKind::Notice)
        | (RelationshipMotifKey::PlayerReturns, RelationshipExpressionKind::Notice)
        | (RelationshipMotifKey::PlayerReturns, RelationshipExpressionKind::Anticipate)
        | (RelationshipMotifKey::FoodGrudge(_), RelationshipExpressionKind::Notice) => {
            AudioCue::Curious
        }
        (RelationshipMotifKey::ComfortRitual, _) | (RelationshipMotifKey::PlayerReturns, _) => {
            AudioCue::Affection
        }
        (RelationshipMotifKey::TrustedFood(_), _) => AudioCue::Curious,
        (RelationshipMotifKey::FoodGrudge(_), _) => AudioCue::FoodReject,
        (RelationshipMotifKey::FamiliarPlace(_), _) => AudioCue::Curious,
    }
}

#[must_use]
pub fn contextual_actions(target: UiTarget) -> Vec<UiAction> {
    match target {
        UiTarget::Creature => vec![
            UiAction::Comfort,
            UiAction::OpenToyChoice,
            UiAction::Inspect,
            UiAction::Rename,
        ],
        UiTarget::Toy(toy) => vec![UiAction::Play(toy), UiAction::Inspect],
        UiTarget::Cave | UiTarget::Plant(_) | UiTarget::FoodObject(_) | UiTarget::OpenWater => {
            vec![UiAction::Inspect]
        }
        UiTarget::Reaction(_) | UiTarget::ComposeField | UiTarget::Actions => Vec::new(),
    }
}

/// Maps a simulation coordinate to a UI anchor; the 3D shell projects world geometry itself.
#[must_use]
pub fn world_to_logical(position: NormalizedPosition) -> (i32, i32) {
    const X_MIN: i32 = 28;
    const X_SPAN: i32 = 264;
    const Y_MIN: i32 = 22;
    const Y_SPAN: i32 = 91;
    let position = position.clamped();
    let x = X_MIN + rounded_ratio(position.x, X_SPAN);
    let y = Y_MIN + rounded_ratio(position.y, Y_SPAN);
    (x, y)
}

#[must_use]
pub fn logical_to_world(x: i32, y: i32) -> NormalizedPosition {
    const X_MIN: i32 = 28;
    const X_SPAN: i32 = 264;
    const Y_MIN: i32 = 22;
    const Y_SPAN: i32 = 91;
    NormalizedPosition::new(
        ((x - X_MIN).clamp(0, X_SPAN) * NormalizedPosition::SCALE + X_SPAN / 2) / X_SPAN,
        ((y - Y_MIN).clamp(0, Y_SPAN) * NormalizedPosition::SCALE + Y_SPAN / 2) / Y_SPAN,
    )
}

#[must_use]
pub fn plan(state: &WorldState, view: &ViewState) -> (ScenePlan, AudioPlan) {
    let mut icons = Vec::new();
    let mut rects = Vec::new();
    let mut text = Vec::new();
    let creature = creature_scene(state, view);
    let objects = object_scenes(state, view);
    let effects = effect_scenes(state, &creature);
    let mut hit_regions = world_hit_regions(state, view);
    add_speech(state, view, &mut rects, &mut text, &mut hit_regions);
    add_persistent_bar(
        state,
        view,
        &mut icons,
        &mut rects,
        &mut text,
        &mut hit_regions,
    );
    add_temporary_mode(view, &mut rects, &mut text, &mut hit_regions);
    let close = match view.mode {
        UiMode::Settings => Some((
            Rect {
                x: 281,
                y: 7,
                w: 26,
                h: 14,
            },
            UiAction::CancelMode,
            "Close",
        )),
        UiMode::Bindings => Some((
            Rect {
                x: 243,
                y: 17,
                w: 30,
                h: 14,
            },
            UiAction::OpenSettings,
            "Back",
        )),
        UiMode::DataManagement => Some((
            Rect {
                x: 238,
                y: 20,
                w: 30,
                h: 14,
            },
            UiAction::OpenSettings,
            "Back",
        )),
        _ => None,
    };
    if let Some((area, action, title)) = close {
        add_button_chrome("modal/close", area, true, false, 25, &mut rects);
        hit_regions.push(hit("modal/close", None, action, area, true, title));
        text.push(label(
            "modal/close-label",
            title,
            area.x + 3,
            area.y + 3,
            29,
        ));
    }
    add_status(view, state.elapsed_ms, &mut rects, &mut text);
    add_hover_and_focus(view, &hit_regions, &mut rects, &mut text);
    for command in &mut text {
        command.scale = view.text_scale.clamp(1, 2);
    }
    ui_art::layout_text(&mut text, &rects, &hit_regions);
    icons.sort_by_key(|command| command.layer);
    rects.sort_by_key(|command| command.layer);
    text.sort_by_key(|command| command.layer);
    (
        ScenePlan {
            creature,
            objects,
            effects,
            icons,
            rects,
            text,
            hit_regions,
            summary: creature_summary(state),
            elapsed_ms: state.elapsed_ms,
            simulation_remainder_ms: state.simulation_remainder_ms,
            reduced_motion: view.reduced_motion,
            reduced_flashes: view.reduced_flashes,
            reduced_shake: view.reduced_shake,
        },
        AudioPlan {
            ambience: vec![AudioCue::AquariumHum],
            events: Vec::new(),
        },
    )
}

fn rounded_ratio(normalized: i32, span: i32) -> i32 {
    (normalized * span + NormalizedPosition::SCALE / 2) / NormalizedPosition::SCALE
}

fn highlight_for(view: &ViewState, id: &str) -> Highlight {
    match (
        view.hovered_region.as_deref() == Some(id),
        view.focused_region.as_deref() == Some(id),
    ) {
        (true, true) => Highlight::HoverFocus,
        (true, false) => Highlight::Hover,
        (false, true) => Highlight::Focus,
        _ => Highlight::None,
    }
}

fn creature_scene(state: &WorldState, view: &ViewState) -> CreatureScene {
    let aquarium = &state.creature.aquarium;
    let private_life =
        state
            .creature
            .private_life
            .active
            .as_ref()
            .map(|activity| PrivateLifeScene {
                id: activity.id,
                kind: activity.kind,
                recipe: activity.recipe,
                phase: activity.phase,
                elapsed_ms: state
                    .elapsed_ms
                    .saturating_sub(activity.phase_started_at_ms),
                payoff_reached: activity.payoff_reached,
            });
    let relationship = state
        .creature
        .relationship_expression
        .active
        .as_ref()
        .map(|beat| RelationshipScene {
            motif: beat.motif,
            recipe: performance_recipe_for(beat.motif, beat.expression_kind),
            phase: beat.phase,
            elapsed_ms: state.elapsed_ms.saturating_sub(beat.phase_started_at_ms),
        });
    let queued = view
        .cue_queue
        .iter()
        .filter(|cue| state.elapsed_ms >= cue.starts_at_ms && state.elapsed_ms < cue.expires_at_ms)
        .map(|cue| ExpressionScene {
            owner: cue.owner,
            cue: cue.kind,
            elapsed_ms: state.elapsed_ms.saturating_sub(cue.starts_at_ms),
        });
    // On equal priority an explicit event receipt wins over continuously projected activity.
    let mut candidates: Vec<_> = [action_relationship_cue(state), private_life_cue(state)]
        .into_iter()
        .flatten()
        .map(|(owner, cue, elapsed_ms)| ExpressionScene {
            owner,
            cue,
            elapsed_ms,
        })
        .collect();
    candidates.extend(queued);
    let expression = candidates
        .into_iter()
        .max_by_key(|cue| cue.owner.priority());
    let pose = match creature_pose(state) {
        "swim" => CreaturePose::Swim,
        "turn" => CreaturePose::Turn,
        "inspect" => CreaturePose::Inspect,
        "eat" => CreaturePose::Eat,
        "sleep" => CreaturePose::Sleep,
        "play" => CreaturePose::Play,
        "react" => CreaturePose::React,
        "recover" => CreaturePose::Recover,
        "settle" => CreaturePose::Settle,
        _ => CreaturePose::Hover,
    };
    CreatureScene {
        position: aquarium.position,
        velocity: aquarium.velocity,
        steering: aquarium.steering,
        facing: aquarium.facing,
        gaze: aquarium.gaze,
        gaze_position: gaze_position(state),
        mood: state.mood(),
        pose,
        action: aquarium.action.clone(),
        action_phase: aquarium.action.as_ref().map(|action| action.phase),
        phase_elapsed_ms: private_life.as_ref().map_or_else(
            || {
                aquarium
                    .action
                    .as_ref()
                    .map_or(state.elapsed_ms, |action| action.elapsed_ms)
            },
            |activity| activity.elapsed_ms,
        ),
        expression,
        speaking: view.speaking,
        mouth_phase: if view.speaking {
            view.mouth_phase.min(2)
        } else {
            0
        },
        private_life,
        relationship,
        highlight: highlight_for(view, "target/creature"),
    }
}

/// Resolve only authoritative targets; absent or consumed objects have no gaze position.
fn gaze_position(state: &WorldState) -> Option<NormalizedPosition> {
    match state.creature.aquarium.gaze {
        GazeTarget::Cursor => state.aquarium.cursor,
        GazeTarget::Food(id) => match state.aquarium.objects.get(&id)? {
            WorldObject::Food(food) if food.disposition != FoodDisposition::Consumed => {
                Some(food.position)
            }
            _ => None,
        },
        GazeTarget::Toy(toy) => state
            .aquarium
            .toy_states
            .get(&toy)
            .map(|object| object.position),
        GazeTarget::Cave => state
            .aquarium
            .objects
            .values()
            .find_map(|object| match object {
                WorldObject::Cave { position } => Some(*position),
                _ => None,
            }),
        GazeTarget::Plant => state
            .aquarium
            .objects
            .values()
            .find_map(|object| match object {
                WorldObject::Plant { position } => Some(*position),
                _ => None,
            }),
        GazeTarget::Player | GazeTarget::None => None,
    }
}

fn object_scenes(state: &WorldState, view: &ViewState) -> Vec<ObjectScene> {
    state
        .aquarium
        .objects
        .iter()
        .filter_map(|(id, object)| {
            let mut velocity = beastie_core::NormalizedVelocity::default();
            let mut carried = false;
            let mut response = ToyResponse::None;
            let (kind, position) = match object {
                WorldObject::Food(food) if food.disposition == FoodDisposition::Consumed => {
                    return None;
                }
                WorldObject::Food(food) => {
                    velocity = food.velocity;
                    (ObjectKind::Food(food.food), food.position)
                }
                WorldObject::Toy { toy, position } => {
                    let position = state
                        .aquarium
                        .toy_states
                        .get(toy)
                        .map_or(*position, |object| {
                            velocity = object.velocity;
                            carried = object.carried;
                            response = object.last_response;
                            object.position
                        });
                    (ObjectKind::Toy(*toy), position)
                }
                WorldObject::Plant { position } => (ObjectKind::Plant, *position),
                WorldObject::Cave { position } => (ObjectKind::Cave, *position),
            };
            Some(ObjectScene {
                id: *id,
                kind,
                position,
                velocity,
                carried,
                response,
                highlight: highlight_for(view, &format!("target/object-{id}")),
            })
        })
        .collect()
}

fn effect_scenes(state: &WorldState, creature: &CreatureScene) -> Vec<EffectScene> {
    let mut effects = Vec::new();
    if let Some(expression) = &creature.expression {
        effects.push(EffectScene {
            owner: expression.owner,
            cue: expression.cue,
            position: creature.position,
            target: UiTarget::Creature,
            elapsed_ms: expression.elapsed_ms,
        });
    }
    if let Some(activity) = &creature.private_life {
        let contact = match activity.kind {
            PrivateLifeKind::ToyPlay(_) => activity.payoff_reached,
            PrivateLifeKind::CaveSettle => {
                matches!(activity.phase, ActivityPhase::Act | ActivityPhase::Settle)
            }
            _ => activity.phase == ActivityPhase::Act,
        };
        if contact {
            let target = match activity.kind {
                PrivateLifeKind::ToyPlay(toy) => state
                    .aquarium
                    .toy_states
                    .get(&toy)
                    .map(|object| (UiTarget::Toy(toy), object.position)),
                PrivateLifeKind::CaveSettle => {
                    state
                        .aquarium
                        .objects
                        .values()
                        .find_map(|object| match object {
                            WorldObject::Cave { position } => Some((UiTarget::Cave, *position)),
                            _ => None,
                        })
                }
                PrivateLifeKind::PlantInspect => {
                    state
                        .aquarium
                        .objects
                        .iter()
                        .find_map(|(id, object)| match object {
                            WorldObject::Plant { position } => {
                                Some((UiTarget::Plant(*id), *position))
                            }
                            _ => None,
                        })
                }
                PrivateLifeKind::BottomForage => Some((
                    UiTarget::OpenWater,
                    NormalizedPosition::new(creature.position.x, NormalizedPosition::SCALE),
                )),
                PrivateLifeKind::OpenWaterDrift => Some((UiTarget::Creature, creature.position)),
            };
            if let Some((target, position)) = target {
                effects.push(EffectScene {
                    owner: SemanticOwner::PrivateLife(activity.id),
                    cue: private_life_recipe_cue(activity.recipe),
                    position,
                    target,
                    elapsed_ms: activity.elapsed_ms,
                });
            }
        }
    }
    if let Some(beat) = &creature.relationship
        && beat.phase == RelationshipBeatPhase::Act
        && let RelationshipMotifKey::SharedToy(toy) = beat.motif
        && let Some(object) = state.aquarium.toy_states.get(&toy)
    {
        effects.push(EffectScene {
            owner: SemanticOwner::StandaloneRelationship(beat.motif),
            cue: PresentationCueKind::Notice,
            position: object.position,
            target: UiTarget::Toy(toy),
            elapsed_ms: beat.elapsed_ms,
        });
    }
    if creature.velocity.x.unsigned_abs() + creature.velocity.y.unsigned_abs() > 25 {
        effects.push(EffectScene {
            owner: SemanticOwner::Ordinary,
            cue: PresentationCueKind::Wake,
            position: creature.position,
            target: UiTarget::Creature,
            elapsed_ms: state.elapsed_ms,
        });
    }
    effects
}

fn world_hit_regions(state: &WorldState, view: &ViewState) -> Vec<HitRegion> {
    if let UiMode::FoodDrop(food) = view.mode {
        return vec![HitRegion {
            id: "world/drop-food".to_owned(),
            target: Some(UiTarget::OpenWater),
            action: UiAction::DropFood(food),
            rect: Rect {
                x: 0,
                y: 0,
                w: LOGICAL_WIDTH,
                h: AQUARIUM_BOTTOM,
            },
            enabled: true,
            label: format!("Drop {}", food_name(food)),
            cursor: CursorKind::FoodDrop,
            shape: HitShape::Rect,
        }];
    }
    if !matches!(view.mode, UiMode::Compose) {
        return Vec::new();
    }
    let (x, y) = world_to_logical(state.creature.aquarium.position);
    let mut hits = vec![HitRegion {
        id: "target/creature".to_owned(),
        target: Some(UiTarget::Creature),
        action: UiAction::OpenContext(UiTarget::Creature),
        rect: Rect {
            x: x - CREATURE_HIT_WIDTH / 2,
            y: y - CREATURE_HIT_HEIGHT / 2,
            w: CREATURE_HIT_WIDTH,
            h: CREATURE_HIT_HEIGHT,
        },
        enabled: true,
        label: state.creature.name.clone(),
        cursor: CursorKind::Pointer,
        shape: HitShape::World(UiTarget::Creature),
    }];
    for (id, object) in &state.aquarium.objects {
        let (target, position, label) = match object {
            WorldObject::Food(food) if !matches!(food.disposition, FoodDisposition::Consumed) => (
                UiTarget::FoodObject(*id),
                food.position,
                state
                    .aquarium
                    .object_names
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| food_name(food.food).to_owned()),
            ),
            WorldObject::Food(_) => continue,
            WorldObject::Toy { toy, position } => {
                let position = state
                    .aquarium
                    .toy_states
                    .get(toy)
                    .map_or(*position, |toy_state| toy_state.position);
                (
                    UiTarget::Toy(*toy),
                    position,
                    state
                        .aquarium
                        .object_names
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| toy_name(*toy).to_owned()),
                )
            }
            WorldObject::Plant { position } => {
                (UiTarget::Plant(*id), *position, "Plant".to_owned())
            }
            WorldObject::Cave { position } => (UiTarget::Cave, *position, "Cave".to_owned()),
        };
        let (x, y) = world_to_logical(position);
        let hit_id = format!("target/object-{id}");
        hits.push(HitRegion {
            id: hit_id.clone(),
            target: Some(target),
            action: UiAction::OpenContext(target),
            rect: Rect {
                x: x - 10,
                y: y - 10,
                w: 20,
                h: 20,
            },
            enabled: true,
            label,
            cursor: CursorKind::Pointer,
            shape: HitShape::World(target),
        });
    }
    hits
}

fn add_persistent_bar(
    state: &WorldState,
    view: &ViewState,
    icons: &mut Vec<IconCommand>,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    let summary = creature_summary(state);
    let text_scale = view.text_scale.clamp(1, 2);
    rects.push(rect(
        "compose/bar-shadow",
        Rect {
            x: 0,
            y: 129,
            w: 320,
            h: 51,
        },
        UI_SHADOW,
        29,
    ));
    rects.push(rect(
        "compose/bar-edge",
        Rect {
            x: 0,
            y: 130,
            w: 320,
            h: 50,
        },
        UI_EDGE,
        30,
    ));
    rects.push(rect(
        "compose/bar",
        Rect {
            x: 1,
            y: 132,
            w: 318,
            h: 48,
        },
        UI_PANEL,
        31,
    ));
    rects.push(rect(
        "compose/bar-glint",
        Rect {
            x: 2,
            y: 132,
            w: 316,
            h: 1,
        },
        UI_EDGE_LIT,
        32,
    ));

    rects.push(rect(
        "compose/mood-dot",
        Rect {
            x: 8,
            y: 139,
            w: 4,
            h: 4,
        },
        mood_color(summary.mood),
        34,
    ));
    {
        text.push(label(
            "compose/summary-name",
            &head_fit(&summary.name, if text_scale >= 2 { 12 } else { 16 }),
            17,
            135,
            34,
        ));
        text.push(label(
            "compose/summary-behavior",
            &head_fit(&summary.behavior, if text_scale >= 2 { 15 } else { 20 }),
            88,
            137,
            34,
        ));
    }

    let input_rect = Rect {
        x: 8,
        y: 153,
        w: 184,
        h: 23,
    };
    let microphone_rect = Rect {
        x: 222,
        y: 153,
        w: 27,
        h: 23,
    };
    let food_rect = Rect {
        x: 253,
        y: 153,
        w: 27,
        h: 23,
    };
    let settings_rect = Rect {
        x: 284,
        y: 153,
        w: 28,
        h: 23,
    };
    let send_rect = Rect {
        x: 195,
        y: 153,
        w: 23,
        h: 23,
    };
    add_inset("compose/input", input_rect, 31, rects);
    let input_capacity = ((input_rect.w - 17) as f32
        / (TextRole::Body.size(text_scale >= 2) * 0.56))
        .floor() as usize;
    let input_value = if view.text_buffer.is_empty() {
        head_fit(&format!("Talk to {}...", summary.name), input_capacity)
    } else {
        tail_fit(&view.text_buffer, input_capacity)
    };
    text.push(label("compose/input-text", &input_value, 14, 160, 34));
    hits.push(hit(
        "compose/input",
        Some(UiTarget::ComposeField),
        UiAction::FocusCompose,
        input_rect,
        true,
        "Message",
    ));
    let microphone_available = view.microphone_enabled
        && !matches!(
            view.microphone_state,
            MicrophoneState::Disabled | MicrophoneState::Unavailable | MicrophoneState::Error
        );
    hits.push(hit(
        "compose/microphone",
        Some(UiTarget::Actions),
        UiAction::PushToTalk,
        microphone_rect,
        microphone_available,
        microphone_label(view.microphone_state),
    ));
    add_button_chrome(
        "compose/microphone",
        microphone_rect,
        microphone_available,
        matches!(
            view.microphone_state,
            MicrophoneState::Listening | MicrophoneState::Recognizing
        ),
        31,
        rects,
    );
    icons.push(icon(
        "ui/button-microphone",
        microphone_rect.x + microphone_rect.w / 2,
        microphone_rect.y + 8,
        35,
    ));
    hits.push(hit(
        "compose/food",
        Some(UiTarget::Actions),
        UiAction::OpenFoodChoice,
        food_rect,
        true,
        "Food",
    ));
    add_button_chrome("compose/food", food_rect, true, false, 31, rects);
    icons.push(icon(
        "ui/button-food",
        food_rect.x + food_rect.w / 2,
        food_rect.y + 8,
        35,
    ));
    if matches!(view.mode, UiMode::Compose) {
        hits.push(hit(
            "compose/settings",
            Some(UiTarget::Actions),
            UiAction::OpenSettings,
            settings_rect,
            true,
            "Settings",
        ));
        add_button_chrome("compose/settings", settings_rect, true, false, 31, rects);
        icons.push(icon(
            "ui/button-settings",
            settings_rect.x + settings_rect.w / 2,
            settings_rect.y + 8,
            35,
        ));
    } else {
        hits.push(hit(
            "compose/close",
            Some(UiTarget::Actions),
            UiAction::CancelMode,
            settings_rect,
            true,
            "Close",
        ));
        add_button_chrome("compose/close", settings_rect, true, false, 31, rects);
        text.push(label(
            "compose/close-label",
            "x",
            settings_rect.x + 9,
            settings_rect.y + 7,
            35,
        ));
    }
    let send_action = if matches!(view.mode, UiMode::Rename) {
        UiAction::SubmitName
    } else {
        UiAction::SubmitText
    };
    let send_label = if matches!(view.mode, UiMode::Rename) {
        "Name"
    } else {
        "Send"
    };
    hits.push(hit(
        "compose/send",
        None,
        send_action,
        send_rect,
        !view.pending && !view.text_buffer.trim().is_empty(),
        send_label,
    ));
    let send_enabled = !view.pending && !view.text_buffer.trim().is_empty();
    add_button_chrome(
        "compose/send",
        send_rect,
        send_enabled,
        send_enabled,
        31,
        rects,
    );
    icons.push(icon(
        "ui/button-send",
        send_rect.x + send_rect.w / 2,
        send_rect.y + 8,
        35,
    ));
    for (id, title, area) in [
        ("speak", "Speak", microphone_rect),
        ("feed", "Feed", food_rect),
        (
            "settings",
            if matches!(view.mode, UiMode::Compose) {
                "Settings"
            } else {
                "Close"
            },
            settings_rect,
        ),
        ("send", send_label, send_rect),
    ] {
        let mut caption = label(
            &format!("compose/control-{id}"),
            title,
            area.x + 2,
            area.y + 15,
            35,
        );
        caption.bounds = Some(Rect {
            x: area.x + 2,
            y: area.y + 15,
            w: area.w - 4,
            h: 7,
        });
        caption.role = TextRole::ControlCaption;
        caption.muted = (id == "send" && !send_enabled) || (id == "speak" && !microphone_available);
        text.push(caption);
    }
}

fn head_fit(value: &str, capacity: usize) -> String {
    if value.chars().count() <= capacity {
        return value.to_owned();
    }
    match capacity {
        0 => String::new(),
        1 => "…".to_owned(),
        _ => format!("{}…", value.chars().take(capacity - 1).collect::<String>()),
    }
}

fn tail_fit(value: &str, capacity: usize) -> String {
    let count = value.chars().count();
    if count <= capacity {
        return value.to_owned();
    }
    match capacity {
        0 => String::new(),
        1 => "…".to_owned(),
        _ => format!(
            "…{}",
            value.chars().skip(count - capacity + 1).collect::<String>()
        ),
    }
}

fn add_temporary_mode(
    view: &ViewState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    match view.mode {
        UiMode::Compose => {}
        UiMode::Context(target) => {
            let actions = contextual_actions(target);
            add_action_strip("context", &actions, 100, rects, text, hits);
        }
        UiMode::FoodChoice => add_action_strip(
            "food",
            &[
                UiAction::SelectFood(FoodId::Berry),
                UiAction::SelectFood(FoodId::Mushroom),
                UiAction::SelectFood(FoodId::Pellet),
            ],
            100,
            rects,
            text,
            hits,
        ),
        UiMode::FoodDrop(food) => {
            rects.push(rect(
                "mode/drop-food-background",
                Rect {
                    x: 79,
                    y: 4,
                    w: 162,
                    h: 18,
                },
                [22, 31, 43, 232],
                28,
            ));
            text.push(label(
                "mode/drop-food-label",
                &format!("Drop {} into open water  [Esc]", food_name(food)),
                84,
                9,
                29,
            ));
        }
        UiMode::ToyChoice => add_action_strip(
            "toy",
            &[
                UiAction::Play(ToyId::Ball),
                UiAction::Play(ToyId::Bell),
                UiAction::Play(ToyId::Sock),
            ],
            100,
            rects,
            text,
            hits,
        ),
        UiMode::OnScreenKeyboard => add_keyboard(view, rects, text, hits),
        UiMode::Settings => add_settings(view, rects, text, hits),
        UiMode::Bindings => add_bindings(view, rects, text, hits),
        UiMode::Rebinding(action) => add_rebinding(action, rects, text),
        UiMode::Rename => add_rename(view, rects, text),
        UiMode::DataManagement => add_data_management(view, rects, text, hits),
        UiMode::ConfirmReset => add_reset_confirmation(rects, text, hits),
    }
}

fn add_settings(
    view: &ViewState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_panel_chrome(
        "settings/panel",
        Rect {
            x: 5,
            y: 3,
            w: 310,
            h: 124,
        },
        25,
        rects,
    );
    text.push(label("settings/title", "Settings", 15, 10, 29));
    for (index, title) in ["Comfort & display", "Sound & speech", "Controls & data"]
        .into_iter()
        .enumerate()
    {
        let area = Rect {
            x: 15 + index as i32 * 97,
            y: 25,
            w: 94,
            h: 17,
        };
        let id = format!("settings/page-{index}");
        hits.push(hit(
            &id,
            None,
            UiAction::SelectSettingsPage(index as u8),
            area,
            true,
            title,
        ));
        add_button_chrome(
            &id,
            area,
            true,
            usize::from(view.settings_page.min(2)) == index,
            25,
            rects,
        );
        text.push(label(
            &format!("{id}-label"),
            title,
            area.x + 5,
            area.y + 5,
            29,
        ));
    }
    let settings = [
        (
            "text-scale",
            "Text size",
            if view.text_scale >= 2 {
                "Large"
            } else {
                "Normal"
            },
            UiAction::SetTextScale(if view.text_scale >= 2 { 1 } else { 2 }),
        ),
        (
            "motion",
            "Reduced motion",
            on_off(view.reduced_motion),
            UiAction::ToggleReducedMotion,
        ),
        (
            "flashes",
            "Reduced flashes",
            on_off(view.reduced_flashes),
            UiAction::ToggleReducedFlashes,
        ),
        (
            "shake",
            "Reduced shake",
            on_off(view.reduced_shake),
            UiAction::ToggleReducedShake,
        ),
        (
            "window-scale",
            "Window scale",
            scale_label(view.window_scale),
            UiAction::CycleWindowScale,
        ),
        (
            "fullscreen",
            "Fullscreen",
            on_off(view.fullscreen),
            UiAction::ToggleFullscreen,
        ),
        (
            "effects-volume",
            "Effects",
            volume_label(view.effects_volume),
            UiAction::CycleEffectsVolume,
        ),
        (
            "speech-volume",
            "Speech",
            volume_label(view.speech_volume),
            UiAction::CycleSpeechVolume,
        ),
        (
            "voice",
            "Voice",
            on_off(view.voice_enabled),
            UiAction::ToggleVoice,
        ),
        (
            "subtitles",
            "Subtitles",
            on_off(view.subtitles),
            UiAction::ToggleSubtitles,
        ),
        (
            "microphone",
            "Microphone",
            on_off(view.microphone_enabled),
            UiAction::ToggleMicrophone,
        ),
        (
            "text-speed",
            "Text speed",
            text_speed_label(view.text_speed),
            UiAction::CycleTextSpeed,
        ),
        ("bindings", "Bindings", "Open", UiAction::OpenBindings),
        (
            "reset-bindings",
            "Reset keys",
            "Reset",
            UiAction::ResetBindings,
        ),
        ("data", "Save & data", "Open", UiAction::OpenDataManagement),
    ];
    let range = match view.settings_page.min(2) {
        0 => 0..6,
        1 => 6..12,
        _ => 12..15,
    };
    for (index, (id, setting, value, action)) in settings[range].iter().copied().enumerate() {
        let column = index / 3;
        let row = index % 3;
        let column_x = 15 + column as i32 * 148;
        let y = 49 + row as i32 * 24;
        text.push(label(
            &format!("settings/{id}-name"),
            setting,
            column_x,
            y + 6,
            29,
        ));
        let button = Rect {
            x: column_x + 89,
            y,
            w: 49,
            h: 20,
        };
        hits.push(hit(
            &format!("settings/{id}"),
            None,
            action,
            button,
            true,
            setting,
        ));
        add_button_chrome(&format!("settings/{id}"), button, true, false, 25, rects);
        text.push(label(
            &format!("settings/{id}-value"),
            value,
            button.x + 6,
            button.y + 6,
            29,
        ));
    }
}

fn add_rename(view: &ViewState, rects: &mut Vec<RectCommand>, text: &mut Vec<TextCommand>) {
    add_panel_chrome(
        "rename/prompt-background",
        Rect {
            x: 74,
            y: 16,
            w: 172,
            h: 27,
        },
        25,
        rects,
    );
    text.push(label(
        "rename/prompt",
        if view.text_buffer.trim().is_empty() {
            "Type a real name"
        } else {
            "Enter confirms, Escape cancels"
        },
        82,
        25,
        29,
    ));
}

fn add_data_management(
    view: &ViewState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_panel_chrome(
        "data/panel",
        Rect {
            x: 43,
            y: 16,
            w: 234,
            h: 108,
        },
        25,
        rects,
    );
    text.push(label("data/title", "Save & local data", 51, 23, 29));
    let actions = [
        ("recover", "Recover backup", UiAction::RecoverBackup, true),
        ("reset", "Reset creature", UiAction::RequestReset, true),
        (
            "transcript",
            if view.transcript_enabled {
                "Transcript: On"
            } else {
                "Transcript: Off"
            },
            UiAction::ToggleTranscript,
            true,
        ),
        (
            "export",
            "Export transcript",
            UiAction::ExportTranscript,
            view.transcript_enabled,
        ),
    ];
    for (index, (id, name, action, enabled)) in actions.into_iter().enumerate() {
        let button = Rect {
            x: 53,
            y: 39 + i32::try_from(index).unwrap_or_default() * 20,
            w: 214,
            h: 17,
        };
        hits.push(hit(
            &format!("data/{id}"),
            None,
            action,
            button,
            enabled,
            name,
        ));
        add_button_chrome(&format!("data/{id}"), button, enabled, false, 25, rects);
        text.push(label(
            &format!("data/{id}-label"),
            name,
            button.x + 6,
            button.y + 5,
            29,
        ));
    }
}

fn add_reset_confirmation(
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_panel_chrome(
        "reset/panel",
        Rect {
            x: 49,
            y: 39,
            w: 222,
            h: 59,
        },
        29,
        rects,
    );
    text.push(label("reset/warning", "Reset this creature?", 57, 48, 32));
    text.push(label(
        "reset/detail",
        "Backup stays recoverable.",
        57,
        61,
        33,
    ));
    for (id, name, action, x) in [
        ("cancel", "Cancel", UiAction::CancelMode, 57),
        ("confirm", "Reset", UiAction::ConfirmReset, 165),
    ] {
        let button = Rect {
            x,
            y: 72,
            w: 98,
            h: 19,
        };
        hits.push(hit(
            &format!("reset/{id}"),
            None,
            action,
            button,
            true,
            name,
        ));
        add_button_chrome(
            &format!("reset/{id}"),
            button,
            true,
            id == "confirm",
            29,
            rects,
        );
        text.push(label(
            &format!("reset/{id}-label"),
            name,
            button.x + 8,
            button.y + 5,
            33,
        ));
    }
}

fn add_bindings(
    view: &ViewState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_panel_chrome(
        "bindings/panel",
        Rect {
            x: 38,
            y: 14,
            w: 244,
            h: 110,
        },
        25,
        rects,
    );
    text.push(label("bindings/title", "Input bindings", 46, 20, 29));
    let rows = [
        (
            BindableAction::PushToTalk,
            "Push to talk",
            view.binding_labels.push_to_talk.as_str(),
        ),
        (
            BindableAction::Food,
            "Food",
            view.binding_labels.food.as_str(),
        ),
        (
            BindableAction::Play,
            "Play",
            view.binding_labels.play.as_str(),
        ),
        (
            BindableAction::Comfort,
            "Comfort",
            view.binding_labels.comfort.as_str(),
        ),
        (
            BindableAction::Settings,
            "Settings",
            view.binding_labels.settings.as_str(),
        ),
        (
            BindableAction::Cancel,
            "Cancel",
            view.binding_labels.cancel.as_str(),
        ),
    ];
    for (index, (action, name, binding)) in rows.into_iter().enumerate() {
        let y = 32 + i32::try_from(index).unwrap_or_default() * 14;
        text.push(label(
            &format!("bindings/{}-name", bindable_id(action)),
            name,
            48,
            y + 4,
            29,
        ));
        let button = Rect {
            x: 181,
            y,
            w: 91,
            h: 14,
        };
        hits.push(hit(
            &format!("bindings/{}", bindable_id(action)),
            None,
            UiAction::BeginRebind(action),
            button,
            true,
            &format!("Rebind {name}"),
        ));
        add_button_chrome(
            &format!("bindings/{}", bindable_id(action)),
            button,
            true,
            false,
            25,
            rects,
        );
        text.push(label(
            &format!("bindings/{}-value", bindable_id(action)),
            binding,
            button.x + 5,
            button.y + 3,
            29,
        ));
    }
}

fn add_rebinding(
    action: BindableAction,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
) {
    add_panel_chrome(
        "bindings/capture/panel",
        Rect {
            x: 53,
            y: 46,
            w: 214,
            h: 42,
        },
        29,
        rects,
    );
    text.push(label(
        "bindings/capture-prompt",
        &format!("Press a key for {}", bindable_name(action)),
        64,
        57,
        32,
    ));
    text.push(label(
        "bindings/capture-cancel",
        "Escape cancels",
        64,
        72,
        32,
    ));
}

fn on_off(value: bool) -> &'static str {
    if value { "On" } else { "Off" }
}

fn scale_label(scale: u8) -> &'static str {
    match scale.clamp(1, 6) {
        1 => "1x",
        2 => "2x",
        3 => "3x",
        4 => "4x",
        5 => "5x",
        _ => "6x",
    }
}

fn volume_label(volume: u8) -> &'static str {
    match volume.min(100) {
        0..=12 => "0%",
        13..=37 => "25%",
        38..=62 => "50%",
        63..=87 => "75%",
        _ => "100%",
    }
}

fn text_speed_label(speed: u8) -> &'static str {
    match speed.min(2) {
        0 => "Instant",
        1 => "Normal",
        _ => "Slow",
    }
}

fn microphone_label(state: MicrophoneState) -> &'static str {
    match state {
        MicrophoneState::Disabled => "Microphone disabled in Settings",
        MicrophoneState::Idle => "Hold to talk",
        MicrophoneState::Listening => "Listening; release to send",
        MicrophoneState::Recognizing => "Recognizing speech",
        MicrophoneState::Unavailable => "Microphone unavailable",
        MicrophoneState::Error => "Microphone error; text remains available",
    }
}

fn bindable_id(action: BindableAction) -> &'static str {
    match action {
        BindableAction::PushToTalk => "push-to-talk",
        BindableAction::Food => "food",
        BindableAction::Play => "play",
        BindableAction::Comfort => "comfort",
        BindableAction::Settings => "settings",
        BindableAction::Cancel => "cancel",
    }
}

fn bindable_name(action: BindableAction) -> &'static str {
    match action {
        BindableAction::PushToTalk => "push to talk",
        BindableAction::Food => "food",
        BindableAction::Play => "play",
        BindableAction::Comfort => "comfort",
        BindableAction::Settings => "settings",
        BindableAction::Cancel => "cancel",
    }
}

fn add_action_strip(
    id: &str,
    actions: &[UiAction],
    y: i32,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    let item_width = 72;
    let width = item_width * i32::try_from(actions.len()).unwrap_or_default() + 4;
    let start = (LOGICAL_WIDTH - width) / 2;
    add_panel_chrome(
        &format!("mode/{id}-panel"),
        Rect {
            x: start,
            y: y - 13,
            w: width,
            h: 40,
        },
        25,
        rects,
    );
    text.push(label(
        &format!("mode/{id}/title"),
        match id {
            "food" => "Choose food",
            "toy" => "Choose a toy",
            _ => "Spend a moment",
        },
        start + 7,
        y - 9,
        29,
    ));
    for (index, action) in actions.iter().copied().enumerate() {
        let button_rect = Rect {
            x: start + 2 + i32::try_from(index).unwrap_or_default() * item_width,
            y: y + 3,
            w: item_width - 2,
            h: 21,
        };
        let label_text = action_label(action);
        let region_id = format!("action/{}", action_id(action));
        hits.push(hit(
            &region_id,
            None,
            action,
            button_rect,
            true,
            &label_text,
        ));
        add_button_chrome(&region_id, button_rect, true, false, 25, rects);
        text.push(label(
            &format!("{region_id}-label"),
            &label_text,
            button_rect.x + 5,
            button_rect.y + 6,
            29,
        ));
    }
}

fn add_keyboard(
    view: &ViewState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    const KEYS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ .,?!";
    add_panel_chrome(
        "keyboard/panel",
        Rect {
            x: 6,
            y: 3,
            w: 308,
            h: 124,
        },
        25,
        rects,
    );
    for (index, character) in KEYS.chars().enumerate() {
        let column = i32::try_from(index % 11).unwrap_or_default();
        let row = i32::try_from(index / 11).unwrap_or_default();
        let key = Rect {
            x: 12 + column * 27,
            y: 9 + row * 27,
            w: 25,
            h: 23,
        };
        let key_name = match character {
            ' ' => "space".to_owned(),
            '?' => "question".to_owned(),
            '!' => "exclamation".to_owned(),
            _ => character.to_ascii_lowercase().to_string(),
        };
        let shown = if character == ' ' {
            "SP".to_owned()
        } else {
            character.to_string()
        };
        add_key(
            &format!("keyboard/{key_name}"),
            &shown,
            UiAction::TypeCharacter(character.to_ascii_lowercase()),
            key,
            true,
            rects,
            text,
            hits,
        );
    }
    for (id, shown, action, x, enabled) in [
        ("keyboard/delete", "Delete", UiAction::Backspace, 12, true),
        ("keyboard/cancel", "Close", UiAction::CancelMode, 112, true),
        (
            "keyboard/send",
            "Send",
            UiAction::SubmitText,
            212,
            !view.text_buffer.trim().is_empty(),
        ),
    ] {
        add_key(
            id,
            shown,
            action,
            Rect {
                x,
                y: 101,
                w: 96,
                h: 23,
            },
            enabled,
            rects,
            text,
            hits,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn add_key(
    id: &str,
    shown: &str,
    action: UiAction,
    key: Rect,
    enabled: bool,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_button_chrome(id, key, enabled, false, 25, rects);
    text.push(label(
        &format!("{id}-label"),
        shown,
        key.x + 5,
        key.y + 6,
        29,
    ));
    hits.push(hit(id, None, action, key, enabled, shown));
}

fn add_speech(
    state: &WorldState,
    view: &ViewState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    let Some(speech) = view
        .speech
        .as_deref()
        .filter(|line| !line.trim().is_empty())
    else {
        return;
    };
    let (creature_x, _) = world_to_logical(state.creature.aquarium.position);
    let layout_text = view.speech_layout_text.as_deref().unwrap_or(speech);
    let font_size = TextRole::Dialogue.size(view.text_scale >= 2);
    let glyph_width = (font_size * 0.56).ceil() as i32;
    let natural_width =
        i32::try_from(layout_text.chars().count()).unwrap_or(300) * glyph_width + 18;
    let mut width = natural_width.clamp(84, 160);
    let line_height = (font_size * 1.2).ceil() as i32;
    let mut lines = speech_line_count(layout_text, ((width - 18) / glyph_width) as usize);
    // Long captions at the large accessibility size may use the full tank width.
    if lines * line_height > 78 {
        width = 300;
        lines = speech_line_count(layout_text, ((width - 18) / glyph_width) as usize);
    }
    let height = (lines * line_height + 16).clamp(27, 94);
    let panel_x = if creature_x >= LOGICAL_WIDTH / 2 {
        5
    } else {
        LOGICAL_WIDTH - 5 - width
    };
    add_panel_chrome(
        "speech/panel",
        Rect {
            x: panel_x,
            y: 5,
            w: width,
            h: height,
        },
        22,
        rects,
    );
    let reactions_y = 5 + height;
    let tail_y = reactions_y + 18;
    let tail_x = if creature_x >= LOGICAL_WIDTH / 2 {
        panel_x + width - 12
    } else {
        panel_x + 3
    };
    rects.push(rect(
        "speech/tail-edge",
        Rect {
            x: tail_x,
            y: tail_y,
            w: 9,
            h: 3,
        },
        UI_EDGE,
        24,
    ));
    rects.push(rect(
        "speech/tail",
        Rect {
            x: tail_x + 2,
            y: tail_y,
            w: 5,
            h: 5,
        },
        UI_PANEL,
        25,
    ));
    text.push(label("speech/text", speech, panel_x + 9, 13, 27));
    let reaction_width = (width - 16) / 3;
    for (index, reaction) in [Reaction::Laugh, Reaction::Disapprove, Reaction::Comfort]
        .into_iter()
        .enumerate()
    {
        let reaction_rect = Rect {
            x: panel_x + 6 + i32::try_from(index).unwrap_or_default() * (reaction_width + 2),
            y: reactions_y,
            w: reaction_width,
            h: 18,
        };
        hits.push(hit(
            &format!("reaction/{}", reaction_name(reaction)),
            Some(UiTarget::Reaction(reaction)),
            UiAction::React(reaction),
            reaction_rect,
            true,
            reaction_name(reaction),
        ));
        add_button_chrome(
            &format!("reaction/{}", reaction_name(reaction)),
            reaction_rect,
            true,
            false,
            26,
            rects,
        );
        add_reaction_icon(reaction, reaction_rect, 30, rects);
    }
}

/// Conservative word wrapping estimate in layout units; native text owns glyph shaping.
fn speech_line_count(text: &str, capacity: usize) -> i32 {
    let capacity = capacity.max(1);
    let mut lines = 0usize;
    for paragraph in text.split('\n') {
        let mut used = 0usize;
        lines += 1;
        for word in paragraph.split_whitespace() {
            let count = word.chars().count();
            if used > 0 && used + 1 + count > capacity {
                lines += 1;
                used = 0;
            } else if used > 0 {
                used += 1;
            }
            lines += count.saturating_sub(1) / capacity;
            used += count.saturating_sub(1) % capacity + 1;
        }
    }
    i32::try_from(lines).unwrap_or(i32::MAX)
}

fn add_status(
    view: &ViewState,
    now_ms: u64,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
) {
    let Some(message) = visible_status(view, now_ms) else {
        return;
    };
    let text_scale = view.text_scale.clamp(1, 2);
    let glyph_width = usize::from(text_scale) * 6;
    let fitted = head_fit(message, 170 / glyph_width * 2);
    add_panel_chrome(
        "status/background",
        Rect {
            x: 145,
            y: 134,
            w: 167,
            h: 16,
        },
        35,
        rects,
    );
    text.push(label("status/message", &fitted, 149, 136, 39));
}

fn visible_status(view: &ViewState, now_ms: u64) -> Option<&str> {
    view.status_message
        .as_deref()
        .or(match view.microphone_state {
            MicrophoneState::Listening => Some("Listening... release to send."),
            MicrophoneState::Recognizing => Some("Working out what you said..."),
            MicrophoneState::Unavailable => Some("Microphone unavailable. Text still works."),
            MicrophoneState::Error => Some("Speech input failed. Text still works."),
            MicrophoneState::Disabled | MicrophoneState::Idle => None,
        })
        .or(view.transcript_status.as_deref())
        .or_else(|| {
            matches!(
                view.active_cue(now_ms),
                Some(PresentationCueKind::AquariumFull)
            )
            .then_some("Aquarium full. Clean up old food first.")
        })
}

fn add_hover_and_focus(
    view: &ViewState,
    hits: &[HitRegion],
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
) {
    for (id, color, command_id) in [
        (
            view.hovered_region
                .as_deref()
                .filter(|hovered| Some(*hovered) != view.focused_region.as_deref()),
            [142, 231, 202, 255],
            "ui/hover",
        ),
        (
            view.focused_region.as_deref(),
            [255, 225, 133, 255],
            "ui/focus",
        ),
    ] {
        let Some(id) = id else { continue };
        let Some(hit_region) = hits
            .iter()
            .find(|hit_region| hit_region.id == id && hit_region.enabled)
        else {
            continue;
        };
        if matches!(hit_region.shape, HitShape::World(_)) {
            continue;
        }
        rects.push(RectCommand {
            id: command_id.to_owned(),
            rect: grow(hit_region.rect, 1),
            color,
            layer: 34,
            outline: true,
        });
    }

    let described = view.hovered_region.as_deref().or_else(|| {
        view.controller_active
            .then_some(view.focused_region.as_deref())
            .flatten()
    });
    let Some(hit_region) = described.and_then(|id| {
        hits.iter()
            .find(|hit_region| hit_region.id == id && hit_region.enabled)
    }) else {
        return;
    };
    // Text-labeled controls already explain themselves. Repeating their label in a tooltip
    // obscures neighboring settings and fields while the player is using them.
    if !matches!(hit_region.shape, HitShape::World(_)) && !hit_region.id.starts_with("reaction/") {
        return;
    }
    let label_width =
        (i32::try_from(hit_region.label.chars().count()).unwrap_or(12) * 6 + 10).clamp(28, 140);
    let x = hit_region.rect.x.clamp(2, LOGICAL_WIDTH - label_width - 2);
    let y = (hit_region.rect.y - 16).clamp(2, AQUARIUM_BOTTOM - 16);
    add_panel_chrome(
        "ui/hover-label-background",
        Rect {
            x,
            y,
            w: label_width,
            h: 14,
        },
        34,
        rects,
    );
    text.push(label("ui/hover-label", &hit_region.label, x + 5, y + 4, 38));
}

#[must_use]
pub fn creature_summary(state: &WorldState) -> CreatureSummary {
    let mood = state.mood();
    CreatureSummary {
        name: state.creature.name.clone(),
        mood,
        mood_label: mood_name(mood).to_owned(),
        behavior: behavior_name(state).to_owned(),
        stage: format!("day {}", state.active_day()),
        discovered_fact: discovered_fact(state),
    }
}

fn discovered_fact(state: &WorldState) -> Option<String> {
    let (food, preference) = state
        .creature
        .preferences
        .iter()
        .filter(|(_, preference)| preference.abs() >= 0.35)
        .max_by(|left, right| left.1.abs().total_cmp(&right.1.abs()))?;
    Some(if *preference > 0.0 {
        format!("likes {}", food_name(*food))
    } else {
        format!("hates {}", food_name(*food))
    })
}

fn creature_pose(state: &WorldState) -> &'static str {
    if let Some(activity) = state.creature.private_life.active.as_ref() {
        return private_life_pose(activity.kind, activity.recipe, activity.phase);
    }
    if let Some(action) = state.creature.aquarium.action.as_ref() {
        return match action.phase {
            ActionPhase::Notice | ActionPhase::Gaze | ActionPhase::Inspect => "inspect",
            ActionPhase::Brake | ActionPhase::Turn => "turn",
            ActionPhase::Approach => "swim",
            ActionPhase::Act => match state.creature.current_intention {
                Intention::Eat | Intention::RejectFood => "eat",
                Intention::Sleep => "sleep",
                Intention::Play => "play",
                _ => "react",
            },
            ActionPhase::Recover => "recover",
        };
    }
    match state.creature.current_intention {
        Intention::Sleep => "sleep",
        Intention::Eat | Intention::RejectFood => "eat",
        Intention::Play => "play",
        Intention::ApproachPlayer | Intention::SeekComfort => "swim",
        _ => match state.creature.aquarium.steering {
            SteeringMode::Approach | SteeringMode::Flee | SteeringMode::Orbit => "swim",
            SteeringMode::Turn | SteeringMode::Brake => "turn",
            SteeringMode::Inspect => "inspect",
            SteeringMode::Settle => "settle",
            SteeringMode::Hover | SteeringMode::Drift => "hover",
        },
    }
}

const fn private_life_pose(
    kind: PrivateLifeKind,
    recipe: ActivityRecipe,
    phase: ActivityPhase,
) -> &'static str {
    match phase {
        ActivityPhase::Notice => "inspect",
        ActivityPhase::Approach => "swim",
        ActivityPhase::Recover => "recover",
        ActivityPhase::Settle => "sleep",
        ActivityPhase::Interrupted => "react",
        ActivityPhase::Act => match (kind, recipe) {
            (PrivateLifeKind::ToyPlay(ToyId::Ball), ActivityRecipe::BallNudge) => "play",
            (PrivateLifeKind::ToyPlay(ToyId::Bell), ActivityRecipe::BellStrike) => "turn",
            (PrivateLifeKind::ToyPlay(ToyId::Sock), ActivityRecipe::SockTug) => "play",
            (PrivateLifeKind::CaveSettle, ActivityRecipe::CaveShelter) => "sleep",
            (PrivateLifeKind::PlantInspect, ActivityRecipe::PlantOrbit) => "swim",
            (PrivateLifeKind::BottomForage, ActivityRecipe::BottomForage) => "inspect",
            (PrivateLifeKind::OpenWaterDrift, ActivityRecipe::OpenWaterDrift) => "hover",
            // A save can be loaded from a newer version. Keep its body calm while the
            // authoritative type remains visible in diagnostics, rather than guessing from intent.
            _ => "hover",
        },
    }
}

fn behavior_name(state: &WorldState) -> &'static str {
    if let Some(activity) = state.creature.private_life.active.as_ref() {
        return private_life_behavior_name(activity.kind, activity.recipe, activity.phase);
    }
    if let Some(beat) = state.creature.relationship_expression.active.as_ref()
        && let RelationshipMotifKey::SharedToy(toy) = beat.motif
    {
        return match (toy, beat.phase) {
            (ToyId::Ball, RelationshipBeatPhase::Notice) => "noticing the ball",
            (ToyId::Ball, RelationshipBeatPhase::Anticipate) => "swimming toward the ball",
            (ToyId::Ball, RelationshipBeatPhase::Act) => "watching the ball",
            (ToyId::Ball, RelationshipBeatPhase::Recover) => "settling after the ball",
            (ToyId::Bell, RelationshipBeatPhase::Notice) => "noticing the bell",
            (ToyId::Bell, RelationshipBeatPhase::Anticipate) => "swimming toward the bell",
            (ToyId::Bell, RelationshipBeatPhase::Act) => "watching the bell",
            (ToyId::Bell, RelationshipBeatPhase::Recover) => "settling after the bell",
            (ToyId::Sock, RelationshipBeatPhase::Notice) => "noticing the sock",
            (ToyId::Sock, RelationshipBeatPhase::Anticipate) => "swimming toward the sock",
            (ToyId::Sock, RelationshipBeatPhase::Act) => "watching the sock",
            (ToyId::Sock, RelationshipBeatPhase::Recover) => "settling after the sock",
        };
    }
    if let Some(action) = state.creature.aquarium.action.as_ref() {
        return match action.phase {
            ActionPhase::Notice => "noticed something",
            ActionPhase::Brake => "stopping",
            ActionPhase::Gaze => "watching",
            ActionPhase::Turn => "turning",
            ActionPhase::Approach => "swimming over",
            ActionPhase::Inspect => "inspecting",
            ActionPhase::Act => match state.creature.current_intention {
                Intention::Eat => "eating",
                Intention::RejectFood => "rejecting food",
                Intention::Play => "playing",
                _ => "doing something",
            },
            ActionPhase::Recover => "settling down",
        };
    }
    match state.creature.current_intention {
        Intention::Sleep => "sleeping",
        Intention::Play => match state.creature.aquarium.steering {
            SteeringMode::Approach => "swimming to a toy",
            _ => "playing",
        },
        Intention::SeekComfort => "seeking comfort",
        Intention::ApproachPlayer => "watching you",
        Intention::RefuseAndStare => "staring",
        Intention::ShowAffection => "staying close",
        _ => match state.creature.aquarium.steering {
            SteeringMode::Drift => "drifting",
            SteeringMode::Flee => "avoiding you",
            SteeringMode::Orbit => "circling",
            SteeringMode::Approach => "swimming over",
            SteeringMode::Inspect => "investigating",
            SteeringMode::Settle => "settling",
            _ => "hovering",
        },
    }
}

const fn private_life_behavior_name(
    kind: PrivateLifeKind,
    recipe: ActivityRecipe,
    phase: ActivityPhase,
) -> &'static str {
    match phase {
        ActivityPhase::Notice => "noticing something to do",
        ActivityPhase::Approach => "heading somewhere on its own",
        ActivityPhase::Recover => "finishing up",
        ActivityPhase::Settle => "settling in the cave",
        ActivityPhase::Interrupted => "changing course",
        ActivityPhase::Act => match (kind, recipe) {
            (PrivateLifeKind::ToyPlay(ToyId::Ball), ActivityRecipe::BallNudge) => {
                "nudging the ball"
            }
            (PrivateLifeKind::ToyPlay(ToyId::Bell), ActivityRecipe::BellStrike) => {
                "striking the bell"
            }
            (PrivateLifeKind::ToyPlay(ToyId::Sock), ActivityRecipe::SockTug) => "tugging the sock",
            (PrivateLifeKind::CaveSettle, ActivityRecipe::CaveShelter) => "resting in the cave",
            (PrivateLifeKind::PlantInspect, ActivityRecipe::PlantOrbit) => "circling the plant",
            (PrivateLifeKind::BottomForage, ActivityRecipe::BottomForage) => "foraging in the sand",
            (PrivateLifeKind::OpenWaterDrift, ActivityRecipe::OpenWaterDrift) => {
                "drifting through open water"
            }
            _ => "following a private routine",
        },
    }
}

fn cue_for_event(event: &GameEvent) -> Option<(SemanticOwner, PresentationCueKind, u64)> {
    let ordinary = SemanticOwner::Ordinary;
    let direct = SemanticOwner::DirectOutcome;
    match event {
        GameEvent::FoodDropped { .. } => Some((ordinary, PresentationCueKind::Notice, 700)),
        GameEvent::FoodConsumed(_) => Some((direct, PresentationCueKind::Crumbs, 900)),
        GameEvent::FoodRejected(_) => Some((direct, PresentationCueKind::Spit, 1_100)),
        GameEvent::ToyPlayed { .. } => Some((direct, PresentationCueKind::Delight, 900)),
        GameEvent::ToyRejected { .. } => Some((direct, PresentationCueKind::Suspicion, 1_100)),
        GameEvent::FoodDropRejected(FoodDropRejectionReason::AquariumFull) => {
            Some((direct, PresentationCueKind::AquariumFull, 1_300))
        }
        GameEvent::Comforted => Some((direct, PresentationCueKind::Comfort, 1_200)),
        GameEvent::SleepStarted => Some((direct, PresentationCueKind::Sleep, 1_000)),
        GameEvent::FoodSettled(_) => Some((ordinary, PresentationCueKind::SandPuff, 700)),
        GameEvent::NonverbalAct(act) => Some((direct, cue_for_nonverbal(*act), 1_100)),
        GameEvent::ActionPhaseChanged {
            to: ActionPhase::Approach,
            ..
        } => Some((ordinary, PresentationCueKind::Wake, 650)),
        GameEvent::PrivateLifeStarted { activity_id, .. } => Some((
            SemanticOwner::PrivateLife(*activity_id),
            PresentationCueKind::Notice,
            800,
        )),
        // The current authoritative activity carries the recipe needed to render this phase. The
        // event remains useful for cancellation and traceability, but does not degrade that exact
        // identity into a generic transition cue.
        GameEvent::PrivateLifePhaseChanged { .. } => None,
        GameEvent::ToyObjectResponded {
            activity_id,
            response,
            ..
        } => Some((
            SemanticOwner::PrivateLife(*activity_id),
            toy_response_cue(*response),
            private_life_phase_duration_for_response(*response),
        )),
        GameEvent::SpeechPerceived(SpeechAttention::Glanced | SpeechAttention::Attended) => {
            Some((ordinary, PresentationCueKind::Notice, 900))
        }
        GameEvent::TalkAccepted { .. } => Some((ordinary, PresentationCueKind::Notice, 700)),
        GameEvent::ActionRelationshipStarted {
            action_id, motif, ..
        } => Some((
            SemanticOwner::ActionRelationship(*action_id),
            match motif {
                RelationshipMotifKey::TrustedFood(_) => PresentationCueKind::PositiveNotice,
                RelationshipMotifKey::FoodGrudge(_) => PresentationCueKind::FoodSuspicion,
                _ => relationship_cue(*motif, RelationshipExpressionKind::Notice),
            },
            1_400,
        )),
        GameEvent::RelationshipBeatStarted {
            motif, expression, ..
        } => Some((
            SemanticOwner::StandaloneRelationship(*motif),
            relationship_cue(*motif, *expression),
            relationship_expression_duration(*motif, *expression),
        )),
        GameEvent::RelationshipBeatPhaseChanged { motif, to, .. } => Some((
            SemanticOwner::StandaloneRelationship(*motif),
            relationship_phase_cue(*motif, *to),
            relationship_phase_duration(*motif, *to),
        )),
        _ => None,
    }
}

fn relationship_cue(
    motif: RelationshipMotifKey,
    expression: RelationshipExpressionKind,
) -> PresentationCueKind {
    match performance_recipe_for(motif, expression) {
        RelationshipPerformanceRecipe::SharedBall(expression) => match expression {
            RelationshipExpressionKind::Notice | RelationshipExpressionKind::Anticipate => {
                PresentationCueKind::Notice
            }
            RelationshipExpressionKind::Seek
            | RelationshipExpressionKind::Ritual
            | RelationshipExpressionKind::Recognize
            | RelationshipExpressionKind::Welcome => PresentationCueKind::BallNudge,
        },
        RelationshipPerformanceRecipe::SharedBell(expression) => match expression {
            RelationshipExpressionKind::Notice | RelationshipExpressionKind::Anticipate => {
                PresentationCueKind::Notice
            }
            _ => PresentationCueKind::BellStrike,
        },
        RelationshipPerformanceRecipe::SharedSock(expression) => match expression {
            RelationshipExpressionKind::Notice | RelationshipExpressionKind::Anticipate => {
                PresentationCueKind::Notice
            }
            _ => PresentationCueKind::SockTug,
        },
        RelationshipPerformanceRecipe::ComfortAttention(RelationshipExpressionKind::Notice) => {
            PresentationCueKind::Notice
        }
        RelationshipPerformanceRecipe::ComfortAttention(_) => PresentationCueKind::Comfort,
        RelationshipPerformanceRecipe::TrustedFoodReceipt(_) => PresentationCueKind::Notice,
        RelationshipPerformanceRecipe::FoodGrudgeReceipt(RelationshipExpressionKind::Notice) => {
            PresentationCueKind::Notice
        }
        RelationshipPerformanceRecipe::FoodGrudgeReceipt(_) => PresentationCueKind::Spit,
        RelationshipPerformanceRecipe::PlayerReturn(expression)
            if expression != RelationshipExpressionKind::Welcome =>
        {
            PresentationCueKind::Notice
        }
        RelationshipPerformanceRecipe::PlayerReturn(_) => PresentationCueKind::Affection,
        RelationshipPerformanceRecipe::FamiliarCave(_)
        | RelationshipPerformanceRecipe::FamiliarPlant(_)
        | RelationshipPerformanceRecipe::FamiliarBottom(_)
        | RelationshipPerformanceRecipe::FamiliarPlayer(_)
        | RelationshipPerformanceRecipe::FamiliarToy(_)
        | RelationshipPerformanceRecipe::FamiliarFood(_)
        | RelationshipPerformanceRecipe::FamiliarPosition(_) => PresentationCueKind::PlaceNotice,
    }
}

fn relationship_phase_cue(
    motif: RelationshipMotifKey,
    phase: RelationshipBeatPhase,
) -> PresentationCueKind {
    match phase {
        RelationshipBeatPhase::Notice | RelationshipBeatPhase::Anticipate => {
            relationship_cue(motif, RelationshipExpressionKind::Notice)
        }
        RelationshipBeatPhase::Act => match motif {
            RelationshipMotifKey::SharedToy(_) => PresentationCueKind::Notice,
            RelationshipMotifKey::FamiliarPlace(destination) => match destination {
                SemanticDestination::Cave => PresentationCueKind::CaveShelter,
                SemanticDestination::Plant => PresentationCueKind::PlantOrbit,
                SemanticDestination::Bottom => PresentationCueKind::BottomForage,
                SemanticDestination::Player
                | SemanticDestination::Toy(_)
                | SemanticDestination::Food(_)
                | SemanticDestination::Position(_) => PresentationCueKind::PlaceNotice,
            },
            _ => relationship_cue(motif, RelationshipExpressionKind::Ritual),
        },
        RelationshipBeatPhase::Recover => PresentationCueKind::Wake,
    }
}

const fn relationship_phase_duration(
    motif: RelationshipMotifKey,
    phase: RelationshipBeatPhase,
) -> u64 {
    match (motif, phase) {
        (RelationshipMotifKey::SharedToy(ToyId::Ball), RelationshipBeatPhase::Notice) => 700,
        (RelationshipMotifKey::SharedToy(ToyId::Ball), RelationshipBeatPhase::Anticipate) => 1_200,
        (RelationshipMotifKey::SharedToy(ToyId::Bell), RelationshipBeatPhase::Act) => 900,
        (RelationshipMotifKey::SharedToy(ToyId::Sock), RelationshipBeatPhase::Act) => 2_600,
        (
            RelationshipMotifKey::FamiliarPlace(SemanticDestination::Cave),
            RelationshipBeatPhase::Act,
        ) => 4_000,
        (
            RelationshipMotifKey::FamiliarPlace(SemanticDestination::Plant),
            RelationshipBeatPhase::Anticipate,
        ) => 2_800,
        (RelationshipMotifKey::ComfortRitual, RelationshipBeatPhase::Act) => 3_200,
        (RelationshipMotifKey::PlayerReturns, RelationshipBeatPhase::Notice) => 1_400,
        (RelationshipMotifKey::TrustedFood(_), RelationshipBeatPhase::Recover) => 1_100,
        (RelationshipMotifKey::FoodGrudge(_), RelationshipBeatPhase::Act) => 1_500,
        (_, RelationshipBeatPhase::Notice) => 1_000,
        (_, RelationshipBeatPhase::Anticipate) => 2_000,
        (_, RelationshipBeatPhase::Act) => 2_000,
        (_, RelationshipBeatPhase::Recover) => 1_800,
    }
}

const fn relationship_expression_duration(
    motif: RelationshipMotifKey,
    expression: RelationshipExpressionKind,
) -> u64 {
    let phase = match expression {
        RelationshipExpressionKind::Notice => RelationshipBeatPhase::Notice,
        RelationshipExpressionKind::Anticipate | RelationshipExpressionKind::Seek => {
            RelationshipBeatPhase::Anticipate
        }
        RelationshipExpressionKind::Ritual
        | RelationshipExpressionKind::Recognize
        | RelationshipExpressionKind::Welcome => RelationshipBeatPhase::Act,
    };
    relationship_phase_duration(motif, phase)
}

const fn private_life_recipe_cue(recipe: ActivityRecipe) -> PresentationCueKind {
    match recipe {
        ActivityRecipe::BallNudge => PresentationCueKind::BallNudge,
        ActivityRecipe::BellStrike => PresentationCueKind::BellStrike,
        ActivityRecipe::SockTug => PresentationCueKind::SockTug,
        ActivityRecipe::CaveShelter => PresentationCueKind::CaveShelter,
        ActivityRecipe::PlantOrbit => PresentationCueKind::PlantOrbit,
        ActivityRecipe::BottomForage => PresentationCueKind::BottomForage,
        ActivityRecipe::OpenWaterDrift => PresentationCueKind::OpenWaterDrift,
    }
}

const fn toy_response_cue(response: ToyResponse) -> PresentationCueKind {
    match response {
        ToyResponse::None => PresentationCueKind::Notice,
        ToyResponse::BallNudged => PresentationCueKind::BallNudge,
        ToyResponse::BellStruck => PresentationCueKind::BellStrike,
        ToyResponse::SockTugged => PresentationCueKind::SockTug,
    }
}

const fn private_life_phase_duration_for_response(response: ToyResponse) -> u64 {
    match response {
        ToyResponse::None => 450,
        ToyResponse::BallNudged => 900,
        ToyResponse::BellStruck => 550,
        ToyResponse::SockTugged => 1_700,
    }
}

fn cue_for_nonverbal(act: NonverbalAct) -> PresentationCueKind {
    match act {
        NonverbalAct::PushFoodAway(_) | NonverbalAct::RefuseToEat => PresentationCueKind::Spit,
        NonverbalAct::TakeToyAway(_) => PresentationCueKind::Suspicion,
        NonverbalAct::UndoTidy => PresentationCueKind::SandPuff,
        NonverbalAct::RefuseAndStare => PresentationCueKind::Recoil,
        NonverbalAct::LeanAgainstPlayer => PresentationCueKind::Affection,
    }
}

fn action_relationship_cue(
    state: &WorldState,
) -> Option<(SemanticOwner, PresentationCueKind, u64)> {
    let action = state.creature.aquarium.action.as_ref()?;
    let context = action.relationship.as_ref()?;
    if matches!(action.phase, ActionPhase::Act | ActionPhase::Recover) {
        return None;
    }
    let kind = match context.motif {
        RelationshipMotifKey::TrustedFood(_) => PresentationCueKind::PositiveNotice,
        RelationshipMotifKey::FoodGrudge(_) => PresentationCueKind::FoodSuspicion,
        _ => return None,
    };
    Some((
        SemanticOwner::ActionRelationship(action.action_id),
        kind,
        action.elapsed_ms,
    ))
}

fn private_life_cue(state: &WorldState) -> Option<(SemanticOwner, PresentationCueKind, u64)> {
    let activity = state.creature.private_life.active.as_ref()?;
    let kind = match activity.phase {
        ActivityPhase::Notice => PresentationCueKind::Notice,
        ActivityPhase::Approach => PresentationCueKind::Wake,
        ActivityPhase::Act => match activity.kind {
            PrivateLifeKind::ToyPlay(_) if !activity.payoff_reached => PresentationCueKind::Notice,
            _ => private_life_recipe_cue(activity.recipe),
        },
        ActivityPhase::Recover if activity.payoff_reached => {
            private_life_recipe_cue(activity.recipe)
        }
        ActivityPhase::Recover => PresentationCueKind::Wake,
        ActivityPhase::Settle => private_life_recipe_cue(activity.recipe),
        ActivityPhase::Interrupted => return None,
    };
    Some((
        SemanticOwner::PrivateLife(activity.id),
        kind,
        state
            .elapsed_ms
            .saturating_sub(activity.phase_started_at_ms),
    ))
}

#[cfg(test)]
fn effective_cue_timing(
    state: &WorldState,
    view: &ViewState,
) -> Option<(PresentationCueKind, u64)> {
    creature_scene(state, view)
        .expression
        .map(|expression| (expression.cue, expression.elapsed_ms))
}

fn rect(id: &str, dimensions: Rect, color: [u8; 4], layer: i16) -> RectCommand {
    RectCommand {
        id: id.to_owned(),
        rect: dimensions,
        color,
        layer,
        outline: false,
    }
}

fn icon(id: &str, x: i32, y: i32, layer: i16) -> IconCommand {
    let kind = match id {
        "ui/button-microphone" => IconKind::Microphone,
        "ui/button-food" => IconKind::Food,
        "ui/button-settings" => IconKind::Settings,
        "ui/button-send" => IconKind::Send,
        _ => unreachable!("unknown geometric icon"),
    };
    IconCommand {
        id: id.to_owned(),
        kind,
        x,
        y,
        layer,
    }
}

fn add_panel_chrome(id: &str, dimensions: Rect, layer: i16, rects: &mut Vec<RectCommand>) {
    rects.push(rect(
        &format!("{id}-shadow"),
        Rect {
            x: dimensions.x + 2,
            y: dimensions.y + 2,
            ..dimensions
        },
        UI_SHADOW,
        layer - 2,
    ));
    rects.push(rect(&format!("{id}-edge"), dimensions, UI_EDGE, layer - 1));
    rects.push(rect(
        id,
        Rect {
            x: dimensions.x + 1,
            y: dimensions.y + 1,
            w: dimensions.w - 2,
            h: dimensions.h - 2,
        },
        UI_PANEL,
        layer,
    ));
    rects.push(rect(
        &format!("{id}-glint"),
        Rect {
            x: dimensions.x + 2,
            y: dimensions.y + 2,
            w: dimensions.w - 4,
            h: 1,
        },
        UI_EDGE_LIT,
        layer + 1,
    ));
}

fn add_reaction_icon(
    reaction: Reaction,
    dimensions: Rect,
    layer: i16,
    rects: &mut Vec<RectCommand>,
) {
    let center_x = dimensions.x + dimensions.w / 2;
    let y = dimensions.y + 6;
    let color = [235, 207, 148, 255];
    match reaction {
        Reaction::Laugh => {
            rects.push(rect(
                "reaction/laugh-left-eye",
                Rect {
                    x: center_x - 7,
                    y,
                    w: 3,
                    h: 2,
                },
                color,
                layer,
            ));
            rects.push(rect(
                "reaction/laugh-right-eye",
                Rect {
                    x: center_x + 4,
                    y,
                    w: 3,
                    h: 2,
                },
                color,
                layer,
            ));
            rects.push(rect(
                "reaction/laugh-mouth",
                Rect {
                    x: center_x - 6,
                    y: y + 4,
                    w: 12,
                    h: 2,
                },
                color,
                layer,
            ));
        }
        Reaction::Disapprove => {
            rects.push(rect(
                "reaction/no-left-eye",
                Rect {
                    x: center_x - 6,
                    y,
                    w: 3,
                    h: 3,
                },
                UI_CORAL,
                layer,
            ));
            rects.push(rect(
                "reaction/no-right-eye",
                Rect {
                    x: center_x + 3,
                    y,
                    w: 3,
                    h: 3,
                },
                UI_CORAL,
                layer,
            ));
            rects.push(rect(
                "reaction/no-mouth",
                Rect {
                    x: center_x - 6,
                    y: y + 6,
                    w: 12,
                    h: 2,
                },
                UI_CORAL,
                layer,
            ));
        }
        Reaction::Comfort => {
            for (id, icon) in [
                (
                    "top",
                    Rect {
                        x: center_x - 5,
                        y,
                        w: 4,
                        h: 3,
                    },
                ),
                (
                    "top-right",
                    Rect {
                        x: center_x + 1,
                        y,
                        w: 4,
                        h: 3,
                    },
                ),
                (
                    "middle",
                    Rect {
                        x: center_x - 5,
                        y: y + 2,
                        w: 10,
                        h: 3,
                    },
                ),
                (
                    "low",
                    Rect {
                        x: center_x - 3,
                        y: y + 5,
                        w: 6,
                        h: 2,
                    },
                ),
                (
                    "tip",
                    Rect {
                        x: center_x - 1,
                        y: y + 7,
                        w: 2,
                        h: 1,
                    },
                ),
            ] {
                rects.push(rect(&format!("reaction/heart-{id}"), icon, UI_CORAL, layer));
            }
        }
    }
}

fn add_inset(id: &str, dimensions: Rect, layer: i16, rects: &mut Vec<RectCommand>) {
    rects.push(rect(
        &format!("{id}-shadow"),
        Rect {
            x: dimensions.x + 1,
            y: dimensions.y + 1,
            ..dimensions
        },
        UI_SHADOW,
        layer,
    ));
    rects.push(rect(&format!("{id}-edge"), dimensions, UI_EDGE, layer + 1));
    rects.push(rect(
        &format!("{id}-background"),
        Rect {
            x: dimensions.x + 1,
            y: dimensions.y + 1,
            w: dimensions.w - 2,
            h: dimensions.h - 2,
        },
        UI_PANEL_INSET,
        layer + 2,
    ));
    rects.push(rect(
        &format!("{id}-inner-glint"),
        Rect {
            x: dimensions.x + 2,
            y: dimensions.y + 2,
            w: dimensions.w - 4,
            h: 1,
        },
        [50, 103, 105, 180],
        layer + 3,
    ));
}

fn add_button_chrome(
    id: &str,
    dimensions: Rect,
    enabled: bool,
    active: bool,
    layer: i16,
    rects: &mut Vec<RectCommand>,
) {
    rects.push(rect(
        &format!("{id}-shadow"),
        Rect {
            x: dimensions.x + 1,
            y: dimensions.y + 1,
            ..dimensions
        },
        UI_SHADOW,
        layer,
    ));
    rects.push(rect(
        &format!("{id}-edge"),
        dimensions,
        if active {
            UI_CORAL
        } else if enabled {
            UI_EDGE
        } else {
            UI_BUTTON_DISABLED
        },
        layer + 1,
    ));
    rects.push(rect(
        &format!("{id}-background"),
        Rect {
            x: dimensions.x + 1,
            y: dimensions.y + 1,
            w: dimensions.w - 2,
            h: dimensions.h - 2,
        },
        if enabled {
            UI_BUTTON
        } else {
            UI_BUTTON_DISABLED
        },
        layer + 2,
    ));
    rects.push(rect(
        &format!("{id}-glint"),
        Rect {
            x: dimensions.x + 2,
            y: dimensions.y + 2,
            w: dimensions.w - 4,
            h: 1,
        },
        if !enabled {
            [41, 65, 72, 255]
        } else if active {
            [239, 159, 126, 230]
        } else {
            UI_EDGE_LIT
        },
        layer + 3,
    ));
}

const fn mood_color(mood: Mood) -> [u8; 4] {
    match mood {
        Mood::Content => [123, 207, 177, 255],
        Mood::Curious => [115, 191, 219, 255],
        Mood::Hungry => [229, 168, 91, 255],
        Mood::Sleepy => [153, 139, 195, 255],
        Mood::Lonely => [113, 143, 179, 255],
        Mood::Resentful => [213, 103, 91, 255],
    }
}

fn label(id: &str, value: &str, x: i32, y: i32, layer: i16) -> TextCommand {
    TextCommand {
        bounds: None,
        muted: false,
        role: TextRole::Body,
        id: id.to_owned(),
        text: value.to_owned(),
        x,
        y,
        layer,
        scale: 1,
    }
}

fn hit(
    id: &str,
    target: Option<UiTarget>,
    action: UiAction,
    dimensions: Rect,
    enabled: bool,
    label: &str,
) -> HitRegion {
    HitRegion {
        id: id.to_owned(),
        target,
        action,
        rect: dimensions,
        enabled,
        label: label.to_owned(),
        cursor: CursorKind::Pointer,
        shape: HitShape::Rect,
    }
}

fn grow(rect: Rect, amount: i32) -> Rect {
    Rect {
        x: rect.x - amount,
        y: rect.y - amount,
        w: rect.w + amount * 2,
        h: rect.h + amount * 2,
    }
}

fn food_name(food: FoodId) -> &'static str {
    match food {
        FoodId::Berry => "berry",
        FoodId::Mushroom => "mushroom",
        FoodId::Pellet => "pellet",
    }
}

fn toy_name(toy: ToyId) -> &'static str {
    match toy {
        ToyId::Ball => "ball",
        ToyId::Bell => "bell",
        ToyId::Sock => "sock",
    }
}

fn mood_name(mood: Mood) -> &'static str {
    match mood {
        Mood::Content => "content",
        Mood::Curious => "curious",
        Mood::Hungry => "hungry",
        Mood::Sleepy => "sleepy",
        Mood::Lonely => "lonely",
        Mood::Resentful => "resentful",
    }
}

fn reaction_name(reaction: Reaction) -> &'static str {
    match reaction {
        Reaction::Laugh => "laugh",
        Reaction::Disapprove => "disapprove",
        Reaction::Comfort => "comfort",
    }
}

fn action_id(action: UiAction) -> String {
    match action {
        UiAction::SelectFood(food) => format!("select-{}", food_name(food)),
        UiAction::Play(toy) => format!("play-{}", toy_name(toy)),
        UiAction::OpenToyChoice => "choose-toy".to_owned(),
        UiAction::Comfort => "comfort".to_owned(),
        UiAction::Inspect => "inspect".to_owned(),
        UiAction::Rename => "rename".to_owned(),
        UiAction::Talk => "talk".to_owned(),
        _ => "action".to_owned(),
    }
}

fn action_label(action: UiAction) -> String {
    match action {
        UiAction::SelectFood(food) => food_name(food).to_owned(),
        UiAction::Play(toy) => format!("play {}", toy_name(toy)),
        UiAction::OpenToyChoice => "play".to_owned(),
        UiAction::Comfort => "comfort".to_owned(),
        UiAction::Inspect => "inspect".to_owned(),
        UiAction::Rename => "rename".to_owned(),
        UiAction::Talk => "talk".to_owned(),
        _ => "action".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use beastie_core::{
        ActionRelationshipContext, ActionTimeline, ActivityPurpose, ActivitySelectionEvidence,
        PrivateLifeActivity, RelationshipSubject, RelationshipTrigger, SemanticDestination,
    };

    fn played_cues(plan: AudioPlan) -> Vec<AudioCue> {
        plan.events
            .into_iter()
            .filter_map(|command| match command {
                AudioCommand::Play { cue, .. } => Some(cue),
                _ => None,
            })
            .collect()
    }

    fn private_activity(
        id: NonZeroU64,
        kind: PrivateLifeKind,
        recipe: ActivityRecipe,
        phase: ActivityPhase,
    ) -> PrivateLifeActivity {
        PrivateLifeActivity {
            id,
            kind,
            subject: Some(kind.subject()),
            purpose: ActivityPurpose::Autonomous,
            recipe,
            phase,
            selected_at_ms: 0,
            phase_started_at_ms: 0,
            selected_from: ActivitySelectionEvidence {
                need_pressure: 0,
                trait_bias: 0,
                preference: 0,
                routine_hour: None,
                relationship_evidence: Vec::new(),
                excluded_families: Vec::new(),
                excluded_subjects: Vec::new(),
                excluded_recipes: Vec::new(),
                urgency_overrode_repetition: false,
            },
            payoff_reached: false,
        }
    }

    #[test]
    fn short_large_caption_uses_the_actual_type_metrics() {
        let world = WorldState::new(42, "Mop");
        let scene = plan(
            &world,
            &ViewState {
                speech: Some("hm. rude giant.".into()),
                text_scale: 2,
                ..ViewState::default()
            },
        )
        .0;
        let panel = scene.rects.iter().find(|r| r.id == "speech/panel").unwrap();
        assert!(
            panel.rect.h <= 30,
            "short speech should not reserve several empty rows"
        );
        assert!(panel.rect.w < 130);
    }

    #[test]
    fn labeled_settings_hover_keeps_neighboring_values_visible() {
        let world = WorldState::new(42, "Mop");
        let scene = plan(
            &world,
            &ViewState {
                mode: UiMode::Settings,
                hovered_region: Some("settings/motion".into()),
                text_scale: 2,
                ..ViewState::default()
            },
        )
        .0;
        assert!(scene.rects.iter().any(|r| r.id == "ui/hover"));
        assert!(scene.text.iter().all(|t| t.id != "ui/hover-label"));
        let reaction = plan(
            &world,
            &ViewState {
                speech: Some("hello".into()),
                hovered_region: Some("reaction/comfort".into()),
                ..ViewState::default()
            },
        )
        .0;
        assert!(reaction.text.iter().any(|t| t.id == "ui/hover-label"));
    }

    #[test]
    fn coordinate_projection_matches_the_safe_3d_arena_and_clamps() {
        assert_eq!(world_to_logical(NormalizedPosition::new(0, 0)), (28, 22));
        assert_eq!(
            world_to_logical(NormalizedPosition::new(10_000, 10_000)),
            (292, 113)
        );
        assert_eq!(
            world_to_logical(NormalizedPosition::new(-1, 20_000)),
            (28, 113)
        );
        let center = logical_to_world(160, 71);
        let projected = world_to_logical(center);
        assert!((projected.0 - 160).abs() <= 1);
        assert!((projected.1 - 71).abs() <= 1);
    }

    #[test]
    fn gaze_tracks_live_cursor_food_and_moving_toy_coordinates() {
        let mut state = WorldState::new(7, "Mop");
        let view = ViewState::default();
        let cursor = NormalizedPosition::new(8300, 1700);
        state.aquarium.cursor = Some(cursor);
        state.creature.aquarium.gaze = GazeTarget::Cursor;
        assert_eq!(plan(&state, &view).0.creature.gaze_position, Some(cursor));
        let food_position = NormalizedPosition::new(2200, 4200);
        state.aquarium.objects.insert(
            100,
            WorldObject::Food(beastie_core::FoodObject {
                id: 100,
                food: FoodId::Berry,
                position: food_position,
                velocity: beastie_core::NormalizedVelocity::default(),
                buoyancy: beastie_core::FoodBuoyancy::Drift,
                disposition: FoodDisposition::Falling,
                age_ms: 0,
                lifetime_ms: 10000,
            }),
        );
        state.creature.aquarium.gaze = GazeTarget::Food(100);
        assert_eq!(
            plan(&state, &view).0.creature.gaze_position,
            Some(food_position)
        );
        let moved_toy = NormalizedPosition::new(6200, 3500);
        state
            .aquarium
            .toy_states
            .get_mut(&ToyId::Ball)
            .expect("ball")
            .position = moved_toy;
        state.creature.aquarium.gaze = GazeTarget::Toy(ToyId::Ball);
        assert_eq!(
            plan(&state, &view).0.creature.gaze_position,
            Some(moved_toy)
        );
    }

    #[test]
    fn gaze_does_not_guess_missing_or_non_spatial_targets() {
        let mut state = WorldState::new(7, "Mop");
        let view = ViewState::default();
        state.aquarium.cursor = None;
        state.aquarium.objects.clear();
        state.aquarium.toy_states.clear();
        for target in [
            GazeTarget::None,
            GazeTarget::Player,
            GazeTarget::Cursor,
            GazeTarget::Food(999),
            GazeTarget::Toy(ToyId::Ball),
            GazeTarget::Cave,
            GazeTarget::Plant,
        ] {
            state.creature.aquarium.gaze = target;
            assert_eq!(
                plan(&state, &view).0.creature.gaze_position,
                None,
                "{target:?}"
            );
        }
    }

    #[test]
    fn food_drop_mode_makes_open_water_one_semantic_target() {
        let state = WorldState::new(7, "Mop");
        let view = ViewState {
            mode: UiMode::FoodDrop(FoodId::Berry),
            ..ViewState::default()
        };
        let (render, _) = plan(&state, &view);
        let drop = render
            .hit_regions
            .iter()
            .find(|hit| hit.id == "world/drop-food")
            .expect("drop target");
        assert_eq!(drop.action, UiAction::DropFood(FoodId::Berry));
        assert_eq!(drop.cursor, CursorKind::FoodDrop);
        assert!(
            render
                .hit_regions
                .iter()
                .any(|hit| hit.id == "compose/input")
        );
    }

    #[test]
    fn same_channel_cues_replace_instead_of_leaving_residue() {
        let mut view = ViewState::default();
        view.enqueue_cue(PresentationCueKind::Crumbs, 900, 1_000);
        view.enqueue_cue(PresentationCueKind::Sleep, 1_000, 1_000);
        assert_eq!(view.cue_queue.len(), 1);
        assert_eq!(view.active_cue(1_000), Some(PresentationCueKind::Sleep));
        view.expire(3_100);
        assert!(view.cue_queue.is_empty());
    }

    #[test]
    fn short_caption_fits_content_and_keeps_all_reaction_targets() {
        let state = WorldState::new(42, "Mop");
        let mut view = ViewState::default();
        view.show_speech("hm. rude giant.".to_owned(), 0);
        let full = plan(&state, &view).0;
        let panel = full
            .rects
            .iter()
            .find(|r| r.id == "speech/panel-edge")
            .expect("caption panel");
        assert!(panel.rect.w < 160);
        assert_eq!(panel.rect.h, 27);
        let reactions: Vec<_> = full
            .hit_regions
            .iter()
            .filter(|hit| hit.id.starts_with("reaction/"))
            .collect();
        assert_eq!(reactions.len(), 3);
        for hit in reactions {
            assert!(hit.enabled);
            assert!(hit.rect.w >= 22);
            assert_eq!(hit.rect.y, panel.rect.y + panel.rect.h);
            assert!(hit.rect.x + hit.rect.w <= panel.rect.x + panel.rect.w);
        }
        view.speech = Some("hm.".to_owned());
        let revealing = plan(&state, &view).0;
        assert_eq!(
            revealing.rects.iter().find(|r| r.id == "speech/panel-edge"),
            Some(panel)
        );
    }

    #[test]
    fn multiline_caption_and_large_text_grow_within_the_water_stage() {
        let state = WorldState::new(42, "Mop");
        let mut view = ViewState {
            text_scale: 2,
            ..ViewState::default()
        };
        view.show_speech("I remember the berry you brought. It tasted sweet, and I liked sharing that quiet moment with you.".to_owned(), 0);
        let scene = plan(&state, &view).0;
        let panel = scene
            .rects
            .iter()
            .find(|r| r.id == "speech/panel-edge")
            .expect("caption panel");
        assert!(panel.rect.h > 27);
        assert!(panel.rect.x >= 0 && panel.rect.x + panel.rect.w <= LOGICAL_WIDTH);
        assert!(
            scene
                .hit_regions
                .iter()
                .filter(|hit| hit.id.starts_with("reaction/"))
                .all(|hit| hit.rect.y + hit.rect.h < COMPOSE_BAR_TOP)
        );
        assert_eq!(
            scene
                .text
                .iter()
                .find(|t| t.id == "speech/text")
                .expect("caption")
                .text,
            view.speech.as_deref().unwrap()
        );
    }

    #[test]
    fn active_speech_keeps_its_caption_and_expiry_restores_compose_focus() {
        let mut view = ViewState::default();
        view.show_speech("hm. rude giant.".to_owned(), 1_000);
        view.focused_region = Some("reaction/laugh".to_owned());
        view.speaking = true;

        view.expire(9_000);
        assert_eq!(view.speech.as_deref(), Some("hm. rude giant."));
        assert_eq!(view.speech_expires_at_ms, Some(9_500));
        assert_eq!(view.focused_region.as_deref(), Some("reaction/laugh"));

        view.speaking = false;
        view.expire(9_500);
        assert!(view.speech.is_none());
        assert!(view.speech_expires_at_ms.is_none());
        assert_eq!(view.focused_region.as_deref(), Some("compose/input"));
    }

    #[test]
    fn audible_speech_gets_attention_without_faking_a_reaction_when_ignored() {
        let mut view = ViewState::default();
        view.observe_events(
            &[GameEvent::SpeechPerceived(SpeechAttention::Attended)],
            1_000,
        );
        assert_eq!(view.active_cue(1_000), Some(PresentationCueKind::Notice));

        let mut ignored = ViewState::default();
        ignored.observe_events(
            &[GameEvent::SpeechPerceived(SpeechAttention::Ignored)],
            1_000,
        );
        assert_eq!(ignored.active_cue(1_000), None);
    }

    #[test]
    fn direct_reactions_preempt_stale_punctuation() {
        let mut view = ViewState::default();
        view.enqueue_cue(PresentationCueKind::Notice, 2_000, 1_000);
        view.enqueue_cue(PresentationCueKind::Wake, 2_000, 1_000);
        view.enqueue_cue(PresentationCueKind::Affection, 1_200, 1_100);
        assert_eq!(view.cue_queue.len(), 1);
        assert_eq!(view.active_cue(1_100), Some(PresentationCueKind::Affection));
    }

    #[test]
    fn repeated_same_channel_cues_do_not_accumulate() {
        let mut view = ViewState::default();
        for index in 0..20 {
            let cue = if index % 2 == 0 {
                PresentationCueKind::Notice
            } else {
                PresentationCueKind::Wake
            };
            view.enqueue_cue(cue, 100, 0);
        }
        assert_eq!(view.cue_queue.len(), 1);
    }

    #[test]
    fn broad_summary_never_exposes_exact_need_or_relationship_values() {
        let mut state = WorldState::new(7, "Mop");
        state.creature.needs.hunger = 0.923_456;
        state.creature.relationship.trust = 0.123_456;
        let encoded = serde_json::to_string(&creature_summary(&state)).expect("summary serializes");
        assert!(!encoded.contains("0.923456"));
        assert!(!encoded.contains("0.123456"));
        assert!(!encoded.contains("trust"));
        assert!(!encoded.contains("hunger"));
    }

    #[test]
    fn deterministic_plan_is_viewport_independent() {
        let state = WorldState::new(42, "Mop");
        let view = ViewState::default();
        assert_eq!(plan(&state, &view), plan(&state, &view));
        let serialized = serde_json::to_string(&plan(&state, &view).0).expect("plan serializes");
        assert!(!serialized.contains("window_width"));
        assert!(!serialized.contains("scale_factor"));
    }

    #[test]
    fn autonomous_travel_and_play_have_matching_behavior_labels() {
        let mut state = WorldState::new(7, "Mop");
        state.creature.current_intention = Intention::Play;
        state.creature.aquarium.steering = SteeringMode::Approach;
        assert_eq!(
            plan(&state, &ViewState::default()).0.summary.behavior,
            "swimming to a toy"
        );

        state.creature.aquarium.steering = SteeringMode::Hover;
        assert_eq!(
            plan(&state, &ViewState::default()).0.summary.behavior,
            "playing"
        );

        state.creature.current_intention = Intention::Idle;
        state.creature.aquarium.steering = SteeringMode::Approach;
        assert_eq!(
            plan(&state, &ViewState::default()).0.summary.behavior,
            "swimming over"
        );
    }

    #[test]
    fn visible_settings_surface_exposes_every_accessibility_control() {
        let state = WorldState::new(7, "Mop");
        let view = ViewState {
            mode: UiMode::Settings,
            ..ViewState::default()
        };
        let pages: Vec<_> = (0..3)
            .map(|settings_page| {
                plan(
                    &state,
                    &ViewState {
                        settings_page,
                        ..view.clone()
                    },
                )
                .0
            })
            .collect();
        let mut plan = pages[0].clone();
        for page in &pages[1..] {
            plan.hit_regions.extend(page.hit_regions.clone());
            plan.text.extend(page.text.clone());
        }
        for id in [
            "settings/text-scale",
            "settings/motion",
            "settings/flashes",
            "settings/shake",
            "settings/window-scale",
            "settings/fullscreen",
            "settings/effects-volume",
            "settings/speech-volume",
            "settings/voice",
            "settings/subtitles",
            "settings/text-speed",
            "settings/bindings",
            "settings/reset-bindings",
            "settings/data",
        ] {
            assert!(plan.hit_regions.iter().any(|hit| hit.id == id), "{id}");
        }
        assert!(plan.hit_regions.iter().any(|hit| {
            hit.id == "settings/subtitles" && hit.action == UiAction::ToggleSubtitles
        }));
        assert!(
            plan.text
                .iter()
                .any(|text| { text.id == "settings/subtitles-value" && text.text == "On" })
        );
        assert!(plan.hit_regions.iter().any(|hit| hit.id == "compose/input"));
    }

    #[test]
    fn bindings_surface_emits_typed_rebind_actions_and_capture_prompt() {
        let state = WorldState::new(7, "Mop");
        let bindings = plan(
            &state,
            &ViewState {
                mode: UiMode::Bindings,
                ..ViewState::default()
            },
        )
        .0;
        for action in [
            BindableAction::Food,
            BindableAction::Play,
            BindableAction::Comfort,
            BindableAction::Settings,
            BindableAction::Cancel,
        ] {
            assert!(bindings.hit_regions.iter().any(|hit| {
                hit.action == UiAction::BeginRebind(action)
                    && hit.id == format!("bindings/{}", bindable_id(action))
            }));
        }
        let capture = plan(
            &state,
            &ViewState {
                mode: UiMode::Rebinding(BindableAction::Food),
                ..ViewState::default()
            },
        )
        .0;
        assert!(
            capture
                .text
                .iter()
                .any(|text| { text.id == "bindings/capture-prompt" && text.text.contains("food") })
        );
    }

    #[test]
    fn accessibility_preferences_materially_change_the_plan() {
        let mut state = WorldState::new(7, "Mop");
        state.elapsed_ms = 1_360;
        let standard = plan(&state, &ViewState::default()).0;
        let accessible = plan(
            &state,
            &ViewState {
                text_scale: 2,
                reduced_motion: true,
                reduced_flashes: true,
                reduced_shake: true,
                cue_queue: vec![PresentationCue {
                    owner: SemanticOwner::DirectOutcome,
                    channel: PresentationChannel::CreatureExpression,
                    kind: PresentationCueKind::Spit,
                    starts_at_ms: 0,
                    expires_at_ms: 2_000,
                }],
                ..ViewState::default()
            },
        )
        .0;
        assert!(accessible.text.iter().all(|text| text.scale == 2));
        assert!(!standard.reduced_motion && !standard.reduced_flashes && !standard.reduced_shake);
        assert!(
            accessible.reduced_motion && accessible.reduced_flashes && accessible.reduced_shake
        );
        assert_eq!(accessible.creature.position, standard.creature.position);
        assert_eq!(
            accessible.creature.expression.unwrap().cue,
            PresentationCueKind::Spit
        );
    }

    #[test]
    fn aquarium_full_rejection_is_queued_visible_and_audible() {
        let state = WorldState::new(7, "Mop");
        let mut view = ViewState::default();
        view.observe_events(
            &[GameEvent::FoodDropRejected(
                FoodDropRejectionReason::AquariumFull,
            )],
            state.elapsed_ms,
        );
        assert_eq!(
            view.active_cue(state.elapsed_ms),
            Some(PresentationCueKind::AquariumFull)
        );
        let (render, audio) = plan(&state, &view);
        assert!(audio.events.is_empty());
        assert_eq!(
            played_cues(audio_plan_for_events(&[GameEvent::FoodDropRejected(
                FoodDropRejectionReason::AquariumFull,
            )])),
            vec![AudioCue::UiReject]
        );
        assert!(
            render
                .text
                .iter()
                .any(|text| { text.id == "status/message" && text.text.contains("Aquarium full") })
        );
    }

    #[test]
    fn action_relationship_context_drives_food_recognition_until_the_direct_outcome() {
        let mut state = WorldState::new(7, "Mop");
        let relationship = |motif| ActionRelationshipContext {
            motif,
            expression_kind: RelationshipExpressionKind::Anticipate,
            evidence: Vec::new(),
            subject: RelationshipSubject::Food(FoodId::Berry),
        };
        state.creature.aquarium.action = Some(ActionTimeline {
            action_id: 41,
            phase: ActionPhase::Gaze,
            elapsed_ms: 240,
            phase_duration_ms: 1_000,
            destination: SemanticDestination::Food(9),
            food_id: Some(9),
            food: Some(FoodId::Berry),
            food_outcome: None,
            relationship: Some(relationship(RelationshipMotifKey::TrustedFood(
                FoodId::Berry,
            ))),
        });
        let view = ViewState::default();
        assert_eq!(
            effective_cue_timing(&state, &view),
            Some((PresentationCueKind::PositiveNotice, 240))
        );

        state
            .creature
            .aquarium
            .action
            .as_mut()
            .unwrap()
            .relationship = Some(relationship(RelationshipMotifKey::FoodGrudge(
            FoodId::Berry,
        )));
        assert_eq!(
            effective_cue_timing(&state, &view),
            Some((PresentationCueKind::FoodSuspicion, 240))
        );

        let mut direct = ViewState::default();
        direct.observe_events(&[GameEvent::FoodRejected(FoodId::Berry)], state.elapsed_ms);
        assert_eq!(
            effective_cue_timing(&state, &direct),
            Some((PresentationCueKind::Spit, 0))
        );
        direct.observe_events(
            &[GameEvent::ActionRelationshipCompleted {
                action_id: 41,
                motif: RelationshipMotifKey::FoodGrudge(FoodId::Berry),
                subject: RelationshipSubject::Food(FoodId::Berry),
            }],
            1_000,
        );
        state.creature.aquarium.action = None;
        assert_eq!(effective_cue_timing(&state, &direct), None);
    }

    #[test]
    fn direct_food_outcome_cancels_bound_voice_before_playing_physical_sound() {
        let plan = audio_plan_for_events(&[
            GameEvent::ActionRelationshipStarted {
                action_id: 41,
                motif: RelationshipMotifKey::FoodGrudge(FoodId::Berry),
                expression: RelationshipExpressionKind::Anticipate,
                subject: RelationshipSubject::Food(FoodId::Berry),
                evidence: Vec::new(),
            },
            GameEvent::ActionRelationshipResolved {
                action_id: 41,
                motif: RelationshipMotifKey::FoodGrudge(FoodId::Berry),
                subject: RelationshipSubject::Food(FoodId::Berry),
                outcome: beastie_core::FoodOutcome::Rejected,
            },
            GameEvent::FoodRejected(FoodId::Berry),
        ]);
        assert!(plan.events.contains(&AudioCommand::CancelOwner {
            owner: SemanticOwner::ActionRelationship(41),
        }));
        assert_eq!(
            played_cues(plan),
            vec![AudioCue::Annoyed, AudioCue::FoodReject]
        );
    }

    #[test]
    fn relationship_interrupt_preempts_stale_presentation_and_cave_settle_is_semantic() {
        let mut view = ViewState::default();
        view.observe_events(
            &[GameEvent::RelationshipBeatStarted {
                motif: RelationshipMotifKey::PlayerReturns,
                expression: RelationshipExpressionKind::Welcome,
                trigger: RelationshipTrigger::PlayerReturn,
                subject: RelationshipSubject::Player,
                evidence: Vec::new(),
            }],
            0,
        );
        view.observe_events(
            &[GameEvent::RelationshipBeatInterrupted(
                RelationshipMotifKey::PlayerReturns,
            )],
            100,
        );
        assert_eq!(view.active_cue(100), None);
        let mut place = ViewState::default();
        place.observe_events(
            &[GameEvent::RelationshipBeatStarted {
                motif: RelationshipMotifKey::FamiliarPlace(SemanticDestination::Cave),
                expression: RelationshipExpressionKind::Recognize,
                trigger: RelationshipTrigger::QuietMoment,
                subject: RelationshipSubject::Place(SemanticDestination::Cave),
                evidence: Vec::new(),
            }],
            0,
        );
        assert_eq!(place.active_cue(0), Some(PresentationCueKind::PlaceNotice));
        place.observe_events(
            &[GameEvent::RelationshipBeatPhaseChanged {
                motif: RelationshipMotifKey::FamiliarPlace(SemanticDestination::Cave),
                from: Some(RelationshipBeatPhase::Notice),
                to: RelationshipBeatPhase::Anticipate,
            }],
            1_000,
        );
        assert_eq!(
            place.active_cue(1_000),
            Some(PresentationCueKind::PlaceNotice)
        );
        assert!(
            played_cues(audio_plan_for_events(&[
                GameEvent::RelationshipBeatCompleted(RelationshipMotifKey::FamiliarPlace(
                    SemanticDestination::Cave
                ),)
            ]))
            .is_empty()
        );
        assert_eq!(
            played_cues(audio_plan_for_events(&[
                GameEvent::RelationshipBeatPhaseChanged {
                    motif: RelationshipMotifKey::FamiliarPlace(SemanticDestination::Cave),
                    from: Some(RelationshipBeatPhase::Anticipate),
                    to: RelationshipBeatPhase::Act,
                }
            ])),
            vec![AudioCue::CaveSettle]
        );
        assert_eq!(
            played_cues(audio_plan_for_events(&[
                GameEvent::RelationshipBeatStarted {
                    motif: RelationshipMotifKey::SharedToy(ToyId::Ball),
                    expression: RelationshipExpressionKind::Anticipate,
                    trigger: RelationshipTrigger::QuietMoment,
                    subject: RelationshipSubject::Toy(ToyId::Ball),
                    evidence: Vec::new(),
                }
            ])),
            vec![AudioCue::Curious]
        );
        let motifs = [
            RelationshipMotifKey::SharedToy(ToyId::Ball),
            RelationshipMotifKey::ComfortRitual,
            RelationshipMotifKey::TrustedFood(FoodId::Berry),
            RelationshipMotifKey::FoodGrudge(FoodId::Berry),
            RelationshipMotifKey::PlayerReturns,
            RelationshipMotifKey::FamiliarPlace(SemanticDestination::Cave),
        ];
        for motif in motifs {
            let plan = audio_plan_for_events(&[GameEvent::RelationshipBeatStarted {
                motif,
                expression: RelationshipExpressionKind::Notice,
                trigger: RelationshipTrigger::RelevantUtterance {
                    subject: Some(RelationshipSubject::Player),
                },
                subject: RelationshipSubject::Player,
                evidence: Vec::new(),
            }]);
            if matches!(motif, RelationshipMotifKey::FamiliarPlace(_)) {
                assert!(played_cues(plan).is_empty());
                continue;
            }
            assert!(!played_cues(plan).is_empty());
        }
    }

    #[test]
    fn paired_rejections_and_comfort_emit_one_semantic_sound_each() {
        assert_eq!(
            played_cues(audio_plan_for_events(&[
                GameEvent::FoodRejected(FoodId::Berry),
                GameEvent::NonverbalAct(NonverbalAct::PushFoodAway(FoodId::Berry)),
            ])),
            vec![AudioCue::FoodReject]
        );
        assert_eq!(
            played_cues(audio_plan_for_events(&[
                GameEvent::ToyRejected {
                    toy: ToyId::Bell,
                    interaction_id: std::num::NonZeroU64::MIN,
                    origin: beastie_core::ToyOrigin::Player,
                },
                GameEvent::NonverbalAct(NonverbalAct::TakeToyAway(ToyId::Bell)),
            ])),
            vec![AudioCue::Annoyed]
        );
        assert_eq!(
            played_cues(audio_plan_for_events(&[
                GameEvent::Comforted,
                GameEvent::NonverbalAct(NonverbalAct::LeanAgainstPlayer),
            ])),
            vec![AudioCue::Affection]
        );
    }

    #[test]
    fn private_toy_responses_select_exact_owned_audio_and_cancel_at_the_boundary() {
        let activity_id = NonZeroU64::MIN;
        let events = [
            GameEvent::ToyObjectResponded {
                toy: ToyId::Ball,
                activity_id,
                response: ToyResponse::BallNudged,
            },
            GameEvent::ToyObjectResponded {
                toy: ToyId::Bell,
                activity_id,
                response: ToyResponse::BellStruck,
            },
            GameEvent::ToyObjectResponded {
                toy: ToyId::Sock,
                activity_id,
                response: ToyResponse::SockTugged,
            },
            GameEvent::PrivateLifeCompleted {
                activity_id,
                kind: PrivateLifeKind::ToyPlay(ToyId::Sock),
                recipe: ActivityRecipe::SockTug,
            },
        ];
        let plan = audio_plan_for_events(&events);
        assert_eq!(
            played_cues(plan.clone()),
            vec![
                AudioCue::BallNudge,
                AudioCue::BellRing,
                AudioCue::SockRustle
            ]
        );
        assert!(plan.events.contains(&AudioCommand::CancelOwner {
            owner: SemanticOwner::PrivateLife(activity_id),
        }));
    }

    #[test]
    fn spoken_receipt_chirps_once_and_talk_acceptance_stays_visual() {
        assert_eq!(
            played_cues(audio_plan_for_events(&[
                GameEvent::SpeechPerceived(SpeechAttention::Attended),
                GameEvent::TalkAccepted {
                    contextual_follow_up: false,
                },
            ])),
            vec![AudioCue::Curious]
        );
        assert!(
            played_cues(audio_plan_for_events(&[GameEvent::TalkAccepted {
                contextual_follow_up: false,
            }]))
            .is_empty()
        );
    }

    #[test]
    fn labeled_controls_remain_explained_during_controller_focus() {
        let state = WorldState::new(7, "Mop");
        let keyboard = plan(&state, &ViewState::default()).0;
        let controller = plan(
            &state,
            &ViewState {
                controller_active: true,
                focused_region: Some("compose/food".to_owned()),
                ..ViewState::default()
            },
        )
        .0;
        assert!(
            keyboard
                .text
                .iter()
                .all(|text| text.id != "compose/input-hints")
        );
        assert!(
            controller
                .text
                .iter()
                .all(|text| text.id != "compose/input-hints")
        );
        assert!(
            controller
                .text
                .iter()
                .any(|text| text.id == "compose/control-feed" && text.text == "Feed")
        );
    }

    #[test]
    fn microphone_states_are_legible_without_freezing_shared_space() {
        let state = WorldState::new(81, "Muck");
        let listening = plan(
            &state,
            &ViewState {
                pending: true,
                microphone_enabled: true,
                microphone_state: MicrophoneState::Listening,
                ..ViewState::default()
            },
        )
        .0;
        for id in [
            "compose/input",
            "compose/microphone",
            "compose/food",
            "compose/settings",
        ] {
            assert!(
                listening
                    .hit_regions
                    .iter()
                    .find(|region| region.id == id)
                    .expect("persistent control")
                    .enabled,
                "{id} should remain usable while cognition is pending"
            );
        }
        assert!(listening.text.iter().any(|command| {
            command.id == "status/message" && command.text.contains("Listening")
        }));

        let unavailable = plan(
            &state,
            &ViewState {
                microphone_enabled: true,
                microphone_state: MicrophoneState::Unavailable,
                ..ViewState::default()
            },
        )
        .0;
        assert!(
            !unavailable
                .hit_regions
                .iter()
                .find(|region| region.id == "compose/microphone")
                .expect("microphone control")
                .enabled
        );
        assert!(unavailable.text.iter().any(|command| {
            command.id == "status/message" && command.text.contains("Text still works")
        }));
    }

    #[test]
    fn save_and_transcript_controls_are_semantic_and_reset_is_confirmed() {
        let state = WorldState::new(7, "Mop");
        let data = plan(
            &state,
            &ViewState {
                mode: UiMode::DataManagement,
                transcript_enabled: true,
                transcript_status: Some("Exported locally".to_owned()),
                ..ViewState::default()
            },
        )
        .0;
        for action in [
            UiAction::RecoverBackup,
            UiAction::RequestReset,
            UiAction::ToggleTranscript,
            UiAction::ExportTranscript,
        ] {
            assert!(data.hit_regions.iter().any(|hit| hit.action == action));
        }
        assert!(
            data.text
                .iter()
                .any(|text| { text.id == "status/message" && text.text == "Exported locally" })
        );
        let confirmation = plan(
            &state,
            &ViewState {
                mode: UiMode::ConfirmReset,
                ..ViewState::default()
            },
        )
        .0;
        assert!(
            confirmation
                .hit_regions
                .iter()
                .any(|hit| hit.action == UiAction::ConfirmReset)
        );
        assert!(
            confirmation
                .hit_regions
                .iter()
                .any(|hit| hit.action == UiAction::CancelMode)
        );
    }

    #[test]
    fn creature_rename_uses_compose_buffer_and_explicit_submit() {
        assert!(contextual_actions(UiTarget::Creature).contains(&UiAction::Rename));
        let state = WorldState::new(7, "Mop");
        let rename = plan(
            &state,
            &ViewState {
                mode: UiMode::Rename,
                text_buffer: "Gob".to_owned(),
                ..ViewState::default()
            },
        )
        .0;
        assert!(
            rename
                .hit_regions
                .iter()
                .any(|hit| hit.id == "compose/send" && hit.action == UiAction::SubmitName)
        );
        assert!(
            rename
                .text
                .iter()
                .any(|text| text.id == "compose/input-text" && text.text == "Gob")
        );
        assert!(rename.text.iter().any(|text| text.id == "rename/prompt"));
    }
    #[test]
    fn persistent_bar_keeps_long_summary_input_and_icon_actions_disjoint() {
        let state = WorldState::new(7, "TwentyFourCharacterName!");
        for text_scale in [1, 2] {
            for controller_active in [false, true] {
                let view = ViewState {
                    text_scale,
                    controller_active,
                    text_buffer: "a deliberately overlong compose buffer that keeps going"
                        .to_owned(),
                    ..ViewState::default()
                };
                let render = plan(&state, &view).0;
                let summary_name = render
                    .text
                    .iter()
                    .find(|command| command.id == "compose/summary-name")
                    .expect("summary name");
                let summary_behavior = render
                    .text
                    .iter()
                    .find(|command| command.id == "compose/summary-behavior")
                    .expect("summary behavior");
                let input = render
                    .text
                    .iter()
                    .find(|command| command.id == "compose/input-text")
                    .expect("input");
                let input_box = render
                    .rects
                    .iter()
                    .find(|command| command.id == "compose/input-background")
                    .expect("input background")
                    .rect;
                assert!(text_right(summary_name) < summary_behavior.x);
                assert!(text_right(summary_behavior) <= LOGICAL_WIDTH - 5);
                assert!(text_right(input) <= input_box.x + input_box.w - 3);
                for action in ["food", "settings", "send"] {
                    let icon = render
                        .icons
                        .iter()
                        .find(|command| command.id == format!("ui/button-{action}"))
                        .expect("action icon");
                    let background = render
                        .rects
                        .iter()
                        .find(|command| command.id == format!("compose/{action}-background"))
                        .expect("action background")
                        .rect;
                    assert!(icon.x >= background.x);
                    assert!(icon.x + 5 <= background.x + background.w);
                    assert!(!rects_overlap(input_box, background));
                }
            }
        }
    }

    #[test]
    fn ui_depth_bands_and_modal_hit_regions_are_unambiguous() {
        let state = WorldState::new(7, "Mop");
        for mode in [
            UiMode::Context(UiTarget::Creature),
            UiMode::FoodChoice,
            UiMode::FoodDrop(FoodId::Berry),
            UiMode::ToyChoice,
            UiMode::Settings,
            UiMode::Bindings,
            UiMode::Rebinding(BindableAction::Food),
            UiMode::Rename,
            UiMode::DataManagement,
            UiMode::ConfirmReset,
            UiMode::OnScreenKeyboard,
        ] {
            let render = plan(
                &state,
                &ViewState {
                    mode,
                    ..ViewState::default()
                },
            )
            .0;
            assert!(
                render.hit_regions.iter().any(|hit| {
                    hit.id == "compose/close" && hit.action == UiAction::CancelMode && hit.enabled
                }),
                "{mode:?} has no pointer-close control"
            );
            for (index, left) in render
                .hit_regions
                .iter()
                .filter(|hit| hit.enabled)
                .enumerate()
            {
                for right in render
                    .hit_regions
                    .iter()
                    .filter(|hit| hit.enabled)
                    .skip(index + 1)
                {
                    assert!(
                        !rects_overlap(left.rect, right.rect),
                        "{mode:?}: {} overlaps {}",
                        left.id,
                        right.id
                    );
                }
            }
            for command in render.rects.iter().filter(|command| command.layer >= 22) {
                assert!(
                    command.rect.x >= 0
                        && command.rect.y >= 0
                        && command.rect.x + command.rect.w <= LOGICAL_WIDTH
                        && command.rect.y + command.rect.h <= LOGICAL_HEIGHT,
                    "{mode:?}: {} is outside the logical frame: {:?}",
                    command.id,
                    command.rect
                );
            }
            let chrome_top = render
                .rects
                .iter()
                .filter(|command| {
                    !command.id.starts_with("compose/")
                        && (command.id.contains("/panel") || command.id.ends_with("-background"))
                })
                .map(|command| command.layer)
                .max()
                .unwrap_or_else(|| panic!("{mode:?} has no modal chrome"));
            let modal_text_bottom = render
                .text
                .iter()
                .filter(|command| !command.id.starts_with("compose/"))
                .map(|command| command.layer)
                .min()
                .expect("modal text");
            assert!(chrome_top <= modal_text_bottom, "{mode:?} depth order");
        }
    }

    #[test]
    fn status_uses_the_summary_row_without_overlapping_modes_or_compose() {
        let state = WorldState::new(7, "Mop");
        for mode in [UiMode::Compose, UiMode::Settings, UiMode::FoodChoice] {
            let render = plan(
                &state,
                &ViewState {
                    mode,
                    text_scale: 2,
                    status_message: Some(
                        "A very long recovery status that must remain inside its own row"
                            .to_owned(),
                    ),
                    ..ViewState::default()
                },
            )
            .0;
            let status = render
                .rects
                .iter()
                .find(|command| command.id == "status/background")
                .expect("status background")
                .rect;
            let compose = render
                .rects
                .iter()
                .find(|command| command.id == "compose/input-background")
                .expect("compose background")
                .rect;
            assert!(!rects_overlap(status, compose));
            for panel in render
                .rects
                .iter()
                .filter(|command| command.id.starts_with("mode/") || command.id.ends_with("/panel"))
            {
                assert!(
                    !rects_overlap(status, panel.rect),
                    "{} overlaps status",
                    panel.id
                );
            }
            let message = render
                .text
                .iter()
                .find(|command| command.id == "status/message")
                .expect("status message");
            assert!(text_right(message) <= status.x + status.w - 3);
            assert!(
                render
                    .text
                    .iter()
                    .any(|command| command.id == "compose/summary-name")
            );
            let name = render
                .text
                .iter()
                .find(|command| command.id == "compose/summary-name")
                .unwrap();
            assert!(!rects_overlap(
                name.bounds.unwrap(),
                message.bounds.unwrap()
            ));
        }
    }

    fn text_right(command: &TextCommand) -> i32 {
        let bounds = command
            .bounds
            .expect("every text command has explicit bounds");
        bounds.x + bounds.w
    }

    fn rects_overlap(left: Rect, right: Rect) -> bool {
        left.x < right.x + right.w
            && left.x + left.w > right.x
            && left.y < right.y + right.h
            && left.y + left.h > right.y
    }

    #[test]
    fn scene_preserves_continuous_authority_and_view_does_not_mutate_it() {
        let mut state = WorldState::new(7, "Mop");
        state.creature.aquarium.position = NormalizedPosition::new(4231, 7189);
        state.creature.aquarium.velocity = beastie_core::NormalizedVelocity { x: 197, y: -63 };
        state.simulation_remainder_ms = 17;
        let before = state.clone();
        let scene = plan(&state, &ViewState::default()).0;
        assert_eq!(scene.creature.position, state.creature.aquarium.position);
        assert_eq!(scene.creature.velocity, state.creature.aquarium.velocity);
        assert_eq!(scene.simulation_remainder_ms, 17);
        assert_eq!(state, before);
        assert!(
            scene
                .hit_regions
                .iter()
                .any(|hit| hit.shape == HitShape::World(UiTarget::Creature))
        );
    }

    #[test]
    fn mutable_toys_keep_catalogue_identity_and_real_position() {
        let mut state = WorldState::new(7, "Mop");
        let toy = state.aquarium.toy_states.get_mut(&ToyId::Sock).unwrap();
        toy.position = NormalizedPosition::new(4001, 6007);
        toy.carried = true;
        toy.last_response = ToyResponse::SockTugged;
        let scene = plan(&state, &ViewState::default()).0;
        let object = scene
            .objects
            .iter()
            .find(|object| object.kind == ObjectKind::Toy(ToyId::Sock))
            .unwrap();
        assert_eq!(object.position, NormalizedPosition::new(4001, 6007));
        assert!(object.carried);
        assert_eq!(object.response, ToyResponse::SockTugged);
        assert!(matches!(
            state.aquarium.objects[&object.id],
            WorldObject::Toy {
                toy: ToyId::Sock,
                ..
            }
        ));
        assert!(
            scene
                .hit_regions
                .iter()
                .any(|hit| hit.id == format!("target/object-{}", object.id))
        );
    }

    #[test]
    fn each_private_recipe_retains_phase_and_requires_contact_for_prop_effects() {
        for (kind, recipe, cue) in [
            (
                PrivateLifeKind::ToyPlay(ToyId::Ball),
                ActivityRecipe::BallNudge,
                PresentationCueKind::BallNudge,
            ),
            (
                PrivateLifeKind::ToyPlay(ToyId::Bell),
                ActivityRecipe::BellStrike,
                PresentationCueKind::BellStrike,
            ),
            (
                PrivateLifeKind::ToyPlay(ToyId::Sock),
                ActivityRecipe::SockTug,
                PresentationCueKind::SockTug,
            ),
            (
                PrivateLifeKind::CaveSettle,
                ActivityRecipe::CaveShelter,
                PresentationCueKind::CaveShelter,
            ),
            (
                PrivateLifeKind::PlantInspect,
                ActivityRecipe::PlantOrbit,
                PresentationCueKind::PlantOrbit,
            ),
            (
                PrivateLifeKind::BottomForage,
                ActivityRecipe::BottomForage,
                PresentationCueKind::BottomForage,
            ),
            (
                PrivateLifeKind::OpenWaterDrift,
                ActivityRecipe::OpenWaterDrift,
                PresentationCueKind::OpenWaterDrift,
            ),
        ] {
            let mut state = WorldState::new(7, "Mop");
            let id = NonZeroU64::new(1).unwrap();
            let mut activity = private_activity(id, kind, recipe, ActivityPhase::Act);
            activity.phase_started_at_ms = 100;
            state.elapsed_ms = 321;
            state.creature.private_life.active = Some(activity);
            let before = plan(&state, &ViewState::default()).0;
            assert_eq!(
                before.creature.private_life.as_ref().unwrap().elapsed_ms,
                221
            );
            assert_eq!(before.creature.phase_elapsed_ms, 221);
            if let PrivateLifeKind::ToyPlay(toy) = kind {
                assert!(
                    !before
                        .effects
                        .iter()
                        .any(|effect| effect.target == UiTarget::Toy(toy))
                );
            }
            state
                .creature
                .private_life
                .active
                .as_mut()
                .unwrap()
                .payoff_reached = true;
            let after = plan(&state, &ViewState::default()).0;
            assert!(
                after
                    .effects
                    .iter()
                    .any(|effect| effect.owner == SemanticOwner::PrivateLife(id)
                        && effect.cue == cue)
            );
            assert_eq!(after.creature.private_life.unwrap().recipe, recipe);
        }
    }

    #[test]
    fn mouth_and_direct_expression_share_authoritative_timing() {
        let mut state = WorldState::new(7, "Mop");
        state.elapsed_ms = 100;
        let mut view = ViewState {
            speaking: true,
            mouth_phase: 2,
            ..ViewState::default()
        };
        view.enqueue_owned_cue(
            SemanticOwner::DirectOutcome,
            PresentationCueKind::Spit,
            1000,
            40,
        );
        let scene = plan(&state, &view).0;
        assert_eq!(scene.creature.mouth_phase, 2);
        let expression = scene.creature.expression.unwrap();
        assert_eq!(
            (expression.owner, expression.cue, expression.elapsed_ms),
            (SemanticOwner::DirectOutcome, PresentationCueKind::Spit, 60)
        );
        view.speaking = false;
        assert_eq!(plan(&state, &view).0.creature.mouth_phase, 0);
    }

    #[test]
    fn modal_hides_world_picking_and_geometry_icons_are_complete() {
        let state = WorldState::new(7, "Mop");
        let scene = plan(&state, &ViewState::default()).0;
        for kind in [
            IconKind::Microphone,
            IconKind::Food,
            IconKind::Settings,
            IconKind::Send,
        ] {
            assert_eq!(
                scene.icons.iter().filter(|icon| icon.kind == kind).count(),
                1
            );
        }
        let modal = plan(
            &state,
            &ViewState {
                mode: UiMode::Settings,
                ..ViewState::default()
            },
        )
        .0;
        assert!(
            modal
                .hit_regions
                .iter()
                .all(|hit| hit.shape == HitShape::Rect)
        );
        assert!(
            !modal
                .hit_regions
                .iter()
                .any(|hit| hit.id == "settings/grid")
        );
    }
}
