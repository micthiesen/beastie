use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use beastie_session::{CommandEnvelope, GameSession};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug)]
pub enum ScenarioStep {
    Session(CommandEnvelope),
    Capture(String),
}

#[derive(Debug)]
pub struct ScenarioRunner {
    steps: VecDeque<ScenarioStep>,
    pub capture_dir: PathBuf,
}

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
        Ok(Self { steps, capture_dir })
    }

    pub fn next(&mut self) -> Option<ScenarioStep> {
        self.steps.pop_front()
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

    let kind: CommandKind = serde_json::from_str(line).map_err(ScenarioError::Json)?;
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
    if capture.command != "capture"
        || capture.name.is_empty()
        || capture.name.len() > 64
        || !capture
            .name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(ScenarioError::CaptureName);
    }
    Ok(ScenarioStep::Capture(capture.name))
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
            parse_step(r#"{"version":1,"command":"capture","name":"room_1"}"#),
            Ok(ScenarioStep::Capture(name)) if name == "room_1"
        ));
    }

    #[test]
    fn rejects_capture_path_traversal() {
        assert!(matches!(
            parse_step(r#"{"version":1,"command":"capture","name":"../save"}"#),
            Err(ScenarioError::CaptureName)
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
