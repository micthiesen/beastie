//! Display-free semantic projection of the authoritative aquarium into a 3D scene.
//!
//! This crate owns UI layout units, presentation timing, semantic hit regions, and
//! presentation-only effects. It never mutates simulation state.

use beastie_core::{
    ActionPhase, ActivityPhase, ActivityRecipe, FoodDisposition, FoodDropRejectionReason, FoodId,
    GameEvent, GazeTarget, Intention, Mood, NonverbalAct, NormalizedPosition, PrivateLifeKind,
    RelationshipBeatPhase, RelationshipExpressionKind, RelationshipMotifKey,
    RelationshipPerformanceRecipe, SemanticDestination, SpeechAttention, SteeringMode, ToyId,
    ToyResponse, WorldObject, WorldState, performance_recipe_for,
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;
use unicode_segmentation::UnicodeSegmentation;

pub const LOGICAL_WIDTH: i32 = 320;
pub const LOGICAL_HEIGHT: i32 = 180;
pub const AQUARIUM_BOTTOM: i32 = 149;
pub const COMPOSE_BAR_TOP: i32 = 149;
pub const CREATURE_HIT_WIDTH: i32 = 52;
pub const CREATURE_HIT_HEIGHT: i32 = 40;
pub const SPEECH_LIFETIME_MS: u64 = 8_000;
pub const SPEECH_RELEASE_MS: u64 = 500;
pub const CUE_QUEUE_LIMIT: usize = 8;
const SPEECH_PANEL_WIDTH: i32 = 140;
const SPEECH_TEXT_WIDTH: i32 = SPEECH_PANEL_WIDTH - 18;
const SPEECH_MAX_LINES: usize = 5;

pub mod typography;
mod ui_art;
use typography::{line_height, typography};
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
    ComposeField,
    Actions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiAction {
    OpenTitle,
    Continue,
    Quit,
    OpenContext(UiTarget),
    CloseContext,
    OpenFoodChoice,
    OpenToyChoice,
    /// Drops the chosen food just in front of the creature: feeding is one click.
    SelectFood(FoodId),
    /// Tap the glass where the pointer is; the shell supplies the aquarium position.
    TapWater,
    Play(ToyId),
    Comfort,
    Inspect,
    Talk,
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
    /// Speech is no longer paged; retained as a no-op so older scenario scripts still parse.
    ChangeSpeechPage(i8),
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
    DismissStatus,
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
    Title,
    /// Normal play. The compose field remains focused and accepts printable text.
    #[default]
    Compose,
    Context(UiTarget),
    /// A live, qualitative reading of an actual aquarium inhabitant or object.
    Inspect(UiTarget),
    FoodChoice,
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
    /// Head tilt and a question mark: an unfamiliar word was heard.
    Curious,
    /// A burst of sparkle: a word was just learned.
    WordLearned,
    /// Rings spreading from a tap on the glass.
    Ripple,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SemanticOwner {
    Ordinary,
    StandaloneRelationship(RelationshipMotifKey),
    ActionRelationship(u64),
    PrivateLife(NonZeroU64),
    ToyInteraction(NonZeroU64),
    DirectOutcome,
}

impl SemanticOwner {
    const fn priority(self) -> u8 {
        match self {
            Self::Ordinary => 0,
            Self::StandaloneRelationship(_) => 1,
            Self::ActionRelationship(_) => 2,
            Self::PrivateLife(_) => 2,
            Self::ToyInteraction(_) | Self::DirectOutcome => 3,
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
    /// Context menu anchor chosen when opened, so moving creatures do not move controls.
    #[serde(default)]
    pub context_above: Option<bool>,
    /// A toy card chooses clear water once when opened; motion never moves its buttons.
    #[serde(default)]
    pub context_card_anchor: Option<(i32, i32)>,
    #[serde(default)]
    pub settings_page: u8,
    /// Stable [`HitRegion::id`] selected by keyboard or controller navigation.
    pub focused_region: Option<String>,
    /// Stable [`HitRegion::id`] beneath the pointer.
    pub hovered_region: Option<String>,
    #[serde(default)]
    pub pressed_region: Option<String>,
    #[serde(default)]
    pub pressed_until_ms: u64,
    pub text_buffer: String,
    /// Session-only select-all state for the active editable value.
    #[serde(default)]
    pub text_selected: bool,
    /// Explicit editing engagement, separate from the default keyboard input destination.
    #[serde(default)]
    pub compose_engaged: bool,
    /// The controller keyboard is editing a creature name rather than a message.
    #[serde(default)]
    pub renaming_with_osk: bool,
    /// Session-only guidance, dismissed by an observed care interaction.
    #[serde(default)]
    pub care_guidance_dismissed: bool,
    /// Recent taps on the glass, for their ripples.
    #[serde(default)]
    pub ripples: Vec<(NormalizedPosition, u64)>,
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
    #[serde(default)]
    pub dismissed_microphone_notice: Option<MicrophoneState>,
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
            context_above: None,
            context_card_anchor: None,
            settings_page: 0,
            focused_region: Some("compose/input".to_owned()),
            hovered_region: None,
            pressed_region: None,
            pressed_until_ms: 0,
            text_buffer: String::new(),
            text_selected: false,
            compose_engaged: false,
            renaming_with_osk: false,
            care_guidance_dismissed: false,
            ripples: Vec::new(),
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
            dismissed_microphone_notice: None,
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
    }

    pub fn show_status(&mut self, status: impl Into<String>, now_ms: u64, duration_ms: u64) {
        self.dismissed_microphone_notice = None;
        self.status_message = Some(status.into());
        self.status_expires_at_ms = Some(now_ms.saturating_add(duration_ms.max(1)));
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
        self.status_expires_at_ms = None;
    }

    pub fn dismiss_status(&mut self) {
        self.clear_status();
        self.transcript_status = None;
        self.dismissed_microphone_notice = matches!(
            self.microphone_state,
            MicrophoneState::Unavailable | MicrophoneState::Error
        )
        .then_some(self.microphone_state);
    }

    /// Projects an authoritative event batch and returns its owned audio commands.
    pub fn observe_events(&mut self, events: &[GameEvent], now_ms: u64) -> AudioPlan {
        self.expire(now_ms);
        self.ripples
            .retain(|(_, at)| now_ms.saturating_sub(*at) < RIPPLE_MS);
        for event in events {
            if let GameEvent::TapNoticed { position, .. } = event {
                self.ripples.push((*position, now_ms));
            }
        }
        if events.iter().any(|event| {
            matches!(
                event,
                GameEvent::FoodDropped { .. }
                    | GameEvent::ToyPlayAccepted {
                        origin: beastie_core::ToyOrigin::Player,
                        ..
                    }
                    | GameEvent::ToyRejected {
                        origin: beastie_core::ToyOrigin::Player,
                        ..
                    }
                    | GameEvent::Comforted
            )
        }) {
            self.care_guidance_dismissed = true;
        }
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
                GameEvent::ToyInteractionResponded {
                    interaction_id,
                    response,
                    ..
                } if *response != ToyResponse::None => {
                    let owner = SemanticOwner::ToyInteraction(*interaction_id);
                    self.cue_queue.retain(|cue| cue.owner != owner);
                    if self.cue_queue.len() < CUE_QUEUE_LIMIT {
                        self.cue_queue.push(PresentationCue {
                            owner,
                            channel: PresentationChannel::Physical,
                            kind: toy_response_cue(*response),
                            starts_at_ms: now_ms,
                            expires_at_ms: now_ms.saturating_add(
                                private_life_phase_duration_for_response(*response),
                            ),
                        });
                    }
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
        if self.dismissed_microphone_notice != Some(self.microphone_state) {
            self.dismissed_microphone_notice = None;
        }
        if self.speech.is_some() && speech_obstructed(self.mode) {
            // Only caption reading time pauses. The simulation, audio and owned
            // progressive reveal continue while the water is occupied by a modal.
            self.speech_expires_at_ms = Some(now_ms.saturating_add(SPEECH_LIFETIME_MS));
        }
        if let (Some(speech), Some(layout)) = (&self.speech, &self.speech_layout_text) {
            // Reading time starts after the progressive reveal completes.
            if speech_revealed_bytes(layout, speech) < layout.len() {
                self.speech_expires_at_ms = Some(now_ms.saturating_add(SPEECH_LIFETIME_MS));
            }
        }
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
            .iter()
            .rev()
            .find(|cue| cue.channel == PresentationChannel::CreatureExpression)
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
            .find(|cue| {
                cue.channel == PresentationChannel::CreatureExpression
                    && now_ms >= cue.starts_at_ms
                    && now_ms < cue.expires_at_ms
            })
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
    /// Radius in logical layout units. Picking keeps the full comfortable rectangle.
    #[serde(default)]
    pub corner_radius: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextCommand {
    pub bounds: Option<Rect>,
    /// Vertically center a left-aligned label within its bounds.
    #[serde(default)]
    pub vertical_centered: bool,
    /// Editable single-line values retain their newest graphemes using renderer font metrics.
    #[serde(default)]
    pub keep_tail: bool,
    /// Steady input decoration. `keep_tail == false` places a caret before placeholder copy.
    #[serde(default)]
    pub input_state: Option<TextInputState>,
    pub muted: bool,
    pub role: TextRole,
    pub id: String,
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub layer: i16,
    pub scale: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextInputState {
    Caret,
    Selected,
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
    /// Opaque panel area: catches the pointer but is never an activatable or focusable control.
    Blocker,
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
    Toy(ToyId),
    FoodItem(FoodId),
    Microphone,
    Food,
    Settings,
    Send,
    Close,
    ChevronRight,
    Inspect,
    Heart,
    Rename,
    Play,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconCommand {
    pub id: String,
    pub kind: IconKind,
    pub bounds: Rect,
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
    /// Actual travel endpoint, independent of what the creature is looking at.
    #[serde(default)]
    pub movement_target: Option<NormalizedPosition>,
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
    /// What the creature wants, shown as a thought bubble while it is not speaking.
    #[serde(default)]
    pub want: Option<beastie_core::Want>,
    /// Something the creature is refusing right now, drawn crossed out in a bubble.
    #[serde(default)]
    pub refusing: Option<beastie_core::Meaning>,
    /// The creature is in the middle of a game with a toy.
    #[serde(default)]
    pub playing: bool,
    /// Where the creature is headed, so its target can be marked: a toy, food, the plant, the
    /// cave, or the bubble it is chasing.
    #[serde(default)]
    pub intent_target: Option<NormalizedPosition>,
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
    pub title_screen: bool,
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
    /// The creature learned what a word means.
    WordLearned,
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
            GameEvent::ToyPlayed { interaction_id, .. }
                if events.iter().any(|event| {
                    matches!(
                        event,
                        GameEvent::ToyInteractionResponded { interaction_id: owner, .. }
                            if owner == interaction_id
                    )
                }) =>
            {
                None
            }
            GameEvent::ToyPlayed { .. } => Some((
                SemanticOwner::DirectOutcome,
                physical,
                AudioCue::ToyImpact,
                700,
            )),
            GameEvent::ToyInteractionResponded {
                interaction_id,
                response,
                ..
            } => {
                let owner = SemanticOwner::ToyInteraction(*interaction_id);
                match response {
                    ToyResponse::None => None,
                    ToyResponse::BallNudged => Some((owner, physical, AudioCue::BallNudge, 680)),
                    ToyResponse::BellStruck => Some((owner, physical, AudioCue::BellRing, 650)),
                    ToyResponse::SockTugged => Some((owner, physical, AudioCue::SockRustle, 620)),
                }
            }
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
            GameEvent::WordHeard { .. } => Some((ordinary, voice, AudioCue::Curious, 650)),
            GameEvent::Emerged => Some((ordinary, voice, AudioCue::Curious, 700)),
            GameEvent::TapNoticed { .. } => Some((
                ordinary,
                PresentationChannel::Ambience,
                AudioCue::BubbleAlternate,
                620,
            )),
            GameEvent::WordLearned { .. } => Some((
                SemanticOwner::DirectOutcome,
                voice,
                AudioCue::WordLearned,
                760,
            )),
            GameEvent::Understood { response, .. } => match response {
                beastie_core::RequestResponse::Comply | beastie_core::RequestResponse::Delight => {
                    Some((SemanticOwner::DirectOutcome, voice, AudioCue::Mrr, 700))
                }
                beastie_core::RequestResponse::Refuse | beastie_core::RequestResponse::Sulk => {
                    Some((SemanticOwner::DirectOutcome, voice, AudioCue::Annoyed, 600))
                }
                beastie_core::RequestResponse::Look => {
                    Some((ordinary, voice, AudioCue::Curious, 600))
                }
            },
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
        UiTarget::ComposeField | UiTarget::Actions => Vec::new(),
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
    let effects = effect_scenes(state, &creature, view);
    let mut hit_regions = world_hit_regions(state, view);
    if !matches!(view.mode, UiMode::Title) {
        add_speech(state, view, &mut rects, &mut text);
        add_persistent_bar(
            state,
            view,
            &mut icons,
            &mut rects,
            &mut text,
            &mut hit_regions,
        );
    }
    add_temporary_mode(
        state,
        view,
        &mut icons,
        &mut rects,
        &mut text,
        &mut hit_regions,
    );
    let close = match view.mode {
        UiMode::Settings => Some((
            Rect {
                x: 277,
                y: 14,
                w: 27,
                h: 15,
            },
            UiAction::CancelMode,
            "Close",
        )),
        UiMode::Bindings => Some((
            Rect {
                x: 274,
                y: 12,
                w: 30,
                h: 16,
            },
            UiAction::OpenSettings,
            "Back",
        )),
        UiMode::DataManagement => Some((
            Rect {
                x: 273,
                y: 13,
                w: 30,
                h: 16,
            },
            UiAction::OpenSettings,
            "Back",
        )),
        _ => None,
    };
    if let Some((area, action, title)) = close {
        add_control(
            "modal/close",
            title,
            action,
            area,
            true,
            false,
            25,
            &mut rects,
            &mut text,
            &mut hit_regions,
        );
    }
    add_status(
        view,
        state.elapsed_ms,
        &mut rects,
        &mut text,
        &mut hit_regions,
    );
    if visible_status(view, state.elapsed_ms).is_none() {
        add_coaching(state, view, &mut rects, &mut text);
    }
    add_hover_and_focus(view, state.elapsed_ms, &hit_regions, &mut rects, &mut text);
    for command in &mut text {
        command.scale = view.text_scale.clamp(1, 2);
    }
    ui_art::layout_text(&mut text, &rects, &hit_regions);
    icons.sort_by_key(|command| command.layer);
    rects.sort_by_key(|command| command.layer);
    text.sort_by_key(|command| command.layer);
    (
        ScenePlan {
            title_screen: matches!(view.mode, UiMode::Title),
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
        .filter(|cue| {
            cue.channel == PresentationChannel::CreatureExpression
                && state.elapsed_ms >= cue.starts_at_ms
                && state.elapsed_ms < cue.expires_at_ms
        })
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
        movement_target: beastie_core::movement_target(state),
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
        want: if view.speaking || view.speech.is_some() {
            None
        } else {
            beastie_core::current_want(state)
        },
        refusing: refusal_subject(state),
        intent_target: intent_target(state),
        playing: state
            .creature
            .interaction_state
            .toy_interaction
            .as_ref()
            .is_some_and(|interaction| {
                interaction.outcome == beastie_core::ToyInteractionOutcome::Accepted
                    && interaction.contacts > 0
            })
            || state
                .creature
                .private_life
                .active
                .as_ref()
                .is_some_and(|activity| {
                    matches!(activity.kind, PrivateLifeKind::ToyPlay(_))
                        && activity.phase == ActivityPhase::Act
                }),
    }
}

/// The object the creature is deliberately heading for, if any.
fn intent_target(state: &WorldState) -> Option<NormalizedPosition> {
    let aquarium = &state.creature.aquarium;
    if let Some(action) = aquarium.action.as_ref()
        && action.food_outcome.is_none()
        && matches!(
            action.phase,
            ActionPhase::Notice
                | ActionPhase::Brake
                | ActionPhase::Gaze
                | ActionPhase::Turn
                | ActionPhase::Approach
        )
    {
        return beastie_core::destination_position(state, action.destination);
    }
    let activity_notice = state
        .creature
        .private_life
        .active
        .as_ref()
        .and_then(|activity| {
            (activity.phase == ActivityPhase::Notice).then(|| {
                // A chased bubble is its own marker; a ring around it reads as an empty circle.
                activity
                    .kind
                    .destination()
                    .and_then(|destination| beastie_core::destination_position(state, destination))
            })?
        });
    if activity_notice.is_some() {
        return activity_notice;
    }
    match aquarium.destination? {
        // A bubble chase shows the bubble itself; a swim to the glass marks nothing.
        SemanticDestination::Position(_) | SemanticDestination::Player => None,
        // Foraging is about the sand itself: mark the floor under the head, not the water.
        SemanticDestination::Bottom => {
            beastie_core::destination_position(state, SemanticDestination::Bottom)
                .map(|position| NormalizedPosition::new(position.x, NormalizedPosition::SCALE))
        }
        destination => beastie_core::destination_position(state, destination),
    }
}

/// What the creature is visibly saying no to: a refused toy offer or food it pushed away.
fn refusal_subject(state: &WorldState) -> Option<beastie_core::Meaning> {
    let creature = &state.creature;
    if let Some((meaning, until_ms)) = creature.refusing
        && state.elapsed_ms < until_ms
    {
        return Some(meaning);
    }
    if let Some(interaction) = creature.interaction_state.toy_interaction.as_ref()
        && interaction.outcome == beastie_core::ToyInteractionOutcome::Rejected
    {
        return Some(beastie_core::Meaning::Toy(interaction.toy));
    }
    creature.aquarium.action.as_ref().and_then(|action| {
        (action.food_outcome == Some(beastie_core::FoodOutcome::Rejected))
            .then_some(action.food)
            .flatten()
            .map(beastie_core::Meaning::Food)
    })
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
                highlight: if matches!((view.mode, kind), (UiMode::Context(UiTarget::Toy(selected)), ObjectKind::Toy(toy)) if selected == toy) { Highlight::Focus } else { highlight_for(view, &format!("target/object-{id}")) },
            })
        })
        .collect()
}

const RIPPLE_MS: u64 = 900;

fn effect_scenes(
    state: &WorldState,
    creature: &CreatureScene,
    view: &ViewState,
) -> Vec<EffectScene> {
    let mut effects = Vec::new();
    for (position, at) in &view.ripples {
        let elapsed_ms = state.elapsed_ms.saturating_sub(*at);
        if elapsed_ms < RIPPLE_MS {
            effects.push(EffectScene {
                owner: SemanticOwner::Ordinary,
                cue: PresentationCueKind::Ripple,
                position: *position,
                target: UiTarget::OpenWater,
                elapsed_ms,
            });
        }
    }
    for cue in &view.cue_queue {
        if cue.channel != PresentationChannel::Physical
            || state.elapsed_ms < cue.starts_at_ms
            || state.elapsed_ms >= cue.expires_at_ms
        {
            continue;
        }
        let toy = match cue.kind {
            PresentationCueKind::BallNudge => ToyId::Ball,
            PresentationCueKind::BellStrike => ToyId::Bell,
            PresentationCueKind::SockTug => ToyId::Sock,
            _ => continue,
        };
        if let Some(object) = state.aquarium.toy_states.get(&toy) {
            effects.push(EffectScene {
                owner: cue.owner,
                cue: cue.kind,
                position: object.position,
                target: UiTarget::Toy(toy),
                elapsed_ms: state.elapsed_ms.saturating_sub(cue.starts_at_ms),
            });
        }
    }
    if let Some(expression) = &creature.expression {
        effects.push(EffectScene {
            owner: expression.owner,
            cue: expression.cue,
            position: creature.position,
            target: UiTarget::Creature,
            elapsed_ms: expression.elapsed_ms,
        });
    }
    if let Some(activity) = &creature.private_life
        && activity.kind == PrivateLifeKind::OpenWaterDrift
        && matches!(
            activity.phase,
            ActivityPhase::Notice | ActivityPhase::Approach | ActivityPhase::Act
        )
        && !(activity.phase == ActivityPhase::Act && activity.elapsed_ms > 250)
    {
        // The bubble Mop is after, rising where it is headed, until the snap pops it.
        effects.push(EffectScene {
            owner: SemanticOwner::PrivateLife(activity.id),
            cue: PresentationCueKind::OpenWaterDrift,
            position: beastie_core::bubble_point(state, activity.id),
            target: UiTarget::OpenWater,
            elapsed_ms: activity.elapsed_ms,
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
    // The aquarium stays touchable under shallow menus: a world click both dismisses the menu
    // and acts, so nothing ever needs a separate close click first.
    let menu_open = matches!(
        view.mode,
        UiMode::Context(_) | UiMode::Inspect(_) | UiMode::ToyChoice | UiMode::FoodChoice
    );
    if !matches!(view.mode, UiMode::Compose) && !menu_open {
        return Vec::new();
    }
    let (x, y) = world_to_logical(state.creature.aquarium.position);
    let mut hits = Vec::new();
    if menu_open {
        hits.push(HitRegion {
            id: "world/dismiss".to_owned(),
            target: Some(UiTarget::OpenWater),
            action: UiAction::CloseContext,
            rect: Rect {
                x: 0,
                y: 0,
                w: LOGICAL_WIDTH,
                h: AQUARIUM_BOTTOM,
            },
            enabled: true,
            label: "Close".to_owned(),
            cursor: CursorKind::Default,
            shape: HitShape::World(UiTarget::OpenWater),
        });
    }
    if !menu_open {
        // Empty water is never a dead click: tapping the glass gets the creature's attention.
        hits.push(HitRegion {
            id: "world/water".to_owned(),
            target: Some(UiTarget::OpenWater),
            action: UiAction::TapWater,
            rect: Rect {
                x: 0,
                y: 0,
                w: LOGICAL_WIDTH,
                h: AQUARIUM_BOTTOM,
            },
            enabled: true,
            label: "Tap the glass".to_owned(),
            cursor: CursorKind::Default,
            shape: HitShape::World(UiTarget::OpenWater),
        });
    }
    hits.push(HitRegion {
        id: "target/creature".to_owned(),
        target: Some(UiTarget::Creature),
        action: UiAction::Comfort,
        rect: Rect {
            x: x - CREATURE_HIT_WIDTH / 2,
            y: y - CREATURE_HIT_HEIGHT / 2,
            w: CREATURE_HIT_WIDTH,
            h: CREATURE_HIT_HEIGHT,
        },
        enabled: true,
        label: format!("Pet {}", head_fit(&state.creature.name, 18)),
        cursor: CursorKind::Pointer,
        shape: HitShape::World(UiTarget::Creature),
    });
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
            action: match target {
                UiTarget::Toy(toy) => UiAction::Play(toy),
                _ => UiAction::OpenContext(target),
            },
            rect: Rect {
                x: x - 10,
                y: y - 10,
                w: 20,
                h: 20,
            },
            enabled: true,
            label: match target {
                UiTarget::Toy(_) => format!("Play with {}", head_fit(&label, 18)),
                _ => label,
            },
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
    let large = view.text_scale >= 2;
    add_interactive_panel(
        "compose/bar",
        Rect {
            x: 2,
            y: 150,
            w: 316,
            h: 28,
        },
        30,
        rects,
        hits,
    );
    let status = format!("{} · {}", summary.mood_label, summary.behavior);
    // The name opens the rarer verbs (inspect, rename); everyday care is one click elsewhere.
    // Its label carries the complete status, which the narrow rail may have to shorten.
    hits.push(hit(
        "compose/creature",
        Some(UiTarget::Creature),
        UiAction::OpenContext(UiTarget::Creature),
        Rect {
            x: 4,
            y: 152,
            w: 52,
            h: 24,
        },
        true,
        &format!(
            "{} is {status}. Click to look closer.",
            head_fit(&summary.name, 18)
        ),
    ));
    text.push(bounded_label(
        "compose/summary-name",
        &summary.name,
        Rect {
            x: 9,
            y: 151,
            w: 46,
            h: 11,
        },
        TextRole::Identity,
        35,
    ));
    text.push(bounded_label(
        "compose/summary-behavior",
        &status,
        Rect {
            x: 9,
            y: 162,
            w: 46,
            h: 15,
        },
        TextRole::Secondary,
        35,
    ));
    for (index, (toy, name)) in [
        (ToyId::Ball, "Ball"),
        (ToyId::Bell, "Bell"),
        (ToyId::Sock, "Sock"),
    ]
    .into_iter()
    .enumerate()
    {
        let area = Rect {
            x: 59 + index as i32 * 20,
            y: 154,
            w: 18,
            h: 20,
        };
        let id = format!("compose/toy-{index}");
        let active = matches!(view.mode, UiMode::Context(UiTarget::Toy(selected)) | UiMode::Inspect(UiTarget::Toy(selected)) if selected == toy);
        add_button_chrome(&id, area, true, active, 31, rects);
        hits.push(hit(
            &id,
            Some(UiTarget::Toy(toy)),
            UiAction::Play(toy),
            area,
            true,
            &format!("Play with {name}"),
        ));
        icons.push(IconCommand {
            id,
            kind: IconKind::Toy(toy),
            bounds: Rect {
                x: area.x + 3,
                y: area.y + 4,
                w: 12,
                h: 12,
            },
            layer: 35,
        });
    }
    // One click feeds: the chosen food drops in front of the creature.
    for (index, food) in [FoodId::Berry, FoodId::Mushroom, FoodId::Pellet]
        .into_iter()
        .enumerate()
    {
        let area = Rect {
            x: 121 + index as i32 * 20,
            y: 154,
            w: 18,
            h: 20,
        };
        let id = format!("compose/food-{}", food_name(food).to_lowercase());
        add_button_chrome(&id, area, true, false, 31, rects);
        hits.push(hit(
            &id,
            Some(UiTarget::Actions),
            UiAction::SelectFood(food),
            area,
            true,
            &format!("Feed {}", food_name(food).to_lowercase()),
        ));
        icons.push(IconCommand {
            id,
            kind: IconKind::FoodItem(food),
            bounds: Rect {
                x: area.x + 3,
                y: area.y + 4,
                w: 12,
                h: 12,
            },
            layer: 35,
        });
    }

    let input = Rect {
        x: 183,
        y: 155,
        w: 93,
        h: 18,
    };
    let send = Rect {
        x: 259,
        y: 156,
        w: 16,
        h: 16,
    };
    let naming = matches!(view.mode, UiMode::OnScreenKeyboard) && view.renaming_with_osk;
    let dedicated_input = matches!(view.mode, UiMode::Rename | UiMode::OnScreenKeyboard);
    if dedicated_input {
        rects.push(rounded_rect(
            "compose/input-background",
            input,
            UI_BUTTON_DISABLED,
            33,
            3,
            false,
        ));
        rects.push(rounded_rect(
            "compose/input-rim",
            input,
            [45, 66, 68, 255],
            34,
            3,
            true,
        ));
    } else {
        add_inset("compose/input", input, 31, rects);
    }
    let editing = !dedicated_input && (view.compose_engaged || !view.text_buffer.is_empty());
    let field_role = if editing {
        TextRole::Body
    } else {
        TextRole::Secondary
    };
    let capacity = (68.0 / (field_role.size(large) * 0.56)).floor() as usize;
    let value = if dedicated_input {
        if view.mode == UiMode::Rename || naming {
            "Naming your creature…"
        } else {
            "Keyboard open"
        }
        .to_owned()
    } else if view.pending && view.text_buffer.is_empty() {
        "Finding words…".to_owned()
    } else if view.text_buffer.is_empty() {
        format!("Talk to {}…", summary.name)
    } else {
        view.text_buffer.clone()
    };
    let keep_tail = !dedicated_input && !view.text_buffer.is_empty();
    let mut field_label = bounded_label(
        "compose/input-text",
        &if keep_tail {
            value
        } else {
            head_fit(&value, capacity)
        },
        Rect {
            x: 187,
            y: 155,
            w: 68,
            h: 18,
        },
        field_role,
        35,
    );
    field_label.muted = dedicated_input;
    field_label.keep_tail = keep_tail;
    if view.mode == UiMode::Compose
        && view.focused_region.as_deref() == Some("compose/input")
        && (view.compose_engaged || !view.text_buffer.is_empty())
    {
        field_label.input_state = Some(input_visual_state(view));
    }
    text.push(field_label);
    hits.push(hit(
        "compose/input",
        Some(UiTarget::ComposeField),
        UiAction::FocusCompose,
        Rect {
            w: input.w - 18,
            ..input
        },
        !dedicated_input,
        "Message",
    ));
    let enabled = !dedicated_input && !view.text_buffer.trim().is_empty();
    hits.push(hit(
        "compose/send",
        None,
        if naming {
            UiAction::SubmitName
        } else {
            UiAction::SubmitText
        },
        send,
        enabled,
        if naming { "Name" } else { "Send" },
    ));
    if enabled {
        rects.push(rounded_rect(
            "compose/send-background",
            send,
            UI_SELECTED,
            34,
            3,
            false,
        ));
    }
    icons.push(IconCommand {
        id: "ui/button-send".into(),
        kind: IconKind::Send,
        bounds: Rect {
            x: 264,
            y: 160,
            w: 8,
            h: 8,
        },
        layer: 35,
    });
    let microphone = Rect {
        x: 280,
        y: 155,
        w: 17,
        h: 18,
    };
    let available = view.microphone_enabled
        && !matches!(
            view.microphone_state,
            MicrophoneState::Disabled | MicrophoneState::Unavailable | MicrophoneState::Error
        );
    hits.push(hit(
        "compose/microphone",
        Some(UiTarget::Actions),
        UiAction::PushToTalk,
        microphone,
        available,
        microphone_label(view.microphone_state),
    ));
    add_button_chrome(
        "compose/microphone",
        microphone,
        available,
        matches!(
            view.microphone_state,
            MicrophoneState::Listening | MicrophoneState::Recognizing
        ),
        31,
        rects,
    );
    icons.push(IconCommand {
        id: "ui/button-microphone".into(),
        kind: IconKind::Microphone,
        bounds: Rect {
            x: 284,
            y: 159,
            w: 9,
            h: 10,
        },
        layer: 35,
    });
    let utility = Rect {
        x: 300,
        y: 155,
        w: 16,
        h: 18,
    };
    let compose = matches!(view.mode, UiMode::Compose);
    let id = if compose {
        "compose/settings"
    } else {
        "compose/close"
    };
    add_button_chrome(id, utility, true, false, 31, rects);
    hits.push(hit(
        id,
        Some(UiTarget::Actions),
        if compose {
            UiAction::OpenSettings
        } else {
            UiAction::CancelMode
        },
        utility,
        true,
        if compose { "Settings" } else { "Close" },
    ));
    icons.push(IconCommand {
        id: "ui/button-settings".into(),
        kind: if compose {
            IconKind::Settings
        } else {
            IconKind::Close
        },
        bounds: Rect {
            x: 304,
            y: 160,
            w: 8,
            h: 8,
        },
        layer: 35,
    });
}

fn head_fit(value: &str, capacity: usize) -> String {
    if value.graphemes(true).count() <= capacity {
        return value.to_owned();
    }
    match capacity {
        0 => String::new(),
        1 => "…".to_owned(),
        _ => format!(
            "{}…",
            value.graphemes(true).take(capacity - 1).collect::<String>()
        ),
    }
}

fn add_temporary_mode(
    state: &WorldState,
    view: &ViewState,
    icons: &mut Vec<IconCommand>,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    match view.mode {
        UiMode::Title => add_title(state, rects, text, hits),
        UiMode::Compose => {}
        UiMode::Inspect(target) => add_inspection(state, view, target, rects, text, hits),
        UiMode::Context(UiTarget::Toy(toy)) => add_toy_context(state, view, toy, rects, text, hits),
        UiMode::Context(target) => {
            let actions = contextual_actions(target);
            let (_, head_y) = world_to_logical(state.creature.aquarium.position);
            // Avoid the head when opened; thereafter the control positions stay stable.
            let above = view.context_above.unwrap_or(head_y > 74);
            let row_y = if above { 21 } else { 113 };
            add_action_strip("context", &actions, row_y, icons, rects, text, hits);
        }
        UiMode::FoodChoice => add_action_strip(
            "food",
            &[
                UiAction::SelectFood(FoodId::Berry),
                UiAction::SelectFood(FoodId::Mushroom),
                UiAction::SelectFood(FoodId::Pellet),
            ],
            100,
            icons,
            rects,
            text,
            hits,
        ),
        UiMode::ToyChoice => add_action_strip(
            "toy",
            &[
                UiAction::Play(ToyId::Ball),
                UiAction::Play(ToyId::Bell),
                UiAction::Play(ToyId::Sock),
            ],
            100,
            icons,
            rects,
            text,
            hits,
        ),
        UiMode::OnScreenKeyboard => add_keyboard(view, rects, text, hits),
        UiMode::Settings => add_settings(view, rects, text, hits),
        UiMode::Bindings => add_bindings(view, rects, text, hits),
        UiMode::Rebinding(action) => add_rebinding(action, rects, text, hits),
        UiMode::Rename => add_rename(state, view, rects, text, hits),
        UiMode::DataManagement => add_data_management(view, rects, text, hits),
        UiMode::ConfirmReset => add_reset_confirmation(rects, text, hits),
    }
}

fn add_title(
    state: &WorldState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    text.push(bounded_label(
        "title/subtitle",
        "A little friend, a big world.",
        Rect {
            x: 88,
            y: 61,
            w: 144,
            h: 12,
        },
        TextRole::Subtitle,
        29,
    ));
    // A creature still waiting to meet the player is met, not continued.
    let begin = if state.creature.hidden_until_met {
        format!("Meet {}", head_fit(&state.creature.name, 12))
    } else {
        "Continue".to_owned()
    };
    for (index, (id, name, action)) in [
        ("continue", begin.as_str(), UiAction::Continue),
        ("settings", "Settings", UiAction::OpenSettings),
        ("quit", "Quit", UiAction::Quit),
    ]
    .into_iter()
    .enumerate()
    {
        add_control(
            &format!("title/{id}"),
            name,
            action,
            Rect {
                x: 124,
                y: 79 + index as i32 * 20,
                w: 72,
                h: 16,
            },
            true,
            index == 0,
            25,
            rects,
            text,
            hits,
        );
    }
    text.push(bounded_label(
        "title/version",
        env!("CARGO_PKG_VERSION"),
        Rect {
            x: 8,
            y: 169,
            w: 35,
            h: 10,
        },
        TextRole::Secondary,
        29,
    ));
}

/// Choose clear water once, keeping the target still beneath the player's pointer.
#[must_use]
pub fn toy_context_anchor(state: &WorldState, toy: ToyId, text_scale: u8) -> (i32, i32) {
    let width = if text_scale >= 2 { 105 } else { 94 };
    let height = if text_scale >= 2 { 54 } else { 48 };
    let objects = object_scenes(state, &ViewState::default());
    let position = objects
        .iter()
        .find(|object| object.kind == ObjectKind::Toy(toy))
        .map_or(NormalizedPosition::new(5000, 5000), |object| {
            object.position
        });
    let (x, y) = world_to_logical(position);
    let (head_x, head_y) = world_to_logical(state.creature.aquarium.position);
    let mut occupied = vec![Rect {
        x: head_x - 29,
        y: head_y - 23,
        w: 58,
        h: 46,
    }];
    for object in &objects {
        let ObjectKind::Toy(kind) = object.kind else {
            continue;
        };
        let (x, y) = world_to_logical(object.position);
        occupied.push(if kind == ToyId::Bell {
            Rect {
                x: x - 13,
                y: y - 21,
                w: 26,
                h: 33,
            }
        } else {
            Rect {
                x: x - 14,
                y: y - 14,
                w: 28,
                h: 28,
            }
        });
    }
    let candidates = [
        (x + 17, y - 10),
        (x + 36, y - height - 12),
        (x - width - 20, y - height - 12),
        (x + 20, y + 18),
        (x - width - 20, y + 18),
        (x - width - 20, y - 10),
        (x - width / 2, y - height - 26),
        (x - width / 2, y + 24),
    ];
    candidates
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| {
            let x = x.clamp(4, 316 - width);
            let y = y.clamp(5, 132 - height);
            let overlap: i32 = occupied
                .iter()
                .map(|other| {
                    let w = ((x + width).min(other.x + other.w) - x.max(other.x)).max(0);
                    let h = ((y + height).min(other.y + other.h) - y.max(other.y)).max(0);
                    w * h
                })
                .sum();
            ((overlap, index), (x, y))
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, anchor)| anchor)
        .unwrap_or((4, 5))
}

fn add_toy_context(
    state: &WorldState,
    view: &ViewState,
    toy: ToyId,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    let large = view.text_scale >= 2;
    let width = if large { 105 } else { 94 };
    let height = if large { 54 } else { 48 };
    let (x, y) = view
        .context_card_anchor
        .unwrap_or_else(|| toy_context_anchor(state, toy, view.text_scale));
    let x = x.clamp(4, 316 - width);
    let y = y.clamp(5, 132 - height);
    add_interactive_panel(
        "mode/context-panel",
        Rect {
            x,
            y,
            w: width,
            h: height,
        },
        25,
        rects,
        hits,
    );
    let (name, detail) = match toy {
        ToyId::Ball => ("Beach ball", "A buoyant toy for nudging."),
        ToyId::Bell => ("Bell", "A little brass chime."),
        ToyId::Sock => ("Sock", "Something soft to tug."),
    };
    text.push(bounded_label(
        "mode/context/title",
        name,
        Rect {
            x: x + 6,
            y: y + 3,
            w: width - 12,
            h: 12,
        },
        TextRole::Body,
        29,
    ));
    text.push(bounded_label(
        "mode/context/detail",
        detail,
        Rect {
            x: x + 6,
            y: y + 15,
            w: width - 12,
            h: height - 38,
        },
        TextRole::Secondary,
        29,
    ));
    for (index, (action, name)) in [
        (UiAction::Play(toy), "Play"),
        (UiAction::Inspect, "Inspect"),
    ]
    .into_iter()
    .enumerate()
    {
        let button_width = (width - 18) / 2;
        add_control(
            &format!("action/{}", action_id(action)),
            name,
            action,
            Rect {
                x: x + 6 + index as i32 * (button_width + 6),
                y: y + height - 21,
                w: button_width,
                h: 15,
            },
            true,
            index == 0,
            25,
            rects,
            text,
            hits,
        );
    }
}

/// The words the player has taught, newest last: a record of shared history, not a meter.
fn learned_words_line(state: &WorldState) -> String {
    let words = state.creature.lexicon.learned_words();
    if words.is_empty() {
        return format!("{} doesn't know any words yet.", state.creature.name);
    }
    let shown = words
        .iter()
        .rev()
        .take(12)
        .rev()
        .map(|(word, _)| word.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("Words: {shown}")
}

fn add_inspection(
    state: &WorldState,
    view: &ViewState,
    target: UiTarget,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    let summary = creature_summary(state);
    let (title, detail) = match target {
        UiTarget::Creature => (
            summary.name.clone(),
            format!(
                "{} · {}\n{}{}",
                summary.mood_label,
                summary.behavior,
                learned_words_line(state),
                summary
                    .discovered_fact
                    .map_or(String::new(), |fact| format!("\n{} {fact}.", summary.name)),
            ),
        ),
        UiTarget::Toy(toy) => {
            let detail = if state
                .aquarium
                .toy_states
                .get(&toy)
                .is_some_and(|value| value.carried)
            {
                format!("{} is carrying this toy.", state.creature.name)
            } else {
                match toy {
                    ToyId::Ball => "A buoyant ball for nudges and play.",
                    ToyId::Bell => "A brass bell suspended beneath its float.",
                    ToyId::Sock => "A soft, bent sock for tugging.",
                }
                .to_owned()
            };
            (toy_name(toy).to_owned(), detail)
        }
        UiTarget::Cave => (
            "Shelter".into(),
            "A quiet hollow for resting and peeking out.".into(),
        ),
        UiTarget::Plant(id) => (
            "Aquarium plant".into(),
            if matches!(
                state.aquarium.objects.get(&id),
                Some(WorldObject::Plant { .. })
            ) {
                "Leaves to swim around and explore."
            } else {
                "This plant is no longer here."
            }
            .into(),
        ),
        UiTarget::FoodObject(id) => match state.aquarium.objects.get(&id) {
            Some(WorldObject::Food(food)) => (
                food_name(food.food).to_owned(),
                match food.disposition {
                    FoodDisposition::Falling => "Falling through the water.",
                    FoodDisposition::Floating => "Floating in the water.",
                    FoodDisposition::Settled => "Resting on the sand.",
                    FoodDisposition::Consumed => "Already eaten.",
                    FoodDisposition::Rejected => "Offered and rejected.",
                }
                .into(),
            ),
            _ => (
                "Food".into(),
                "This piece of food is no longer here.".into(),
            ),
        },
        _ => (
            "The aquarium".into(),
            format!(
                "{} is {}.\nA little room to live, explore and play.",
                state.creature.name, summary.behavior
            ),
        ),
    };
    let (head_x, _) = world_to_logical(state.creature.aquarium.position);
    let x = view
        .context_card_anchor
        .map_or(if head_x >= 160 { 7 } else { 173 }, |(x, _)| x)
        .clamp(4, 176);
    // Card height uses the same bundled faces and wrapping as glyph rendering.
    // Round the complete block once, keeping only the authored inter-part gaps.
    let text_height = |value: &str, role: TextRole| {
        let size = role.size(view.text_scale >= 2);
        typography().height(value, 124.0, size).ceil() as i32
    };
    let title_height = text_height(&title, TextRole::Identity).clamp(10, 33);
    let detail_height = text_height(&detail, TextRole::Body).max(8);
    let height = title_height
        .saturating_add(detail_height)
        .saturating_add(38)
        .min(143);
    let y = (AQUARIUM_BOTTOM - height - 2).clamp(4, 31);
    add_interactive_panel(
        "inspect/panel",
        Rect {
            x,
            y,
            w: 140,
            h: height,
        },
        25,
        rects,
        hits,
    );
    text.push(bounded_label(
        "inspect/title",
        &title,
        Rect {
            x: x + 8,
            y: y + 6,
            w: 124,
            h: title_height,
        },
        TextRole::Identity,
        29,
    ));
    let mut description = bounded_label(
        "inspect/detail",
        &detail,
        Rect {
            x: x + 8,
            y: y + 6 + title_height + 3,
            w: 124,
            h: height - title_height - 38,
        },
        TextRole::Body,
        29,
    );
    description.vertical_centered = false;
    text.push(description);
    add_control(
        "inspect/close",
        "Close",
        UiAction::CancelMode,
        Rect {
            x: x + 80,
            y: y + height - 22,
            w: 52,
            h: 16,
        },
        true,
        false,
        25,
        rects,
        text,
        hits,
    );
}

fn add_settings(
    view: &ViewState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_interactive_panel(
        "settings/panel",
        Rect {
            x: 171,
            y: 9,
            w: 140,
            h: 129,
        },
        25,
        rects,
        hits,
    );
    text.push(bounded_label(
        "settings/title",
        "Settings",
        Rect {
            x: 179,
            y: 14,
            w: 91,
            h: 15,
        },
        TextRole::Title,
        29,
    ));
    for (index, title) in ["Display", "Sound", "Controls"].into_iter().enumerate() {
        let area = Rect {
            x: 178 + index as i32 * 43,
            y: 33,
            w: 41,
            h: 15,
        };
        let id = format!("settings/page-{index}");
        let active = usize::from(view.settings_page.min(2)) == index;
        add_button_chrome(&id, area, true, active, 25, rects);
        hits.push(hit(
            &id,
            None,
            UiAction::SelectSettingsPage(index as u8),
            area,
            true,
            title,
        ));
        text.push(bounded_label(
            &format!("{id}-label"),
            title,
            Rect {
                x: area.x + 2,
                y: area.y + 2,
                w: area.w - 4,
                h: 11,
            },
            TextRole::Control,
            29,
        ));
    }
    if view.settings_page >= 2 {
        for (index, (id, title, detail, action)) in [
            (
                "bindings",
                "Input bindings",
                "Keyboard & controller",
                UiAction::OpenBindings,
            ),
            (
                "data",
                "Save & data",
                "Backup, transcript and reset",
                UiAction::OpenDataManagement,
            ),
            (
                "title",
                "Title screen",
                "Return to the main menu",
                UiAction::OpenTitle,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let y = 54 + index as i32 * 25;
            let area = Rect {
                x: 178,
                y,
                w: 126,
                h: 22,
            };
            add_button_chrome(&format!("settings/{id}"), area, true, false, 25, rects);
            hits.push(hit(
                &format!("settings/{id}"),
                None,
                action,
                area,
                true,
                title,
            ));
            text.push(bounded_label(
                &format!("settings/{id}-name"),
                title,
                Rect {
                    x: 183,
                    y: y + 1,
                    w: 115,
                    h: 11,
                },
                TextRole::Body,
                29,
            ));
            text.push(bounded_label(
                &format!("settings/{id}-detail"),
                detail,
                Rect {
                    x: 183,
                    y: y + 12,
                    w: 115,
                    h: 9,
                },
                TextRole::Secondary,
                29,
            ));
        }
        return;
    }
    let display = [
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
    ];
    let sound = [
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
            "Spoken replies",
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
    ];
    let settings = if view.settings_page == 0 {
        &display
    } else {
        &sound
    };
    for (index, (id, name, value, action)) in settings.iter().copied().enumerate() {
        let y = 52 + index as i32 * 14;
        let area = Rect {
            x: 178,
            y,
            w: 126,
            h: 14,
        };
        let control = Rect {
            x: 271,
            y: y + 1,
            w: 33,
            h: 12,
        };
        hits.push(hit(
            &format!("settings/{id}"),
            None,
            action,
            area,
            true,
            name,
        ));
        text.push(bounded_label(
            &format!("settings/{id}-name"),
            name,
            Rect {
                x: 180,
                y: y + 1,
                w: 89,
                h: 12,
            },
            TextRole::Secondary,
            29,
        ));
        if matches!(value, "On" | "Off") {
            add_toggle(
                &format!("settings/{id}"),
                control,
                value == "On",
                27,
                rects,
                text,
            );
        } else {
            add_inset(&format!("settings/{id}"), control, 25, rects);
            text.push(bounded_label(
                &format!("settings/{id}-value"),
                &format!("{value} ›"),
                Rect {
                    x: control.x + 2,
                    y: control.y + 1,
                    w: control.w - 4,
                    h: 10,
                },
                TextRole::ControlCaption,
                29,
            ));
        }
    }
}

fn add_toggle(
    id: &str,
    area: Rect,
    on: bool,
    layer: i16,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
) {
    let track = Rect {
        x: area.x + area.w - 17,
        y: area.y + (area.h - 8) / 2,
        w: 17,
        h: 8,
    };
    rects.push(rounded_rect(
        &format!("{id}-track"),
        track,
        if on {
            [71, 128, 110, 255]
        } else {
            [39, 65, 70, 255]
        },
        layer,
        4,
        false,
    ));
    rects.push(rounded_rect(
        &format!("{id}-thumb"),
        Rect {
            x: track.x + if on { 10 } else { 1 },
            y: track.y + 1,
            w: 6,
            h: 6,
        },
        UI_PRIMARY,
        layer + 1,
        3,
        false,
    ));
    text.push(bounded_label(
        &format!("{id}-value"),
        on_off(on),
        Rect {
            x: area.x,
            y: area.y,
            w: area.w - 19,
            h: area.h,
        },
        TextRole::ControlCaption,
        layer + 2,
    ));
}

fn add_rename(
    state: &WorldState,
    view: &ViewState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_interactive_panel(
        "rename/prompt-background",
        Rect {
            x: 71,
            y: 39,
            w: 178,
            h: 92,
        },
        25,
        rects,
        hits,
    );
    text.push(bounded_label(
        "rename/title",
        "Name your creature",
        Rect {
            x: 79,
            y: 43,
            w: 162,
            h: 15,
        },
        TextRole::Title,
        29,
    ));
    text.push(bounded_label(
        "rename/prompt",
        &format!("Currently {}", state.creature.name),
        Rect {
            x: 79,
            y: 59,
            w: 162,
            h: 11,
        },
        TextRole::Secondary,
        29,
    ));
    let field = Rect {
        x: 79,
        y: 73,
        w: 162,
        h: 20,
    };
    add_inset("rename/input", field, 25, rects);
    hits.push(hit(
        "rename/input",
        Some(UiTarget::ComposeField),
        UiAction::FocusCompose,
        field,
        true,
        "Creature name",
    ));
    let name = if view.text_buffer.is_empty() {
        "New name…"
    } else {
        &view.text_buffer
    };
    let mut name_label = bounded_label(
        "rename/value",
        name,
        Rect {
            x: 84,
            y: 74,
            w: 118,
            h: 18,
        },
        TextRole::Body,
        29,
    );
    name_label.keep_tail = !view.text_buffer.is_empty();
    if view.focused_region.as_deref() == Some("rename/input") {
        name_label.input_state = Some(input_visual_state(view));
    }
    text.push(name_label);
    text.push(bounded_label(
        "rename/count",
        &format!("{} / 24", view.text_buffer.chars().count()),
        Rect {
            x: 205,
            y: 75,
            w: 32,
            h: 16,
        },
        TextRole::ControlCaption,
        29,
    ));
    text.push(bounded_label(
        "rename/hint",
        "Enter to save · Esc to cancel",
        Rect {
            x: 79,
            y: 95,
            w: 162,
            h: 11,
        },
        TextRole::Secondary,
        29,
    ));
    add_control(
        "rename/cancel",
        "Cancel",
        UiAction::CancelMode,
        Rect {
            x: 79,
            y: 110,
            w: 77,
            h: 16,
        },
        true,
        false,
        25,
        rects,
        text,
        hits,
    );
    add_control(
        "rename/submit",
        "Save name",
        UiAction::SubmitName,
        Rect {
            x: 164,
            y: 110,
            w: 77,
            h: 16,
        },
        !view.text_buffer.trim().is_empty() && view.text_buffer.chars().count() <= 24,
        true,
        25,
        rects,
        text,
        hits,
    );
}

fn add_data_management(
    view: &ViewState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_interactive_panel(
        "data/panel",
        Rect {
            x: 119,
            y: 8,
            w: 192,
            h: 133,
        },
        25,
        rects,
        hits,
    );
    text.push(bounded_label(
        "data/title",
        "Save & data",
        Rect {
            x: 127,
            y: 13,
            w: 128,
            h: 16,
        },
        TextRole::Title,
        29,
    ));
    for (index, (id, title, detail, value, action, enabled)) in [
        (
            "recover",
            "Recover backup",
            "Previous saved state",
            "Recover",
            UiAction::RecoverBackup,
            true,
        ),
        (
            "transcript",
            "Transcript",
            "Save conversations locally",
            on_off(view.transcript_enabled),
            UiAction::ToggleTranscript,
            true,
        ),
        (
            "export",
            "Export transcript",
            "Copy the local transcript",
            "Export",
            UiAction::ExportTranscript,
            view.transcript_enabled,
        ),
        (
            "reset",
            "Reset creature",
            "Keep a recoverable backup",
            "Reset",
            UiAction::RequestReset,
            true,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let y = 35 + index as i32 * 25;
        let area = Rect {
            x: 127,
            y,
            w: 176,
            h: 23,
        };
        rects.push(rounded_rect(
            &format!("data/{id}-row"),
            area,
            UI_BUTTON,
            26,
            3,
            false,
        ));
        text.push(bounded_label(
            &format!("data/{id}-title"),
            title,
            Rect {
                x: 131,
                y: y + 1,
                w: 126,
                h: 11,
            },
            TextRole::Body,
            29,
        ));
        text.push(bounded_label(
            &format!("data/{id}-detail"),
            detail,
            Rect {
                x: 131,
                y: y + 12,
                w: 126,
                h: 10,
            },
            TextRole::Secondary,
            29,
        ));
        let control = Rect {
            x: 262,
            y: y + 4,
            w: 37,
            h: 16,
        };
        if id == "transcript" {
            hits.push(hit("data/transcript", None, action, control, true, title));
            add_toggle(
                "data/transcript",
                control,
                view.transcript_enabled,
                27,
                rects,
                text,
            );
        } else {
            add_control(
                &format!("data/{id}"),
                value,
                action,
                control,
                enabled,
                false,
                25,
                rects,
                text,
                hits,
            );
            if id == "reset" {
                rects.push(rounded_rect(
                    "data/reset-danger",
                    control,
                    UI_DANGER,
                    29,
                    3,
                    true,
                ));
            }
        }
    }
}

fn add_reset_confirmation(
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_interactive_panel(
        "reset/panel",
        Rect {
            x: 58,
            y: 40,
            w: 204,
            h: 77,
        },
        29,
        rects,
        hits,
    );
    text.push(bounded_label(
        "reset/warning",
        "Start again?",
        Rect {
            x: 68,
            y: 47,
            w: 184,
            h: 15,
        },
        TextRole::Title,
        33,
    ));
    text.push(bounded_label(
        "reset/detail",
        "Your current creature will be replaced.\nYour backup stays recoverable.",
        Rect {
            x: 68,
            y: 64,
            w: 184,
            h: 25,
        },
        TextRole::Secondary,
        33,
    ));
    add_control(
        "reset/cancel",
        "Cancel",
        UiAction::CancelMode,
        Rect {
            x: 68,
            y: 94,
            w: 88,
            h: 17,
        },
        true,
        true,
        29,
        rects,
        text,
        hits,
    );
    let reset = Rect {
        x: 164,
        y: 94,
        w: 88,
        h: 17,
    };
    add_control(
        "reset/confirm",
        "Reset creature",
        UiAction::ConfirmReset,
        reset,
        true,
        false,
        29,
        rects,
        text,
        hits,
    );
    rects.push(rounded_rect(
        "reset/confirm-danger",
        reset,
        UI_DANGER,
        33,
        3,
        true,
    ));
}

fn add_bindings(
    view: &ViewState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_interactive_panel(
        "bindings/panel",
        Rect {
            x: 126,
            y: 6,
            w: 186,
            h: 139,
        },
        25,
        rects,
        hits,
    );
    text.push(bounded_label(
        "bindings/title",
        "Input bindings",
        Rect {
            x: 134,
            y: 12,
            w: 133,
            h: 16,
        },
        TextRole::Title,
        29,
    ));
    for (index, (action, name, binding)) in [
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
    ]
    .into_iter()
    .enumerate()
    {
        let y = 33 + index as i32 * 15;
        let id = format!("bindings/{}", bindable_id(action));
        text.push(bounded_label(
            &format!("{id}-name"),
            name,
            Rect {
                x: 135,
                y,
                w: 75,
                h: 14,
            },
            TextRole::Secondary,
            29,
        ));
        add_control(
            &id,
            binding,
            UiAction::BeginRebind(action),
            Rect {
                x: 215,
                y,
                w: 88,
                h: 14,
            },
            true,
            false,
            25,
            rects,
            text,
            hits,
        );
    }
    add_control(
        "settings/reset-bindings",
        "Reset keys",
        UiAction::ResetBindings,
        Rect {
            x: 235,
            y: 126,
            w: 68,
            h: 14,
        },
        true,
        false,
        25,
        rects,
        text,
        hits,
    );
    text.push(bounded_label(
        "bindings/hint",
        "Select a key to change it.",
        Rect {
            x: 135,
            y: 125,
            w: 96,
            h: 16,
        },
        TextRole::Secondary,
        29,
    ));
}

fn add_rebinding(
    action: BindableAction,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_interactive_panel(
        "bindings/capture/panel",
        Rect {
            x: 77,
            y: 39,
            w: 166,
            h: 80,
        },
        29,
        rects,
        hits,
    );
    text.push(bounded_label(
        "bindings/capture-prompt",
        &format!("Press a key for {}", bindable_name(action)),
        Rect {
            x: 85,
            y: 44,
            w: 150,
            h: 22,
        },
        TextRole::Body,
        33,
    ));
    add_inset(
        "bindings/waiting",
        Rect {
            x: 138,
            y: 69,
            w: 44,
            h: 20,
        },
        29,
        rects,
    );
    text.push(bounded_label(
        "bindings/waiting-label",
        "…",
        Rect {
            x: 141,
            y: 71,
            w: 38,
            h: 16,
        },
        TextRole::Control,
        33,
    ));
    text.push(bounded_label(
        "bindings/capture-cancel",
        "Esc cancels",
        Rect {
            x: 85,
            y: 96,
            w: 75,
            h: 15,
        },
        TextRole::Secondary,
        33,
    ));
    add_control(
        "bindings/cancel",
        "Cancel",
        UiAction::CancelMode,
        Rect {
            x: 177,
            y: 96,
            w: 58,
            h: 16,
        },
        true,
        false,
        29,
        rects,
        text,
        hits,
    );
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
        MicrophoneState::Disabled => "Mic off. Enable in Settings › Sound.",
        MicrophoneState::Idle => "Hold to talk; release to send.",
        MicrophoneState::Listening => "Listening; release to send",
        MicrophoneState::Recognizing => "Recognizing speech",
        MicrophoneState::Unavailable => {
            "Microphone unavailable. Check your input device; text still works."
        }
        MicrophoneState::Error => {
            "Microphone error. Check your input device and retry; text still works."
        }
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

#[allow(clippy::too_many_arguments)]
fn add_action_strip(
    id: &str,
    actions: &[UiAction],
    y: i32,
    icons: &mut Vec<IconCommand>,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    let context = id == "context";
    let card_height = if context { 26 } else { 34 };
    let item_width = if actions.len() >= 4 { 60 } else { 54 };
    let width = (item_width + 6) * actions.len() as i32 + 6;
    let start = (LOGICAL_WIDTH - width) / 2;
    add_interactive_panel(
        &format!("mode/{id}-panel"),
        Rect {
            x: start,
            y: y - 18,
            w: width,
            h: card_height + 24,
        },
        25,
        rects,
        hits,
    );
    text.push(bounded_label(
        &format!("mode/{id}/title"),
        match id {
            "food" => "Choose food",
            "toy" => "Choose a toy",
            _ => "Look closer",
        },
        Rect {
            x: start + 6,
            y: y - 16,
            w: width - 12,
            h: 14,
        },
        TextRole::Body,
        29,
    ));
    for (index, action) in actions.iter().copied().enumerate() {
        let area = Rect {
            x: start + 6 + index as i32 * (item_width + 6),
            y,
            w: item_width,
            h: card_height,
        };
        let region = format!("action/{}", action_id(action));
        let (kind, name) = match action {
            UiAction::SelectFood(food) => (
                IconKind::FoodItem(food),
                match food {
                    FoodId::Berry => "Berry",
                    FoodId::Mushroom => "Mushroom",
                    FoodId::Pellet => "Pellet",
                }
                .to_owned(),
            ),
            UiAction::Play(toy) => (
                IconKind::Toy(toy),
                match toy {
                    ToyId::Ball => "Ball",
                    ToyId::Bell => "Bell",
                    ToyId::Sock => "Sock",
                }
                .to_owned(),
            ),
            UiAction::Comfort => (IconKind::Heart, "Comfort".to_owned()),
            UiAction::OpenToyChoice => (IconKind::Play, "Play".to_owned()),
            UiAction::Rename => (IconKind::Rename, "Rename".to_owned()),
            UiAction::Inspect => (IconKind::Inspect, "Inspect".to_owned()),
            _ => (IconKind::Inspect, action_label(action)),
        };
        add_button_chrome(&region, area, true, false, 25, rects);
        hits.push(hit(&region, None, action, area, true, &name));
        icons.push(IconCommand {
            id: region.clone(),
            kind,
            bounds: if context {
                Rect {
                    x: area.x + 6,
                    y: area.y + 8,
                    w: 10,
                    h: 10,
                }
            } else {
                Rect {
                    x: area.x + (area.w - 14) / 2,
                    y: area.y + 4,
                    w: 14,
                    h: 14,
                }
            },
            layer: 29,
        });
        text.push(bounded_label(
            &format!("{region}-label"),
            &name,
            if context {
                Rect {
                    x: area.x + 20,
                    y: area.y + 3,
                    w: area.w - 24,
                    h: 20,
                }
            } else {
                Rect {
                    x: area.x + 3,
                    y: area.y + 20,
                    w: area.w - 6,
                    h: 12,
                }
            },
            TextRole::Control,
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
    add_interactive_panel(
        "keyboard/panel",
        Rect {
            x: 5,
            y: 80,
            w: 310,
            h: 67,
        },
        25,
        rects,
        hits,
    );
    text.push(bounded_label(
        "keyboard/title",
        if view.renaming_with_osk {
            "Name your creature"
        } else {
            "Talk to your creature"
        },
        Rect {
            x: 12,
            y: 82,
            w: 235,
            h: 10,
        },
        TextRole::Body,
        29,
    ));
    if view.renaming_with_osk {
        text.push(bounded_label(
            "keyboard/count",
            &format!("{} / 24", view.text_buffer.chars().count()),
            Rect {
                x: 266,
                y: 82,
                w: 40,
                h: 10,
            },
            TextRole::ControlCaption,
            29,
        ));
    }
    add_inset(
        "keyboard/input",
        Rect {
            x: 12,
            y: 93,
            w: 296,
            h: 14,
        },
        25,
        rects,
    );
    let value = if view.text_buffer.is_empty() {
        if view.renaming_with_osk {
            "New name…"
        } else {
            "A thought for your creature…"
        }
        .to_owned()
    } else {
        view.text_buffer.clone()
    };
    let mut input_label = bounded_label(
        "keyboard/input-text",
        &value,
        Rect {
            x: 17,
            y: 93,
            w: 286,
            h: 14,
        },
        TextRole::Body,
        29,
    );
    input_label.keep_tail = !view.text_buffer.is_empty();
    input_label.input_state = Some(input_visual_state(view));
    text.push(input_label);
    for (index, character) in "ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars().enumerate() {
        let area = Rect {
            x: 12 + (index % 13) as i32 * 23,
            y: 108 + (index / 13) as i32 * 13,
            w: 20,
            h: 12,
        };
        add_key(
            &format!("keyboard/{}", character.to_ascii_lowercase()),
            &character.to_string(),
            UiAction::TypeCharacter(character.to_ascii_lowercase()),
            area,
            true,
            rects,
            text,
            hits,
        );
    }
    for (index, (id, character)) in [
        (".", '.'),
        (",", ','),
        ("question", '?'),
        ("exclamation", '!'),
    ]
    .into_iter()
    .enumerate()
    {
        add_key(
            &format!("keyboard/{id}"),
            &character.to_string(),
            UiAction::TypeCharacter(character),
            Rect {
                x: 12 + index as i32 * 18,
                y: 134,
                w: 15,
                h: 12,
            },
            true,
            rects,
            text,
            hits,
        );
    }
    for (id, name, action, x, w, enabled) in [
        ("space", "Space", UiAction::TypeCharacter(' '), 84, 67, true),
        ("delete", "Delete", UiAction::Backspace, 154, 48, true),
        ("cancel", "Close", UiAction::CancelMode, 205, 43, true),
        (
            "send",
            if view.renaming_with_osk {
                "Save name"
            } else {
                "Send"
            },
            if view.renaming_with_osk {
                UiAction::SubmitName
            } else {
                UiAction::SubmitText
            },
            251,
            57,
            !view.text_buffer.trim().is_empty(),
        ),
    ] {
        add_control(
            &format!("keyboard/{id}"),
            name,
            action,
            Rect {
                x,
                y: 134,
                w,
                h: 12,
            },
            enabled,
            id == "send",
            25,
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
    add_control(
        id, shown, action, key, enabled, false, 25, rects, text, hits,
    );
}

/// Logical half extents of the creature's head around its projected center, with a little
/// room for animation; the speech bubble never covers this box.
const HEAD_HALF_WIDTH: i32 = 14;
const HEAD_HALF_HEIGHT: i32 = 14;
/// Space between the head box and the bubble, which the bubble's tail crosses.
const SPEECH_TAIL_GAP: i32 = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BubbleSide {
    Above,
    Below,
    Right,
    Left,
}

fn add_speech(
    state: &WorldState,
    view: &ViewState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
) {
    if speech_obstructed(view.mode) {
        return;
    }
    let Some(speech) = view
        .speech
        .as_deref()
        .filter(|line| !line.trim().is_empty())
    else {
        return;
    };
    // The creature's lines are short and its own: a small bubble at its head, sized to the whole
    // utterance so the reveal never reflows it, with a pixel tail pointing at the speaker.
    let layout_text = view.speech_layout_text.as_deref().unwrap_or(speech);
    let revealed = speech_revealed_bytes(layout_text, speech);
    let shown = &layout_text[..revealed.min(layout_text.len())];
    let font_size = TextRole::Dialogue.size(view.text_scale >= 2);
    let natural = typography().width(layout_text, font_size).ceil() as i32;
    let text_width = natural.clamp(14, SPEECH_TEXT_WIDTH);
    // Lines are short by design. A malformed overlong reply is bounded and ellipsized rather
    // than paged, so the bubble always stays a bubble.
    let lines = typography()
        .lines(layout_text, text_width as f32, font_size)
        .len()
        .clamp(1, SPEECH_MAX_LINES);
    let body_height = (lines as f32 * line_height(font_size)).ceil() as i32;
    let width = text_width + 14;
    let height = body_height + 10;
    let bubble = speech_bubble_rect(state, view.mode, width, height);
    let (head_x, head_y) = world_to_logical(state.creature.aquarium.position);
    let side = bubble.1;
    let bubble = bubble.0;
    // The bubble is see-through to input: clicks beside the head still reach the creature or
    // the water, so a line of speech never creates a dead click.
    add_panel_chrome("speech/panel", bubble, 12, rects);
    // Three stepped pixels lead from the bubble toward the head.
    for step in 0..3 {
        let size = 3 - step;
        // Each step shrinks and moves two pixels on, so the tail stays inside the gap.
        let (x, y) = match side {
            BubbleSide::Above | BubbleSide::Below => {
                let x = (head_x + 2).clamp(bubble.x + 4, bubble.x + bubble.w - 6) - step;
                let y = if side == BubbleSide::Above {
                    bubble.y + bubble.h + step * 2
                } else {
                    bubble.y - size - step * 2
                };
                (x, y)
            }
            BubbleSide::Right | BubbleSide::Left => {
                let y = head_y.clamp(bubble.y + 4, bubble.y + bubble.h - 6) - step;
                let x = if side == BubbleSide::Left {
                    bubble.x + bubble.w + step * 2
                } else {
                    bubble.x - size - step * 2
                };
                (x, y)
            }
        };
        rects.push(rounded_rect(
            &format!("speech/tail-{step}"),
            Rect {
                x,
                y,
                w: size,
                h: size,
            },
            UI_CORAL,
            13,
            0,
            false,
        ));
    }
    let mut caption = label("speech/text", shown, bubble.x + 7, bubble.y + 5, 16);
    caption.role = TextRole::Dialogue;
    caption.bounds = Some(Rect {
        x: bubble.x + 7,
        y: bubble.y + 5,
        w: text_width,
        h: body_height,
    });
    text.push(caption);
}

/// The creature's projected head box in logical coordinates.
fn head_box(state: &WorldState) -> Rect {
    let (x, y) = world_to_logical(state.creature.aquarium.position);
    Rect {
        x: x - HEAD_HALF_WIDTH,
        y: y - HEAD_HALF_HEIGHT,
        w: HEAD_HALF_WIDTH * 2,
        h: HEAD_HALF_HEIGHT * 2,
    }
}

/// Place the bubble above the head when it fits, then below, then beside, always inside the
/// water, clear of the rail, and never over the head. Settings keeps its panel column clear.
fn speech_bubble_rect(
    state: &WorldState,
    mode: UiMode,
    width: i32,
    height: i32,
) -> (Rect, BubbleSide) {
    let (head_x, head_y) = world_to_logical(state.creature.aquarium.position);
    let head = head_box(state);
    let min_x = 4;
    let max_x = if mode == UiMode::Settings {
        166
    } else {
        LOGICAL_WIDTH - 4
    };
    let (min_y, max_y) = (4, AQUARIUM_BOTTOM - 2);
    let clamp_x = |x: i32| x.clamp(min_x, (max_x - width).max(min_x));
    let clamp_y = |y: i32| y.clamp(min_y, (max_y - height).max(min_y));
    let centered_x = clamp_x(head_x + 6 - width / 3);
    let beside_y = clamp_y(head_y - height / 2);
    let candidates = [
        (
            Rect {
                x: centered_x,
                y: head.y - SPEECH_TAIL_GAP - height,
                w: width,
                h: height,
            },
            BubbleSide::Above,
        ),
        (
            Rect {
                x: centered_x,
                y: head.y + head.h + SPEECH_TAIL_GAP,
                w: width,
                h: height,
            },
            BubbleSide::Below,
        ),
        (
            Rect {
                x: head.x + head.w + SPEECH_TAIL_GAP,
                y: beside_y,
                w: width,
                h: height,
            },
            BubbleSide::Right,
        ),
        (
            Rect {
                x: head.x - SPEECH_TAIL_GAP - width,
                y: beside_y,
                w: width,
                h: height,
            },
            BubbleSide::Left,
        ),
    ];
    let fits = |rect: Rect| {
        rect.x >= min_x && rect.x + rect.w <= max_x && rect.y >= min_y && rect.y + rect.h <= max_y
    };
    candidates
        .into_iter()
        .find(|(rect, _)| fits(*rect) && !rectangles_overlap(*rect, head))
        .unwrap_or_else(|| {
            // Nothing fits whole: stay inside the water on the roomier side of the head.
            let (rect, side) = if head_x < (min_x + max_x) / 2 {
                candidates[2]
            } else {
                candidates[3]
            };
            (
                Rect {
                    x: clamp_x(rect.x),
                    y: clamp_y(rect.y),
                    ..rect
                },
                side,
            )
        })
}

/// Temporary care and editing surfaces own the water while open. Settings leaves
/// a complete caption column beside it; other modes cannot guarantee that space.
fn speech_obstructed(mode: UiMode) -> bool {
    !matches!(mode, UiMode::Compose | UiMode::Settings)
}

/// Reveal is a prefix of the owned utterance, rounded down to whole graphemes.
fn speech_revealed_bytes(layout: &str, speech: &str) -> usize {
    if !layout.starts_with(speech) {
        return 0;
    }
    layout
        .grapheme_indices(true)
        .take_while(|(start, grapheme)| start + grapheme.len() <= speech.len())
        .last()
        .map_or(0, |(start, grapheme)| start + grapheme.len())
}

fn add_status(
    view: &ViewState,
    now_ms: u64,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    let Some(message) = visible_status(view, now_ms) else {
        return;
    };
    let dismissible = view.status_message.is_some()
        || view.transcript_status.is_some()
        || matches!(
            view.microphone_state,
            MicrophoneState::Unavailable | MicrophoneState::Error
        );
    let side_width = match view.mode {
        UiMode::Settings => Some(156),
        UiMode::Bindings => Some(111),
        UiMode::DataManagement => Some(104),
        _ => None,
    };
    let panel_width = side_width.unwrap_or(296);
    let x = if side_width.is_some() { 7 } else { 12 };
    let width = panel_width
        - if dismissible && side_width.is_none() {
            49
        } else {
            14
        };
    let size = TextRole::Secondary.size(view.text_scale >= 2);
    let lines = typography()
        .lines(message, width as f32, size)
        .len()
        .clamp(1, 8);
    let footer = if dismissible && side_width.is_some() {
        19
    } else {
        0
    };
    let h = ((lines as f32 * line_height(size)).ceil() as i32 + 10).max(20) + footer;
    let y = if side_width.is_some()
        || matches!(
            view.mode,
            UiMode::FoodChoice | UiMode::ToyChoice | UiMode::OnScreenKeyboard
        ) {
        5
    } else {
        [148 - h, 5]
            .into_iter()
            .enumerate()
            .min_by_key(|(preference, y)| {
                (
                    panel_overlap(
                        Rect {
                            x,
                            y: *y,
                            w: panel_width,
                            h,
                        },
                        rects,
                    ),
                    *preference,
                )
            })
            .map_or(148 - h, |(_, y)| y)
    };
    add_interactive_panel(
        "status/background",
        Rect {
            x,
            y,
            w: panel_width,
            h,
        },
        36,
        rects,
        hits,
    );
    text.push(bounded_label(
        "status/message",
        message,
        Rect {
            x: x + 7,
            y: y + 5,
            w: width,
            h: h - 10 - footer,
        },
        TextRole::Secondary,
        40,
    ));
    if dismissible {
        add_control(
            "status/dismiss",
            "Close",
            UiAction::DismissStatus,
            Rect {
                x: x + panel_width - 37,
                y: if footer > 0 { y + h - 19 } else { y + 3 },
                w: 31,
                h: 14,
            },
            true,
            false,
            36,
            rects,
            text,
            hits,
        );
    }
}

/// The next thing worth trying, while the player is still learning how Mop learns. Hints come
/// from the creature's actual state and disappear once the player has done the thing.
#[must_use]
pub fn coaching_hint(state: &WorldState, view: &ViewState) -> Option<String> {
    if view.mode != UiMode::Compose || !view.text_buffer.is_empty() {
        return None;
    }
    let creature = &state.creature;
    let name = head_fit(&creature.name, 16);
    let words = creature.lexicon.learned_count();
    let counters = creature.development.interactions;
    if words >= 5 && counters.requests >= 1 {
        return None;
    }
    let want = beastie_core::current_want(state);
    let thing = |meaning: beastie_core::Meaning| match meaning {
        beastie_core::Meaning::Toy(toy) => toy_name(toy).to_lowercase(),
        beastie_core::Meaning::Food(food) => food_name(food).to_lowercase(),
        _ => "that".to_owned(),
    };
    // One hearing is never a lesson; the first echo is the moment to say it again.
    if words == 0
        && creature.lexicon.words.values().any(|word| {
            word.heard == 1
                && !word.evidence.is_empty()
                && state.elapsed_ms.saturating_sub(word.last_heard_ms) < 15_000
        })
    {
        return Some(format!("{name} tried to say it! Say it once more."));
    }
    if let Some(beastie_core::Want::NameOf(meaning)) = want
        && words < 2
    {
        return Some(format!(
            "{name} wonders what the {} is called. Type its name!",
            thing(meaning)
        ));
    }
    if matches!(want, Some(beastie_core::Want::Food(_))) && counters.feeds == 0 {
        return Some(format!("{name} is hungry. Pick a food below."));
    }
    if words == 0 {
        return Some(if counters.plays == 0 {
            format!("Click a toy to play with {name}.")
        } else {
            format!("Talk to {name} while it plays. It learns your words.")
        });
    }
    if counters.requests == 0 {
        let (word, _) = creature.lexicon.learned_words().last().cloned()?;
        return Some(format!("{name} knows “{word}”. Say it to ask for it."));
    }
    if creature
        .lexicon
        .word_for(beastie_core::Meaning::Creature)
        .is_none()
        && matches!(
            creature.aquarium.gaze,
            GazeTarget::Player | GazeTarget::Cursor
        )
    {
        return Some(format!(
            "{name} doesn't know its name yet. Say it while it looks at you."
        ));
    }
    None
}

fn add_coaching(
    state: &WorldState,
    view: &ViewState,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
) {
    let Some(hint) = coaching_hint(state, view) else {
        return;
    };
    let size = TextRole::Secondary.size(view.text_scale >= 2);
    let width = (typography().width(&hint, size).ceil() as i32 + 16).min(260);
    let height = typography().height(&hint, (width - 16) as f32, size).ceil() as i32 + 8;
    let x = LOGICAL_WIDTH / 2 - width / 2;
    let candidates = [5, 148 - height];
    let y = candidates
        .into_iter()
        .min_by_key(|y| {
            panel_overlap(
                Rect {
                    x,
                    y: *y,
                    w: width,
                    h: height,
                },
                rects,
            )
        })
        .unwrap_or(5);
    rects.push(rounded_rect(
        "coach/background",
        Rect {
            x,
            y,
            w: width,
            h: height,
        },
        [12, 42, 47, 214],
        34,
        4,
        false,
    ));
    text.push(bounded_label(
        "coach/hint",
        &hint,
        Rect {
            x: x + 8,
            y: y + 4,
            w: width - 16,
            h: height - 8,
        },
        TextRole::Secondary,
        38,
    ));
}

fn panel_overlap(area: Rect, rects: &[RectCommand]) -> i32 {
    rects
        .iter()
        .filter(|part| {
            !part.outline
                && (part.id.contains("/panel")
                    || part.id.ends_with("-panel")
                    || part.id.ends_with("-background"))
        })
        .map(|part| {
            let w =
                ((area.x + area.w).min(part.rect.x + part.rect.w) - area.x.max(part.rect.x)).max(0);
            let h =
                ((area.y + area.h).min(part.rect.y + part.rect.h) - area.y.max(part.rect.y)).max(0);
            w * h
        })
        .sum()
}

fn visible_status(view: &ViewState, now_ms: u64) -> Option<&str> {
    view.status_message
        .as_deref()
        .or(
            if view.dismissed_microphone_notice == Some(view.microphone_state) {
                None
            } else {
                match view.microphone_state {
                    MicrophoneState::Listening => Some("Listening... release to send."),
                    MicrophoneState::Recognizing => Some("Working out what you said..."),
                    MicrophoneState::Unavailable => {
                        Some("Microphone unavailable. Check input device. Text still works.")
                    }
                    MicrophoneState::Error => {
                        Some("Speech input failed. Check input device and retry. Text still works.")
                    }
                    MicrophoneState::Disabled | MicrophoneState::Idle => None,
                }
            },
        )
        .or(view.transcript_status.as_deref())
        .or_else(|| {
            matches!(
                view.active_cue(now_ms),
                Some(PresentationCueKind::AquariumFull)
            )
            .then_some("Tank is full. Wait for old food to clear.")
        })
}

fn add_hover_and_focus(
    view: &ViewState,
    now_ms: u64,
    hits: &[HitRegion],
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
) {
    for (id, color, command_id) in [
        (
            view.hovered_region
                .as_deref()
                .filter(|hovered| Some(*hovered) != view.focused_region.as_deref()),
            [138, 206, 186, 255],
            "ui/hover",
        ),
        (
            view.focused_region.as_deref(),
            [250, 233, 178, 255],
            "ui/focus",
        ),
        (
            view.pressed_region
                .as_deref()
                .filter(|_| now_ms < view.pressed_until_ms),
            [189, 237, 214, 255],
            "ui/pressed",
        ),
    ] {
        let Some(region) = id.and_then(|id| hits.iter().find(|hit| hit.id == id && hit.enabled))
        else {
            continue;
        };
        if matches!(region.shape, HitShape::World(_)) {
            continue;
        }
        if region.id == "compose/input"
            && !view.compose_engaged
            && view.text_buffer.is_empty()
            && !view.controller_active
        {
            continue;
        }
        let radius = if matches!(
            region.id.as_str(),
            "compose/microphone" | "compose/settings" | "compose/close"
        ) {
            9
        } else {
            3
        };
        let layer = if region.id.starts_with("status/") {
            41
        } else {
            34
        };
        rects.push(rounded_rect(
            command_id,
            region.rect,
            color,
            layer,
            radius,
            true,
        ));
    }
    let described = view
        .hovered_region
        .as_deref()
        .or(view.focused_region.as_deref());
    let Some(region) = described.and_then(|id| hits.iter().find(|hit| hit.id == id)) else {
        return;
    };
    let microphone_help = region.id == "compose/microphone";
    let utility = region.id.starts_with("compose/toy-")
        || region.id.starts_with("compose/food-")
        || matches!(
            region.id.as_str(),
            "compose/settings" | "compose/close" | "compose/send" | "compose/creature"
        );
    if !microphone_help && !utility && !matches!(region.shape, HitShape::World(_)) {
        return;
    }
    // Plain water needs no caption; it would only cover the creature's coaching line.
    if region.id.starts_with("world/") {
        return;
    }
    if visible_status(view, now_ms).is_some() {
        return;
    }
    let value = if microphone_help {
        microphone_label(view.microphone_state)
    } else {
        &region.label
    };
    let size = TextRole::Secondary.size(view.text_scale >= 2);
    let maximum_width = match view.mode {
        UiMode::Settings => 156,
        UiMode::Bindings => 111,
        UiMode::DataManagement => 104,
        _ => 220,
    };
    let width = (typography().width(value, size).ceil() as i32 + 12).clamp(32, maximum_width);
    let height = typography().height(value, (width - 12) as f32, size).ceil() as i32 + 8;
    let x = (region.rect.x + region.rect.w / 2 - width / 2).clamp(4, 316 - width);
    let y = (region.rect.y - height - 4).clamp(4, 148 - height);
    let overlap = |x: i32, y: i32| {
        panel_overlap(
            Rect {
                x,
                y,
                w: width,
                h: height,
            },
            rects,
        )
    };
    // Close is already named inside the open panel. If its local dock hint would
    // cover that panel, moving the same word to a distant corner adds no guidance.
    if region.id == "compose/close" && overlap(x, y) > 0 {
        return;
    }
    let (x, y) = [
        (x, y),
        (4, 5),
        (316 - width, 5),
        (4, 148 - height),
        (316 - width, 148 - height),
    ]
    .into_iter()
    .enumerate()
    .min_by_key(|(index, (x, y))| (overlap(*x, *y), *index))
    .map(|(_, position)| position)
    .unwrap_or((x, y));
    let id = if microphone_help {
        "ui/microphone-help"
    } else {
        "ui/hover-label"
    };
    add_panel_chrome(
        &format!("{id}-background"),
        Rect {
            x,
            y,
            w: width,
            h: height,
        },
        36,
        rects,
    );
    text.push(bounded_label(
        id,
        value,
        Rect {
            x: x + 6,
            y: y + 4,
            w: width - 12,
            h: height - 8,
        },
        TextRole::Secondary,
        40,
    ));
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
    if let Some(attention) = accepted_attention_name(state) {
        return attention;
    }
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

/// An accepted target can own attention while a previous action still owns the
/// recovery pose and payoff cue. Describe only the current gaze, never an early
/// movement, contact or enjoyment result.
fn accepted_attention_name(state: &WorldState) -> Option<&'static str> {
    if state.creature.current_intention == Intention::Sleep {
        return None;
    }
    let aquarium = &state.creature.aquarium;
    match aquarium.gaze {
        GazeTarget::Food(id) => {
            let action = aquarium.action.as_ref()?;
            if action.food_id != Some(id)
                || action.destination != SemanticDestination::Food(id)
                || action.food_outcome.is_some()
                || matches!(action.phase, ActionPhase::Act | ActionPhase::Recover)
            {
                return None;
            }
            let WorldObject::Food(food) = state.aquarium.objects.get(&id)? else {
                return None;
            };
            if matches!(
                food.disposition,
                FoodDisposition::Consumed | FoodDisposition::Rejected
            ) {
                return None;
            }
            Some(match food.food {
                FoodId::Berry => "watching the berry",
                FoodId::Mushroom => "watching the mushroom",
                FoodId::Pellet => "watching the pellet",
            })
        }
        GazeTarget::Toy(toy) => {
            let interaction = state.creature.interaction_state.toy_interaction.as_ref()?;
            if interaction.toy != toy
                || interaction.origin != beastie_core::ToyOrigin::Player
                || interaction.outcome != beastie_core::ToyInteractionOutcome::Accepted
                || interaction.phase != beastie_core::ToyInteractionPhase::Approach
                || state.creature.current_intention != Intention::Play
                || aquarium.destination != Some(SemanticDestination::Toy(toy))
            {
                return None;
            }
            Some(match toy {
                ToyId::Ball => "watching the ball",
                ToyId::Bell => "watching the bell",
                ToyId::Sock => "watching the sock",
            })
        }
        _ => None,
    }
}

const fn private_life_behavior_name(
    kind: PrivateLifeKind,
    recipe: ActivityRecipe,
    phase: ActivityPhase,
) -> &'static str {
    match phase {
        ActivityPhase::Notice | ActivityPhase::Approach => match kind {
            PrivateLifeKind::ToyPlay(ToyId::Ball) => "going for the ball",
            PrivateLifeKind::ToyPlay(ToyId::Bell) => "going for the bell",
            PrivateLifeKind::ToyPlay(ToyId::Sock) => "going for the sock",
            PrivateLifeKind::CaveSettle => "heading for the cave",
            PrivateLifeKind::PlantInspect => "off to the plant",
            PrivateLifeKind::BottomForage => "looking for crumbs",
            PrivateLifeKind::OpenWaterDrift => "chasing a bubble",
        },
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
                "snapping at a bubble"
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
        // An accepted offer perks up at once, before the swim to the toy begins.
        GameEvent::ToyPlayAccepted {
            origin: beastie_core::ToyOrigin::Player,
            ..
        } => Some((direct, PresentationCueKind::PositiveNotice, 600)),
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
        GameEvent::WordHeard { .. } => Some((ordinary, PresentationCueKind::Curious, 1_400)),
        // Asleep, it does not hear words, but it stirs: the player still sees the talk land.
        GameEvent::TalkIgnored => Some((ordinary, PresentationCueKind::Sleep, 900)),
        GameEvent::Emerged => Some((direct, PresentationCueKind::Curious, 1_600)),
        GameEvent::TapNoticed { approached, .. } => Some((
            ordinary,
            if *approached {
                PresentationCueKind::Curious
            } else {
                PresentationCueKind::Notice
            },
            900,
        )),
        GameEvent::WordLearned { .. } => Some((direct, PresentationCueKind::WordLearned, 2_200)),
        GameEvent::Understood { response, .. } => match response {
            beastie_core::RequestResponse::Comply => {
                Some((direct, PresentationCueKind::PositiveNotice, 700))
            }
            beastie_core::RequestResponse::Delight => {
                Some((direct, PresentationCueKind::Delight, 1_200))
            }
            beastie_core::RequestResponse::Refuse => {
                Some((direct, PresentationCueKind::Suspicion, 1_100))
            }
            beastie_core::RequestResponse::Sulk => Some((direct, PresentationCueKind::Recoil, 900)),
            beastie_core::RequestResponse::Look => {
                Some((ordinary, PresentationCueKind::Notice, 700))
            }
        },
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
        corner_radius: 0,
    }
}

fn add_interactive_panel(
    id: &str,
    dimensions: Rect,
    layer: i16,
    rects: &mut Vec<RectCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_panel_chrome(id, dimensions, layer, rects);
    hits.retain(|hit| {
        hit.shape != HitShape::Rect
            || hit.id.starts_with("world/")
            || !rectangles_overlap(hit.rect, dimensions)
    });
    let mut blocker = hit(
        &format!("{id}/pointer-blocker"),
        None,
        UiAction::FocusCompose,
        dimensions,
        false,
        "",
    );
    blocker.shape = HitShape::Blocker;
    blocker.cursor = CursorKind::Default;
    hits.push(blocker);
}

fn rectangles_overlap(left: Rect, right: Rect) -> bool {
    left.x < right.x + right.w
        && left.x + left.w > right.x
        && left.y < right.y + right.h
        && left.y + left.h > right.y
}

fn add_panel_chrome(id: &str, dimensions: Rect, layer: i16, rects: &mut Vec<RectCommand>) {
    let radius = if dimensions.h >= 24 { 4 } else { 3 };
    rects.push(rounded_rect(id, dimensions, UI_PANEL, layer, radius, false));
    rects.push(rounded_rect(
        &format!("{id}-edge"),
        dimensions,
        UI_CORAL,
        layer + 1,
        radius,
        true,
    ));
}

fn add_inset(id: &str, dimensions: Rect, layer: i16, rects: &mut Vec<RectCommand>) {
    rects.push(rounded_rect(
        &format!("{id}-background"),
        dimensions,
        UI_PANEL_INSET,
        layer + 2,
        3,
        false,
    ));
    rects.push(rounded_rect(
        &format!("{id}-rim"),
        dimensions,
        UI_EDGE,
        layer + 3,
        3,
        true,
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
    let radius = if matches!(
        id,
        "compose/microphone" | "compose/settings" | "compose/close"
    ) {
        (dimensions.w.min(dimensions.h) / 2) as u8
    } else {
        3
    };
    rects.push(rounded_rect(
        &format!("{id}-background"),
        dimensions,
        if !enabled {
            UI_BUTTON_DISABLED
        } else {
            match id {
                // Care has a distinct enamel face; utility navigation rests on the rail.
                "compose/food" => UI_SELECTED,
                "compose/settings" | "compose/close" => UI_PANEL,
                _ if active => UI_SELECTED,
                _ => UI_BUTTON,
            }
        },
        layer + 2,
        radius,
        false,
    ));
    rects.push(rounded_rect(
        &format!("{id}-rim"),
        dimensions,
        if active || id == "compose/food" {
            UI_CORAL
        } else {
            UI_EDGE
        },
        layer + 3,
        radius,
        true,
    ));
}

fn rounded_rect(
    id: &str,
    area: Rect,
    color: [u8; 4],
    layer: i16,
    radius: u8,
    outline: bool,
) -> RectCommand {
    RectCommand {
        corner_radius: radius,
        outline,
        ..rect(id, area, color, layer)
    }
}

fn bounded_label(id: &str, value: &str, area: Rect, role: TextRole, layer: i16) -> TextCommand {
    TextCommand {
        bounds: Some(area),
        role,
        vertical_centered: true,
        ..label(id, value, area.x, area.y, layer)
    }
}

#[allow(clippy::too_many_arguments)]
fn add_control(
    id: &str,
    value: &str,
    action: UiAction,
    area: Rect,
    enabled: bool,
    primary: bool,
    layer: i16,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    add_button_chrome(id, area, enabled, false, layer, rects);
    if primary && enabled {
        rects.push(rounded_rect(
            &format!("{id}-primary"),
            area,
            UI_PRIMARY,
            layer + 3,
            3,
            false,
        ));
    }
    hits.push(hit(id, None, action, area, enabled, value));
    let vertical_inset = if area.h <= 12 { 1 } else { 2 };
    let mut caption = bounded_label(
        &format!("{id}-label"),
        value,
        Rect {
            x: area.x + 3,
            y: area.y + vertical_inset,
            w: area.w - 6,
            h: area.h - vertical_inset * 2,
        },
        if primary && enabled {
            TextRole::PrimaryControl
        } else {
            TextRole::Control
        },
        layer + 4,
    );
    caption.muted = !enabled;
    text.push(caption);
}

fn label(id: &str, value: &str, x: i32, y: i32, layer: i16) -> TextCommand {
    TextCommand {
        bounds: None,
        vertical_centered: false,
        keep_tail: false,
        input_state: None,
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

fn input_visual_state(view: &ViewState) -> TextInputState {
    if view.text_selected && !view.text_buffer.is_empty() {
        TextInputState::Selected
    } else {
        TextInputState::Caret
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
    #[test]
    fn every_player_input_is_acknowledged_in_the_same_frame() {
        use beastie_core::{FoodId, NormalizedPosition, PlayerEvent, SeededRandom, ToyId, step};
        let inputs = [
            PlayerEvent::Comfort,
            PlayerEvent::Play(ToyId::Ball),
            PlayerEvent::Play(ToyId::Bell),
            PlayerEvent::Play(ToyId::Sock),
            PlayerEvent::DropFood {
                food: FoodId::Berry,
                position: NormalizedPosition::new(5_000, 3_000),
            },
            PlayerEvent::Tap(NormalizedPosition::new(8_000, 2_000)),
            PlayerEvent::Utterance("zorp".to_owned()),
        ];
        for seed in [1_u64, 2, 3, 4] {
            for input in &inputs {
                let mut world = WorldState::new(seed, "Mop");
                let mut rng = SeededRandom::new(seed);
                let events = step(&mut world, std::slice::from_ref(input), 0, &mut rng);
                assert!(
                    events.iter().any(|event| cue_for_event(event).is_some()),
                    "seed {seed}: {input:?} produced no visible cue: {events:?}"
                );
            }
            let mut asleep = WorldState::new(seed, "Mop");
            let mut rng = SeededRandom::new(seed);
            asleep.creature.current_intention = beastie_core::Intention::Sleep;
            let events = step(
                &mut asleep,
                &[PlayerEvent::Utterance("hello".to_owned())],
                0,
                &mut rng,
            );
            assert!(events.iter().any(|event| cue_for_event(event).is_some()));
        }
    }

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
            bubble: None,
        }
    }

    #[test]
    fn toy_card_avoids_nearby_toys_and_keeps_its_opening_anchor() {
        let mut state = WorldState::new(7, "Mop");
        for text_scale in [1, 2] {
            let anchor = toy_context_anchor(&state, ToyId::Ball, text_scale);
            let view = ViewState {
                mode: UiMode::Context(UiTarget::Toy(ToyId::Ball)),
                context_card_anchor: Some(anchor),
                text_scale,
                ..Default::default()
            };
            let scene = plan(&state, &view).0;
            let card = scene
                .rects
                .iter()
                .find(|r| r.id == "mode/context-panel")
                .unwrap()
                .rect;
            let (hx, hy) = world_to_logical(state.creature.aquarium.position);
            assert!(!rects_overlap(
                card,
                Rect {
                    x: hx - 29,
                    y: hy - 23,
                    w: 58,
                    h: 46
                }
            ));
            for object in scene
                .objects
                .iter()
                .filter(|o| matches!(o.kind, ObjectKind::Toy(_)))
            {
                let (x, y) = world_to_logical(object.position);
                assert!(
                    !rects_overlap(
                        card,
                        Rect {
                            x: x - 12,
                            y: y - 12,
                            w: 24,
                            h: 24
                        }
                    ),
                    "card hides {:?}",
                    object.kind
                );
            }
            let before: Vec<_> = scene
                .hit_regions
                .iter()
                .filter(|hit| hit.id.starts_with("action/"))
                .map(|hit| hit.rect)
                .collect();
            let original = state.creature.aquarium.position;
            state.creature.aquarium.position = NormalizedPosition::new(9000, 9000);
            let after: Vec<_> = plan(&state, &view)
                .0
                .hit_regions
                .iter()
                .filter(|hit| hit.id.starts_with("action/"))
                .map(|hit| hit.rect)
                .collect();
            assert_eq!(before, after);
            state.creature.aquarium.position = original;
        }
    }

    #[test]
    fn title_has_only_real_navigation_and_no_care_targets() {
        let scene = plan(
            &WorldState::new(7, "Mop"),
            &ViewState {
                mode: UiMode::Title,
                ..Default::default()
            },
        )
        .0;
        assert!(scene.title_screen);
        assert_eq!(
            scene
                .hit_regions
                .iter()
                .map(|hit| hit.action)
                .collect::<Vec<_>>(),
            vec![UiAction::Continue, UiAction::OpenSettings, UiAction::Quit]
        );
        assert!(!scene.rects.iter().any(|r| r.id == "compose/bar"));
    }

    #[test]
    fn toy_context_keeps_real_actions_in_a_readable_nearby_card() {
        let state = WorldState::new(7, "Mop");
        let scene = plan(
            &state,
            &ViewState {
                mode: UiMode::Context(UiTarget::Toy(ToyId::Ball)),
                ..Default::default()
            },
        )
        .0;
        assert!(
            scene
                .objects
                .iter()
                .any(|object| object.kind == ObjectKind::Toy(ToyId::Ball)
                    && object.highlight == Highlight::Focus)
        );
        for action in [UiAction::Play(ToyId::Ball), UiAction::Inspect] {
            assert!(scene.hit_regions.iter().any(|hit| hit.action == action));
        }
        let panel = scene
            .rects
            .iter()
            .find(|r| r.id == "mode/context-panel")
            .unwrap();
        assert!(panel.rect.w <= LOGICAL_WIDTH / 3);
        for command in scene
            .text
            .iter()
            .filter(|text| text.id.starts_with("mode/context/") || text.id.starts_with("action/"))
        {
            assert!(
                rect_contains(panel.rect, command.bounds.unwrap()),
                "{}",
                command.id
            );
            assert!(command.bounds.unwrap().h >= command.role.size(false).ceil() as i32 + 2);
        }
    }

    #[test]
    fn inspection_projects_live_facts_and_keeps_its_opening_anchor() {
        let mut state = WorldState::new(42, "Mop");
        let mut view = ViewState {
            mode: UiMode::Inspect(UiTarget::Creature),
            context_card_anchor: Some((173, 31)),
            ..ViewState::default()
        };
        let first = plan(&state, &view).0;
        let panel = first
            .rects
            .iter()
            .find(|rect| rect.id == "inspect/panel")
            .unwrap()
            .rect;
        state.creature.name = "Gob".to_owned();
        state.creature.current_intention = Intention::Play;
        state.creature.aquarium.steering = SteeringMode::Approach;
        state.creature.aquarium.position = NormalizedPosition::new(10_000, 10_000);
        let before_projection = state.clone();
        let moved = plan(&state, &view).0;
        assert_eq!(
            moved
                .rects
                .iter()
                .find(|rect| rect.id == "inspect/panel")
                .unwrap()
                .rect
                .x,
            panel.x
        );
        assert!(
            moved
                .text
                .iter()
                .any(|text| text.id == "inspect/title" && text.text == "Gob")
        );
        assert!(
            moved
                .text
                .iter()
                .any(|text| text.id == "inspect/detail" && text.text.contains("swimming to a toy"))
        );
        assert_eq!(state, before_projection);

        view.mode = UiMode::Inspect(UiTarget::Toy(ToyId::Sock));
        let resting = plan(&state, &view).0;
        assert!(
            resting
                .text
                .iter()
                .any(|text| text.id == "inspect/detail" && text.text.contains("tugging"))
        );
        state
            .aquarium
            .toy_states
            .get_mut(&ToyId::Sock)
            .unwrap()
            .carried = true;
        let carried = plan(&state, &view).0;
        assert!(carried.text.iter().any(|text| text.id == "inspect/detail" && text.text == "Gob is carrying this toy."));
        assert_eq!(
            carried
                .rects
                .iter()
                .find(|rect| rect.id == "inspect/panel")
                .unwrap()
                .rect
                .x,
            panel.x
        );
    }

    #[test]
    fn inspection_tracks_food_disposition_and_missing_objects() {
        let mut state = WorldState::new(42, "Mop");
        let view = ViewState {
            mode: UiMode::Inspect(UiTarget::FoodObject(900)),
            context_card_anchor: Some((7, 31)),
            text_scale: 2,
            ..ViewState::default()
        };
        for (disposition, description) in [
            (FoodDisposition::Falling, "Falling through the water."),
            (FoodDisposition::Floating, "Floating in the water."),
            (FoodDisposition::Settled, "Resting on the sand."),
            (FoodDisposition::Consumed, "Already eaten."),
            (FoodDisposition::Rejected, "Offered and rejected."),
        ] {
            state.aquarium.objects.insert(
                900,
                WorldObject::Food(beastie_core::FoodObject {
                    id: 900,
                    food: FoodId::Berry,
                    position: NormalizedPosition::new(5000, 5000),
                    velocity: beastie_core::NormalizedVelocity::default(),
                    buoyancy: beastie_core::FoodBuoyancy::Drift,
                    disposition,
                    age_ms: 0,
                    lifetime_ms: 10_000,
                }),
            );
            let scene = plan(&state, &view).0;
            assert!(scene.text.iter().any(|text| text.id == "inspect/title" && text.text == food_name(FoodId::Berry)));
            assert!(
                scene
                    .text
                    .iter()
                    .any(|text| text.id == "inspect/detail" && text.text == description)
            );
        }
        state.aquarium.objects.remove(&900);
        let missing = plan(&state, &view).0;
        assert!(missing.text.iter().any(|text| text.id == "inspect/detail"
            && text.text == "This piece of food is no longer here."));

        let plant_id = state
            .aquarium
            .objects
            .iter()
            .find_map(|(id, object)| matches!(object, WorldObject::Plant { .. }).then_some(*id))
            .expect("initial plant");
        let plant_view = ViewState {
            mode: UiMode::Inspect(UiTarget::Plant(plant_id)),
            ..view
        };
        assert!(
            plan(&state, &plant_view)
                .0
                .text
                .iter()
                .any(|text| text.id == "inspect/detail"
                    && text.text == "Leaves to swim around and explore.")
        );
        state.aquarium.objects.remove(&plant_id);
        assert!(plan(&state, &plant_view).0.text.iter().any(
            |text| text.id == "inspect/detail" && text.text == "This plant is no longer here."
        ));
    }

    #[test]
    fn panel_blockers_remove_covered_controls_from_pointer_and_focus_targets() {
        let state = WorldState::new(7, "Mop");
        for (mode, panel_id) in [
            (UiMode::Settings, "settings/panel"),
            (UiMode::Bindings, "bindings/panel"),
            (UiMode::DataManagement, "data/panel"),
            (UiMode::Rename, "rename/prompt-background"),
            (UiMode::Inspect(UiTarget::Creature), "inspect/panel"),
            (UiMode::OnScreenKeyboard, "keyboard/panel"),
        ] {
            for text_scale in [1, 2] {
                let scene = plan(
                    &state,
                    &ViewState {
                        mode,
                        text_scale,
                        speech: Some("Hello".to_owned()),
                        ..ViewState::default()
                    },
                )
                .0;
                let blocker_id = format!("{panel_id}/pointer-blocker");
                let blocker_index = scene
                    .hit_regions
                    .iter()
                    .position(|hit| hit.id == blocker_id)
                    .unwrap();
                let blocker = &scene.hit_regions[blocker_index];
                assert_eq!(blocker.shape, HitShape::Blocker);
                assert!(!blocker.enabled, "a panel must never enter keyboard focus");
                assert!(
                    scene.hit_regions[..blocker_index].iter().all(|hit| {
                        hit.shape != HitShape::Rect
                            || hit.id.starts_with("world/")
                            || !rects_overlap(hit.rect, blocker.rect)
                    }),
                    "{mode:?} left a covered lower control available to focus"
                );
                let corner = (blocker.rect.x + 1, blocker.rect.y + 1);
                let pointer = scene
                    .hit_regions
                    .iter()
                    .rev()
                    .find(|hit| {
                        (hit.enabled || hit.shape == HitShape::Blocker)
                            && hit.rect.contains(corner.0, corner.1)
                    })
                    .unwrap();
                assert_eq!(
                    pointer.id, blocker_id,
                    "{mode:?} panel padding leaks pointer input"
                );
                let panel_layer = scene
                    .rects
                    .iter()
                    .find(|rect| rect.id == panel_id)
                    .unwrap()
                    .layer;
                assert!(
                    scene
                        .text
                        .iter()
                        .filter(|text| text.id.starts_with("speech/")
                            || text.id.starts_with("reaction/"))
                        .all(|text| text.layer < panel_layer)
                );
            }
        }
    }

    #[test]
    fn rename_validation_counts_unicode_characters_and_keeps_save_explicit() {
        let state = WorldState::new(7, "Mop");
        for (buffer, enabled) in [
            (String::new(), false),
            ("   ".to_owned(), false),
            ("雪".repeat(24), true),
            ("雪".repeat(25), false),
        ] {
            let scene = plan(
                &state,
                &ViewState {
                    mode: UiMode::Rename,
                    text_buffer: buffer.clone(),
                    ..ViewState::default()
                },
            )
            .0;
            let save = scene
                .hit_regions
                .iter()
                .find(|hit| hit.action == UiAction::SubmitName)
                .unwrap();
            assert_eq!(save.enabled, enabled, "{buffer:?}");
            assert!(scene.text.iter().any(|text| text.id == "rename/count"
                && text.text == format!("{} / 24", buffer.chars().count())));
            assert!(
                scene
                    .hit_regions
                    .iter()
                    .any(|hit| hit.id == "rename/cancel" && hit.enabled)
            );
        }
    }

    #[test]
    fn on_screen_keyboard_owns_its_message_or_name_field() {
        let state = WorldState::new(7, "Mop");
        for renaming_with_osk in [false, true] {
            let scene = plan(
                &state,
                &ViewState {
                    mode: UiMode::OnScreenKeyboard,
                    renaming_with_osk,
                    text_buffer: "雪".to_owned(),
                    text_scale: 2,
                    ..ViewState::default()
                },
            )
            .0;
            assert!(
                scene
                    .text
                    .iter()
                    .any(|text| text.id == "keyboard/input-text" && text.text == "雪")
            );
            let submit = scene
                .hit_regions
                .iter()
                .find(|hit| hit.id == "keyboard/send")
                .unwrap();
            assert!(submit.enabled);
            assert_eq!(
                submit.action,
                if renaming_with_osk {
                    UiAction::SubmitName
                } else {
                    UiAction::SubmitText
                }
            );
            assert_eq!(
                scene
                    .text
                    .iter()
                    .any(|text| text.id == "keyboard/count" && text.text == "1 / 24"),
                renaming_with_osk
            );
            for id in ["compose/input", "compose/send"] {
                assert!(
                    !scene
                        .hit_regions
                        .iter()
                        .find(|hit| hit.id == id)
                        .unwrap()
                        .enabled
                );
            }
            for character in 'a'..='z' {
                assert!(
                    scene
                        .hit_regions
                        .iter()
                        .any(|hit| hit.enabled && hit.action == UiAction::TypeCharacter(character))
                );
            }
            for action in [
                UiAction::Backspace,
                UiAction::TypeCharacter(' '),
                UiAction::CancelMode,
            ] {
                assert!(
                    scene
                        .hit_regions
                        .iter()
                        .any(|hit| hit.id.starts_with("keyboard/")
                            && hit.enabled
                            && hit.action == action)
                );
            }
        }
    }

    #[test]
    fn dismissing_a_microphone_failure_preserves_state_and_new_failures_reappear() {
        let state = WorldState::new(7, "Mop");
        for failure in [MicrophoneState::Unavailable, MicrophoneState::Error] {
            let mut view = ViewState {
                microphone_enabled: true,
                microphone_state: failure,
                ..ViewState::default()
            };
            let failed = plan(&state, &view).0;
            assert!(
                failed
                    .hit_regions
                    .iter()
                    .any(|hit| hit.action == UiAction::DismissStatus && hit.enabled)
            );
            view.dismiss_status();
            assert_eq!(view.microphone_state, failure);
            let dismissed = plan(&state, &view).0;
            assert!(
                !dismissed
                    .text
                    .iter()
                    .any(|text| text.id == "status/message")
            );
            assert!(
                !dismissed
                    .hit_regions
                    .iter()
                    .find(|hit| hit.id == "compose/microphone")
                    .unwrap()
                    .enabled
            );
            view.expire(100);
            assert!(visible_status(&view, 100).is_none());
            view.microphone_state = MicrophoneState::Idle;
            view.expire(200);
            view.microphone_state = failure;
            assert!(visible_status(&view, 200).is_some());
            view.dismiss_status();
            view.show_status("A new input attempt failed", 300, 100);
            assert_eq!(
                visible_status(&view, 300),
                Some("A new input attempt failed")
            );
            view.expire(400);
            assert!(visible_status(&view, 400).is_some());
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
        let caption = scene
            .text
            .iter()
            .find(|text| text.id == "speech/text")
            .unwrap();
        let bounds = caption.bounds.unwrap();
        let size = caption.role.size(true);
        assert_eq!(bounds.h, line_height(size).ceil() as i32);
        assert_eq!(panel.rect.h, bounds.h + 10, "one line reserves one line");
        assert!(rect_contains(panel.rect, bounds));
        assert!(bounds.h >= size.ceil() as i32 + 2);
        assert!(panel.rect.w <= LOGICAL_WIDTH / 2);
        assert!(
            typography().width("hm. rude giant.", size) <= bounds.w as f32,
            "the bubble is sized to its words"
        );
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
        assert!(reaction.text.iter().all(|t| t.id != "ui/hover-label"));
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
    fn tap_water_region_is_present_in_compose_and_absent_in_settings() {
        let state = WorldState::new(7, "Mop");
        let compose = plan(&state, &ViewState::default()).0;
        let water = compose
            .hit_regions
            .iter()
            .find(|hit| hit.id == "world/water")
            .expect("open water is a tap target");
        assert_eq!(water.action, UiAction::TapWater);
        assert_eq!(water.shape, HitShape::World(UiTarget::OpenWater));
        assert!(water.enabled && !water.label.is_empty());
        let creature = compose
            .hit_regions
            .iter()
            .find(|hit| hit.id == "target/creature")
            .unwrap();
        assert_eq!(creature.action, UiAction::Comfort, "one click pets");
        for hit in compose.hit_regions.iter().filter(|hit| {
            matches!(hit.target, Some(UiTarget::Toy(_))) && hit.id.starts_with("target/")
        }) {
            let Some(UiTarget::Toy(toy)) = hit.target else {
                unreachable!()
            };
            assert_eq!(hit.action, UiAction::Play(toy), "one click plays");
        }

        let settings = plan(
            &state,
            &ViewState {
                mode: UiMode::Settings,
                ..ViewState::default()
            },
        )
        .0;
        assert!(
            settings
                .hit_regions
                .iter()
                .all(|hit| hit.id != "world/water"
                    && hit.action != UiAction::TapWater
                    && !matches!(hit.shape, HitShape::World(_)))
        );
    }

    #[test]
    fn menu_open_world_click_dismisses_and_world_targets_still_act() {
        let state = WorldState::new(7, "Mop");
        for mode in [
            UiMode::Context(UiTarget::Creature),
            UiMode::Inspect(UiTarget::Creature),
            UiMode::ToyChoice,
            UiMode::FoodChoice,
        ] {
            let scene = plan(
                &state,
                &ViewState {
                    mode,
                    ..ViewState::default()
                },
            )
            .0;
            let water: Vec<_> = scene
                .hit_regions
                .iter()
                .filter(|hit| hit.shape == HitShape::World(UiTarget::OpenWater))
                .collect();
            assert_eq!(water.len(), 1, "{mode:?}");
            assert_eq!(water[0].id, "world/dismiss");
            assert_eq!(water[0].action, UiAction::CloseContext, "{mode:?}");
            assert!(water[0].enabled);
            assert!(
                scene
                    .hit_regions
                    .iter()
                    .any(|hit| hit.id == "target/creature"
                        && hit.enabled
                        && hit.action == UiAction::Comfort),
                "{mode:?}: the creature stays one click away under a menu"
            );
            assert!(
                scene
                    .hit_regions
                    .iter()
                    .all(|hit| hit.action != UiAction::TapWater),
                "{mode:?}: a dismissing click is not also a tap"
            );
        }
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
    fn contextual_actions_keep_their_opening_anchor_while_the_creature_moves() {
        for above in [false, true] {
            let mut state = WorldState::new(42, "Mop");
            let view = ViewState {
                mode: UiMode::Context(UiTarget::Creature),
                context_above: Some(above),
                ..Default::default()
            };
            let mut opening = None;
            for y in [0, 5000, 10_000] {
                state.creature.aquarium.position = NormalizedPosition::new(5000, y);
                let scene = plan(&state, &view).0;
                let controls: Vec<_> = scene
                    .hit_regions
                    .iter()
                    .filter(|h| h.id.starts_with("action/"))
                    .map(|h| h.rect)
                    .collect();
                if let Some(expected) = &opening {
                    assert_eq!(&controls, expected);
                } else {
                    opening = Some(controls);
                }
                let panel = scene
                    .rects
                    .iter()
                    .find(|r| r.id == "mode/context-panel-edge")
                    .unwrap();
                assert!(
                    panel.rect.y + panel.rect.h < COMPOSE_BAR_TOP,
                    "context card must leave the care dock available"
                );
            }
        }
    }

    #[test]
    fn contextual_actions_open_clear_of_the_head_throughout_the_tank() {
        for y in (0..=10_000).step_by(100) {
            let mut state = WorldState::new(42, "Mop");
            state.creature.aquarium.position = NormalizedPosition::new(5000, y);
            let (head_x, head_y) = world_to_logical(state.creature.aquarium.position);
            let head = Rect {
                x: head_x - CREATURE_HIT_WIDTH / 2,
                y: head_y - CREATURE_HIT_HEIGHT / 2,
                w: CREATURE_HIT_WIDTH,
                h: CREATURE_HIT_HEIGHT,
            };
            for text_scale in [1, 2] {
                let scene = plan(
                    &state,
                    &ViewState {
                        mode: UiMode::Context(UiTarget::Creature),
                        text_scale,
                        ..Default::default()
                    },
                )
                .0;
                let panel = scene
                    .rects
                    .iter()
                    .find(|r| r.id == "mode/context-panel-edge")
                    .unwrap();
                assert!(
                    !rects_overlap(head, panel.rect),
                    "head at {head_y}: {:?}",
                    panel.rect
                );
                assert!(panel.rect.y >= 0 && panel.rect.y + panel.rect.h <= AQUARIUM_BOTTOM);
                for hit in scene
                    .hit_regions
                    .iter()
                    .filter(|h| h.id.starts_with("action/"))
                {
                    assert!(
                        hit.rect.y >= panel.rect.y
                            && hit.rect.y + hit.rect.h <= panel.rect.y + panel.rect.h
                    );
                }
            }
        }
    }

    #[test]
    fn speech_and_rename_text_explain_their_purpose() {
        let state = WorldState::new(42, "Mop");
        let rename = plan(
            &state,
            &ViewState {
                mode: UiMode::Rename,
                ..Default::default()
            },
        )
        .0;
        assert!(
            rename
                .text
                .iter()
                .any(|t| t.id == "rename/prompt" && t.text == "Currently Mop")
        );
        assert!(
            rename
                .text
                .iter()
                .any(|t| t.id == "rename/value" && t.text == "New name…")
        );
        assert!(
            rename
                .text
                .iter()
                .any(|t| t.id == "rename/count" && t.text == "0 / 24")
        );
        for text_scale in [1, 2] {
            let speech = plan(
                &state,
                &ViewState {
                    speech: Some("Hello".to_owned()),
                    text_scale,
                    ..Default::default()
                },
            )
            .0;
            // The bubble's tail names the speaker: no label, no paging, no reaction chips.
            let speech_text: Vec<_> = speech
                .text
                .iter()
                .filter(|t| t.id.starts_with("speech/") || t.id.starts_with("reaction/"))
                .collect();
            assert_eq!(speech_text.len(), 1);
            assert_eq!(speech_text[0].id, "speech/text");
            assert_eq!(speech_text[0].text, "Hello");
            assert_eq!(speech_text[0].role, TextRole::Dialogue);
            assert!(speech.rects.iter().any(|r| r.id == "speech/tail-0"));
        }
    }

    fn speech_parts(scene: &ScenePlan) -> (Rect, Rect, Vec<Rect>) {
        let panel = scene
            .rects
            .iter()
            .find(|rect| rect.id == "speech/panel")
            .expect("speech bubble")
            .rect;
        let caption = scene
            .text
            .iter()
            .find(|text| text.id == "speech/text")
            .and_then(|text| text.bounds)
            .expect("caption bounds");
        let tail = scene
            .rects
            .iter()
            .filter(|rect| rect.id.starts_with("speech/tail-"))
            .map(|rect| rect.rect)
            .collect();
        (panel, caption, tail)
    }

    #[test]
    fn speech_bubble_stays_in_the_water_clear_of_the_rail_head_and_settings() {
        let speeches = [
            "hm.".to_owned(),
            "ball? mop wants the red ball now, please".to_owned(),
            "e\u{301} 🌿 👩‍🔬 café".to_owned(),
            "W".repeat(512),
        ];
        for text_scale in [1, 2] {
            for mode in [UiMode::Compose, UiMode::Settings] {
                for x in [0, 1_500, 3_000, 5_000, 6_500, 8_000, 10_000] {
                    for y in [0, 2_500, 5_000, 7_500, 10_000] {
                        for speech in &speeches {
                            let mut state = WorldState::new(7, "Mop");
                            state.creature.aquarium.position = NormalizedPosition::new(x, y);
                            let mut view = ViewState {
                                mode,
                                text_scale,
                                ..ViewState::default()
                            };
                            view.show_speech(speech.clone(), 0);
                            let scene = plan(&state, &view).0;
                            let (panel, caption, tail) = speech_parts(&scene);
                            let at =
                                format!("{mode:?} scale {text_scale} at ({x}, {y}) {speech:.12}");
                            let water = Rect {
                                x: 4,
                                y: 4,
                                w: LOGICAL_WIDTH - 8,
                                h: AQUARIUM_BOTTOM - 6,
                            };
                            assert!(rect_contains(water, panel), "{at}: {panel:?}");
                            assert!(panel.y + panel.h < COMPOSE_BAR_TOP, "{at}");
                            assert!(rect_contains(panel, caption), "{at}");
                            let head = head_box(&state);
                            assert!(!rects_overlap(panel, head), "{at}: covers the head");
                            for part in &tail {
                                assert!(rect_contains(water, *part), "{at}");
                                assert!(!rects_overlap(*part, head), "{at}: tail on the head");
                            }
                            if mode == UiMode::Settings {
                                let settings = scene
                                    .rects
                                    .iter()
                                    .find(|rect| rect.id == "settings/panel")
                                    .unwrap()
                                    .rect;
                                assert!(!rects_overlap(panel, settings), "{at}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn speech_bubble_follows_the_head() {
        let mut previous: Option<(i32, Rect)> = None;
        for x in [500, 2_500, 4_500, 6_500, 8_500] {
            let mut state = WorldState::new(7, "Mop");
            state.creature.aquarium.position = NormalizedPosition::new(x, 6_000);
            let mut view = ViewState::default();
            view.show_speech("mop sees ball".to_owned(), 0);
            let (panel, _, tail) = speech_parts(&plan(&state, &view).0);
            let head = head_box(&state);
            let head_center = head.x + head.w / 2;
            // Bubble sits above the head and its tail lies between the two.
            assert!(panel.y + panel.h <= head.y, "{x}");
            assert!(
                panel.x <= head_center && head_center <= panel.x + panel.w,
                "{x}"
            );
            for part in &tail {
                assert!(
                    part.y >= panel.y + panel.h && part.y + part.h <= head.y,
                    "{x}"
                );
            }
            if let Some((last_head, last_panel)) = previous {
                assert_eq!(
                    panel.x - last_panel.x,
                    head_center - last_head,
                    "moves with the head"
                );
            }
            previous = Some((head_center, panel));
        }
        // Near the surface there is no room above, so the bubble hangs below the head.
        let mut state = WorldState::new(7, "Mop");
        state.creature.aquarium.position = NormalizedPosition::new(5_000, 0);
        let mut view = ViewState::default();
        view.show_speech("mop sees ball".to_owned(), 0);
        let (panel, _, _) = speech_parts(&plan(&state, &view).0);
        assert!(panel.y >= head_box(&state).y + head_box(&state).h);
    }

    #[test]
    fn short_speech_reserves_only_its_exact_line_box() {
        let state = WorldState::new(7, "Mop");
        for text_scale in [1, 2] {
            let mut view = ViewState {
                text_scale,
                ..ViewState::default()
            };
            view.show_speech("A little quiet place.".to_owned(), 0);
            let scene = plan(&state, &view).0;
            let caption = scene
                .text
                .iter()
                .find(|text| text.id == "speech/text")
                .unwrap();
            let bounds = caption.bounds.unwrap();
            let size = caption.role.size(text_scale >= 2);
            assert_eq!(bounds.h, line_height(size).ceil() as i32);
            assert_eq!(
                typography().layout_lines(&caption.text, bounds.w as f32, bounds.h as f32, size),
                std::slice::from_ref(&caption.text)
            );
        }
    }

    #[test]
    fn multiline_speech_grows_within_the_water() {
        let state = WorldState::new(42, "Mop");
        let mut view = ViewState {
            text_scale: 2,
            ..ViewState::default()
        };
        let line = "mop remembers the sweet berry you brought";
        view.show_speech(line.to_owned(), 0);
        let scene = plan(&state, &view).0;
        let (panel, caption, _) = speech_parts(&scene);
        let size = TextRole::Dialogue.size(true);
        let lines = typography().lines(line, caption.w as f32, size).len();
        assert!(lines > 1);
        assert_eq!(caption.h, (lines as f32 * line_height(size)).ceil() as i32);
        assert!(panel.y + panel.h < COMPOSE_BAR_TOP);
        let text = scene.text.iter().find(|t| t.id == "speech/text").unwrap();
        assert_eq!(text.text, line, "the whole line is shown at once");
    }

    #[test]
    fn bubble_bounds_stay_fixed_through_unicode_reveal() {
        let state = WorldState::new(7, "Mop");
        let full = "cafe\u{301} 🌿 and 👩‍🔬, mop likes it";
        for text_scale in [1, 2] {
            let mut view = ViewState {
                text_scale,
                ..ViewState::default()
            };
            view.show_speech(full.to_owned(), 0);
            let (panel, body, _) = speech_parts(&plan(&state, &view).0);
            for (start, grapheme) in full.grapheme_indices(true) {
                let end = start + grapheme.len();
                view.speech = Some(full[..end].to_owned());
                let revealed = plan(&state, &view).0;
                let (revealed_panel, revealed_body, _) = speech_parts(&revealed);
                assert_eq!(revealed_panel, panel);
                assert_eq!(revealed_body, body);
                let caption = revealed
                    .text
                    .iter()
                    .find(|text| text.id == "speech/text")
                    .unwrap();
                assert_eq!(caption.text, full[..end]);
            }
        }
    }

    #[test]
    fn overlong_reply_is_bounded_and_ellipsized_rather_than_paged() {
        let state = WorldState::new(7, "Mop");
        let speech = "W".repeat(512);
        for text_scale in [1, 2] {
            let mut view = ViewState {
                text_scale,
                ..ViewState::default()
            };
            view.show_speech(speech.clone(), 0);
            let scene = plan(&state, &view).0;
            let caption = scene
                .text
                .iter()
                .find(|text| text.id == "speech/text")
                .unwrap();
            let bounds = caption.bounds.unwrap();
            let size = caption.role.size(text_scale >= 2);
            assert_eq!(
                bounds.h,
                (SPEECH_MAX_LINES as f32 * line_height(size)).ceil() as i32
            );
            let lines =
                typography().layout_lines(&caption.text, bounds.w as f32, bounds.h as f32, size);
            assert_eq!(lines.len(), SPEECH_MAX_LINES);
            assert!(lines.last().unwrap().ends_with('…'));
            assert!(
                scene
                    .hit_regions
                    .iter()
                    .all(|hit| !hit.id.starts_with("speech/") && !hit.id.starts_with("reaction/"))
            );
        }
    }

    #[test]
    fn speech_bubble_never_swallows_a_click() {
        let mut state = WorldState::new(7, "Mop");
        state.creature.aquarium.position = NormalizedPosition::new(5_000, 6_000);
        let mut view = ViewState::default();
        view.show_speech("ball! mop wants ball".to_owned(), 0);
        let scene = plan(&state, &view).0;
        let (panel, _, _) = speech_parts(&scene);
        // Only world targets lie under the bubble, so a click there still reaches the water.
        for hit in scene
            .hit_regions
            .iter()
            .filter(|hit| rects_overlap(hit.rect, panel))
        {
            assert!(matches!(hit.shape, HitShape::World(_)), "{}", hit.id);
        }
        assert!(
            scene
                .hit_regions
                .iter()
                .any(|hit| hit.id == "world/water" && rect_contains(hit.rect, panel))
        );
    }

    #[test]
    fn obstructing_modals_hide_whole_speech_and_pause_only_its_reading_deadline() {
        let state = WorldState::new(7, "Mop");
        for mode in [
            UiMode::Rename,
            UiMode::OnScreenKeyboard,
            UiMode::Bindings,
            UiMode::Rebinding(BindableAction::PushToTalk),
            UiMode::DataManagement,
            UiMode::ConfirmReset,
            UiMode::FoodChoice,
            UiMode::Context(UiTarget::Creature),
            UiMode::Inspect(UiTarget::Creature),
        ] {
            let mut view = ViewState {
                mode,
                ..ViewState::default()
            };
            view.show_speech("A complete little thought.".into(), 0);
            let owned = view.speech_layout_text.clone();
            let visible = view.speech.clone();
            view.show_status("Saved", 0, 1);
            view.expire(4 * SPEECH_LIFETIME_MS);
            assert_eq!(view.speech, visible);
            assert_eq!(view.speech_layout_text, owned);
            assert!(!view.speaking);
            assert!(
                view.status_message.is_none(),
                "unrelated deadlines still expire"
            );
            let scene = plan(&state, &view).0;
            assert!(
                scene.text.iter().all(
                    |text| !text.id.starts_with("speech/") && !text.id.starts_with("reaction/")
                )
            );
            assert!(
                scene
                    .hit_regions
                    .iter()
                    .all(|hit| !hit.id.starts_with("speech/") && !hit.id.starts_with("reaction/"))
            );
            let deadline = view.speech_expires_at_ms.unwrap();
            view.mode = UiMode::Compose;
            view.expire(deadline - 1);
            assert!(view.speech.is_some());
            view.expire(deadline);
            assert!(view.speech.is_none());
        }
        let mut visible = ViewState {
            mode: UiMode::Settings,
            ..ViewState::default()
        };
        visible.show_speech("Still readable beside Settings.".into(), 0);
        visible.expire(SPEECH_LIFETIME_MS);
        assert!(
            visible.speech.is_none(),
            "visible Settings captions retain ordinary reading expiry"
        );
    }

    #[test]
    fn progressive_speech_reveal_never_splits_a_grapheme() {
        let state = WorldState::new(7, "Mop");
        let mut view = ViewState::default();
        view.show_speech("e\u{301} beside the water".to_owned(), 0);
        view.speech = Some("e".to_owned());
        let partial = plan(&state, &view).0;
        assert!(
            partial
                .text
                .iter()
                .find(|text| text.id == "speech/text")
                .unwrap()
                .text
                .is_empty()
        );
        view.speech = Some("e\u{301}".to_owned());
        let complete = plan(&state, &view).0;
        assert_eq!(
            complete
                .text
                .iter()
                .find(|text| text.id == "speech/text")
                .unwrap()
                .text,
            "e\u{301}"
        );
    }

    #[test]
    fn speech_waits_for_its_reveal_before_expiring() {
        let speech = "mop found the ball again".to_owned();
        let mut view = ViewState::default();
        view.show_speech(speech.clone(), 0);
        view.speech = Some("mop".to_owned());
        view.expire(SPEECH_LIFETIME_MS + 1);
        assert_eq!(view.speech.as_deref(), Some("mop"));
        let expiry = view.speech_expires_at_ms.unwrap();
        assert_eq!(
            expiry,
            2 * SPEECH_LIFETIME_MS + 1,
            "reading time restarts while the reveal is incomplete"
        );
        view.speech = Some(speech);
        view.expire(expiry - 1);
        assert!(view.speech.is_some());
        view.expire(expiry);
        assert!(view.speech.is_none());
        assert!(view.speech_layout_text.is_none());
    }

    #[test]
    fn active_speech_keeps_its_caption_and_never_moves_focus() {
        let mut view = ViewState::default();
        view.show_speech("hm. rude giant.".to_owned(), 1_000);
        view.focused_region = Some("compose/input".to_owned());
        view.speaking = true;

        view.expire(9_000);
        assert_eq!(view.speech.as_deref(), Some("hm. rude giant."));
        assert_eq!(view.speech_expires_at_ms, Some(9_500));

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
    fn accepted_food_attention_names_each_target_during_private_recovery_and_anticipation() {
        for (food, expected) in [
            (FoodId::Berry, "watching the berry"),
            (FoodId::Mushroom, "watching the mushroom"),
            (FoodId::Pellet, "watching the pellet"),
        ] {
            let mut state = WorldState::new(42, "Mop");
            let mut rng = beastie_core::SeededRandom::new(42);
            state.creature.private_life.active = Some(private_activity(
                NonZeroU64::new(1).unwrap(),
                PrivateLifeKind::BottomForage,
                ActivityRecipe::BottomForage,
                ActivityPhase::Recover,
            ));
            let events = beastie_core::step(
                &mut state,
                &[beastie_core::PlayerEvent::DropFood {
                    food,
                    position: NormalizedPosition::new(7_000, 3_000),
                }],
                0,
                &mut rng,
            );
            assert!(events.iter().any(|event| matches!(event, GameEvent::FoodDropped { food: actual, .. } if *actual == food)));
            assert_eq!(
                state.creature.private_life.active.as_ref().unwrap().phase,
                ActivityPhase::Recover
            );
            assert_eq!(
                state.creature.aquarium.velocity,
                beastie_core::NormalizedVelocity::default()
            );
            for phase in [
                ActionPhase::Notice,
                ActionPhase::Brake,
                ActionPhase::Gaze,
                ActionPhase::Turn,
                ActionPhase::Approach,
                ActionPhase::Inspect,
            ] {
                state.creature.aquarium.action.as_mut().unwrap().phase = phase;
                let before = state.clone();
                for text_scale in [1, 2] {
                    let scene = plan(
                        &state,
                        &ViewState {
                            text_scale,
                            ..ViewState::default()
                        },
                    )
                    .0;
                    assert_eq!(scene.summary.behavior, expected);
                    assert_eq!(
                        scene.creature.pose,
                        CreaturePose::Recover,
                        "attention must not replace the private recovery pose"
                    );
                    let label = scene
                        .text
                        .iter()
                        .find(|text| text.id == "compose/summary-behavior")
                        .unwrap();
                    assert_eq!(
                        label.text,
                        format!("{} · {expected}", scene.summary.mood_label)
                    );
                    assert_eq!(label.scale, text_scale);
                    assert!(
                        label.bounds.unwrap().h as f32
                            >= TextRole::Secondary.size(text_scale >= 2) * 2.4
                    );
                }
                assert_eq!(state, before, "projection never advances food or recovery");
            }
            let accepted = state.clone();
            for phase in [ActionPhase::Act, ActionPhase::Recover] {
                state.creature.aquarium.action.as_mut().unwrap().phase = phase;
                assert_eq!(
                    accepted_attention_name(&state),
                    None,
                    "payoff phases keep their existing reading"
                );
            }
            state = accepted.clone();
            state.creature.aquarium.gaze = GazeTarget::Player;
            assert_eq!(accepted_attention_name(&state), None);
            state = accepted.clone();
            let food_id = state
                .creature
                .aquarium
                .action
                .as_ref()
                .unwrap()
                .food_id
                .unwrap();
            state.aquarium.objects.remove(&food_id);
            assert_eq!(
                accepted_attention_name(&state),
                None,
                "missing food is not current attention"
            );
            state = accepted.clone();
            state
                .creature
                .aquarium
                .action
                .as_mut()
                .unwrap()
                .food_outcome = Some(beastie_core::FoodOutcome::Rejected);
            assert_eq!(accepted_attention_name(&state), None);
            state = accepted;
            state.creature.current_intention = Intention::Sleep;
            assert_eq!(accepted_attention_name(&state), None);
        }
    }

    #[test]
    fn accepted_toy_attention_survives_food_recovery_without_inventing_a_toy_payoff() {
        for (toy, expected) in [
            (ToyId::Ball, "watching the ball"),
            (ToyId::Bell, "watching the bell"),
            (ToyId::Sock, "watching the sock"),
        ] {
            let mut state = WorldState::new(42, "Mop");
            let mut rng = beastie_core::SeededRandom::new(42);
            let mut view = ViewState::default();
            state.creature.preferences.insert(FoodId::Berry, 0.9);
            state.creature.toy_preferences.insert(toy, 0.9);
            let position = state.creature.aquarium.position;
            beastie_core::step(
                &mut state,
                &[beastie_core::PlayerEvent::DropFood {
                    food: FoodId::Berry,
                    position,
                }],
                0,
                &mut rng,
            );
            let mut consumed = false;
            for _ in 0..120 {
                let events = beastie_core::step(&mut state, &[], 1_000, &mut rng);
                if events.contains(&GameEvent::FoodConsumed(FoodId::Berry)) {
                    view.observe_events(&events, state.elapsed_ms);
                    consumed = true;
                    break;
                }
            }
            assert!(consumed, "real feeding must reach its physical payoff");
            let recovery = state.creature.aquarium.action.clone();
            assert_eq!(recovery.as_ref().unwrap().phase, ActionPhase::Recover);
            beastie_core::step(&mut state, &[], 33, &mut rng);
            let events = beastie_core::step(
                &mut state,
                &[beastie_core::PlayerEvent::Play(toy)],
                0,
                &mut rng,
            );
            assert!(events.iter().any(|event| matches!(event, GameEvent::ToyPlayAccepted { toy: actual, origin: beastie_core::ToyOrigin::Player, .. } if *actual == toy)));
            assert!(!events.iter().any(|event| matches!(
                event,
                GameEvent::ToyContacted { .. } | GameEvent::ToyPlayed { .. }
            )));
            view.observe_events(&events, state.elapsed_ms);
            let before = state.clone();
            let scene = plan(&state, &view).0;
            assert_eq!(scene.summary.behavior, expected);
            assert_eq!(scene.creature.pose, CreaturePose::Recover);
            // The accepted offer is acknowledged at once, before any toy contact, and the
            // acknowledgement is not the toy's delight payoff.
            assert_eq!(
                scene.creature.expression.as_ref().unwrap().cue,
                PresentationCueKind::PositiveNotice
            );
            assert_eq!(state.creature.aquarium.action, recovery);
            assert_eq!(state, before);

            for phase in [
                beastie_core::ToyInteractionPhase::Contact,
                beastie_core::ToyInteractionPhase::Resolved,
                beastie_core::ToyInteractionPhase::Recovery,
                beastie_core::ToyInteractionPhase::Interrupted,
            ] {
                let mut changed = state.clone();
                changed
                    .creature
                    .interaction_state
                    .toy_interaction
                    .as_mut()
                    .unwrap()
                    .phase = phase;
                assert_eq!(accepted_attention_name(&changed), None);
            }
            for outcome in [
                beastie_core::ToyInteractionOutcome::Rejected,
                beastie_core::ToyInteractionOutcome::Interrupted,
            ] {
                let mut changed = state.clone();
                changed
                    .creature
                    .interaction_state
                    .toy_interaction
                    .as_mut()
                    .unwrap()
                    .outcome = outcome;
                assert_eq!(accepted_attention_name(&changed), None);
            }
            let mut changed = state.clone();
            changed
                .creature
                .interaction_state
                .toy_interaction
                .as_mut()
                .unwrap()
                .origin = beastie_core::ToyOrigin::Autonomous;
            assert_eq!(
                accepted_attention_name(&changed),
                None,
                "private routines keep their recipe labels"
            );
            let mut changed = state.clone();
            changed.creature.aquarium.gaze = GazeTarget::Player;
            assert_eq!(accepted_attention_name(&changed), None);
            let mut changed = state.clone();
            changed.creature.aquarium.destination = Some(SemanticDestination::Cave);
            assert_eq!(accepted_attention_name(&changed), None);
            let mut changed = state.clone();
            changed.creature.current_intention = Intention::Sleep;
            assert_eq!(accepted_attention_name(&changed), None);
        }
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
            "settings/data",
            "settings/microphone",
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
            BindableAction::PushToTalk,
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
        assert!(
            bindings
                .hit_regions
                .iter()
                .any(|hit| { hit.action == UiAction::ResetBindings && hit.enabled })
        );
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
                .any(|text| { text.id == "status/message" && text.text.contains("Tank is full") })
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
    fn direct_toy_response_keeps_physical_identity_and_delight_without_double_audio() {
        for (toy, response, cue, sound) in [
            (
                ToyId::Ball,
                ToyResponse::BallNudged,
                PresentationCueKind::BallNudge,
                AudioCue::BallNudge,
            ),
            (
                ToyId::Bell,
                ToyResponse::BellStruck,
                PresentationCueKind::BellStrike,
                AudioCue::BellRing,
            ),
            (
                ToyId::Sock,
                ToyResponse::SockTugged,
                PresentationCueKind::SockTug,
                AudioCue::SockRustle,
            ),
        ] {
            let interaction_id = NonZeroU64::MIN;
            let events = [
                GameEvent::ToyInteractionResponded {
                    toy,
                    interaction_id,
                    response,
                },
                GameEvent::ToyPlayed {
                    toy,
                    interaction_id,
                    origin: beastie_core::ToyOrigin::Player,
                },
            ];
            let world = WorldState::new(7, "Contact");
            let mut view = ViewState::default();
            let audio = view.observe_events(&events, world.elapsed_ms);
            assert_eq!(played_cues(audio.clone()), vec![sound]);
            assert!(audio.events.iter().any(|command| matches!(command,
                AudioCommand::Play { owner: SemanticOwner::ToyInteraction(id), cue: actual, .. }
                    if *id == interaction_id && *actual == sound
            )));
            assert_eq!(
                view.active_cue(world.elapsed_ms),
                Some(PresentationCueKind::Delight)
            );
            let scene = plan(&world, &view).0;
            assert_eq!(
                scene.creature.expression.unwrap().cue,
                PresentationCueKind::Delight
            );
            assert!(scene.effects.iter().any(|effect| {
                effect.owner == SemanticOwner::ToyInteraction(interaction_id)
                    && effect.target == UiTarget::Toy(toy)
                    && effect.cue == cue
                    && effect.elapsed_ms == 0
            }));
            // Independent private-life counters may carry the same number.
            view.observe_events(
                &[GameEvent::PrivateLifeInterrupted {
                    activity_id: interaction_id,
                    phase: ActivityPhase::Act,
                    by: beastie_core::ActivityInterruptionOwner::Player,
                }],
                world.elapsed_ms,
            );
            assert!(
                plan(&world, &view).0.effects.iter().any(|effect| {
                    effect.owner == SemanticOwner::ToyInteraction(interaction_id)
                })
            );
            view.expire(world.elapsed_ms + 2_000);
            assert!(view.cue_queue.is_empty());
        }
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
                focused_region: Some("compose/food-berry".to_owned()),
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
        // Icon-only rail buttons are named while focused, so a controller player knows
        // what the berry button does before pressing it.
        assert!(
            controller
                .text
                .iter()
                .any(|text| text.id == "ui/hover-label" && text.text == "Feed berry")
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
            "compose/food-berry",
            "compose/toy-0",
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
    fn creature_rename_uses_a_dedicated_field_and_explicit_submit() {
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
                .any(|hit| hit.id == "rename/submit"
                    && hit.action == UiAction::SubmitName
                    && hit.enabled)
        );
        assert!(
            rename
                .text
                .iter()
                .any(|text| text.id == "rename/value" && text.text == "Gob")
        );
        assert!(rename.text.iter().any(|text| text.id == "rename/prompt"));
        let field = rename
            .hit_regions
            .iter()
            .find(|hit| hit.id == "rename/input")
            .unwrap();
        assert!(field.enabled && field.action == UiAction::FocusCompose);
        let value = rename
            .text
            .iter()
            .find(|text| text.id == "rename/value")
            .unwrap();
        assert!(rect_contains(field.rect, value.bounds.unwrap()));
        for id in ["compose/input", "compose/send"] {
            assert!(
                !rename
                    .hit_regions
                    .iter()
                    .find(|hit| hit.id == id)
                    .unwrap()
                    .enabled
            );
        }
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
                assert!(
                    render
                        .text
                        .iter()
                        .any(|t| t.id == "compose/summary-behavior")
                );
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
                let editable = render
                    .hit_regions
                    .iter()
                    .find(|hit| hit.id == "compose/input")
                    .unwrap()
                    .rect;
                let send = render
                    .hit_regions
                    .iter()
                    .find(|hit| hit.id == "compose/send")
                    .unwrap()
                    .rect;
                assert!(rect_contains(input_box, editable));
                assert!(rect_contains(input_box, send));
                assert!(!rects_overlap(editable, send));
                assert!(!rects_overlap(summary_name.bounds.unwrap(), input_box));
                assert!(rect_contains(editable, input.bounds.unwrap()));
                assert!(input.keep_tail);
                assert_eq!(input.text, view.text_buffer);
                assert!(input.bounds.unwrap().x + input.bounds.unwrap().w < send.x);
                for action in [
                    "toy-0",
                    "toy-1",
                    "toy-2",
                    "food-berry",
                    "food-mushroom",
                    "food-pellet",
                    "settings",
                ] {
                    let background = render
                        .rects
                        .iter()
                        .find(|command| command.id == format!("compose/{action}-background"))
                        .expect("action background")
                        .rect;
                    assert!(!rects_overlap(input_box, background), "{action}");
                    assert!(
                        !rects_overlap(summary_name.bounds.unwrap(), background),
                        "{action}"
                    );
                }
                // Every rail hit target is disjoint from the others and names its action.
                let rail: Vec<_> = render
                    .hit_regions
                    .iter()
                    .filter(|hit| hit.id.starts_with("compose/") && hit.enabled)
                    .collect();
                assert!(rail.len() >= 9);
                for (index, hit) in rail.iter().enumerate() {
                    assert!(!hit.label.is_empty(), "{}", hit.id);
                    assert!(hit.rect.y >= COMPOSE_BAR_TOP, "{}", hit.id);
                    for other in &rail[index + 1..] {
                        assert!(
                            !rects_overlap(hit.rect, other.rect),
                            "{} overlaps {}",
                            hit.id,
                            other.id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn only_nonempty_editable_values_request_measured_tail_retention() {
        let state = WorldState::new(7, "Mop");
        let draft = format!("{}a\u{301}👩‍🔬Z", "WMi ".repeat(80));
        for (mode, expected) in [
            (UiMode::Compose, "compose/input-text"),
            (UiMode::Rename, "rename/value"),
            (UiMode::OnScreenKeyboard, "keyboard/input-text"),
        ] {
            for value in ["", draft.as_str()] {
                let scene = plan(
                    &state,
                    &ViewState {
                        mode,
                        text_buffer: value.to_owned(),
                        ..ViewState::default()
                    },
                )
                .0;
                let editable = scene.text.iter().find(|text| text.id == expected).unwrap();
                assert_eq!(editable.keep_tail, !value.is_empty());
                if !value.is_empty() {
                    assert_eq!(editable.text, value);
                }
                assert!(
                    scene
                        .text
                        .iter()
                        .all(|text| !text.keep_tail || text.id == expected)
                );
            }
        }
    }

    #[test]
    fn head_fit_preserves_combining_and_joined_emoji_graphemes() {
        let value = "a\u{301}👩‍🔬XZ";
        assert_eq!(head_fit(value, 4), value);
        assert_eq!(head_fit(value, 3), "a\u{301}👩‍🔬…");
        assert_eq!(head_fit(value, 2), "a\u{301}…");
        assert_eq!(head_fit(value, 1), "…");
        assert_eq!(head_fit(value, 0), "");
    }

    #[test]
    fn older_text_commands_default_to_head_layout() {
        let command = label("legacy", "A label", 1, 2, 3);
        let mut serialized = serde_json::to_value(&command).unwrap();
        serialized.as_object_mut().unwrap().remove("keep_tail");
        serialized.as_object_mut().unwrap().remove("input_state");
        let decoded: TextCommand = serde_json::from_value(serialized).unwrap();
        assert!(!decoded.keep_tail);
        assert_eq!(decoded.input_state, None);
        assert_eq!(decoded, command);
    }

    #[test]
    fn older_view_state_defaults_to_unselected_text() {
        let mut serialized = serde_json::to_value(ViewState::default()).unwrap();
        serialized.as_object_mut().unwrap().remove("text_selected");
        let decoded: ViewState = serde_json::from_value(serialized).unwrap();
        assert!(!decoded.text_selected);
    }

    #[test]
    fn input_decorations_follow_editing_focus_and_never_select_placeholders() {
        let state = WorldState::new(7, "Mop");
        for (mode, focused, engaged, expected_id) in [
            (
                UiMode::Compose,
                "compose/input",
                true,
                Some("compose/input-text"),
            ),
            (UiMode::Compose, "compose/settings", true, None),
            (UiMode::Rename, "rename/input", false, Some("rename/value")),
            (UiMode::Rename, "rename/save", false, None),
            (
                UiMode::OnScreenKeyboard,
                "keyboard/a",
                false,
                Some("keyboard/input-text"),
            ),
            (UiMode::Settings, "compose/input", true, None),
        ] {
            for (value, selected) in [("", false), ("", true), ("Mop", false), ("Mop", true)] {
                let scene = plan(
                    &state,
                    &ViewState {
                        mode,
                        focused_region: Some(focused.into()),
                        compose_engaged: engaged,
                        text_buffer: value.into(),
                        text_selected: selected,
                        ..ViewState::default()
                    },
                )
                .0;
                let decorated: Vec<_> = scene
                    .text
                    .iter()
                    .filter(|text| text.input_state.is_some())
                    .collect();
                if let Some(id) = expected_id {
                    assert_eq!(decorated.len(), 1, "{mode:?} {focused}");
                    assert_eq!(decorated[0].id, id);
                    assert_eq!(
                        decorated[0].input_state,
                        Some(if selected && !value.is_empty() {
                            TextInputState::Selected
                        } else {
                            TextInputState::Caret
                        })
                    );
                } else {
                    assert!(decorated.is_empty(), "{mode:?} {focused}");
                }
            }
        }
        assert!(
            plan(&state, &ViewState::default())
                .0
                .text
                .iter()
                .all(|text| text.input_state.is_none())
        );
    }

    #[test]
    fn keyboard_keeps_full_sized_keys_in_the_lower_band() {
        let state = WorldState::new(7, "Mop");
        for text_scale in [1, 2] {
            let scene = plan(
                &state,
                &ViewState {
                    mode: UiMode::OnScreenKeyboard,
                    text_scale,
                    renaming_with_osk: true,
                    ..ViewState::default()
                },
            )
            .0;
            let panel = scene
                .rects
                .iter()
                .find(|rect| rect.id == "keyboard/panel")
                .unwrap()
                .rect;
            assert!(panel.y >= 80 && panel.y + panel.h <= 147);
            let keys: Vec<_> = scene
                .hit_regions
                .iter()
                .filter(|hit| hit.id.starts_with("keyboard/") && hit.shape == HitShape::Rect)
                .collect();
            assert_eq!(keys.len(), 34);
            for key in keys {
                assert!(key.rect.h >= 12, "{}", key.id);
                assert!(key.rect.x >= panel.x && key.rect.x + key.rect.w <= panel.x + panel.w);
                assert!(key.rect.y >= panel.y && key.rect.y + key.rect.h <= panel.y + panel.h);
            }
            for character in 'a'..='z' {
                assert!(
                    scene
                        .hit_regions
                        .iter()
                        .any(|hit| hit.id == format!("keyboard/{character}")
                            && hit.action == UiAction::TypeCharacter(character))
                );
            }
            for label in scene
                .text
                .iter()
                .filter(|text| text.id.starts_with("keyboard/"))
            {
                let bounds = label.bounds.unwrap();
                assert!(
                    bounds.h as f32 >= label.role.size(text_scale >= 2) * 1.2,
                    "{}",
                    label.id
                );
                assert!(bounds.y >= panel.y && bounds.y + bounds.h <= panel.y + panel.h);
            }
        }
    }

    #[test]
    fn inspection_uses_content_height_with_clear_title_body_and_close_gaps() {
        for text_scale in [1, 2] {
            for name in ["Mop".to_owned(), "W".repeat(24), "🌿".repeat(24)] {
                let mut state = WorldState::new(7, &name);
                state.creature.preferences.insert(FoodId::Mushroom, -0.8);
                for target in [
                    UiTarget::Creature,
                    UiTarget::Cave,
                    UiTarget::Toy(ToyId::Bell),
                    UiTarget::FoodObject(900),
                ] {
                    let scene = plan(
                        &state,
                        &ViewState {
                            mode: UiMode::Inspect(target),
                            context_card_anchor: Some((173, 31)),
                            text_scale,
                            ..ViewState::default()
                        },
                    )
                    .0;
                    let panel = scene
                        .rects
                        .iter()
                        .find(|rect| rect.id == "inspect/panel")
                        .unwrap()
                        .rect;
                    let title = scene
                        .text
                        .iter()
                        .find(|text| text.id == "inspect/title")
                        .unwrap()
                        .bounds
                        .unwrap();
                    let detail = scene
                        .text
                        .iter()
                        .find(|text| text.id == "inspect/detail")
                        .unwrap();
                    let body = detail.bounds.unwrap();
                    let size = detail.role.size(text_scale >= 2);
                    let measured = typography().lines(&detail.text, body.w as f32, size);
                    assert_eq!(
                        body.h,
                        typography()
                            .height(&detail.text, body.w as f32, size)
                            .ceil() as i32
                    );
                    assert_eq!(
                        typography().layout_lines(&detail.text, body.w as f32, body.h as f32, size),
                        *measured
                    );
                    let close = scene
                        .hit_regions
                        .iter()
                        .find(|hit| hit.id == "inspect/close")
                        .unwrap()
                        .rect;
                    assert_eq!(panel.x, 173);
                    assert!(panel.y >= 4 && panel.y + panel.h <= AQUARIUM_BOTTOM - 2);
                    assert!(title.y + title.h + 3 <= body.y);
                    assert!(body.y + body.h + 5 <= close.y);
                    assert!(!detail.vertical_centered);
                    if target != UiTarget::Creature {
                        assert!(panel.h < if text_scale >= 2 { 102 } else { 82 });
                    }
                }
            }
        }
    }

    #[test]
    fn redundant_modal_close_hint_stays_hidden_and_settings_help_remains() {
        let state = WorldState::new(7, "Mop");
        for text_scale in [1, 2] {
            let scene = plan(
                &state,
                &ViewState {
                    mode: UiMode::Settings,
                    hovered_region: Some("compose/close".into()),
                    text_scale,
                    ..ViewState::default()
                },
            )
            .0;
            assert!(scene.hit_regions.iter().any(|hit| hit.id == "modal/close"));
            assert!(scene.text.iter().all(|text| text.id != "ui/hover-label"));
            let scene = plan(
                &state,
                &ViewState {
                    hovered_region: Some("compose/settings".into()),
                    text_scale,
                    ..ViewState::default()
                },
            )
            .0;
            assert!(
                scene
                    .text
                    .iter()
                    .any(|text| text.id == "ui/hover-label" && text.text.contains("Settings"))
            );
        }
    }

    #[test]
    fn binding_rows_keep_focus_gutters_and_large_text_clear_of_navigation() {
        let state = WorldState::new(7, "Mop");
        for text_scale in [1, 2] {
            let render = plan(
                &state,
                &ViewState {
                    mode: UiMode::Bindings,
                    text_scale,
                    status_message: Some("Settings saved".to_owned()),
                    ..ViewState::default()
                },
            )
            .0;
            let panel = render
                .rects
                .iter()
                .find(|command| command.id == "bindings/panel")
                .unwrap()
                .rect;
            let back = render
                .hit_regions
                .iter()
                .find(|hit| hit.id == "modal/close")
                .unwrap();
            let rows: Vec<_> = render
                .hit_regions
                .iter()
                .filter(|hit| matches!(hit.action, UiAction::BeginRebind(_)))
                .collect();
            assert_eq!(rows.len(), 6);
            let mut previous_bottom = back.rect.y + back.rect.h;
            for row in rows {
                assert!(previous_bottom < row.rect.y);
                assert!(row.rect.w >= 80 && row.rect.h >= 14);
                assert!(row.rect.x > panel.x && row.rect.y > panel.y);
                assert!(row.rect.x + row.rect.w < panel.x + panel.w);
                assert!(row.rect.y + row.rect.h < panel.y + panel.h);
                let value = render
                    .text
                    .iter()
                    .find(|text| text.id == format!("{}-label", row.id))
                    .unwrap();
                let bounds = value.bounds.unwrap();
                assert!(bounds.h >= value.role.size(text_scale == 2).ceil() as i32 + 2);
                assert!(bounds.x >= row.rect.x && bounds.y >= row.rect.y);
                assert!(bounds.x + bounds.w <= row.rect.x + row.rect.w);
                assert!(bounds.y + bounds.h <= row.rect.y + row.rect.h);
                previous_bottom = row.rect.y + row.rect.h;
            }
            let status = render
                .rects
                .iter()
                .find(|command| command.id == "status/background")
                .unwrap();
            assert!(!rects_overlap(panel, status.rect));
        }
    }

    #[test]
    fn ui_depth_bands_and_modal_hit_regions_are_unambiguous() {
        let state = WorldState::new(7, "Mop");
        for mode in [
            UiMode::Context(UiTarget::Creature),
            UiMode::FoodChoice,
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
                .filter(|hit| {
                    hit.enabled && hit.shape == HitShape::Rect && !hit.id.starts_with("world/")
                })
                .enumerate()
            {
                for right in render
                    .hit_regions
                    .iter()
                    .filter(|hit| {
                        hit.enabled && hit.shape == HitShape::Rect && !hit.id.starts_with("world/")
                    })
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
    fn prior_receipts_and_new_errors_leave_care_and_centered_modal_actions_clear() {
        let state = WorldState::new(7, "Café 🌿");
        for text_scale in [1, 2] {
            for mode in [
                UiMode::FoodChoice,
                UiMode::ToyChoice,
                UiMode::OnScreenKeyboard,
                UiMode::Rename,
                UiMode::Rebinding(BindableAction::PushToTalk),
                UiMode::ConfirmReset,
            ] {
                let mut view = ViewState {
                    mode,
                    text_scale,
                    focused_region: None,
                    ..ViewState::default()
                };
                let quiet = plan(&state, &view).0;
                for receipt in [
                    "Your creature is now called Café 🌿.",
                    "Could not save the change. Try again.",
                ] {
                    view.status_message = Some(receipt.to_owned());
                    let scene = plan(&state, &view).0;
                    let status = scene
                        .rects
                        .iter()
                        .find(|part| part.id == "status/background")
                        .unwrap()
                        .rect;
                    assert_eq!(
                        panel_overlap(status, &quiet.rects),
                        0,
                        "{mode:?} scale {text_scale}"
                    );
                    if matches!(
                        mode,
                        UiMode::FoodChoice | UiMode::ToyChoice | UiMode::OnScreenKeyboard
                    ) {
                        assert_eq!(status.y, 5);
                    }
                    let message = scene
                        .text
                        .iter()
                        .find(|text| text.id == "status/message")
                        .unwrap();
                    assert_eq!(message.text, receipt);
                    let bounds = message.bounds.unwrap();
                    let lines = typography().layout_lines(
                        receipt,
                        bounds.w as f32,
                        bounds.h as f32,
                        message.role.size(text_scale >= 2),
                    );
                    assert!(
                        !lines.concat().contains('…'),
                        "{mode:?} error text remains available"
                    );
                    for action in quiet
                        .hit_regions
                        .iter()
                        .filter(|hit| hit.enabled && hit.shape == HitShape::Rect)
                    {
                        assert!(
                            scene
                                .hit_regions
                                .iter()
                                .any(|hit| hit.id == action.id && hit.enabled)
                        );
                        assert!(
                            !rects_overlap(status, action.rect),
                            "{mode:?} notice covers {}",
                            action.id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn mushroom_tooltip_keeps_the_complete_name_at_both_text_sizes() {
        let mut state = WorldState::new(7, "Mop");
        state.aquarium.objects.insert(
            100,
            WorldObject::Food(beastie_core::FoodObject {
                id: 100,
                food: FoodId::Mushroom,
                position: NormalizedPosition::new(2200, 4200),
                velocity: beastie_core::NormalizedVelocity::default(),
                buoyancy: beastie_core::FoodBuoyancy::Drift,
                disposition: FoodDisposition::Falling,
                age_ms: 0,
                lifetime_ms: 10_000,
            }),
        );
        for text_scale in [1, 2] {
            let scene = plan(
                &state,
                &ViewState {
                    text_scale,
                    hovered_region: Some("target/object-100".into()),
                    ..ViewState::default()
                },
            )
            .0;
            let tooltip = scene
                .text
                .iter()
                .find(|text| text.id == "ui/hover-label")
                .unwrap();
            assert_eq!(tooltip.text, "mushroom");
            let bounds = tooltip.bounds.unwrap();
            let size = tooltip.role.size(text_scale >= 2);
            assert!(typography().width(&tooltip.text, size) <= bounds.w as f32);
            assert_eq!(
                typography().layout_lines(&tooltip.text, bounds.w as f32, bounds.h as f32, size),
                ["mushroom"]
            );
        }
    }

    #[test]
    fn status_uses_available_space_without_overlapping_modes_or_compose() {
        let state = WorldState::new(7, "Mop");
        for mode in [
            UiMode::Compose,
            UiMode::Settings,
            UiMode::Bindings,
            UiMode::DataManagement,
            UiMode::OnScreenKeyboard,
        ] {
            let quiet = plan(
                &state,
                &ViewState {
                    mode,
                    text_scale: 2,
                    ..ViewState::default()
                },
            )
            .0;
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
                .hit_regions
                .iter()
                .find(|command| command.id == "compose/input")
                .expect("compose field")
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
            assert!(rect_contains(status, message.bounds.unwrap()));
            assert!(text_right(message) <= status.x + status.w - 3);
            for control in quiet
                .hit_regions
                .iter()
                .filter(|hit| hit.enabled && hit.shape == HitShape::Rect)
            {
                assert!(
                    render
                        .hit_regions
                        .iter()
                        .any(|hit| hit.id == control.id && hit.enabled),
                    "{mode:?} notice hid {}",
                    control.id
                );
            }
            if let Some(name) = render
                .text
                .iter()
                .find(|command| command.id == "compose/summary-name")
            {
                assert!(!rects_overlap(
                    name.bounds.unwrap(),
                    message.bounds.unwrap()
                ));
            }
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

    fn rect_contains(outer: Rect, inner: Rect) -> bool {
        inner.x >= outer.x
            && inner.y >= outer.y
            && inner.x + inner.w <= outer.x + outer.w
            && inner.y + inner.h <= outer.y + outer.h
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
    fn compose_focus_stays_within_field_and_nameplate_fits_identity() {
        let state = WorldState::new(7, "Mop");
        let scene = plan(
            &state,
            &ViewState {
                compose_engaged: true,
                focused_region: Some("compose/input".to_owned()),
                ..Default::default()
            },
        )
        .0;
        let focus = scene.rects.iter().find(|r| r.id == "ui/focus").unwrap();
        let field = scene
            .hit_regions
            .iter()
            .find(|r| r.id == "compose/input")
            .unwrap();
        assert!(focus.outline);
        assert!(focus.corner_radius > 0);
        assert_eq!(focus.rect, field.rect);
        let send = scene
            .hit_regions
            .iter()
            .find(|hit| hit.id == "compose/send")
            .unwrap();
        assert!(!rects_overlap(focus.rect, send.rect));
        let identity = scene
            .text
            .iter()
            .find(|t| t.id == "compose/summary-name")
            .unwrap()
            .bounds
            .unwrap();
        assert!(identity.y >= 150);
        assert!(!rects_overlap(identity, field.rect));
    }

    #[test]
    fn quiet_compose_keeps_its_hit_target_and_care_guidance_yields_to_care() {
        let state = WorldState::new(7, "Mop");
        let mut view = ViewState::default();
        let quiet = plan(&state, &view).0;
        assert!(!quiet.text.iter().any(|t| t.id == "compose/care-invitation"));
        assert!(
            quiet
                .rects
                .iter()
                .any(|r| r.id == "compose/input-background")
        );
        for scale in [1, 2] {
            view.text_scale = scale;
            view.compose_engaged = true;
            let editing = plan(&state, &view).0;
            assert_eq!(
                quiet
                    .hit_regions
                    .iter()
                    .find(|h| h.id == "compose/input")
                    .unwrap()
                    .rect,
                editing
                    .hit_regions
                    .iter()
                    .find(|h| h.id == "compose/input")
                    .unwrap()
                    .rect
            );
            assert!(
                editing
                    .text
                    .iter()
                    .any(|t| t.id == "compose/input-text" && t.role == TextRole::Body)
            );
            assert!(
                !editing
                    .text
                    .iter()
                    .any(|t| t.id == "compose/care-invitation")
            );
        }
        view.compose_engaged = false;
        view.observe_events(&[GameEvent::Comforted], 0);
        assert!(
            !plan(&state, &view)
                .0
                .text
                .iter()
                .any(|t| t.id == "compose/care-invitation")
        );
        view.hovered_region = Some("target/creature".into());
        assert!(
            plan(&state, &view)
                .0
                .text
                .iter()
                .any(|t| t.id == "ui/hover-label" && t.text == "Pet Mop")
        );
    }

    #[test]
    fn autonomous_play_does_not_dismiss_the_players_care_invitation() {
        let world = WorldState::new(7, "Mop");
        let mut view = ViewState::default();
        view.observe_events(
            &[GameEvent::ToyPlayed {
                toy: ToyId::Ball,
                interaction_id: NonZeroU64::new(1).unwrap(),
                origin: beastie_core::ToyOrigin::Autonomous,
            }],
            0,
        );
        assert!(!view.care_guidance_dismissed);
        view.observe_events(
            &[GameEvent::ToyPlayAccepted {
                toy: ToyId::Ball,
                interaction_id: NonZeroU64::new(2).unwrap(),
                origin: beastie_core::ToyOrigin::Player,
            }],
            0,
        );
        assert!(
            !plan(&world, &view)
                .0
                .text
                .iter()
                .any(|t| t.id == "compose/care-invitation")
        );
    }

    #[test]
    fn microphone_help_survives_disabled_hover_and_reachable_settings_focus() {
        let state = WorldState::new(7, "Mop");
        for scale in [1, 2] {
            for microphone in [
                MicrophoneState::Disabled,
                MicrophoneState::Idle,
                MicrophoneState::Listening,
                MicrophoneState::Recognizing,
                MicrophoneState::Unavailable,
                MicrophoneState::Error,
            ] {
                let mut view = ViewState {
                    text_scale: scale,
                    microphone_enabled: microphone != MicrophoneState::Disabled,
                    microphone_state: microphone,
                    hovered_region: Some("compose/microphone".into()),
                    ..Default::default()
                };
                let scene = plan(&state, &view).0;
                let status = visible_status(&view, 0);
                let has_status = status.is_some();
                let description_id = if status.is_some() {
                    "status/message"
                } else {
                    "ui/microphone-help"
                };
                let help = scene.text.iter().find(|t| t.id == description_id).unwrap();
                assert_eq!(
                    help.text,
                    status.unwrap_or_else(|| microphone_label(microphone))
                );
                assert_eq!(help.role, TextRole::Secondary);
                let bounds = help.bounds.unwrap();
                assert!(bounds.h >= help.role.size(scale == 2).ceil() as i32);
                assert!(bounds.x >= 0 && bounds.x + bounds.w <= LOGICAL_WIDTH);
                assert!(bounds.y >= 0 && bounds.y + bounds.h < COMPOSE_BAR_TOP);
                if microphone == MicrophoneState::Disabled {
                    assert!(help.text.contains("Settings › Sound"));
                }
                assert_eq!(
                    scene
                        .text
                        .iter()
                        .filter(|t| t.id == "status/message" || t.id == "ui/microphone-help")
                        .count(),
                    1
                );
                if matches!(
                    microphone,
                    MicrophoneState::Disabled
                        | MicrophoneState::Unavailable
                        | MicrophoneState::Error
                ) {
                    assert!(
                        !scene
                            .hit_regions
                            .iter()
                            .find(|h| h.id == "compose/microphone")
                            .unwrap()
                            .enabled
                    );
                    view.hovered_region = None;
                    view.focused_region = Some("compose/settings".into());
                    let focused = plan(&state, &view).0;
                    assert!(
                        focused
                            .hit_regions
                            .iter()
                            .any(|hit| hit.id == "compose/settings" && hit.enabled)
                    );
                    if has_status {
                        assert!(focused.text.iter().any(|text| text.id == "status/message"));
                    }
                }
            }
        }
    }

    #[test]
    fn microphone_hover_never_covers_existing_feedback_or_modal_controls() {
        let state = WorldState::new(7, "Mop");
        for mode in [UiMode::Compose, UiMode::Settings, UiMode::FoodChoice] {
            let mut view = ViewState {
                mode,
                hovered_region: Some("compose/microphone".into()),
                text_scale: 2,
                ..Default::default()
            };
            let scene = plan(&state, &view).0;
            let help = scene
                .text
                .iter()
                .find(|t| t.id == "ui/microphone-help")
                .unwrap()
                .bounds
                .unwrap();
            assert!(help.y >= 0 && help.y + help.h <= AQUARIUM_BOTTOM);
            for hit in scene
                .hit_regions
                .iter()
                .filter(|h| matches!(h.shape, HitShape::Rect))
            {
                assert!(!rects_overlap(help, hit.rect));
            }
            view.show_status("I heard only part of that. Please try again.", 0, 4000);
            let scene = plan(&state, &view).0;
            assert!(!scene.text.iter().any(|t| t.id == "ui/microphone-help"));
            assert!(
                scene
                    .text
                    .iter()
                    .any(|t| t.id == "status/message" && t.text.contains("only part"))
            );
            view.expire(4001);
            assert!(
                plan(&state, &view)
                    .0
                    .text
                    .iter()
                    .any(|t| t.id == "ui/microphone-help")
            );
        }
    }

    #[test]
    fn rail_identity_stays_below_speech_and_modal_content() {
        let state = WorldState::new(7, "Mop");
        let quiet = plan(&state, &ViewState::default()).0;
        assert!(
            quiet
                .text
                .iter()
                .any(|t| t.id == "compose/summary-name" && t.text == "Mop")
        );
        assert!(!quiet.text.iter().any(|t| t.id == "status/message"));
        for view in [
            ViewState {
                speech: Some("Hello".to_owned()),
                ..Default::default()
            },
            ViewState {
                mode: UiMode::Settings,
                ..Default::default()
            },
        ] {
            let scene = plan(&state, &view).0;
            assert!(
                scene
                    .text
                    .iter()
                    .any(|t| t.id == "compose/summary-name" && t.bounds.unwrap().y >= 150)
            );
            assert!(!scene.rects.iter().any(|r| r.id == "compose/identity-plate"));
        }
    }

    #[test]
    fn modal_hides_world_picking_and_controls_have_explicit_labels() {
        let state = WorldState::new(7, "Mop");
        let scene = plan(&state, &ViewState::default()).0;
        for (id, action) in [
            ("compose/send", None),
            ("compose/microphone", None),
            ("compose/settings", None),
            (
                "compose/creature",
                Some(UiAction::OpenContext(UiTarget::Creature)),
            ),
            ("compose/toy-0", Some(UiAction::Play(ToyId::Ball))),
            ("compose/toy-2", Some(UiAction::Play(ToyId::Sock))),
            (
                "compose/food-berry",
                Some(UiAction::SelectFood(FoodId::Berry)),
            ),
            (
                "compose/food-pellet",
                Some(UiAction::SelectFood(FoodId::Pellet)),
            ),
        ] {
            let hit = scene
                .hit_regions
                .iter()
                .find(|hit| hit.id == id)
                .unwrap_or_else(|| panic!("{id}"));
            assert!(!hit.label.is_empty(), "{id}");
            if let Some(action) = action {
                assert_eq!(hit.action, action, "{id}: one click acts");
            }
        }
        for utility in ["send", "speak", "settings", "feed"] {
            assert!(
                !scene
                    .text
                    .iter()
                    .any(|text| text.id == format!("compose/control-{utility}"))
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
                .all(|hit| !matches!(hit.shape, HitShape::World(_)))
        );
        assert!(
            modal
                .hit_regions
                .iter()
                .any(|hit| hit.shape == HitShape::Blocker && !hit.enabled)
        );
        assert!(
            !modal
                .hit_regions
                .iter()
                .any(|hit| hit.id == "settings/grid")
        );
    }

    fn teach(state: &mut WorldState, word: &str, meaning: beastie_core::Meaning) {
        for at in 0..2 {
            state.creature.lexicon.hear(word, &[(meaning, 3)], at);
        }
    }

    fn coach_text(state: &WorldState, view: &ViewState) -> Option<String> {
        plan(state, view)
            .0
            .text
            .iter()
            .find(|text| text.id == "coach/hint")
            .map(|text| text.text.clone())
    }

    #[test]
    fn coaching_hint_leads_from_naming_to_asking_and_then_steps_aside() {
        let mut state = WorldState::new(4, "Mop");
        let mut rng = beastie_core::SeededRandom::new(4);
        state.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        state.creature.needs.hunger = 0.2;
        beastie_core::step(
            &mut state,
            &[beastie_core::PlayerEvent::Play(ToyId::Ball)],
            0,
            &mut rng,
        );
        assert_eq!(
            beastie_core::current_want(&state),
            Some(beastie_core::Want::NameOf(beastie_core::Meaning::Toy(
                ToyId::Ball
            )))
        );
        let view = ViewState::default();
        let naming = coaching_hint(&state, &view).expect("a first lesson is suggested");
        assert!(naming.contains("what the ball is called"), "{naming}");
        assert_eq!(coach_text(&state, &view), Some(naming));

        teach(&mut state, "ball", beastie_core::Meaning::Toy(ToyId::Ball));
        assert_eq!(state.creature.lexicon.learned_count(), 1);
        assert_eq!(
            coaching_hint(&state, &view).as_deref(),
            Some("Mop knows “ball”. Say it to ask for it.")
        );

        // Typing, or any other surface, hides the hint.
        let typing = ViewState {
            text_buffer: "ba".to_owned(),
            ..ViewState::default()
        };
        assert_eq!(coaching_hint(&state, &typing), None);
        assert_eq!(coach_text(&state, &typing), None);
        let settings = ViewState {
            mode: UiMode::Settings,
            ..ViewState::default()
        };
        assert_eq!(coaching_hint(&state, &settings), None);

        for (word, meaning) in [
            ("bell", beastie_core::Meaning::Toy(ToyId::Bell)),
            ("sock", beastie_core::Meaning::Toy(ToyId::Sock)),
            ("berry", beastie_core::Meaning::Food(FoodId::Berry)),
            ("shroom", beastie_core::Meaning::Food(FoodId::Mushroom)),
        ] {
            teach(&mut state, word, meaning);
        }
        assert_eq!(state.creature.lexicon.learned_count(), 5);
        assert!(
            coaching_hint(&state, &view).is_some(),
            "five words without a request still invites the first request"
        );
        state.creature.development.interactions.requests = 1;
        assert_eq!(coaching_hint(&state, &view), None);
        assert_eq!(coach_text(&state, &view), None);
    }

    #[test]
    fn want_bubble_is_projected_only_while_the_creature_is_not_speaking() {
        let mut state = WorldState::new(4, "Mop");
        state.creature.needs.hunger = 0.8;
        let want = beastie_core::current_want(&state);
        assert!(matches!(want, Some(beastie_core::Want::Food(_))));
        let quiet = plan(&state, &ViewState::default()).0;
        assert_eq!(quiet.creature.want, want);

        let mut talking = ViewState::default();
        talking.show_speech("mop hungry".to_owned(), 0);
        assert_eq!(plan(&state, &talking).0.creature.want, None);
        let voiced = ViewState {
            speaking: true,
            ..ViewState::default()
        };
        assert_eq!(plan(&state, &voiced).0.creature.want, None);

        state.creature.needs.hunger = 0.2;
        state.creature.needs.energy = 0.9;
        state.creature.needs.comfort = 0.8;
        state.creature.needs.curiosity = 0.3;
        assert_eq!(plan(&state, &ViewState::default()).0.creature.want, None);
    }
}
