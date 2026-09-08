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
const FIRST_FRAME_HEARTBEAT: &str = "first-frame.json";
const FIRST_FRAME_HEARTBEAT_TEMP: &str = ".first-frame.json.tmp";

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SpeechTraceOwner {
    pub dialogue_generation: u64,
    pub dialogue_request_id: u64,
    pub tts_request_id: u64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct DialogueTraceOwner {
    pub dialogue_generation: u64,
    pub dialogue_request_id: u64,
}

/// Dialogue diagnostics intentionally carry only bounded outcome metadata. They exclude player
/// input, generated text, and memory/belief identifiers so feel bundles remain safe to share.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DialogueHealthTrace {
    pub owner: DialogueTraceOwner,
    pub backend: beastie_protocol::TranscriptBackend,
    pub fallback: bool,
    pub fallback_reason: Option<beastie_protocol::DialogueFallbackReason>,
    pub retry_count: u8,
    pub duplicate_suppressed: bool,
    pub reply_word_count: usize,
    pub recalled_memory: bool,
    pub recalled_belief: bool,
    pub accepted_by_session: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PresentationTraceState {
    pub subtitles_enabled: bool,
    pub active_dialogue_owner: Option<DialogueTraceOwner>,
    pub caption_owner: Option<DialogueTraceOwner>,
    pub pending_mouth_owner: Option<SpeechTraceOwner>,
    pub active_mouth_owner: Option<SpeechTraceOwner>,
}

pub struct AudioTraceFrame<'a> {
    pub commands: &'a [beastie_view::AudioCommand],
    pub cue_ids: &'a [&'a str],
    pub speech_active: bool,
    pub one_shot_active: bool,
    pub ambience_duck: f32,
    pub speech_owner: Option<SpeechTraceOwner>,
    pub presentation: PresentationTraceState,
}

/// Submission time advances independently from delayed or out-of-order GPU readback.
#[derive(Default)]
struct FrameTimeline {
    scheduled: u64,
    completed: u64,
}
impl FrameTimeline {
    fn schedule(&mut self) -> u64 {
        let frame = self.scheduled;
        self.scheduled += 1;
        frame
    }
    const fn playback_ms(&self) -> u64 {
        playback_ms_for_frame(self.scheduled)
    }
}

/// Streaming, privacy-safe evidence recorder for a visible scripted session.
pub struct FeelRecorder {
    directory: PathBuf,
    inputs: JsonlWriter,
    markers: JsonlWriter,
    events: JsonlWriter,
    state: JsonlWriter,
    audio: JsonlWriter,
    ffmpeg: Option<Ffmpeg>,
    timeline: FrameTimeline,
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
            timeline: FrameTimeline::default(),
            speech_index: 0,
        })
    }

    #[must_use]
    pub const fn playback_ms(&self) -> u64 {
        self.timeline.playback_ms()
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
        frame: AudioTraceFrame<'_>,
        simulation_ms: u64,
    ) -> Result<(), FeelError> {
        self.audio.write(&json!({
            "version": FORMAT_VERSION,
            "playback_ms": self.playback_ms(),
            "simulation_ms": simulation_ms,
            "kind": "cues",
            "cue_ids": frame.cue_ids,
            "commands": frame.commands,
            "speech_active": frame.speech_active,
            "one_shot_active": frame.one_shot_active,
            "ambience_duck": frame.ambience_duck,
            "speech_owner": frame.speech_owner,
            "subtitles_enabled": frame.presentation.subtitles_enabled,
            "active_dialogue_owner": frame.presentation.active_dialogue_owner,
            "caption_owner": frame.presentation.caption_owner,
            "pending_mouth_owner": frame.presentation.pending_mouth_owner,
            "active_mouth_owner": frame.presentation.active_mouth_owner,
        }))
    }

    pub fn record_speech(
        &mut self,
        wav: &[u8],
        owner: SpeechTraceOwner,
        simulation_ms: u64,
    ) -> Result<(), FeelError> {
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
            "speech_owner": owner,
        }))?;
        self.speech_index = self.speech_index.saturating_add(1);
        Ok(())
    }

    pub fn record_tts_lifecycle(
        &mut self,
        kind: &str,
        owner: SpeechTraceOwner,
        wav_present: Option<bool>,
        simulation_ms: u64,
    ) -> Result<(), FeelError> {
        self.audio.write(&json!({
            "version": FORMAT_VERSION,
            "playback_ms": self.playback_ms(),
            "simulation_ms": simulation_ms,
            "kind": kind,
            "speech_owner": owner,
            "wav_present": wav_present,
        }))
    }

    pub fn record_dialogue_health(
        &mut self,
        health: DialogueHealthTrace,
        simulation_ms: u64,
    ) -> Result<(), FeelError> {
        self.events.write(&json!({
            "version": FORMAT_VERSION,
            "playback_ms": self.playback_ms(),
            "simulation_ms": simulation_ms,
            "kind": "dialogue_health",
            "health": health,
        }))
    }

    /// Reserve the current frame before asynchronous GPU readback. Commands and audio for
    /// the next submitted frame must not depend on when older screenshots complete.
    pub fn schedule_frame(&mut self) -> u64 {
        self.timeline.schedule()
    }

    pub fn record_frame(
        &mut self,
        frame_index: u64,
        rgba: &[u8],
        world: &beastie_core::WorldState,
        view: &ViewState,
        presentation: PresentationTraceState,
    ) -> Result<(), FeelError> {
        let expected = PRESENTATION_WIDTH as usize * PRESENTATION_HEIGHT as usize * 4;
        if rgba.len() != expected {
            return Err(FeelError::UnexpectedFrameSize {
                actual: rgba.len(),
                expected,
            });
        }
        if frame_index != self.timeline.completed || frame_index >= self.timeline.scheduled {
            return Err(FeelError::FrameOrder {
                expected: self.timeline.completed,
                actual: frame_index,
            });
        }
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
                action: world.creature.aquarium.action.clone(),
                mood: world.mood(),
                needs: &world.creature.needs,
                relationship: &world.creature.relationship,
                relationship_expression: &world.creature.relationship_expression,
                private_life: &world.creature.private_life,
                toy_states: &world.aquarium.toy_states,
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
                last_arrived_destination: world.creature.idle_life.last_arrived_destination,
                settled_until_ms: world.creature.idle_life.settled_until_ms,
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
                subtitles_enabled: presentation.subtitles_enabled,
                active_dialogue_owner: presentation.active_dialogue_owner,
                caption_owner: presentation.caption_owner,
                pending_mouth_owner: presentation.pending_mouth_owner,
                active_mouth_owner: presentation.active_mouth_owner,
            },
        })?;
        if frame_index == 0 {
            // The runner may treat this file as proof that startup completed. Flush the matching
            // state record first, then publish the heartbeat with an atomic rename so it can
            // never observe a partial record or a heartbeat for a frame absent from state.jsonl.
            self.state.flush()?;
            write_first_frame_heartbeat(&self.directory, playback_ms, world.elapsed_ms)?;
        }
        self.timeline.completed = self.timeline.completed.saturating_add(1);
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), FeelError> {
        if self.timeline.completed != self.timeline.scheduled {
            return Err(FeelError::IncompleteFrames {
                scheduled: self.timeline.scheduled,
                completed: self.timeline.completed,
            });
        }
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

#[derive(Serialize)]
struct FirstFrameHeartbeat {
    version: u32,
    frame_index: u64,
    playback_ms: u64,
    simulation_ms: u64,
}

fn write_first_frame_heartbeat(
    directory: &Path,
    playback_ms: u64,
    simulation_ms: u64,
) -> Result<(), FeelError> {
    let temporary = directory.join(FIRST_FRAME_HEARTBEAT_TEMP);
    let published = directory.join(FIRST_FRAME_HEARTBEAT);
    let mut file = File::create(&temporary).map_err(|source| FeelError::WriteHeartbeat {
        path: temporary.clone(),
        source,
    })?;
    serde_json::to_writer(
        &mut file,
        &FirstFrameHeartbeat {
            version: FORMAT_VERSION,
            frame_index: 0,
            playback_ms,
            simulation_ms,
        },
    )
    .map_err(FeelError::Serialize)?;
    file.write_all(b"\n")
        .and_then(|()| file.sync_all())
        .map_err(|source| FeelError::WriteHeartbeat {
            path: temporary.clone(),
            source,
        })?;
    fs::rename(&temporary, &published).map_err(|source| FeelError::PublishHeartbeat {
        from: temporary,
        to: published,
        source,
    })
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
    /// Exact activity identity, selection evidence, phase, and contact state. This is simulation
    /// truth, not a presentation inference from the status label.
    private_life: &'a beastie_core::PrivateLifeState,
    toy_states: &'a std::collections::BTreeMap<beastie_core::ToyId, beastie_core::ToyObjectState>,
    development: &'a beastie_core::Development,
    routines: &'a [beastie_core::Routine],
    favorite_locations: Vec<FavoriteLocationFrame>,
    last_arrived_destination: Option<beastie_core::SemanticDestination>,
    settled_until_ms: u64,
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
    subtitles_enabled: bool,
    active_dialogue_owner: Option<DialogueTraceOwner>,
    caption_owner: Option<DialogueTraceOwner>,
    pending_mouth_owner: Option<SpeechTraceOwner>,
    active_mouth_owner: Option<SpeechTraceOwner>,
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

fn terminate_encoder_process(child: &mut Child, stdin: &mut Option<ChildStdin>) {
    // Closing stdin lets a healthy encoder finish, while kill covers recorder failures where
    // the caller cannot run `finish`. Always wait after either path so the process is reaped.
    drop(stdin.take());
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
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
                "1280x720",
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

impl Drop for Ffmpeg {
    fn drop(&mut self) {
        terminate_encoder_process(&mut self.child, &mut self.stdin);
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
    #[error("capture frame {actual} completed out of order; expected {expected}")]
    FrameOrder { expected: u64, actual: u64 },
    #[error("capture ended with {completed} of {scheduled} submitted frames")]
    IncompleteFrames { scheduled: u64, completed: u64 },
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
    #[error("failed to write first-frame heartbeat {path}: {source}")]
    WriteHeartbeat {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to atomically publish first-frame heartbeat from {from} to {to}: {source}")]
    PublishHeartbeat {
        from: PathBuf,
        to: PathBuf,
        source: std::io::Error,
    },
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
    #[cfg(unix)]
    use super::terminate_encoder_process;
    use super::{
        DialogueHealthTrace, DialogueTraceOwner, FIRST_FRAME_HEARTBEAT, playback_ms_for_frame,
        redacted_command, redacted_events, write_first_frame_heartbeat,
    };
    use beastie_session::{CommandEnvelope, SESSION_PROTOCOL_VERSION, SessionCommand};
    use std::fs;
    #[cfg(unix)]
    use std::process::{Command, Stdio};

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

    #[test]
    fn dialogue_health_trace_contains_only_outcome_metadata() {
        let encoded = serde_json::to_string(&DialogueHealthTrace {
            owner: DialogueTraceOwner {
                dialogue_generation: 2,
                dialogue_request_id: 9,
            },
            backend: beastie_protocol::TranscriptBackend::Fixture,
            fallback: false,
            fallback_reason: None,
            retry_count: 1,
            duplicate_suppressed: true,
            reply_word_count: 4,
            recalled_memory: true,
            recalled_belief: false,
            accepted_by_session: true,
        })
        .expect("health metadata serializes");
        assert!(encoded.contains("reply_word_count"));
        assert!(encoded.contains("recalled_memory"));
        assert!(!encoded.contains("private words"));
        assert!(!encoded.contains("say"));
        assert!(!encoded.contains("memory_id"));
    }

    #[test]
    fn first_frame_heartbeat_is_published_as_complete_json() {
        let directory = std::env::temp_dir().join(format!(
            "beastie-first-frame-heartbeat-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("temporary directory");
        write_first_frame_heartbeat(&directory, 0, 250).expect("heartbeat");
        let source = fs::read_to_string(directory.join(FIRST_FRAME_HEARTBEAT)).expect("published");
        let value: serde_json::Value = serde_json::from_str(&source).expect("complete JSON");
        assert_eq!(value["frame_index"], 0);
        assert_eq!(value["simulation_ms"], 250);
        assert!(!directory.join(".first-frame.json.tmp").exists());
        fs::remove_dir_all(directory).expect("cleanup");
    }
    #[test]
    fn audio_and_command_time_advance_on_submission_before_readback() {
        let mut timeline = super::FrameTimeline::default();
        assert_eq!(timeline.schedule(), 0);
        assert_eq!(timeline.schedule(), 1);
        assert_eq!(timeline.completed, 0);
        assert_eq!(timeline.playback_ms(), 33);
        timeline.completed = 1;
        assert_eq!(timeline.playback_ms(), 33);
        assert_eq!(timeline.schedule(), 2);
        assert_eq!(timeline.playback_ms(), 50);
    }

    #[cfg(unix)]
    #[test]
    fn dropping_an_active_encoder_reaps_the_child() {
        let mut child = Command::new("sh")
            .args(["-c", "sleep 60"])
            .stdin(Stdio::piped())
            .spawn()
            .expect("test encoder child");
        let mut stdin = child.stdin.take();
        terminate_encoder_process(&mut child, &mut stdin);
        assert!(child.try_wait().expect("child status").is_some());
    }
}
