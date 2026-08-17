use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use beastie_session::{CommandEnvelope, GameSession};
use beastie_view::UiAction;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug)]
pub enum ScenarioStep {
    Session(CommandEnvelope),
    Ui(UiAction),
    SetSubtitles(bool),
    Capture(String),
    Marker(String),
    /// One synthetic 60 Hz frame of a `wait` control.
    WaitTick {
        milliseconds: u64,
    },
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
    enum ScenarioUiAction {
        OpenFood,
        OpenToys,
        OpenSettings,
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
    }

    let kind: CommandKind = serde_json::from_str(line).map_err(ScenarioError::Json)?;
    if kind.command == "ui" {
        let control: UiControl = serde_json::from_str(line).map_err(ScenarioError::Json)?;
        if control.version != beastie_session::SESSION_PROTOCOL_VERSION || control.command != "ui" {
            return Err(ScenarioError::Command(
                "unsupported UI control version".to_owned(),
            ));
        }
        let action = match control.action {
            ScenarioUiAction::OpenFood => UiAction::OpenFoodChoice,
            ScenarioUiAction::OpenToys => UiAction::OpenToyChoice,
            ScenarioUiAction::OpenSettings => UiAction::OpenSettings,
            ScenarioUiAction::OpenBindings => UiAction::OpenBindings,
            ScenarioUiAction::OpenData => UiAction::OpenDataManagement,
            ScenarioUiAction::RequestReset => UiAction::RequestReset,
            ScenarioUiAction::Close => UiAction::CancelMode,
        };
        return Ok(ScenarioStep::Ui(action));
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
    fn parses_session_and_capture_steps() {
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"inspect"}"#),
            Ok(ScenarioStep::Session(_))
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"ui","action":"open_settings"}"#),
            Ok(ScenarioStep::Ui(UiAction::OpenSettings))
        ));
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"set_subtitles","enabled":false}"#),
            Ok(ScenarioStep::SetSubtitles(false))
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
