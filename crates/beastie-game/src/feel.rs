use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

use beastie_session::{CommandEnvelope, Observation, SessionCommand};
use beastie_view::{UiAction, ViewState};
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

use crate::renderer::{PRESENTATION_HEIGHT, PRESENTATION_WIDTH};

const FORMAT_VERSION: u32 = 1;
const FRAME_RATE: u64 = 60;

/// Streaming, privacy-safe evidence recorder for a visible scripted session.
pub struct FeelRecorder {
    directory: PathBuf,
    inputs: JsonlWriter,
    markers: JsonlWriter,
    events: JsonlWriter,
    state: JsonlWriter,
    audio: JsonlWriter,
    ffmpeg: Option<Ffmpeg>,
    frame_index: u64,
    speech_index: u32,
}

impl FeelRecorder {
    pub fn create(directory: PathBuf) -> Result<Self, FeelError> {
        fs::create_dir_all(&directory).map_err(|source| FeelError::CreateDirectory {
            path: directory.clone(),
            source,
        })?;
        Ok(Self {
            inputs: JsonlWriter::create(directory.join("inputs.jsonl"))?,
            markers: JsonlWriter::create(directory.join("markers.jsonl"))?,
            events: JsonlWriter::create(directory.join("events.jsonl"))?,
            state: JsonlWriter::create(directory.join("state.jsonl"))?,
            audio: JsonlWriter::create(directory.join("audio.jsonl"))?,
            ffmpeg: Some(Ffmpeg::spawn(&directory.join("session.mp4"))?),
            directory,
            frame_index: 0,
            speech_index: 0,
        })
    }

    #[must_use]
    pub const fn playback_ms(&self) -> u64 {
        playback_ms_for_frame(self.frame_index)
    }

    pub fn record_command(
        &mut self,
        command: &CommandEnvelope,
        simulation_ms: u64,
    ) -> Result<(), FeelError> {
        self.inputs.write(&json!({
            "version": FORMAT_VERSION,
            "playback_ms": self.playback_ms(),
            "simulation_ms": simulation_ms,
            "kind": "session_command",
            "input": redacted_command(command),
        }))
    }

    pub fn record_ui(&mut self, action: UiAction, simulation_ms: u64) -> Result<(), FeelError> {
        self.inputs.write(&json!({
            "version": FORMAT_VERSION,
            "playback_ms": self.playback_ms(),
            "simulation_ms": simulation_ms,
            "kind": "ui_action",
            "input": action,
        }))
    }

    pub fn record_native(
        &mut self,
        kind: &str,
        input: Value,
        simulation_ms: u64,
    ) -> Result<(), FeelError> {
        self.inputs.write(&json!({
            "version": FORMAT_VERSION,
            "playback_ms": self.playback_ms(),
            "simulation_ms": simulation_ms,
            "kind": kind,
            "input": input,
        }))
    }

    pub fn record_marker(&mut self, name: &str, simulation_ms: u64) -> Result<(), FeelError> {
        let input = json!({
            "version": FORMAT_VERSION,
            "playback_ms": self.playback_ms(),
            "simulation_ms": simulation_ms,
            "kind": "marker",
            "name": name,
        });
        self.inputs.write(&input)?;
        self.markers.write(&json!({
            "version": FORMAT_VERSION,
            "name": name,
            "playback_ms": self.playback_ms(),
            "simulation_ms": simulation_ms,
        }))
    }

    pub fn record_observation(
        &mut self,
        observation: &Observation,
        simulation_ms: u64,
    ) -> Result<(), FeelError> {
        // Deliberately omit dialogue requests and saved world snapshots. Either may contain
        // player or generated prose, while events and input status explain the causal path.
        self.events.write(&json!({
            "version": FORMAT_VERSION,
            "playback_ms": self.playback_ms(),
            "simulation_ms": simulation_ms,
            "sequence": observation.sequence,
            "events": redacted_events(&observation.events),
            "spoken_input": observation.spoken_input,
        }))
    }

    pub fn record_audio(
        &mut self,
        cue_ids: &[&str],
        speech_active: bool,
        one_shot_active: bool,
        ambience_duck: f32,
        simulation_ms: u64,
    ) -> Result<(), FeelError> {
        // Motif/reason stays in the synchronized events trace. Adding it here would require the
        // app-owned audio queue to carry the authoritative beat alongside each sound ID; keeping
        // this method cue-only avoids fabricating a relationship association at recording time.
        self.audio.write(&json!({
            "version": FORMAT_VERSION,
            "playback_ms": self.playback_ms(),
            "simulation_ms": simulation_ms,
            "kind": "cues",
            "cue_ids": cue_ids,
            "speech_active": speech_active,
            "one_shot_active": one_shot_active,
            "ambience_duck": ambience_duck,
        }))
    }

    pub fn record_speech(&mut self, wav: &[u8], simulation_ms: u64) -> Result<(), FeelError> {
        let name = format!("speech-{:03}.wav", self.speech_index);
        let path = self.directory.join(&name);
        fs::write(&path, wav).map_err(|source| FeelError::WriteTrace {
            path: path.clone(),
            source,
        })?;
        self.audio.write(&json!({
            "version": FORMAT_VERSION,
            "playback_ms": self.playback_ms(),
            "simulation_ms": simulation_ms,
            "kind": "speech",
            "path": name,
        }))?;
        self.speech_index = self.speech_index.saturating_add(1);
        Ok(())
    }

    pub fn record_frame(
        &mut self,
        rgba: &[u8],
        world: &beastie_core::WorldState,
        view: &ViewState,
    ) -> Result<(), FeelError> {
        let expected = PRESENTATION_WIDTH as usize * PRESENTATION_HEIGHT as usize * 4;
        if rgba.len() != expected {
            return Err(FeelError::UnexpectedFrameSize {
                actual: rgba.len(),
                expected,
            });
        }
        let frame_index = self.frame_index;
        let playback_ms = playback_ms_for_frame(frame_index);
        self.ffmpeg
            .as_mut()
            .ok_or(FeelError::Finalized)?
            .write_frame(rgba)?;
        self.state.write(&StateRecord {
            version: FORMAT_VERSION,
            frame_index,
            playback_ms,
            simulation_ms: world.elapsed_ms,
            creature: CreatureFrame {
                position: world.creature.aquarium.position,
                velocity: world.creature.aquarium.velocity,
                facing: world.creature.aquarium.facing,
                gaze: world.creature.aquarium.gaze,
                steering: world.creature.aquarium.steering,
                destination: world.creature.aquarium.destination,
                intention: world.creature.current_intention,
                action: world.creature.aquarium.action,
                mood: world.mood(),
                needs: &world.creature.needs,
                relationship: &world.creature.relationship,
                relationship_expression: &world.creature.relationship_expression,
                development: &world.creature.development,
                routines: &world.creature.routines,
                favorite_locations: world
                    .creature
                    .favorite_locations
                    .iter()
                    .map(|(destination, count)| FavoriteLocationFrame {
                        destination: *destination,
                        count: *count,
                    })
                    .collect(),
                initiated_behavior: world.creature.initiated_behavior.as_ref(),
                memory_count: world.creature.memories.len(),
            },
            view: ViewFrame {
                mode: view.mode,
                focused_region: view.focused_region.as_deref(),
                hovered_region: view.hovered_region.as_deref(),
                pending: view.pending,
                speech_present: view.speech.is_some(),
                speaking: view.speaking,
                mouth_phase: view.mouth_phase,
                microphone: view.microphone_state,
                status: view.status_message.as_deref(),
                status_expires_at_ms: view.status_expires_at_ms,
                cue: view.active_cue(world.elapsed_ms),
            },
        })?;
        self.frame_index = self.frame_index.saturating_add(1);
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), FeelError> {
        if let Some(mut ffmpeg) = self.ffmpeg.take() {
            ffmpeg.finish()?;
        }
        self.inputs.flush()?;
        self.markers.flush()?;
        self.events.flush()?;
        self.state.flush()?;
        self.audio.flush()?;
        Ok(())
    }
}

impl Drop for FeelRecorder {
    fn drop(&mut self) {
        if let Some(mut ffmpeg) = self.ffmpeg.take() {
            let _ = ffmpeg.finish();
        }
    }
}

#[derive(Serialize)]
struct StateRecord<'a> {
    version: u32,
    frame_index: u64,
    playback_ms: u64,
    simulation_ms: u64,
    creature: CreatureFrame<'a>,
    view: ViewFrame<'a>,
}

#[derive(Serialize)]
struct CreatureFrame<'a> {
    position: beastie_core::NormalizedPosition,
    velocity: beastie_core::NormalizedVelocity,
    facing: beastie_core::Facing,
    gaze: beastie_core::GazeTarget,
    steering: beastie_core::SteeringMode,
    destination: Option<beastie_core::SemanticDestination>,
    intention: beastie_core::Intention,
    action: Option<beastie_core::ActionTimeline>,
    mood: beastie_core::Mood,
    needs: &'a beastie_core::Needs,
    relationship: &'a beastie_core::Relationship,
    /// The bounded, typed relationship-expression ledger is safe to inspect: it contains motif
    /// keys and memory IDs, never player text or model output. Keeping it beside the frame makes
    /// active/recent beats and preemptions replayable without exposing UI meters.
    relationship_expression: &'a beastie_core::RelationshipExpressionState,
    development: &'a beastie_core::Development,
    routines: &'a [beastie_core::Routine],
    favorite_locations: Vec<FavoriteLocationFrame>,
    initiated_behavior: Option<&'a beastie_core::InitiatedBehavior>,
    memory_count: usize,
}

#[derive(Serialize)]
struct FavoriteLocationFrame {
    destination: beastie_core::SemanticDestination,
    count: u32,
}

#[derive(Serialize)]
struct ViewFrame<'a> {
    mode: beastie_view::UiMode,
    focused_region: Option<&'a str>,
    hovered_region: Option<&'a str>,
    pending: bool,
    speech_present: bool,
    speaking: bool,
    mouth_phase: u8,
    microphone: beastie_view::MicrophoneState,
    status: Option<&'a str>,
    status_expires_at_ms: Option<u64>,
    cue: Option<beastie_view::PresentationCueKind>,
}

struct JsonlWriter {
    path: PathBuf,
    writer: BufWriter<File>,
}

impl JsonlWriter {
    fn create(path: PathBuf) -> Result<Self, FeelError> {
        let file = File::create(&path).map_err(|source| FeelError::OpenTrace {
            path: path.clone(),
            source,
        })?;
        Ok(Self {
            path,
            writer: BufWriter::new(file),
        })
    }

    fn write(&mut self, value: &impl Serialize) -> Result<(), FeelError> {
        serde_json::to_writer(&mut self.writer, value).map_err(FeelError::Serialize)?;
        self.writer
            .write_all(b"\n")
            .map_err(|source| FeelError::WriteTrace {
                path: self.path.clone(),
                source,
            })
    }

    fn flush(&mut self) -> Result<(), FeelError> {
        self.writer.flush().map_err(|source| FeelError::WriteTrace {
            path: self.path.clone(),
            source,
        })
    }
}

struct Ffmpeg {
    child: Child,
    stdin: Option<ChildStdin>,
    output: PathBuf,
}

impl Ffmpeg {
    fn spawn(output: &Path) -> Result<Self, FeelError> {
        let mut child = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "rawvideo",
                "-pixel_format",
                "rgba",
                "-video_size",
                "640x360",
                "-framerate",
                "60",
                "-i",
                "pipe:0",
                "-an",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
            ])
            .arg(output)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(FeelError::SpawnFfmpeg)?;
        let stdin = child.stdin.take().ok_or(FeelError::MissingFfmpegPipe)?;
        Ok(Self {
            child,
            stdin: Some(stdin),
            output: output.to_path_buf(),
        })
    }

    fn write_frame(&mut self, rgba: &[u8]) -> Result<(), FeelError> {
        let stdin = self.stdin.as_mut().ok_or(FeelError::Finalized)?;
        stdin.write_all(rgba).map_err(FeelError::WriteFfmpeg)
    }

    fn finish(&mut self) -> Result<(), FeelError> {
        drop(self.stdin.take());
        let status = self.child.wait().map_err(FeelError::WaitFfmpeg)?;
        if status.success() {
            Ok(())
        } else {
            Err(FeelError::FfmpegFailed {
                output: self.output.clone(),
                status: status.to_string(),
            })
        }
    }
}

#[must_use]
pub const fn playback_ms_for_frame(frame_index: u64) -> u64 {
    frame_index.saturating_mul(1_000) / FRAME_RATE
}

fn redacted_command(command: &CommandEnvelope) -> Value {
    let mut value = serde_json::to_value(command).expect("session commands serialize");
    if matches!(
        command.command,
        SessionCommand::Talk { .. } | SessionCommand::SpeechCandidate { .. }
    ) {
        value["text"] = Value::String("<redacted>".to_owned());
    }
    value
}

fn redacted_events(events: &[beastie_core::GameEvent]) -> Value {
    let mut value = serde_json::to_value(events).expect("game events serialize");
    redact_text_fields(&mut value);
    value
}

fn redact_text_fields(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                redact_text_fields(value);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if matches!(key.as_str(), "name" | "text") {
                    *value = Value::String("<redacted>".to_owned());
                } else {
                    redact_text_fields(value);
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

#[derive(Debug, Error)]
pub enum FeelError {
    #[error("failed to create feel directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to open feel trace {path}: {source}")]
    OpenTrace {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write feel trace {path}: {source}")]
    WriteTrace {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to encode feel trace JSON: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to start ffmpeg: {0}")]
    SpawnFfmpeg(std::io::Error),
    #[error("ffmpeg did not expose its input pipe")]
    MissingFfmpegPipe,
    #[error("failed to stream a frame to ffmpeg: {0}")]
    WriteFfmpeg(std::io::Error),
    #[error("failed while waiting for ffmpeg: {0}")]
    WaitFfmpeg(std::io::Error),
    #[error("ffmpeg failed while writing {output}: {status}")]
    FfmpegFailed { output: PathBuf, status: String },
    #[error("feel recorder is already finalized")]
    Finalized,
    #[error("presentation frame had {actual} bytes, expected {expected}")]
    UnexpectedFrameSize { actual: usize, expected: usize },
}

#[cfg(test)]
mod tests {
    use beastie_session::{CommandEnvelope, SESSION_PROTOCOL_VERSION, SessionCommand};

    use super::{playback_ms_for_frame, redacted_command, redacted_events};

    #[test]
    fn frame_timestamps_follow_the_sixty_hertz_timeline() {
        assert_eq!(playback_ms_for_frame(0), 0);
        assert_eq!(playback_ms_for_frame(1), 16);
        assert_eq!(playback_ms_for_frame(60), 1_000);
    }

    #[test]
    fn private_talk_and_speech_candidate_text_are_never_serialized() {
        for command in [
            SessionCommand::Talk {
                text: "private words".to_owned(),
            },
            SessionCommand::SpeechCandidate {
                text: "private words".to_owned(),
                confidence: beastie_protocol::AcousticConfidence::new(900).expect("valid"),
            },
        ] {
            let encoded = redacted_command(&CommandEnvelope {
                version: SESSION_PROTOCOL_VERSION,
                command,
            })
            .to_string();
            assert!(!encoded.contains("private words"));
            assert!(encoded.contains("<redacted>"));
        }
    }

    #[test]
    fn event_names_are_redacted_from_the_observability_trace() {
        let encoded = redacted_events(&[beastie_core::GameEvent::NameAssigned {
            target: beastie_core::NamingTarget::Creature,
            name: "private pet name".to_owned(),
        }])
        .to_string();
        assert!(!encoded.contains("private pet name"));
        assert!(encoded.contains("<redacted>"));
    }
}
