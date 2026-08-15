//! Display-free projection of the authoritative aquarium into a 320x180 scene.
//!
//! This crate owns logical pixels, presentation timing, semantic hit regions, and
//! presentation-only effects. It never mutates simulation state.

use beastie_core::{
    ActionPhase, FoodDisposition, FoodDropRejectionReason, FoodId, GameEvent, GazeTarget,
    Intention, Mood, NonverbalAct, NormalizedPosition, Reaction, SteeringMode, ToyId, WorldObject,
    WorldState,
};
use serde::{Deserialize, Serialize};

pub const LOGICAL_WIDTH: i32 = 320;
pub const LOGICAL_HEIGHT: i32 = 180;
pub const AQUARIUM_BOTTOM: i32 = 129;
pub const COMPOSE_BAR_TOP: i32 = 130;
/// The selected 80x80 character has a 54x42 opaque footprint and is drawn at 2x.
pub const CREATURE_CANVAS_SIZE: i32 = 160;
pub const CREATURE_HIT_WIDTH: i32 = 108;
pub const CREATURE_HIT_HEIGHT: i32 = 84;
pub const SPEECH_LIFETIME_MS: u64 = 8_000;
pub const CUE_QUEUE_LIMIT: usize = 8;

const UI_SHADOW: [u8; 4] = [4, 10, 16, 220];
const UI_EDGE: [u8; 4] = [129, 112, 76, 255];
const UI_EDGE_LIT: [u8; 4] = [190, 169, 111, 255];
const UI_PANEL: [u8; 4] = [12, 29, 39, 248];
const UI_PANEL_INSET: [u8; 4] = [7, 19, 29, 255];
const UI_BUTTON: [u8; 4] = [24, 52, 61, 255];
const UI_BUTTON_DISABLED: [u8; 4] = [19, 31, 39, 230];
const UI_CORAL: [u8; 4] = [194, 103, 84, 255];

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
    SetTextScale(u8),
    ToggleReducedMotion,
    ToggleReducedFlashes,
    ToggleReducedShake,
    TogglePixelGrid,
    CycleWindowScale,
    ToggleFullscreen,
    CycleEffectsVolume,
    CycleSpeechVolume,
    ToggleVoice,
    ToggleSubtitles,
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
    Food,
    Play,
    Comfort,
    Settings,
    Cancel,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationCue {
    pub kind: PresentationCueKind,
    pub starts_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewState {
    pub mode: UiMode,
    /// Stable [`HitRegion::id`] selected by keyboard or controller navigation.
    pub focused_region: Option<String>,
    /// Stable [`HitRegion::id`] beneath the pointer.
    pub hovered_region: Option<String>,
    pub text_buffer: String,
    pub pending: bool,
    pub speech: Option<String>,
    pub speech_expires_at_ms: Option<u64>,
    #[serde(default)]
    pub cue_queue: Vec<PresentationCue>,
    #[serde(default)]
    pub pixel_grid: bool,
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
    pub transcript_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingLabels {
    pub food: String,
    pub play: String,
    pub comfort: String,
    pub settings: String,
    pub cancel: String,
}

impl Default for BindingLabels {
    fn default() -> Self {
        Self {
            food: "F1".to_owned(),
            play: "F2".to_owned(),
            comfort: "F3".to_owned(),
            settings: "F10".to_owned(),
            cancel: "Escape".to_owned(),
        }
    }
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            mode: UiMode::Compose,
            focused_region: Some("compose/input".to_owned()),
            hovered_region: None,
            text_buffer: String::new(),
            pending: false,
            speech: None,
            speech_expires_at_ms: None,
            cue_queue: Vec::new(),
            pixel_grid: false,
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
            text_speed: default_text_speed(),
            binding_labels: BindingLabels::default(),
            controller_active: false,
            speaking: false,
            mouth_phase: 0,
            transcript_enabled: false,
            status_message: None,
            transcript_status: None,
        }
    }
}

impl ViewState {
    pub fn show_speech(&mut self, speech: String, now_ms: u64) {
        self.speech = Some(speech);
        self.speech_expires_at_ms = Some(now_ms.saturating_add(SPEECH_LIFETIME_MS));
    }

    /// Appends important reactions instead of replacing the cue currently on screen.
    pub fn observe_events(&mut self, events: &[GameEvent], now_ms: u64) {
        self.expire(now_ms);
        for event in events {
            let cue = cue_for_event(event);
            if let Some((kind, duration_ms)) = cue {
                self.enqueue_cue(kind, duration_ms, now_ms);
            }
        }
    }

    pub fn expire(&mut self, now_ms: u64) {
        if self
            .speech_expires_at_ms
            .is_some_and(|expires| now_ms >= expires)
        {
            self.speech = None;
            self.speech_expires_at_ms = None;
        }
        self.cue_queue.retain(|cue| now_ms < cue.expires_at_ms);
    }

    pub fn enqueue_cue(&mut self, kind: PresentationCueKind, duration_ms: u64, now_ms: u64) {
        self.cue_queue.retain(|cue| cue.expires_at_ms > now_ms);
        if let Some(last) = self
            .cue_queue
            .last_mut()
            .filter(|cue| cue.kind == kind && cue.expires_at_ms > now_ms)
        {
            last.expires_at_ms = last
                .expires_at_ms
                .max(now_ms.saturating_add(duration_ms.max(1)));
            return;
        }
        if matches!(
            kind,
            PresentationCueKind::Affection
                | PresentationCueKind::Comfort
                | PresentationCueKind::Suspicion
                | PresentationCueKind::Spit
                | PresentationCueKind::AquariumFull
        ) {
            // Direct player feedback must be legible when it happens. Do not make comfort or a
            // forceful refusal wait behind seconds of low-priority ambient/action punctuation.
            self.cue_queue.clear();
        }
        if self.cue_queue.len() >= CUE_QUEUE_LIMIT {
            return;
        }
        let starts_at_ms = self
            .cue_queue
            .last()
            .map_or(now_ms, |cue| cue.expires_at_ms.max(now_ms));
        self.cue_queue.push(PresentationCue {
            kind,
            starts_at_ms,
            expires_at_ms: starts_at_ms.saturating_add(duration_ms.max(1)),
        });
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpriteFlip {
    None,
    Horizontal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpriteCommand {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub layer: i16,
    #[serde(default)]
    pub frame: u8,
    pub flip: SpriteFlip,
    /// Optional whole-pixel source crop. `None` selects the complete asset.
    pub source_rect: Option<Rect>,
    /// Positive integer nearest-neighbor scale.
    pub scale: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RectCommand {
    pub id: String,
    pub rect: Rect,
    pub color: [u8; 4],
    pub layer: i16,
    /// An outline has no scaling semantics and remains one logical pixel wide.
    #[serde(default)]
    pub outline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextCommand {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderPlan {
    pub sprites: Vec<SpriteCommand>,
    pub rects: Vec<RectCommand>,
    pub text: Vec<TextCommand>,
    pub hit_regions: Vec<HitRegion>,
    pub summary: CreatureSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioCue {
    AquariumHum,
    Bubble,
    Wake,
    FoodDrop,
    FoodEat,
    FoodReject,
    Sand,
    Affection,
    Sleep,
    UiReject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioPlan {
    pub ambience: Vec<AudioCue>,
    pub events: Vec<AudioCue>,
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

/// Maps a simulation coordinate to a whole logical pixel in the visible water volume.
#[must_use]
pub fn world_to_logical(position: NormalizedPosition) -> (i32, i32) {
    const X_MIN: i32 = 4;
    const X_SPAN: i32 = 311;
    const Y_MIN: i32 = 4;
    const Y_SPAN: i32 = 121;
    let position = position.clamped();
    let x = X_MIN + rounded_ratio(position.x, X_SPAN);
    let y = Y_MIN + rounded_ratio(position.y, Y_SPAN);
    (x, y)
}

#[must_use]
pub fn logical_to_world(x: i32, y: i32) -> NormalizedPosition {
    const X_MIN: i32 = 4;
    const X_SPAN: i32 = 311;
    const Y_MIN: i32 = 4;
    const Y_SPAN: i32 = 121;
    NormalizedPosition::new(
        ((x - X_MIN).clamp(0, X_SPAN) * NormalizedPosition::SCALE + X_SPAN / 2) / X_SPAN,
        ((y - Y_MIN).clamp(0, Y_SPAN) * NormalizedPosition::SCALE + Y_SPAN / 2) / Y_SPAN,
    )
}

#[must_use]
pub fn plan(state: &WorldState, view: &ViewState) -> (RenderPlan, AudioPlan) {
    let now_ms = state.elapsed_ms;
    let mut sprites = environment_sprites(state, view);
    let mut rects = environment_rects(state, view);
    let mut text = Vec::new();
    let mut hit_regions = world_hit_regions(state, view.mode);

    add_objects(state, &mut sprites);
    add_creature(state, view, &mut sprites, &mut rects);
    add_speech(state, view, &mut rects, &mut text, &mut hit_regions);
    add_persistent_bar(
        state,
        view,
        &mut sprites,
        &mut rects,
        &mut text,
        &mut hit_regions,
    );
    add_temporary_mode(view, &mut rects, &mut text, &mut hit_regions);
    add_status(view, state.elapsed_ms, &mut rects, &mut text);
    add_hover_and_focus(view, &hit_regions, &mut rects, &mut text);
    if view.pixel_grid {
        add_pixel_grid(&mut rects);
    }
    let text_scale = view.text_scale.clamp(1, 2);
    for command in &mut text {
        command.scale = text_scale;
    }
    if view.reduced_flashes {
        for command in &mut rects {
            if command.id.starts_with("effect/") || command.id.starts_with("aquarium/caustic-") {
                command.color[3] = command.color[3].min(80);
            }
        }
    }

    sprites.sort_by_key(|command| command.layer);
    rects.sort_by_key(|command| command.layer);
    text.sort_by_key(|command| command.layer);
    let active_cue = view.active_cue(now_ms);
    let events = active_cue.into_iter().map(audio_for_cue).collect();
    (
        RenderPlan {
            sprites,
            rects,
            text,
            hit_regions,
            summary: creature_summary(state),
        },
        AudioPlan {
            ambience: vec![AudioCue::AquariumHum],
            events,
        },
    )
}

fn rounded_ratio(normalized: i32, span: i32) -> i32 {
    (normalized * span + NormalizedPosition::SCALE / 2) / NormalizedPosition::SCALE
}

fn environment_sprites(state: &WorldState, view: &ViewState) -> Vec<SpriteCommand> {
    let elapsed_ms = if view.reduced_motion {
        0
    } else {
        state.elapsed_ms
    };
    vec![
        SpriteCommand {
            id: "aquarium/background".to_owned(),
            x: 0,
            y: 0,
            layer: 0,
            frame: 0,
            flip: SpriteFlip::None,
            // Crop the unwanted surface at 1:1 instead of resampling the art.
            source_rect: Some(Rect {
                x: 0,
                y: 25,
                w: 320,
                h: 130,
            }),
            scale: 1,
        },
        framed_sprite(
            "aquarium/caustics",
            0,
            0,
            1,
            u8::try_from((elapsed_ms / 900) % 4).unwrap_or_default(),
        ),
        framed_sprite(
            "aquarium/distant-bubbles",
            0,
            0,
            3,
            u8::try_from((elapsed_ms / 700) % 4).unwrap_or_default(),
        ),
    ]
}

fn environment_rects(state: &WorldState, view: &ViewState) -> Vec<RectCommand> {
    let elapsed_ms = if view.reduced_motion {
        0
    } else {
        state.elapsed_ms
    };
    let cycle = (state.elapsed_ms % beastie_core::ACTIVE_DAY_MS) * 3 / beastie_core::ACTIVE_DAY_MS;
    let tint = match cycle {
        0 => [47, 123, 130, 12],
        1 => [151, 113, 87, 12],
        _ => [18, 36, 73, 34],
    };
    let mut rects = vec![
        rect(
            "aquarium/light-cycle",
            Rect {
                x: 0,
                y: 0,
                w: LOGICAL_WIDTH,
                h: AQUARIUM_BOTTOM + 1,
            },
            tint,
            4,
        ),
        rect(
            "aquarium/sand-bed",
            Rect {
                x: 0,
                y: 112,
                w: LOGICAL_WIDTH,
                h: 18,
            },
            [64, 76, 68, 255],
            5,
        ),
    ];
    for index in 0..7_i32 {
        let phase = i32::try_from((elapsed_ms / 1_100) % 19).unwrap_or_default();
        let x = (index * 53 + phase * 2) % LOGICAL_WIDTH;
        let y = (index * 29 + phase * 3) % 108;
        rects.push(rect(
            &format!("aquarium/bubble-{index}-top"),
            Rect { x, y, w: 3, h: 1 },
            [171, 225, 216, 130],
            3,
        ));
        rects.push(rect(
            &format!("aquarium/bubble-{index}-side"),
            Rect {
                x: x - 1,
                y: y + 1,
                w: 1,
                h: 2,
            },
            [171, 225, 216, 90],
            3,
        ));
    }
    for index in 0..5_i32 {
        let x = 18 + index * 70 + i32::try_from((elapsed_ms / 1_600) % 6).unwrap_or_default();
        rects.push(rect(
            &format!("aquarium/caustic-{index}"),
            Rect {
                x,
                y: 18 + index % 2 * 26,
                w: 24,
                h: 1,
            },
            [164, 220, 194, 28],
            3,
        ));
    }
    for index in 0..12_i32 {
        let phase = i32::try_from((elapsed_ms / 420) % 31).unwrap_or_default();
        let x = (index * 47 + phase * (index % 3 + 1)) % LOGICAL_WIDTH;
        let y = (index * 23 + phase) % 110;
        rects.push(rect(
            &format!("aquarium/particle-{index}"),
            Rect { x, y, w: 1, h: 1 },
            [178, 221, 190, 85],
            2,
        ));
    }
    rects
}

fn add_objects(state: &WorldState, sprites: &mut Vec<SpriteCommand>) {
    for object in state.aquarium.objects.values() {
        let (asset, position, layer, source_rect) = match object {
            WorldObject::Food(food) if !matches!(food.disposition, FoodDisposition::Consumed) => {
                (food_asset(food.food), food.position, 10, None)
            }
            WorldObject::Food(_) => continue,
            WorldObject::Toy { toy, position } => (
                "aquarium/toys",
                *position,
                9,
                Some(Rect {
                    x: toy_sheet_x(*toy),
                    y: 0,
                    w: 32,
                    h: 32,
                }),
            ),
            WorldObject::Plant { position } => (
                "aquarium/plants",
                *position,
                6,
                Some(Rect {
                    x: 0,
                    y: 0,
                    w: 32,
                    h: 64,
                }),
            ),
            WorldObject::Cave { position } => ("aquarium/cave", *position, 5, None),
        };
        let (x, y) = world_to_logical(position);
        sprites.push(SpriteCommand {
            id: asset.to_owned(),
            x: x - 8,
            y: y - 8,
            layer,
            frame: 0,
            flip: SpriteFlip::None,
            source_rect,
            scale: 1,
        });
    }
}

fn add_creature(
    state: &WorldState,
    view: &ViewState,
    sprites: &mut Vec<SpriteCommand>,
    rects: &mut Vec<RectCommand>,
) {
    let creature = state.creature.aquarium;
    let (center_x, center_y) = world_to_logical(creature.position);
    let mut x =
        (center_x - CREATURE_CANVAS_SIZE / 2).clamp(-52, LOGICAL_WIDTH - CREATURE_CANVAS_SIZE + 52);
    let mut y = (center_y - CREATURE_CANVAS_SIZE / 2)
        .clamp(-50, AQUARIUM_BOTTOM - CREATURE_CANVAS_SIZE + 50);
    let cue = view.active_cue(state.elapsed_ms);
    if dialogue_active(view) {
        x = if center_x >= LOGICAL_WIDTH / 2 {
            x.clamp(168, 184)
        } else {
            x.clamp(-16, -12)
        };
        y = y.clamp(-20, -8);
    } else if matches!(creature.gaze, GazeTarget::Player) || cue.is_some_and(cue_has_body_override)
    {
        // Full-body front-facing acting is the dialogue close-up. Keep every opaque pixel of the
        // largest curated state inside the 320x130 water stage even if world movement reached an
        // intentionally permissive edge position.
        x = x.clamp(-16, 184);
        y = y.clamp(-20, -8);
    }
    let pose = creature_pose(state);
    let flip = if matches!(creature.facing, beastie_core::Facing::Left) {
        SpriteFlip::Horizontal
    } else {
        SpriteFlip::None
    };
    let elapsed_ms = if view.reduced_motion {
        0
    } else {
        state.elapsed_ms
    };
    let bob = if view.reduced_motion {
        0
    } else {
        bob_offset(state)
    };
    if !view.reduced_shake
        && matches!(
            view.active_cue(state.elapsed_ms),
            Some(PresentationCueKind::Recoil | PresentationCueKind::Spit)
        )
    {
        x += if (state.elapsed_ms / 80).is_multiple_of(2) {
            -2
        } else {
            2
        };
    }
    let (body_id, body_flip, frame) = body_sprite(state, view, pose, elapsed_ms, flip);
    sprites.push(SpriteCommand {
        id: body_id,
        x,
        y: y + bob,
        layer: 12,
        frame,
        flip: body_flip,
        source_rect: None,
        scale: 2,
    });
    if !view.reduced_motion
        && creature.velocity.x.unsigned_abs() + creature.velocity.y.unsigned_abs() > 25
    {
        sprites.push(framed_sprite(
            "aquarium/wake",
            if matches!(flip, SpriteFlip::Horizontal) {
                x + 51
            } else {
                x - 13
            },
            y + 37,
            11,
            u8::try_from((elapsed_ms / 180) % 4).unwrap_or_default(),
        ));
    }
    if let Some(cue) = view.active_cue(state.elapsed_ms) {
        sprites.push(effect_sprite(
            cue,
            x,
            y + bob,
            body_flip,
            u8::try_from((elapsed_ms / 180) % 4).unwrap_or_default(),
        ));
    }
    if matches!(creature.steering, SteeringMode::Settle) && center_y > 120 {
        sprites.push(framed_sprite(
            "aquarium/sand-puff",
            center_x - 22,
            center_y + 16,
            11,
            u8::try_from((elapsed_ms / 250) % 4).unwrap_or_default(),
        ));
    }

    if let Some(action) = creature.action {
        let width =
            i32::try_from(action.elapsed_ms.saturating_mul(24) / action.phase_duration_ms.max(1))
                .unwrap_or(24)
                .clamp(1, 24);
        rects.push(rect(
            "creature/action-phase",
            Rect {
                x: center_x - 12,
                y: (y - 3).max(1),
                w: width,
                h: 1,
            },
            phase_color(action.phase),
            16,
        ));
    }
}

fn world_hit_regions(state: &WorldState, mode: UiMode) -> Vec<HitRegion> {
    if let UiMode::FoodDrop(food) = mode {
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
        }];
    }
    if !matches!(mode, UiMode::Compose) {
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
            WorldObject::Toy { toy, position } => (
                UiTarget::Toy(*toy),
                *position,
                state
                    .aquarium
                    .object_names
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| toy_name(*toy).to_owned()),
            ),
            WorldObject::Plant { position } => {
                (UiTarget::Plant(*id), *position, "Plant".to_owned())
            }
            WorldObject::Cave { position } => (UiTarget::Cave, *position, "Cave".to_owned()),
        };
        let (x, y) = world_to_logical(position);
        hits.push(HitRegion {
            id: format!("target/object-{id}"),
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
        });
    }
    hits
}

fn add_persistent_bar(
    state: &WorldState,
    view: &ViewState,
    sprites: &mut Vec<SpriteCommand>,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    let summary = creature_summary(state);
    let text_scale = view.text_scale.clamp(1, 2);
    let glyph_width = i32::from(text_scale) * 6;
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

    sprites.push(ui_sprite("ui/status-pearl", 4, 132, 33));
    rects.push(rect(
        "compose/mood-dot",
        Rect {
            x: 18,
            y: 142,
            w: 3,
            h: 3,
        },
        mood_color(summary.mood),
        34,
    ));
    if visible_status(view, state.elapsed_ms).is_none() {
        text.push(label(
            "compose/summary-name",
            &head_fit(&summary.name, if text_scale >= 2 { 8 } else { 16 }),
            28,
            136,
            34,
        ));
        text.push(label(
            "compose/summary-behavior",
            &head_fit(&summary.behavior, if text_scale >= 2 { 10 } else { 22 }),
            if text_scale >= 2 { 135 } else { 142 },
            136,
            34,
        ));
    }

    let input_rect = Rect {
        x: 5,
        y: 153,
        w: 230,
        h: 23,
    };
    let food_rect = Rect {
        x: 240,
        y: 153,
        w: 23,
        h: 23,
    };
    let settings_rect = Rect {
        x: 267,
        y: 153,
        w: 23,
        h: 23,
    };
    let send_rect = Rect {
        x: 294,
        y: 153,
        w: 22,
        h: 23,
    };
    add_inset("compose/input", input_rect, 31, rects);
    let input_value = if view.pending {
        format!("{} is thinking...", summary.name)
    } else if view.text_buffer.is_empty() {
        format!("Talk to {}...", summary.name)
    } else {
        tail_fit(
            &view.text_buffer,
            usize::try_from((input_rect.w - 11) / glyph_width).unwrap_or(1),
        )
    };
    text.push(label("compose/input-text", &input_value, 11, 160, 34));
    hits.push(hit(
        "compose/input",
        Some(UiTarget::ComposeField),
        UiAction::FocusCompose,
        input_rect,
        !view.pending,
        "Message",
    ));
    hits.push(hit(
        "compose/food",
        Some(UiTarget::Actions),
        UiAction::OpenFoodChoice,
        food_rect,
        !view.pending,
        "Food",
    ));
    add_button_chrome("compose/food", food_rect, !view.pending, false, 31, rects);
    sprites.push(ui_sprite(
        "ui/button-food",
        food_rect.x + 2,
        food_rect.y + 2,
        35,
    ));
    hits.push(hit(
        "compose/settings",
        Some(UiTarget::Actions),
        UiAction::OpenSettings,
        settings_rect,
        !view.pending,
        "Settings",
    ));
    add_button_chrome(
        "compose/settings",
        settings_rect,
        !view.pending,
        false,
        31,
        rects,
    );
    sprites.push(ui_sprite(
        "ui/button-settings",
        settings_rect.x + 2,
        settings_rect.y + 2,
        35,
    ));
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
    if send_enabled {
        sprites.push(ui_sprite(
            "ui/button-send",
            send_rect.x + 2,
            send_rect.y + 2,
            35,
        ));
    }
}

fn head_fit(value: &str, capacity: usize) -> String {
    if value.chars().count() <= capacity {
        return value.to_owned();
    }
    match capacity {
        0 => String::new(),
        1 => "~".to_owned(),
        _ => format!("{}~", value.chars().take(capacity - 1).collect::<String>()),
    }
}

fn tail_fit(value: &str, capacity: usize) -> String {
    let count = value.chars().count();
    if count <= capacity {
        return value.to_owned();
    }
    match capacity {
        0 => String::new(),
        1 => "~".to_owned(),
        _ => format!(
            "~{}",
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
                "mode/drop-food",
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
    text.push(label("settings/title", "Settings", 12, 8, 29));
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
            "grid",
            "Pixel grid",
            on_off(view.pixel_grid),
            UiAction::TogglePixelGrid,
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
    for (index, (id, setting, value, action)) in settings.into_iter().enumerate() {
        let column = i32::try_from(index / 8).unwrap_or_default();
        let row = i32::try_from(index % 8).unwrap_or_default();
        let column_x = 12 + column * 151;
        let y = 18 + row * 13;
        text.push(label(
            &format!("settings/{id}-name"),
            setting,
            column_x,
            y + 3,
            29,
        ));
        let button = Rect {
            x: column_x + 96,
            y,
            w: 48,
            h: 11,
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
            button.x + 5,
            button.y + 3,
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
    text.push(label(
        "reset/warning",
        "Reset this creature? Backup stays recoverable.",
        57,
        48,
        32,
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
        let y = 35 + i32::try_from(index).unwrap_or_default() * 17;
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
        "bindings/capture-panel",
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

fn bindable_id(action: BindableAction) -> &'static str {
    match action {
        BindableAction::Food => "food",
        BindableAction::Play => "play",
        BindableAction::Comfort => "comfort",
        BindableAction::Settings => "settings",
        BindableAction::Cancel => "cancel",
    }
}

fn bindable_name(action: BindableAction) -> &'static str {
    match action {
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
            y,
            w: width,
            h: 27,
        },
        25,
        rects,
    );
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
    let panel_x = if creature_x >= LOGICAL_WIDTH / 2 {
        5
    } else {
        155
    };
    add_panel_chrome(
        "speech/panel",
        Rect {
            x: panel_x,
            y: 5,
            w: 160,
            h: 47,
        },
        22,
        rects,
    );
    rects.push(rect(
        "speech/tail-edge",
        Rect {
            x: if panel_x < 100 {
                panel_x + 148
            } else {
                panel_x + 3
            },
            y: 70,
            w: 9,
            h: 3,
        },
        UI_EDGE,
        24,
    ));
    rects.push(rect(
        "speech/tail",
        Rect {
            x: if panel_x < 100 {
                panel_x + 150
            } else {
                panel_x + 5
            },
            y: 70,
            w: 5,
            h: 5,
        },
        UI_PANEL,
        25,
    ));
    text.push(label("speech/text", speech, panel_x + 9, 13, 27));
    for (index, reaction) in [Reaction::Laugh, Reaction::Disapprove, Reaction::Comfort]
        .into_iter()
        .enumerate()
    {
        let reaction_rect = Rect {
            x: panel_x + 6 + i32::try_from(index).unwrap_or_default() * 50,
            y: 52,
            w: 48,
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
    let fitted = head_fit(message, 288 / glyph_width);
    add_panel_chrome(
        "status/background",
        Rect {
            x: 4,
            y: 130,
            w: 312,
            h: 22,
        },
        35,
        rects,
    );
    text.push(label("status/message", &fitted, 10, 136, 39));
}

fn visible_status(view: &ViewState, now_ms: u64) -> Option<&str> {
    view.status_message
        .as_deref()
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

fn add_pixel_grid(rects: &mut Vec<RectCommand>) {
    for x in 0..LOGICAL_WIDTH {
        rects.push(rect(
            &format!("debug/grid-x-{x}"),
            Rect {
                x,
                y: 0,
                w: 1,
                h: LOGICAL_HEIGHT,
            },
            [255, 0, 255, if x % 8 == 0 { 36 } else { 10 }],
            100,
        ));
    }
    for y in 0..LOGICAL_HEIGHT {
        rects.push(rect(
            &format!("debug/grid-y-{y}"),
            Rect {
                x: 0,
                y,
                w: LOGICAL_WIDTH,
                h: 1,
            },
            [0, 255, 255, if y % 8 == 0 { 36 } else { 10 }],
            100,
        ));
    }
}

/// Guaranteed hard-pixel expression rig used even when optional overlay art is absent.
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
    if let Some(action) = state.creature.aquarium.action {
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

fn behavior_name(state: &WorldState) -> &'static str {
    if let Some(action) = state.creature.aquarium.action {
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
        Intention::SeekComfort => "seeking comfort",
        Intention::ApproachPlayer => "watching you",
        Intention::RefuseAndStare => "staring",
        Intention::ShowAffection => "staying close",
        _ => match state.creature.aquarium.steering {
            SteeringMode::Drift => "drifting",
            SteeringMode::Flee => "avoiding you",
            SteeringMode::Orbit => "circling",
            SteeringMode::Inspect => "investigating",
            SteeringMode::Settle => "settling",
            _ => "hovering",
        },
    }
}

fn cue_for_event(event: &GameEvent) -> Option<(PresentationCueKind, u64)> {
    match event {
        GameEvent::FoodDropped { .. } => Some((PresentationCueKind::Notice, 700)),
        GameEvent::FoodConsumed(_) => Some((PresentationCueKind::Crumbs, 900)),
        GameEvent::FoodRejected(_) => Some((PresentationCueKind::Spit, 1_100)),
        GameEvent::ToyRejected(_) => Some((PresentationCueKind::Suspicion, 1_100)),
        GameEvent::FoodDropRejected(FoodDropRejectionReason::AquariumFull) => {
            Some((PresentationCueKind::AquariumFull, 1_300))
        }
        GameEvent::Comforted => Some((PresentationCueKind::Comfort, 1_200)),
        GameEvent::SleepStarted => Some((PresentationCueKind::Sleep, 1_000)),
        GameEvent::FoodSettled(_) => Some((PresentationCueKind::SandPuff, 700)),
        GameEvent::NonverbalAct(act) => Some((cue_for_nonverbal(*act), 1_100)),
        GameEvent::ActionPhaseChanged {
            to: ActionPhase::Approach,
            ..
        } => Some((PresentationCueKind::Wake, 650)),
        _ => None,
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

fn audio_for_cue(cue: PresentationCueKind) -> AudioCue {
    match cue {
        PresentationCueKind::Notice | PresentationCueKind::Wake => AudioCue::Wake,
        PresentationCueKind::Recoil
        | PresentationCueKind::Suspicion
        | PresentationCueKind::Spit => AudioCue::FoodReject,
        PresentationCueKind::AquariumFull => AudioCue::UiReject,
        PresentationCueKind::Delight | PresentationCueKind::Crumbs => AudioCue::FoodEat,
        PresentationCueKind::Affection | PresentationCueKind::Comfort => AudioCue::Affection,
        PresentationCueKind::SandPuff => AudioCue::Sand,
        PresentationCueKind::Sleep => AudioCue::Sleep,
    }
}

fn animation_frame(pose: &str, elapsed_ms: u64) -> u8 {
    let frame_ms = match pose {
        "swim" => 160,
        "turn" | "eat" | "play" => 240,
        "sleep" => 1_200,
        _ => 700,
    };
    u8::try_from((elapsed_ms / frame_ms) % 4).unwrap_or_default()
}

fn body_sprite(
    state: &WorldState,
    view: &ViewState,
    pose: &str,
    elapsed_ms: u64,
    side_flip: SpriteFlip,
) -> (String, SpriteFlip, u8) {
    let cue_timing = view.active_cue_timing(state.elapsed_ms);
    let cue = cue_timing.map(|(kind, _)| kind);
    let mood = visual_mood_name(state, cue);
    let faces_player = view.speaking
        || matches!(state.creature.aquarium.gaze, GazeTarget::Player)
        || matches!(
            cue,
            Some(
                PresentationCueKind::Delight
                    | PresentationCueKind::Affection
                    | PresentationCueKind::Comfort
            )
        );
    if let Some((asset, flip)) = cue
        .filter(|kind| !dialogue_active(view) || cue_preempts_dialogue(*kind))
        .and_then(|kind| reaction_body_asset(kind, side_flip))
    {
        let cue_elapsed_ms = if view.reduced_motion {
            0
        } else {
            cue_timing.map_or(0, |(_, elapsed_ms)| elapsed_ms)
        };
        return (
            asset.to_owned(),
            flip,
            u8::try_from((cue_elapsed_ms / 240).min(3)).unwrap_or_default(),
        );
    }
    if dialogue_active(view) {
        return (
            format!("creature-v1/talk/{mood}-south"),
            SpriteFlip::None,
            if view.speaking {
                view.mouth_phase.min(2)
            } else {
                u8::try_from((elapsed_ms / 160) % 3).unwrap_or_default()
            },
        );
    }
    if let Some(asset) = action_body_asset(pose) {
        return (
            asset.to_owned(),
            side_flip,
            animation_frame(pose, elapsed_ms),
        );
    }
    let direction = if faces_player { "south" } else { "east" };
    (
        format!("creature-v1/mood/{mood}-{direction}"),
        if faces_player {
            SpriteFlip::None
        } else {
            side_flip
        },
        animation_frame("hover", elapsed_ms),
    )
}

fn dialogue_active(view: &ViewState) -> bool {
    view.speaking || view.speech.is_some()
}

const fn cue_has_body_override(cue: PresentationCueKind) -> bool {
    matches!(
        cue,
        PresentationCueKind::Notice
            | PresentationCueKind::Recoil
            | PresentationCueKind::Delight
            | PresentationCueKind::Suspicion
            | PresentationCueKind::Affection
            | PresentationCueKind::Comfort
            | PresentationCueKind::Spit
            | PresentationCueKind::Crumbs
            | PresentationCueKind::AquariumFull
    )
}

const fn cue_preempts_dialogue(cue: PresentationCueKind) -> bool {
    matches!(
        cue,
        PresentationCueKind::Recoil
            | PresentationCueKind::Suspicion
            | PresentationCueKind::Affection
            | PresentationCueKind::Comfort
            | PresentationCueKind::Spit
            | PresentationCueKind::AquariumFull
    )
}

fn reaction_body_asset(
    cue: PresentationCueKind,
    side_flip: SpriteFlip,
) -> Option<(&'static str, SpriteFlip)> {
    match cue {
        PresentationCueKind::Notice => {
            Some(("creature-v1/reaction/notice-south", SpriteFlip::None))
        }
        PresentationCueKind::Recoil
        | PresentationCueKind::Spit
        | PresentationCueKind::AquariumFull => Some(("creature-v1/reject-food", side_flip)),
        PresentationCueKind::Suspicion => {
            Some(("creature-v1/reaction/toy-refusal-east", side_flip))
        }
        PresentationCueKind::Delight | PresentationCueKind::Affection => {
            Some(("creature-v1/reaction/affection-south", SpriteFlip::None))
        }
        PresentationCueKind::Comfort => {
            Some(("creature-v1/reaction/comfort-south", SpriteFlip::None))
        }
        PresentationCueKind::Crumbs => Some(("creature-v1/eat", side_flip)),
        PresentationCueKind::SandPuff | PresentationCueKind::Wake | PresentationCueKind::Sleep => {
            None
        }
    }
}

fn action_body_asset(pose: &str) -> Option<&'static str> {
    match pose {
        "swim" | "turn" => Some("creature-v1/swim"),
        "eat" => Some("creature-v1/eat"),
        "sleep" => Some("creature-v1/sleep"),
        "play" => Some("creature-v1/play"),
        "hover" | "inspect" | "react" | "recover" | "settle" => None,
        _ => None,
    }
}

fn visual_mood_name(state: &WorldState, cue: Option<PresentationCueKind>) -> &'static str {
    match cue {
        Some(PresentationCueKind::Recoil)
        | Some(PresentationCueKind::Spit)
        | Some(PresentationCueKind::AquariumFull) => "resentful",
        Some(PresentationCueKind::Suspicion) | Some(PresentationCueKind::Notice) => "curious",
        Some(PresentationCueKind::Sleep) => "sleepy",
        Some(PresentationCueKind::Delight)
        | Some(PresentationCueKind::Affection)
        | Some(PresentationCueKind::Comfort)
        | Some(PresentationCueKind::Crumbs)
        | Some(PresentationCueKind::SandPuff)
        | Some(PresentationCueKind::Wake) => "content",
        None => mood_name(state.mood()),
    }
}

fn bob_offset(state: &WorldState) -> i32 {
    const BOB: [i32; 8] = [0, -1, -1, -2, -1, -1, 0, 0];
    let index = usize::try_from((state.elapsed_ms / 320 + state.seed % 8) % 8).unwrap_or_default();
    BOB[index]
}

fn phase_color(phase: ActionPhase) -> [u8; 4] {
    match phase {
        ActionPhase::Notice | ActionPhase::Gaze => [244, 219, 128, 210],
        ActionPhase::Brake | ActionPhase::Turn | ActionPhase::Approach => [119, 209, 205, 210],
        ActionPhase::Inspect => [196, 158, 214, 210],
        ActionPhase::Act => [242, 137, 107, 230],
        ActionPhase::Recover => [151, 206, 153, 190],
    }
}

fn framed_sprite(id: &str, x: i32, y: i32, layer: i16, frame: u8) -> SpriteCommand {
    SpriteCommand {
        id: id.to_owned(),
        x,
        y,
        layer,
        frame,
        flip: SpriteFlip::None,
        source_rect: None,
        scale: 1,
    }
}

fn effect_sprite(
    cue: PresentationCueKind,
    body_x: i32,
    body_y: i32,
    flip: SpriteFlip,
    frame: u8,
) -> SpriteCommand {
    let left = matches!(flip, SpriteFlip::Horizontal);
    let (id, x, y) = match cue {
        PresentationCueKind::Notice
        | PresentationCueKind::Suspicion
        | PresentationCueKind::Recoil
        | PresentationCueKind::AquariumFull => (
            "creature-v1/effect/attention",
            body_x + if left { 10 } else { 116 },
            body_y + 8,
        ),
        PresentationCueKind::Delight
        | PresentationCueKind::Affection
        | PresentationCueKind::Comfort => {
            ("creature-v1/effect/affection", body_x + 108, body_y + 4)
        }
        PresentationCueKind::Spit | PresentationCueKind::Crumbs => (
            "creature-v1/effect/mouth-particles",
            body_x + if left { 8 } else { 120 },
            body_y + 66,
        ),
        PresentationCueKind::SandPuff => ("aquarium/sand-puff", body_x + 64, body_y + 112),
        PresentationCueKind::Wake => (
            "aquarium/wake",
            body_x + if left { 112 } else { 4 },
            body_y + 64,
        ),
        PresentationCueKind::Sleep => ("creature-v1/effect/sleep", body_x + 108, body_y + 2),
    };
    framed_sprite(id, x, y, 15, frame)
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

fn ui_sprite(id: &str, x: i32, y: i32, layer: i16) -> SpriteCommand {
    SpriteCommand {
        id: id.to_owned(),
        x,
        y,
        layer,
        frame: 0,
        flip: SpriteFlip::None,
        source_rect: None,
        scale: 1,
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
        layer,
    ));
    rects.push(rect(&format!("{id}-edge"), dimensions, UI_EDGE, layer + 1));
    rects.push(rect(
        id,
        Rect {
            x: dimensions.x + 1,
            y: dimensions.y + 1,
            w: dimensions.w - 2,
            h: dimensions.h - 2,
        },
        UI_PANEL,
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
        UI_EDGE_LIT,
        layer + 3,
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
        if active { UI_CORAL } else { UI_EDGE },
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
        if active {
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

fn food_asset(food: FoodId) -> &'static str {
    match food {
        FoodId::Berry => "food/berry",
        FoodId::Mushroom => "food/mushroom",
        FoodId::Pellet => "food/pellet",
    }
}

fn toy_sheet_x(toy: ToyId) -> i32 {
    match toy {
        ToyId::Ball => 0,
        ToyId::Bell => 32,
        ToyId::Sock => 64,
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
        UiAction::Comfort => "comfort".to_owned(),
        UiAction::Inspect => "inspect".to_owned(),
        UiAction::Rename => "rename".to_owned(),
        UiAction::Talk => "talk".to_owned(),
        _ => "action".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use beastie_core::{
        ActionTimeline, FoodBuoyancy, FoodObject, NormalizedVelocity, SemanticDestination,
    };

    #[test]
    fn coordinate_projection_rounds_to_whole_pixels_and_clamps() {
        assert_eq!(world_to_logical(NormalizedPosition::new(0, 0)), (4, 4));
        assert_eq!(
            world_to_logical(NormalizedPosition::new(10_000, 10_000)),
            (315, 125)
        );
        assert_eq!(
            world_to_logical(NormalizedPosition::new(-1, 20_000)),
            (4, 125)
        );
        let center = logical_to_world(160, 71);
        let projected = world_to_logical(center);
        assert!((projected.0 - 160).abs() <= 1);
        assert!((projected.1 - 71).abs() <= 1);
    }

    #[test]
    fn default_plan_is_aquarium_only_and_compose_is_persistent() {
        let state = WorldState::new(7, "Mop");
        let (render, audio) = plan(&state, &ViewState::default());
        assert!(
            render
                .sprites
                .iter()
                .any(|sprite| sprite.id == "aquarium/background")
        );
        assert!(
            render
                .sprites
                .iter()
                .any(|sprite| sprite.id.starts_with("creature-v1/mood/") && sprite.scale == 2)
        );
        let background = render
            .sprites
            .iter()
            .find(|sprite| sprite.id == "aquarium/background")
            .expect("background sprite");
        assert_eq!(
            background.source_rect,
            Some(Rect {
                x: 0,
                y: 25,
                w: 320,
                h: 130
            })
        );
        assert_eq!(background.scale, 1);
        assert!(
            render
                .sprites
                .iter()
                .all(|sprite| !sprite.id.starts_with("room/"))
        );
        assert!(
            render
                .hit_regions
                .iter()
                .any(|hit| hit.id == "compose/input")
        );
        assert_eq!(audio.ambience, vec![AudioCue::AquariumHum]);
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
    fn objects_use_authoritative_continuous_positions() {
        let mut state = WorldState::new(7, "Mop");
        state.aquarium.objects.insert(
            9,
            WorldObject::Food(FoodObject {
                id: 9,
                food: FoodId::Mushroom,
                position: NormalizedPosition::new(2_500, 7_500),
                velocity: NormalizedVelocity::default(),
                buoyancy: FoodBuoyancy::Sink,
                disposition: FoodDisposition::Falling,
                age_ms: 0,
                lifetime_ms: 10_000,
            }),
        );
        let (render, _) = plan(&state, &ViewState::default());
        let sprite = render
            .sprites
            .iter()
            .find(|sprite| sprite.id == "food/mushroom")
            .expect("food sprite");
        let expected = world_to_logical(NormalizedPosition::new(2_500, 7_500));
        assert_eq!((sprite.x, sprite.y), (expected.0 - 8, expected.1 - 8));
        assert!(
            render
                .hit_regions
                .iter()
                .any(|hit| hit.id == "target/object-9")
        );
    }

    #[test]
    fn action_phases_are_legible_in_pose_behavior_and_progress() {
        let mut state = WorldState::new(7, "Mop");
        state.creature.aquarium.action = Some(ActionTimeline {
            phase: ActionPhase::Inspect,
            elapsed_ms: 500,
            phase_duration_ms: 1_000,
            destination: SemanticDestination::Food(1),
            food_id: Some(1),
        });
        let (render, _) = plan(&state, &ViewState::default());
        assert!(
            render
                .sprites
                .iter()
                .any(|sprite| sprite.id.starts_with("creature-v1/mood/"))
        );
        assert_eq!(render.summary.behavior, "inspecting");
        assert_eq!(
            render
                .rects
                .iter()
                .find(|rect| rect.id == "creature/action-phase")
                .expect("phase indicator")
                .rect
                .w,
            12
        );
    }

    #[test]
    fn cue_queue_preserves_order_and_does_not_overwrite() {
        let mut view = ViewState::default();
        view.enqueue_cue(PresentationCueKind::Crumbs, 900, 1_000);
        view.enqueue_cue(PresentationCueKind::Sleep, 1_000, 1_000);
        assert_eq!(view.active_cue(1_000), Some(PresentationCueKind::Crumbs));
        assert_eq!(view.active_cue(1_899), Some(PresentationCueKind::Crumbs));
        assert_eq!(view.active_cue(1_900), Some(PresentationCueKind::Sleep));
        view.expire(3_100);
        assert!(view.cue_queue.is_empty());
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
    fn cue_queue_is_bounded() {
        let mut view = ViewState::default();
        for index in 0..20 {
            let cue = if index % 2 == 0 {
                PresentationCueKind::Notice
            } else {
                PresentationCueKind::Wake
            };
            view.enqueue_cue(cue, 100, 0);
        }
        assert_eq!(view.cue_queue.len(), CUE_QUEUE_LIMIT);
    }

    #[test]
    fn toy_rejection_coalesces_to_one_immediate_toy_refusal() {
        let state = WorldState::new(7, "Mop");
        let mut view = ViewState::default();
        view.enqueue_cue(PresentationCueKind::Wake, 2_000, state.elapsed_ms);
        view.observe_events(
            &[
                GameEvent::ToyRejected(ToyId::Sock),
                GameEvent::NonverbalAct(NonverbalAct::TakeToyAway(ToyId::Sock)),
            ],
            state.elapsed_ms,
        );
        assert_eq!(view.cue_queue.len(), 1);
        assert_eq!(
            view.active_cue(state.elapsed_ms),
            Some(PresentationCueKind::Suspicion)
        );
        let body = plan(&state, &view)
            .0
            .sprites
            .into_iter()
            .find(|command| command.layer == 12)
            .expect("toy refusal body");
        assert_eq!(body.id, "creature-v1/reaction/toy-refusal-east");
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
    fn hover_and_controller_focus_use_the_same_stable_region() {
        let state = WorldState::new(7, "Mop");
        let view = ViewState {
            hovered_region: Some("target/creature".to_owned()),
            focused_region: Some("target/creature".to_owned()),
            ..ViewState::default()
        };
        let (render, _) = plan(&state, &view);
        assert!(render.rects.iter().all(|rect| rect.id != "ui/hover"));
        assert!(render.rects.iter().any(|rect| rect.id == "ui/focus"));
        assert!(
            render
                .text
                .iter()
                .any(|text| text.id == "ui/hover-label" && text.text == "Mop")
        );
    }

    #[test]
    fn pixel_grid_is_development_only_view_state() {
        let state = WorldState::new(7, "Mop");
        let off = plan(&state, &ViewState::default()).0;
        assert!(
            !off.rects
                .iter()
                .any(|rect| rect.id.starts_with("debug/grid"))
        );
        let on = plan(
            &state,
            &ViewState {
                pixel_grid: true,
                ..ViewState::default()
            },
        )
        .0;
        assert_eq!(
            on.rects
                .iter()
                .filter(|rect| rect.id.starts_with("debug/grid"))
                .count(),
            usize::try_from(LOGICAL_WIDTH + LOGICAL_HEIGHT).unwrap()
        );
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
    fn every_mood_uses_distinct_full_body_sprite_without_procedural_face() {
        let mut signatures = BTreeSet::new();
        for mood in [
            Mood::Content,
            Mood::Curious,
            Mood::Hungry,
            Mood::Sleepy,
            Mood::Lonely,
            Mood::Resentful,
        ] {
            let mut state = WorldState::new(7, "Mop");
            state.creature.needs.hunger = 0.2;
            state.creature.needs.energy = 0.8;
            state.creature.needs.curiosity = 0.2;
            state.creature.relationship.resentment = 0.0;
            state.creature.relationship.bond = 0.5;
            state.aquarium.player_present = true;
            match mood {
                Mood::Content => {}
                Mood::Curious => state.creature.needs.curiosity = 0.9,
                Mood::Hungry => state.creature.needs.hunger = 0.9,
                Mood::Sleepy => state.creature.needs.energy = 0.1,
                Mood::Lonely => state.aquarium.player_present = false,
                Mood::Resentful => state.creature.relationship.resentment = 0.8,
            }
            assert_eq!(state.mood(), mood);
            let render = plan(&state, &ViewState::default()).0;
            let body = render
                .sprites
                .iter()
                .find(|command| command.layer == 12)
                .expect("full-body mood sprite");
            assert!(body.id.starts_with("creature-v1/mood/"));
            assert!(
                !render
                    .rects
                    .iter()
                    .any(|command| command.id.starts_with("face/")),
                "shipped sprite art must not be covered by procedural facial geometry"
            );
            signatures.insert(body.id.clone());
        }
        assert_eq!(signatures.len(), 6);
    }

    #[test]
    fn player_attention_turns_the_full_body_toward_the_viewer() {
        let mut state = WorldState::new(7, "Mop");
        state.elapsed_ms = 1_000;
        state.creature.aquarium.gaze = GazeTarget::Cursor;
        let side = plan(&state, &ViewState::default())
            .0
            .sprites
            .into_iter()
            .find(|command| command.layer == 12)
            .expect("side body");
        state.creature.aquarium.gaze = GazeTarget::Player;
        let front = plan(&state, &ViewState::default())
            .0
            .sprites
            .into_iter()
            .find(|command| command.layer == 12)
            .expect("front body");
        assert!(side.id.ends_with("-east"));
        assert!(front.id.ends_with("-south"));
        assert_eq!(front.flip, SpriteFlip::None);
    }

    #[test]
    fn affection_overrides_action_pose_and_keeps_the_close_up_visible() {
        let mut state = WorldState::new(7, "Mop");
        state.creature.aquarium.position = NormalizedPosition::new(10_000, 10_000);
        state.creature.aquarium.action = Some(ActionTimeline {
            phase: ActionPhase::Act,
            elapsed_ms: 400,
            phase_duration_ms: 1_000,
            destination: SemanticDestination::Player,
            food_id: None,
        });
        let mut view = ViewState::default();
        view.enqueue_cue(PresentationCueKind::Affection, 1_000, state.elapsed_ms);
        let body = plan(&state, &view)
            .0
            .sprites
            .into_iter()
            .find(|command| command.layer == 12)
            .expect("affection body");
        assert_eq!(body.id, "creature-v1/reaction/affection-south");
        assert!((-16..=184).contains(&body.x));
        assert!((-20..=-8).contains(&body.y));
    }

    #[test]
    fn authored_reactions_start_at_the_first_frame_and_hold_the_last() {
        let cases = [
            (
                PresentationCueKind::Notice,
                "creature-v1/reaction/notice-south",
            ),
            (PresentationCueKind::Spit, "creature-v1/reject-food"),
            (
                PresentationCueKind::Suspicion,
                "creature-v1/reaction/toy-refusal-east",
            ),
            (
                PresentationCueKind::Comfort,
                "creature-v1/reaction/comfort-south",
            ),
            (PresentationCueKind::Crumbs, "creature-v1/eat"),
        ];
        for (cue, expected_id) in cases {
            let mut state = WorldState::new(7, "Mop");
            state.elapsed_ms = 10_000;
            let mut view = ViewState::default();
            view.enqueue_cue(cue, 2_000, state.elapsed_ms);
            let first = plan(&state, &view)
                .0
                .sprites
                .into_iter()
                .find(|command| command.layer == 12)
                .expect("reaction body");
            assert_eq!(first.id, expected_id);
            assert_eq!(first.frame, 0);

            state.elapsed_ms += 1_500;
            let held = plan(&state, &view)
                .0
                .sprites
                .into_iter()
                .find(|command| command.layer == 12)
                .expect("held reaction body");
            assert_eq!(held.id, expected_id);
            assert_eq!(held.frame, 3);
        }
    }

    #[test]
    fn dialogue_replaces_stale_punctuation_but_not_direct_reactions() {
        let state = WorldState::new(7, "Mop");
        let dialogue_body_for = |cue| {
            let mut view = ViewState::default();
            view.show_speech("berry again".to_owned(), state.elapsed_ms);
            view.enqueue_cue(cue, 2_000, state.elapsed_ms);
            plan(&state, &view)
                .0
                .sprites
                .into_iter()
                .find(|command| command.layer == 12)
                .expect("dialogue body")
                .id
        };
        assert!(dialogue_body_for(PresentationCueKind::Crumbs).starts_with("creature-v1/talk/"));
        assert_eq!(
            dialogue_body_for(PresentationCueKind::Spit),
            "creature-v1/reject-food"
        );
        assert_eq!(
            dialogue_body_for(PresentationCueKind::Comfort),
            "creature-v1/reaction/comfort-south"
        );
    }

    #[test]
    fn authored_reaction_body_stays_legible_at_aquarium_edges() {
        let mut state = WorldState::new(7, "Mop");
        state.creature.aquarium.position = NormalizedPosition::new(10_000, 0);
        let mut view = ViewState::default();
        view.enqueue_cue(PresentationCueKind::Crumbs, 1_000, state.elapsed_ms);
        let body = plan(&state, &view)
            .0
            .sprites
            .into_iter()
            .find(|command| command.layer == 12)
            .expect("edge reaction body");
        assert_eq!(body.id, "creature-v1/eat");
        assert!((-16..=184).contains(&body.x));
        assert!((-20..=-8).contains(&body.y));
    }

    #[test]
    fn key_cues_use_authored_sprite_effects() {
        let state = WorldState::new(7, "Mop");
        for cue in [
            PresentationCueKind::Notice,
            PresentationCueKind::Affection,
            PresentationCueKind::Spit,
            PresentationCueKind::SandPuff,
            PresentationCueKind::Wake,
            PresentationCueKind::Sleep,
        ] {
            let mut view = ViewState::default();
            view.enqueue_cue(cue, 1_000, state.elapsed_ms);
            let effects = plan(&state, &view)
                .0
                .sprites
                .into_iter()
                .filter(|command| {
                    command.id.starts_with("creature-v1/effect/")
                        || matches!(command.id.as_str(), "aquarium/wake" | "aquarium/sand-puff")
                })
                .collect::<Vec<_>>();
            assert_eq!(effects.len(), 1, "{cue:?} needs one authored effect sprite");
        }
    }

    #[test]
    fn visible_settings_surface_exposes_every_accessibility_control() {
        let state = WorldState::new(7, "Mop");
        let view = ViewState {
            mode: UiMode::Settings,
            ..ViewState::default()
        };
        let plan = plan(&state, &view).0;
        for id in [
            "settings/text-scale",
            "settings/motion",
            "settings/flashes",
            "settings/shake",
            "settings/grid",
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
                    kind: PresentationCueKind::Spit,
                    starts_at_ms: 0,
                    expires_at_ms: 2_000,
                }],
                ..ViewState::default()
            },
        )
        .0;
        assert!(accessible.text.iter().all(|text| text.scale == 2));
        let standard_particle = standard
            .rects
            .iter()
            .find(|rect| rect.id == "aquarium/particle-1")
            .expect("standard particle");
        let reduced_particle = accessible
            .rects
            .iter()
            .find(|rect| rect.id == "aquarium/particle-1")
            .expect("reduced particle");
        assert_ne!(standard_particle.rect, reduced_particle.rect);
        assert!(
            accessible
                .rects
                .iter()
                .filter(|rect| rect.id.starts_with("effect/"))
                .all(|rect| rect.color[3] <= 80)
        );
    }

    #[test]
    fn body_pose_maps_only_to_shipped_animation_sets() {
        assert_eq!(action_body_asset("hover"), None);
        assert_eq!(action_body_asset("settle"), None);
        assert_eq!(action_body_asset("swim"), Some("creature-v1/swim"));
        assert_eq!(action_body_asset("turn"), Some("creature-v1/swim"));
        assert_eq!(action_body_asset("eat"), Some("creature-v1/eat"));
        assert_eq!(action_body_asset("react"), None);
        assert_eq!(action_body_asset("recover"), None);
        assert_eq!(action_body_asset("sleep"), Some("creature-v1/sleep"));
        assert_eq!(action_body_asset("play"), Some("creature-v1/play"));
        assert_eq!(action_body_asset("unknown"), None);
    }

    #[test]
    fn environment_props_come_only_from_authoritative_objects() {
        let state = WorldState::new(7, "Mop");
        let render = plan(&state, &ViewState::default()).0;
        let expected_caves = state
            .aquarium
            .objects
            .values()
            .filter(|object| matches!(object, WorldObject::Cave { .. }))
            .count();
        let expected_plants = state
            .aquarium
            .objects
            .values()
            .filter(|object| matches!(object, WorldObject::Plant { .. }))
            .count();
        let expected_toys = state
            .aquarium
            .objects
            .values()
            .filter(|object| matches!(object, WorldObject::Toy { .. }))
            .count();
        assert_eq!(
            render
                .sprites
                .iter()
                .filter(|sprite| sprite.id == "aquarium/cave")
                .count(),
            expected_caves
        );
        assert_eq!(
            render
                .sprites
                .iter()
                .filter(|sprite| sprite.id == "aquarium/plants")
                .count(),
            expected_plants
        );
        assert_eq!(
            render
                .sprites
                .iter()
                .filter(|sprite| sprite.id == "aquarium/toys")
                .count(),
            expected_toys
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
        assert!(audio.events.contains(&AudioCue::UiReject));
        assert!(
            render
                .text
                .iter()
                .any(|text| { text.id == "status/message" && text.text.contains("Aquarium full") })
        );
    }

    #[test]
    fn controls_are_explained_on_focus_instead_of_permanent_prose() {
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
                .any(|text| text.id == "ui/hover-label" && text.text == "Food")
        );
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
                assert!(text_right(input) <= input_box.x + input_box.w - 5);
                for action in ["food", "settings", "send"] {
                    let icon = render
                        .sprites
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
                    assert!(icon.x + 19 <= background.x + background.w);
                    assert!(!rects_overlap(input_box, background));
                }
            }
        }
    }

    #[test]
    fn ui_depth_bands_and_modal_hit_regions_are_unambiguous() {
        let state = WorldState::new(7, "Mop");
        for mode in [
            UiMode::FoodChoice,
            UiMode::ToyChoice,
            UiMode::Settings,
            UiMode::Bindings,
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
                .expect("modal chrome");
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
    fn speech_chooses_the_side_opposite_the_creature() {
        for (creature_x, expect_panel_left) in [(8_500, true), (5_000, true), (1_500, false)] {
            let mut state = WorldState::new(7, "Mop");
            state.creature.aquarium.position = NormalizedPosition::new(creature_x, 4_000);
            let render = plan(
                &state,
                &ViewState {
                    speech: Some("berry remains bad".to_owned()),
                    ..ViewState::default()
                },
            )
            .0;
            let panel = render
                .rects
                .iter()
                .find(|command| command.id == "speech/panel")
                .expect("speech panel")
                .rect;
            let body = render
                .sprites
                .iter()
                .find(|command| command.layer == 12)
                .expect("creature body");
            if expect_panel_left {
                assert!(panel.x + panel.w <= body.x);
            } else {
                assert!(body.x + CREATURE_CANVAS_SIZE <= panel.x);
            }
            let reactions = render
                .hit_regions
                .iter()
                .filter(|hit| matches!(hit.target, Some(UiTarget::Reaction(_))))
                .collect::<Vec<_>>();
            assert_eq!(reactions.len(), 3);
            for pair in reactions.windows(2) {
                assert!(!rects_overlap(pair[0].rect, pair[1].rect));
            }
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
            assert!(text_right(message) <= status.x + status.w - 6);
            assert!(
                render.text.iter().all(|command| {
                    command.id != "compose/summary-name"
                        && command.id != "compose/summary-behavior"
                        && command.id != "compose/input-hints"
                }),
                "status replaces, rather than overlaps, the summary row"
            );
        }
    }

    fn text_right(command: &TextCommand) -> i32 {
        command.x
            + i32::try_from(command.text.chars().count()).unwrap_or(i32::MAX)
                * 6
                * i32::from(command.scale)
    }

    fn rects_overlap(left: Rect, right: Rect) -> bool {
        left.x < right.x + right.w
            && left.x + left.w > right.x
            && left.y < right.y + right.h
            && left.y + left.h > right.y
    }

    #[test]
    fn protocol_mouth_phase_selects_full_body_talking_frames() {
        let mut state = WorldState::new(7, "Mop");
        state.elapsed_ms = 1_000;
        let mouth = |phase| {
            plan(
                &state,
                &ViewState {
                    speaking: true,
                    mouth_phase: phase,
                    ..ViewState::default()
                },
            )
            .0
            .sprites
            .into_iter()
            .find(|sprite| sprite.id.starts_with("creature-v1/talk/"))
            .expect("full-body talking sprite")
        };
        assert_eq!(mouth(0).frame, 0);
        assert_eq!(mouth(1).frame, 1);
        assert_eq!(mouth(2).frame, 2);
        assert!(mouth(0).id.ends_with("-south"));
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
}
