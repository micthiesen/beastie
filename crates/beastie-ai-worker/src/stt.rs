//! Bounded offline speech-recognition worker and external Moonshine engine boundary.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::Duration;

use beastie_protocol::{
    AcousticConfidence, RecognitionErrorCode, RecognitionOutcome, RecognitionReply,
    RecognitionRequest, STT_PROTOCOL_VERSION, validate_recognition_reply,
    validate_recognition_request,
};
use sha2::{Digest, Sha256};

use crate::bounded::{BoundedLine, read_bounded_line};
use crate::process::{ContainedChild, UnixProcessGroup};

pub const MAX_STT_LINE_BYTES: usize = 4_096;
pub const MAX_STT_SECONDS: usize = 30;
pub const STT_SAMPLE_RATE: u32 = 16_000;
pub const MOONSHINE_ENGINE_PROTOCOL_VERSION: u32 = 1;
pub const MOONSHINE_MODEL_ID: &str = "moonshine-voice-v0.1.2-tiny-streaming-arch2";
const MAX_WAV_FILE_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone)]
pub struct MoonshineConfig {
    pub executable: PathBuf,
    pub model_dir: PathBuf,
    pub audio_root: PathBuf,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAudio {
    path: PathBuf,
    samples: Vec<i16>,
}

impl ValidatedAudio {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn samples(&self) -> &[i16] {
        &self.samples
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttBackendError {
    Unavailable,
    Failed,
    Timeout,
    UnsupportedLanguage,
}

pub trait SttBackend {
    fn recognize(
        &mut self,
        request: &RecognitionRequest,
        audio: &ValidatedAudio,
    ) -> Result<RecognitionOutcome, SttBackendError>;
}

#[derive(Debug, Default)]
pub struct FixtureSttBackend;

impl SttBackend for FixtureSttBackend {
    fn recognize(
        &mut self,
        _request: &RecognitionRequest,
        audio: &ValidatedAudio,
    ) -> Result<RecognitionOutcome, SttBackendError> {
        if audio.samples.iter().all(|sample| *sample == 0) {
            return Ok(RecognitionOutcome::NoSpeech {});
        }
        Ok(RecognitionOutcome::Recognized {
            text: "hello beastie".to_owned(),
            confidence: AcousticConfidence::new(900).expect("fixture confidence is bounded"),
        })
    }
}

#[derive(Debug)]
pub struct MoonshineBackend {
    config: MoonshineConfig,
    engine: Option<RunningEngine>,
}

impl MoonshineBackend {
    #[must_use]
    pub fn new(config: MoonshineConfig) -> Self {
        Self {
            config,
            engine: None,
        }
    }

    fn ensure_engine(&mut self) -> Result<(), SttBackendError> {
        if let Some(engine) = self.engine.as_mut() {
            match engine.child.try_wait() {
                Ok(None) => return Ok(()),
                Ok(Some(_)) | Err(_) => self.stop_engine(),
            }
        }

        let mut command = Command::new(&self.config.executable);
        command
            .arg("--model-dir")
            .arg(&self.config.model_dir)
            .arg("--audio-root")
            .arg(&self.config.audio_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = ContainedChild::spawn(&mut command, UnixProcessGroup::New)
            .map_err(|_| SttBackendError::Unavailable)?;
        let stdin = child.stdin.take().ok_or(SttBackendError::Unavailable)?;
        let stdout = child.stdout.take().ok_or(SttBackendError::Unavailable)?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let reader = std::thread::spawn(move || {
            let mut stdout = BufReader::new(stdout);
            loop {
                let message = match read_bounded_line(&mut stdout, MAX_STT_LINE_BYTES) {
                    Ok(Some(BoundedLine::Line(line))) => EngineMessage::Line(line),
                    Ok(Some(BoundedLine::Invalid)) => EngineMessage::Invalid,
                    Ok(None) | Err(_) => EngineMessage::Closed,
                };
                let closed = matches!(message, EngineMessage::Closed);
                if sender.send(message).is_err() || closed {
                    break;
                }
            }
        });
        self.engine = Some(RunningEngine {
            child,
            stdin,
            receiver,
            reader: Some(reader),
        });
        Ok(())
    }

    fn attempt(
        &mut self,
        request: &RecognitionRequest,
    ) -> Result<RecognitionOutcome, SttBackendError> {
        self.ensure_engine()?;
        let Some(engine) = self.engine.as_mut() else {
            return Err(SttBackendError::Unavailable);
        };
        serde_json::to_writer(&mut engine.stdin, request).map_err(|_| SttBackendError::Failed)?;
        engine
            .stdin
            .write_all(b"\n")
            .and_then(|()| engine.stdin.flush())
            .map_err(|_| SttBackendError::Failed)?;
        let line = match engine.receiver.recv_timeout(self.config.timeout) {
            Ok(EngineMessage::Line(line)) => line,
            Ok(EngineMessage::Invalid | EngineMessage::Closed) => {
                return Err(SttBackendError::Failed);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => return Err(SttBackendError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(SttBackendError::Failed),
        };
        let reply =
            serde_json::from_str::<RecognitionReply>(&line).map_err(|_| SttBackendError::Failed)?;
        let reply =
            validate_recognition_reply(request, reply).map_err(|_| SttBackendError::Failed)?;
        Ok(reply.outcome)
    }

    fn stop_engine(&mut self) {
        if let Some(mut engine) = self.engine.take() {
            engine.child.terminate_tree();
            drop(engine.stdin);
            if let Some(reader) = engine.reader.take() {
                let _ = reader.join();
            }
        }
    }
}

impl SttBackend for MoonshineBackend {
    fn recognize(
        &mut self,
        request: &RecognitionRequest,
        _audio: &ValidatedAudio,
    ) -> Result<RecognitionOutcome, SttBackendError> {
        let result = self.attempt(request);
        if result.is_err() {
            self.stop_engine();
        }
        result
    }
}

impl Drop for MoonshineBackend {
    fn drop(&mut self) {
        self.stop_engine();
    }
}

#[derive(Debug)]
struct RunningEngine {
    child: ContainedChild,
    stdin: ChildStdin,
    receiver: Receiver<EngineMessage>,
    reader: Option<JoinHandle<()>>,
}

#[derive(Debug)]
enum EngineMessage {
    Line(String),
    Invalid,
    Closed,
}

pub fn run_stt_jsonl(
    mut input: impl io::BufRead,
    mut output: impl Write,
    audio_root: &Path,
    backend: &mut dyn SttBackend,
) -> io::Result<()> {
    while let Some(line) = read_bounded_line(&mut input, MAX_STT_LINE_BYTES)? {
        let reply = match line {
            BoundedLine::Line(line) => process_stt_line(&line, audio_root, backend),
            BoundedLine::Invalid => {
                recognition_error_reply(0, RecognitionErrorCode::InvalidRequest)
            }
        };
        serde_json::to_writer(&mut output, &reply)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}

#[must_use]
pub fn process_stt_line(
    line: &str,
    audio_root: &Path,
    backend: &mut dyn SttBackend,
) -> RecognitionReply {
    let Ok(request) = serde_json::from_str::<RecognitionRequest>(line) else {
        return recognition_error_reply(0, RecognitionErrorCode::InvalidRequest);
    };
    if validate_recognition_request(&request).is_err() {
        return recognition_error_reply(request.request_id, RecognitionErrorCode::InvalidRequest);
    }
    let audio = match load_validated_audio(audio_root, &request.audio_key) {
        Ok(audio) => audio,
        Err(AudioError::Unavailable) => {
            return recognition_error_reply(
                request.request_id,
                RecognitionErrorCode::AudioUnavailable,
            );
        }
        Err(AudioError::Invalid) => {
            return recognition_error_reply(request.request_id, RecognitionErrorCode::InvalidAudio);
        }
        Err(AudioError::Io) => {
            return recognition_error_reply(
                request.request_id,
                RecognitionErrorCode::RecognitionFailed,
            );
        }
    };
    let outcome = match backend.recognize(&request, &audio) {
        Ok(outcome) => outcome,
        Err(SttBackendError::Unavailable) => RecognitionOutcome::Error {
            code: RecognitionErrorCode::BackendUnavailable,
        },
        Err(SttBackendError::Failed) => RecognitionOutcome::Error {
            code: RecognitionErrorCode::RecognitionFailed,
        },
        Err(SttBackendError::Timeout) => RecognitionOutcome::Error {
            code: RecognitionErrorCode::Timeout,
        },
        Err(SttBackendError::UnsupportedLanguage) => RecognitionOutcome::Error {
            code: RecognitionErrorCode::UnsupportedLanguage,
        },
    };
    let reply = RecognitionReply {
        protocol_version: STT_PROTOCOL_VERSION,
        request_id: request.request_id,
        outcome,
    };
    validate_recognition_reply(&request, reply).unwrap_or_else(|_| {
        recognition_error_reply(request.request_id, RecognitionErrorCode::RecognitionFailed)
    })
}

fn recognition_error_reply(request_id: u64, code: RecognitionErrorCode) -> RecognitionReply {
    RecognitionReply {
        protocol_version: STT_PROTOCOL_VERSION,
        request_id,
        outcome: RecognitionOutcome::Error { code },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioError {
    Unavailable,
    Invalid,
    Io,
}

fn load_validated_audio(audio_root: &Path, audio_key: &str) -> Result<ValidatedAudio, AudioError> {
    let path = audio_root.join(format!("{audio_key}.wav"));
    let metadata = fs::symlink_metadata(&path).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => AudioError::Unavailable,
        _ => AudioError::Io,
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_file()
        || metadata.len() > MAX_WAV_FILE_BYTES
    {
        return Err(AudioError::Invalid);
    }
    let mut file = File::open(&path).map_err(|_| AudioError::Io)?;
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    Read::by_ref(&mut file)
        .take(MAX_WAV_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AudioError::Io)?;
    if bytes.len() as u64 > MAX_WAV_FILE_BYTES
        || format!("{:x}", Sha256::digest(&bytes)) != audio_key
    {
        return Err(AudioError::Invalid);
    }
    let samples = parse_pcm16_mono_16k_wav(&bytes)?;
    Ok(ValidatedAudio { path, samples })
}

fn parse_pcm16_mono_16k_wav(bytes: &[u8]) -> Result<Vec<i16>, AudioError> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(AudioError::Invalid);
    }
    let riff_size = read_u32(bytes, 4)? as usize;
    if riff_size.checked_add(8) != Some(bytes.len()) {
        return Err(AudioError::Invalid);
    }

    let mut offset = 12_usize;
    let mut format_seen = false;
    let mut data = None;
    while offset < bytes.len() {
        let header_end = offset.checked_add(8).ok_or(AudioError::Invalid)?;
        if header_end > bytes.len() {
            return Err(AudioError::Invalid);
        }
        let size = read_u32(bytes, offset + 4)? as usize;
        let content_start = header_end;
        let content_end = content_start.checked_add(size).ok_or(AudioError::Invalid)?;
        if content_end > bytes.len() {
            return Err(AudioError::Invalid);
        }
        match &bytes[offset..offset + 4] {
            b"fmt " if !format_seen && size == 16 => {
                let format = read_u16(bytes, content_start)?;
                let channels = read_u16(bytes, content_start + 2)?;
                let sample_rate = read_u32(bytes, content_start + 4)?;
                let byte_rate = read_u32(bytes, content_start + 8)?;
                let block_align = read_u16(bytes, content_start + 12)?;
                let bits_per_sample = read_u16(bytes, content_start + 14)?;
                if format != 1
                    || channels != 1
                    || sample_rate != STT_SAMPLE_RATE
                    || byte_rate != STT_SAMPLE_RATE * 2
                    || block_align != 2
                    || bits_per_sample != 16
                {
                    return Err(AudioError::Invalid);
                }
                format_seen = true;
            }
            b"fmt " => return Err(AudioError::Invalid),
            b"data" if data.is_none() => data = Some(&bytes[content_start..content_end]),
            b"data" => return Err(AudioError::Invalid),
            _ => {}
        }
        offset = content_end
            .checked_add(size % 2)
            .ok_or(AudioError::Invalid)?;
    }

    let data = data.ok_or(AudioError::Invalid)?;
    if !format_seen
        || data.is_empty()
        || data.len() % 2 != 0
        || data.len() > STT_SAMPLE_RATE as usize * MAX_STT_SECONDS * 2
    {
        return Err(AudioError::Invalid);
    }
    Ok(data
        .chunks_exact(2)
        .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
        .collect())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, AudioError> {
    let end = offset.checked_add(2).ok_or(AudioError::Invalid)?;
    let value = bytes.get(offset..end).ok_or(AudioError::Invalid)?;
    Ok(u16::from_le_bytes(
        value.try_into().map_err(|_| AudioError::Invalid)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, AudioError> {
    let end = offset.checked_add(4).ok_or(AudioError::Invalid)?;
    let value = bytes.get(offset..end).ok_or(AudioError::Invalid)?;
    Ok(u32::from_le_bytes(
        value.try_into().map_err(|_| AudioError::Invalid)?,
    ))
}

impl fmt::Display for SttBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "STT backend error: {self:?}")
    }
}

impl std::error::Error for SttBackendError {}
