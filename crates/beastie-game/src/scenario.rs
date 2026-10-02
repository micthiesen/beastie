use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use beastie_session::{CommandEnvelope, GameSession};
use beastie_view::{MicrophoneState, UiAction, ViewState};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug)]
pub enum ScenarioStep {
    Session(CommandEnvelope),
    Ui(UiAction),
    ControllerUi(UiAction),
    TextInput(String),
    /// Static presentation coverage only, never evidence of perception or canonical state.
    UiPreview(UiPreview),
    /// Shell-only result of attempting to acquire a bounded microphone capture.
    ///
    /// This is intentionally separate from `SpeechStarted`: an unavailable device never reaches
    /// the simulation, while an acquired capture may legitimately produce perception.
    MicrophoneAcquisition(MicrophoneAcquisition),
    SetSubtitles(bool),
    Capture(String),
    Marker(String),
    /// One synthetic 60 Hz frame of a `wait` control.
    WaitTick {
        milliseconds: u64,
    },
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiPreview {
    #[serde(default)]
    pub clear: bool,
    pub focus: Option<String>,
    pub hover: Option<String>,
    pub pressed: Option<String>,
    pub selected: Option<bool>,
    pub microphone: Option<MicrophoneState>,
    pub pending: Option<bool>,
    pub caption: Option<String>,
    pub status: Option<String>,
}

impl UiPreview {
    fn validate(&self) -> Result<(), ScenarioError> {
        for id in [&self.focus, &self.hover, &self.pressed]
            .into_iter()
            .flatten()
        {
            if id.is_empty()
                || id.len() > 96
                || !id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_'))
            {
                return Err(ScenarioError::Command(
                    "invalid UI preview region".to_owned(),
                ));
            }
        }
        for text in [&self.caption, &self.status].into_iter().flatten() {
            validate_text(text)?;
        }
        Ok(())
    }

    pub fn apply(self, view: &mut ViewState, now_ms: u64) {
        if self.clear {
            view.hovered_region = None;
            view.focused_region = None;
            view.pressed_region = None;
            view.pressed_until_ms = 0;
            view.text_selected = false;
            view.clear_status();
            view.clear_speech();
            view.pending = false;
        }
        if let Some(region) = self.focus {
            view.focused_region = Some(region);
        }
        if let Some(region) = self.hover {
            view.hovered_region = Some(region);
        }
        if let Some(region) = self.pressed {
            view.pressed_region = Some(region);
            view.pressed_until_ms = now_ms.saturating_add(100);
        }
        if let Some(selected) = self.selected {
            view.text_selected = selected && !view.text_buffer.is_empty();
        }
        if let Some(state) = self.microphone {
            view.microphone_state = state;
            view.microphone_enabled = state != MicrophoneState::Disabled;
        }
        if let Some(pending) = self.pending {
            view.pending = pending;
        }
        if let Some(caption) = self.caption {
            view.show_speech(caption, now_ms);
        }
        if let Some(status) = self.status {
            view.show_status(status, now_ms, 12_000);
        }
    }
}

fn validate_text(text: &str) -> Result<(), ScenarioError> {
    if text.chars().count() > crate::input::MAX_TALK_CHARACTERS
        || text.chars().any(char::is_control)
    {
        return Err(ScenarioError::Command(
            "UI text must contain at most 512 printable characters".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicrophoneAcquisition {
    Acquired,
    Unavailable,
}

#[derive(Debug)]
pub struct ScenarioRunner {
    steps: VecDeque<ScenarioStep>,
    pub capture_dir: PathBuf,
    wait: Option<WaitState>,
}

#[derive(Debug)]
struct WaitState {
    requested_ms: u64,
    elapsed_ms: u64,
    frame: u64,
}

const MAX_WAIT_MS: u64 = 15 * 60_000;

impl ScenarioRunner {
    pub fn load(path: &Path, capture_dir: PathBuf) -> Result<Self, ScenarioError> {
        let source = fs::read_to_string(path).map_err(ScenarioError::Io)?;
        let mut steps = VecDeque::new();
        for (index, line) in source.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            steps.push_back(parse_step(line).map_err(|source| ScenarioError::Line {
                line: index + 1,
                source: Box::new(source),
            })?);
        }
        if steps.is_empty() {
            return Err(ScenarioError::Empty);
        }
        Ok(Self {
            steps,
            capture_dir,
            wait: None,
        })
    }

    pub fn next(&mut self) -> Option<ScenarioStep> {
        if let Some(tick) = self.next_wait_tick() {
            return Some(tick);
        }
        loop {
            match self.steps.pop_front()? {
                ScenarioStep::WaitTick { milliseconds } => {
                    self.wait = Some(WaitState {
                        requested_ms: milliseconds,
                        elapsed_ms: 0,
                        frame: 0,
                    });
                    if let Some(tick) = self.next_wait_tick() {
                        return Some(tick);
                    }
                }
                step => return Some(step),
            }
        }
    }

    fn next_wait_tick(&mut self) -> Option<ScenarioStep> {
        let wait = self.wait.as_mut()?;
        let remaining = wait.requested_ms.saturating_sub(wait.elapsed_ms);
        if remaining == 0 {
            self.wait = None;
            return None;
        }
        // Bresenham-style frame boundaries retain an exact 1,000 ms per 60 frames.
        // A non-frame-aligned requested duration has one bounded final partial frame.
        let next_boundary = wait.frame.saturating_add(1).saturating_mul(1_000) / 60;
        let previous_boundary = wait.frame.saturating_mul(1_000) / 60;
        let cadence_ms = next_boundary.saturating_sub(previous_boundary);
        let milliseconds = cadence_ms.min(remaining);
        wait.frame = wait.frame.saturating_add(1);
        wait.elapsed_ms = wait.elapsed_ms.saturating_add(milliseconds);
        Some(ScenarioStep::WaitTick { milliseconds })
    }
}

fn parse_step(line: &str) -> Result<ScenarioStep, ScenarioError> {
    #[derive(Deserialize)]
    struct CommandKind {
        command: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Capture {
        version: u32,
        command: String,
        name: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wait {
        version: u32,
        command: String,
        milliseconds: u64,
    }

    // A named wait keeps final-review intent explicit in JSONL while remaining a normal
    // frame-producing wait in the game shell. The shell can therefore hold an inspect state
    // without inventing a UI meter or a second timing mechanism.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct InspectHold {
        version: u32,
        command: String,
        milliseconds: u64,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Marker {
        version: u32,
        command: String,
        name: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SetSubtitles {
        version: u32,
        command: String,
        enabled: bool,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum MicrophoneAcquisitionOutcome {
        Acquired,
        Unavailable,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct MicrophoneAcquisitionControl {
        version: u32,
        command: String,
        outcome: MicrophoneAcquisitionOutcome,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum ScenarioUiAction {
        Title,
        Continue,
        SelectBall,
        OpenFood,
        OpenToys,
        OpenSettings,
        SettingsSound,
        SettingsControls,
        LargeText,
        NormalText,
        CycleWindowScale,
        ToggleFullscreen,
        Rename,
        Talk,
        SelectBerry,
        ReducedMotion,
        CreatureActions,
        OpenBindings,
        OpenData,
        RequestReset,
        Close,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct UiControl {
        version: u32,
        command: String,
        action: ScenarioUiAction,
        #[serde(default)]
        controller: bool,
    }

    let kind: CommandKind = serde_json::from_str(line).map_err(ScenarioError::Json)?;
    if kind.command == "ui_action" {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ActionControl {
            version: u32,
            command: String,
            action: UiAction,
            #[serde(default)]
            controller: bool,
        }
        let control: ActionControl = serde_json::from_str(line).map_err(ScenarioError::Json)?;
        validate_control(control.version, &control.command, "ui_action")?;
        return Ok(if control.controller {
            ScenarioStep::ControllerUi(control.action)
        } else {
            ScenarioStep::Ui(control.action)
        });
    }
    if kind.command == "text_input" {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct TextControl {
            version: u32,
            command: String,
            text: String,
        }
        let control: TextControl = serde_json::from_str(line).map_err(ScenarioError::Json)?;
        validate_control(control.version, &control.command, "text_input")?;
        validate_text(&control.text)?;
        return Ok(ScenarioStep::TextInput(control.text));
    }
    if kind.command == "ui_preview" {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PreviewControl {
            version: u32,
            command: String,
            preview: UiPreview,
        }
        let control: PreviewControl = serde_json::from_str(line).map_err(ScenarioError::Json)?;
        validate_control(control.version, &control.command, "ui_preview")?;
        control.preview.validate()?;
        return Ok(ScenarioStep::UiPreview(control.preview));
    }
    if kind.command == "ui" {
        let control: UiControl = serde_json::from_str(line).map_err(ScenarioError::Json)?;
        if control.version != beastie_session::SESSION_PROTOCOL_VERSION || control.command != "ui" {
            return Err(ScenarioError::Command(
                "unsupported UI control version".to_owned(),
            ));
        }
        let action = match control.action {
            ScenarioUiAction::Title => UiAction::OpenTitle,
            ScenarioUiAction::Continue => UiAction::Continue,
            ScenarioUiAction::SelectBall => {
                UiAction::OpenContext(beastie_view::UiTarget::Toy(beastie_core::ToyId::Ball))
            }
            ScenarioUiAction::OpenFood => UiAction::OpenFoodChoice,
            ScenarioUiAction::OpenToys => UiAction::OpenToyChoice,
            ScenarioUiAction::OpenSettings => UiAction::SelectSettingsPage(0),
            ScenarioUiAction::SettingsSound => UiAction::SelectSettingsPage(1),
            ScenarioUiAction::SettingsControls => UiAction::SelectSettingsPage(2),
            ScenarioUiAction::LargeText => UiAction::SetTextScale(2),
            ScenarioUiAction::NormalText => UiAction::SetTextScale(1),
            ScenarioUiAction::CycleWindowScale => UiAction::CycleWindowScale,
            ScenarioUiAction::ToggleFullscreen => UiAction::ToggleFullscreen,
            ScenarioUiAction::Rename => UiAction::Rename,
            ScenarioUiAction::Talk => UiAction::Talk,
            ScenarioUiAction::SelectBerry => UiAction::SelectFood(beastie_core::FoodId::Berry),
            ScenarioUiAction::ReducedMotion => UiAction::ToggleReducedMotion,
            ScenarioUiAction::CreatureActions => {
                UiAction::OpenContext(beastie_view::UiTarget::Creature)
            }
            ScenarioUiAction::OpenBindings => UiAction::OpenBindings,
            ScenarioUiAction::OpenData => UiAction::OpenDataManagement,
            ScenarioUiAction::RequestReset => UiAction::RequestReset,
            ScenarioUiAction::Close => UiAction::CancelMode,
        };
        return Ok(if control.controller {
            ScenarioStep::ControllerUi(action)
        } else {
            ScenarioStep::Ui(action)
        });
    }
    if kind.command == "set_subtitles" {
        let setting: SetSubtitles = serde_json::from_str(line).map_err(ScenarioError::Json)?;
        if setting.version != beastie_session::SESSION_PROTOCOL_VERSION
            || setting.command != "set_subtitles"
        {
            return Err(ScenarioError::Command(
                "unsupported subtitle control version".to_owned(),
            ));
        }
        return Ok(ScenarioStep::SetSubtitles(setting.enabled));
    }
    if kind.command == "microphone_acquisition" {
        let control: MicrophoneAcquisitionControl =
            serde_json::from_str(line).map_err(ScenarioError::Json)?;
        if control.version != beastie_session::SESSION_PROTOCOL_VERSION
            || control.command != "microphone_acquisition"
        {
            return Err(ScenarioError::Command(
                "unsupported microphone acquisition control version".to_owned(),
            ));
        }
        let outcome = match control.outcome {
            MicrophoneAcquisitionOutcome::Acquired => MicrophoneAcquisition::Acquired,
            MicrophoneAcquisitionOutcome::Unavailable => MicrophoneAcquisition::Unavailable,
        };
        return Ok(ScenarioStep::MicrophoneAcquisition(outcome));
    }
    if kind.command == "wait" {
        let wait: Wait = serde_json::from_str(line).map_err(ScenarioError::Json)?;
        if wait.version != beastie_session::SESSION_PROTOCOL_VERSION || wait.command != "wait" {
            return Err(ScenarioError::Command(
                "unsupported wait control version".to_owned(),
            ));
        }
        if wait.milliseconds == 0 || wait.milliseconds > MAX_WAIT_MS {
            return Err(ScenarioError::WaitBounds);
        }
        return Ok(ScenarioStep::WaitTick {
            milliseconds: wait.milliseconds,
        });
    }
    if kind.command == "inspect_hold" {
        let hold: InspectHold = serde_json::from_str(line).map_err(ScenarioError::Json)?;
        if hold.version != beastie_session::SESSION_PROTOCOL_VERSION
            || hold.command != "inspect_hold"
        {
            return Err(ScenarioError::Command(
                "unsupported inspect hold control version".to_owned(),
            ));
        }
        if hold.milliseconds < 1_500 || hold.milliseconds > MAX_WAIT_MS {
            return Err(ScenarioError::InspectHoldBounds);
        }
        return Ok(ScenarioStep::WaitTick {
            milliseconds: hold.milliseconds,
        });
    }
    if kind.command == "marker" {
        let marker: Marker = serde_json::from_str(line).map_err(ScenarioError::Json)?;
        if marker.version != beastie_session::SESSION_PROTOCOL_VERSION || marker.command != "marker"
        {
            return Err(ScenarioError::Command(
                "unsupported marker control version".to_owned(),
            ));
        }
        if !safe_stem(&marker.name) {
            return Err(ScenarioError::MarkerName);
        }
        return Ok(ScenarioStep::Marker(marker.name));
    }
    if kind.command != "capture" {
        return GameSession::parse_command(line)
            .map(ScenarioStep::Session)
            .map_err(|error| ScenarioError::Command(error.to_string()));
    }
    let capture: Capture = serde_json::from_str(line).map_err(ScenarioError::Json)?;
    if capture.version != beastie_session::SESSION_PROTOCOL_VERSION {
        return Err(ScenarioError::Command(format!(
            "unsupported version {}",
            capture.version
        )));
    }
    if capture.command != "capture" || !safe_stem(&capture.name) {
        return Err(ScenarioError::CaptureName);
    }
    Ok(ScenarioStep::Capture(capture.name))
}

fn validate_control(version: u32, command: &str, expected: &str) -> Result<(), ScenarioError> {
    if version != beastie_session::SESSION_PROTOCOL_VERSION || command != expected {
        return Err(ScenarioError::Command(format!(
            "unsupported {expected} control version"
        )));
    }
    Ok(())
}

fn safe_stem(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

#[derive(Debug, Error)]
pub enum ScenarioError {
    #[error("failed to read scenario: {0}")]
    Io(std::io::Error),
    #[error("scenario line {line} is invalid: {source}")]
    Line {
        line: usize,
        source: Box<ScenarioError>,
    },
    #[error("scenario JSON is malformed: {0}")]
    Json(serde_json::Error),
    #[error("scenario command is invalid: {0}")]
    Command(String),
    #[error("capture name must contain only ASCII letters, numbers, '-' or '_'")]
    CaptureName,
    #[error("marker name must contain only ASCII letters, numbers, '-' or '_'")]
    MarkerName,
    #[error("wait must be between 1 ms and 15 minutes")]
    WaitBounds,
    #[error("inspect hold must be between 1,500 ms and 15 minutes")]
    InspectHoldBounds,
    #[error("scenario contains no commands")]
    Empty,
}

#[cfg(test)]
mod tests {
    use beastie_core::{FoodId, NormalizedPosition, ToyId};
    use beastie_session::{CommandEnvelope, SessionCommand};

    use super::*;

    #[test]
    fn typed_ui_controls_cover_targets_settings_rebinding_and_safe_data_actions() {
        for action in [
            UiAction::OpenContext(beastie_view::UiTarget::Toy(ToyId::Sock)),
            UiAction::OpenContext(beastie_view::UiTarget::Plant(1)),
            UiAction::Inspect,
            UiAction::SelectFood(FoodId::Mushroom),
            UiAction::BeginRebind(beastie_view::BindableAction::Cancel),
            UiAction::ToggleTranscript,
            UiAction::RecoverBackup,
            UiAction::ConfirmReset,
            UiAction::CycleEffectsVolume,
            UiAction::ToggleSubtitles,
            UiAction::TypeCharacter('水'),
        ] {
            let source = serde_json::json!({"version":1, "command":"ui_action", "action":action})
                .to_string();
            assert!(
                matches!(parse_step(&source), Ok(ScenarioStep::Ui(parsed)) if parsed == action)
            );
        }
        assert!(matches!(
            parse_step(
                r#"{"version":1,"command":"ui_action","action":"rename","controller":true}"#
            ),
            Ok(ScenarioStep::ControllerUi(UiAction::Rename))
        ));
        assert!(
            matches!(parse_step(r#"{"version":1,"command":"text_input","text":"é水"}"#), Ok(ScenarioStep::TextInput(text)) if text == "é水")
        );
    }

    #[test]
    fn ui_capture_vocabulary_rejects_unbounded_and_unknown_inputs() {
        for source in [
            r#"{"version":2,"command":"ui_action","action":"inspect"}"#,
            r#"{"version":1,"command":"ui_action","action":"inspect","unexpected":true}"#,
            r#"{"version":1,"command":"text_input","text":"bad\ninput"}"#,
            r#"{"version":1,"command":"ui_preview","preview":{"focus":"../player-file"}}"#,
            r#"{"version":1,"command":"ui_preview","preview":{"world":{"name":"fake"}}}"#,
        ] {
            assert!(parse_step(source).is_err(), "{source}");
        }
        let source =
            serde_json::json!({"version":1, "command":"text_input", "text":"a".repeat(513)})
                .to_string();
        assert!(parse_step(&source).is_err());
    }

    #[test]
    fn presentation_preview_changes_only_ephemeral_view_state() {
        let source = r#"{"version":1,"command":"ui_preview","preview":{"microphone":"unavailable","status":"Text still works.","focus":"compose/settings"}}"#;
        let ScenarioStep::UiPreview(preview) = parse_step(source).unwrap() else {
            panic!("preview");
        };
        let mut view = ViewState::default();
        preview.apply(&mut view, 20);
        assert_eq!(view.microphone_state, MicrophoneState::Unavailable);
        assert_eq!(view.focused_region.as_deref(), Some("compose/settings"));
        assert_eq!(view.status_expires_at_ms, Some(12_020));
        assert!(view.speech.is_none());
        assert!(view.cue_queue.is_empty());
    }

    #[test]
    fn redesign_fixtures_parse_and_keep_synthetic_previews_separate_from_data_flows() {
        for source in [
            include_str!("../../../fixtures/scenarios/ui-redesign-surfaces.jsonl"),
            include_str!("../../../fixtures/scenarios/ui-redesign-settings.jsonl"),
            include_str!("../../../fixtures/scenarios/ui-redesign-data-flow.jsonl"),
            include_str!("../../../fixtures/scenarios/ui-redesign-capacity.jsonl"),
        ] {
            let steps = source
                .lines()
                .map(parse_step)
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(
                steps
                    .iter()
                    .any(|step| matches!(step, ScenarioStep::Capture(_)))
            );
            assert!(
                !steps
                    .iter()
                    .any(|step| matches!(step, ScenarioStep::UiPreview(_)))
            );
        }
        let steps = include_str!("../../../fixtures/scenarios/ui-redesign-presentation.jsonl")
            .lines()
            .map(parse_step)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            steps
                .iter()
                .any(|step| matches!(step, ScenarioStep::UiPreview(_)))
        );
        assert!(
            matches!(&steps[0], ScenarioStep::Marker(name) if name == "presentation-only-previews")
        );
    }

    #[test]
    fn parses_session_and_capture_steps() {
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"inspect"}"#),
            Ok(ScenarioStep::Session(_))
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"ui","action":"open_settings"}"#),
            Ok(ScenarioStep::Ui(UiAction::SelectSettingsPage(0)))
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"ui","action":"cycle_window_scale"}"#),
            Ok(ScenarioStep::Ui(UiAction::CycleWindowScale))
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"ui","action":"toggle_fullscreen"}"#),
            Ok(ScenarioStep::Ui(UiAction::ToggleFullscreen))
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"set_subtitles","enabled":false}"#),
            Ok(ScenarioStep::SetSubtitles(false))
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"microphone_acquisition","outcome":"acquired"}"#),
            Ok(ScenarioStep::MicrophoneAcquisition(
                MicrophoneAcquisition::Acquired
            ))
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"capture","name":"room_1"}"#),
            Ok(ScenarioStep::Capture(name)) if name == "room_1"
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"marker","name":"settled"}"#),
            Ok(ScenarioStep::Marker(name)) if name == "settled"
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"inspect_hold","milliseconds":1500}"#),
            Ok(ScenarioStep::WaitTick { milliseconds: 1500 })
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"resume","elapsed_ms":900000}"#),
            Ok(ScenarioStep::Session(CommandEnvelope {
                command: SessionCommand::Resume { elapsed_ms: 900000 },
                ..
            }))
        ));
    }

    #[test]
    fn rejects_capture_path_traversal() {
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"capture","name":"../save"}"#),
            Err(ScenarioError::CaptureName)
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"inspect_hold","milliseconds":1499}"#),
            Err(ScenarioError::InspectHoldBounds)
        ));
    }

    #[test]
    fn waits_use_exact_sixty_hertz_cadence_and_support_five_minutes() {
        let mut runner = ScenarioRunner {
            steps: VecDeque::from([ScenarioStep::WaitTick {
                milliseconds: 1_000,
            }]),
            capture_dir: PathBuf::new(),
            wait: None,
        };
        let ticks = std::iter::from_fn(|| runner.next())
            .map(|step| match step {
                ScenarioStep::WaitTick { milliseconds } => milliseconds,
                _ => panic!("wait only"),
            })
            .collect::<Vec<_>>();
        assert_eq!(ticks.len(), 60);
        assert_eq!(ticks.iter().sum::<u64>(), 1_000);
        assert!(ticks.iter().all(|tick| matches!(tick, 16 | 17)));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"wait","milliseconds":300000}"#),
            Ok(ScenarioStep::WaitTick {
                milliseconds: 300_000
            })
        ));
    }

    #[test]
    fn rejects_out_of_bounds_waits_and_unsafe_markers() {
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"wait","milliseconds":900001}"#),
            Err(ScenarioError::WaitBounds)
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"marker","name":"../private"}"#),
            Err(ScenarioError::MarkerName)
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"microphone_acquisition","outcome":"missing"}"#),
            Err(ScenarioError::Json(_))
        ));
    }

    #[test]
    fn parses_each_toy_play_as_a_typed_session_command() {
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"play","toy":"sock"}"#),
            Ok(ScenarioStep::Session(CommandEnvelope {
                command: SessionCommand::Play { toy: ToyId::Sock },
                ..
            }))
        ));
    }

    #[test]
    fn dialogue_race_fixture_is_valid_and_restores_subtitles() {
        let source = include_str!("../../../fixtures/scenarios/feel/dialogue-races.jsonl");
        let steps = source
            .lines()
            .map(parse_step)
            .collect::<Result<Vec<_>, _>>()
            .expect("dialogue race fixture should parse");
        assert!(
            steps
                .iter()
                .any(|step| matches!(step, ScenarioStep::SetSubtitles(false)))
        );
        assert!(
            steps
                .iter()
                .any(|step| matches!(step, ScenarioStep::SetSubtitles(true)))
        );
    }

    #[test]
    fn bad_conditions_models_microphone_acquisition_without_faking_perception() {
        let source = include_str!("../../../fixtures/scenarios/feel/bad-conditions.jsonl");
        let steps = source
            .lines()
            .map(parse_step)
            .collect::<Result<Vec<_>, _>>()
            .expect("bad conditions fixture should parse");
        assert!(steps.iter().any(|step| {
            matches!(
                step,
                ScenarioStep::MicrophoneAcquisition(MicrophoneAcquisition::Unavailable)
            )
        }));
        assert!(!steps.iter().any(|step| {
            matches!(
                step,
                ScenarioStep::Session(CommandEnvelope {
                    command: SessionCommand::SpeechStarted,
                    ..
                })
            )
        }));
    }

    #[test]
    fn parses_pointer_tracking_and_physical_food_drop() {
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"cursor","position":{"x":7000,"y":1000}}"#),
            Ok(ScenarioStep::Session(CommandEnvelope {
                command: SessionCommand::Cursor {
                    position: Some(NormalizedPosition { x: 7000, y: 1000 })
                },
                ..
            }))
        ));
        assert!(matches!(
            parse_step(
                r#"{"version":1,"command":"drop_food","food":"berry","position":{"x":7000,"y":1000}}"#
            ),
            Ok(ScenarioStep::Session(CommandEnvelope {
                command: SessionCommand::DropFood {
                    food: FoodId::Berry,
                    position: NormalizedPosition { x: 7000, y: 1000 }
                },
                ..
            }))
        ));
    }
}
