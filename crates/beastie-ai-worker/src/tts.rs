//! Bounded, deterministic speech synthesis and WAV caching.
//!
//! The native sherpa implementation is intentionally compiled only with the
//! `experimental-gpl-tts` feature. sherpa-onnx 1.13.5 statically embeds GPLv3
//! espeak-ng, so that feature must not be enabled in distributed builds.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use beastie_protocol::{
    TTS_PROTOCOL_VERSION, TtsErrorCode, TtsOutcome, TtsReply, TtsRequest, validate_tts_request,
};
use sha2::{Digest, Sha256};

use crate::bounded::{BoundedLine, read_bounded_line};
use crate::process::{ContainedChild, UnixProcessGroup};

pub const KITTEN_MODEL_ID: &str = "kitten-nano-en-v0_8-int8";
pub const KITTEN_MODEL_SHA256: &str =
    "0ba1e21eda9c8bcc4a70ada7e0d27fefc9ba775aaa037547248ec71f9a3d9b7d";
pub const KITTEN_VOICES_SHA256: &str =
    "d520519c4a3519d44fcfcd943ed0b1e3c5da5cee0eea501d922fac1a93cd24dc";
pub const MAX_TTS_TEXT_BYTES: usize = 2_048;
pub const MAX_TTS_SECONDS: usize = 30;
pub const MAX_TTS_LINE_BYTES: usize = 4_096;
const CACHE_FORMAT_VERSION: &str = "beastie-tts-cache-v1";
const MAX_WAV_BYTES: u64 = 44 + 192_000 * MAX_TTS_SECONDS as u64 * 2;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceSettings {
    pub speaker_id: u8,
    pub speed: f32,
    pub silence_scale: f32,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            // Kitten v0.8 Jasper. Keep this explicit so the cache remains stable.
            speaker_id: 0,
            speed: 1.0,
            silence_scale: 0.2,
        }
    }
}

impl VoiceSettings {
    pub fn validate(self) -> Result<Self, TtsError> {
        if self.speaker_id > 7 {
            return Err(TtsError::InvalidVoice(self.speaker_id));
        }
        if !self.speed.is_finite() || !(0.5..=2.0).contains(&self.speed) {
            return Err(TtsError::InvalidSpeed);
        }
        if !self.silence_scale.is_finite() || !(0.0..=2.0).contains(&self.silence_scale) {
            return Err(TtsError::InvalidSilenceScale);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SynthesizedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

pub trait TtsSynthesizer {
    fn cache_identity(&self) -> &str;

    fn synthesize(
        &mut self,
        text: &str,
        settings: VoiceSettings,
    ) -> Result<SynthesizedAudio, TtsError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedSpeech {
    pub path: PathBuf,
    pub cache_hit: bool,
    pub cache_key: String,
}

#[derive(Debug)]
pub enum TtsError {
    EmptyText,
    TextTooLong,
    EmbeddedNul,
    InvalidVoice(u8),
    InvalidSpeed,
    InvalidSilenceScale,
    InvalidAudio,
    OutputTooLong,
    Backend(String),
    Io(io::Error),
}

impl fmt::Display for TtsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText => formatter.write_str("TTS text is empty"),
            Self::TextTooLong => formatter.write_str("TTS text exceeds 2048 UTF-8 bytes"),
            Self::EmbeddedNul => formatter.write_str("TTS text contains an embedded NUL"),
            Self::InvalidVoice(id) => write!(formatter, "Kitten speaker ID {id} is outside 0..=7"),
            Self::InvalidSpeed => {
                formatter.write_str("TTS speed must be finite and within 0.5..=2.0")
            }
            Self::InvalidSilenceScale => {
                formatter.write_str("TTS silence scale must be finite and within 0.0..=2.0")
            }
            Self::InvalidAudio => formatter.write_str("TTS backend returned invalid audio"),
            Self::OutputTooLong => formatter.write_str("TTS output exceeds the 30 second limit"),
            Self::Backend(message) => write!(formatter, "TTS backend failed: {message}"),
            Self::Io(error) => write!(formatter, "TTS cache I/O failed: {error}"),
        }
    }
}

impl std::error::Error for TtsError {}

impl From<io::Error> for TtsError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn synthesize_to_cache(
    cache_dir: &Path,
    text: &str,
    settings: VoiceSettings,
    synthesizer: &mut dyn TtsSynthesizer,
) -> Result<CachedSpeech, TtsError> {
    validate_text(text)?;
    let settings = settings.validate()?;
    fs::create_dir_all(cache_dir)?;

    let key = backend_cache_key(text, settings, synthesizer.cache_identity());
    let path = cache_dir.join(format!("{key}.wav"));
    if validate_cached_wav(&path).is_ok() {
        return Ok(CachedSpeech {
            path,
            cache_hit: true,
            cache_key: key,
        });
    }
    if path.exists() {
        fs::remove_file(&path)?;
    }

    let audio = synthesizer.synthesize(text, settings)?;
    validate_audio(&audio)?;
    write_wav_atomic(&path, &audio)?;
    Ok(CachedSpeech {
        path,
        cache_hit: false,
        cache_key: key,
    })
}

pub fn run_tts_jsonl(
    mut input: impl BufRead,
    mut output: impl Write,
    cache_dir: &Path,
    synthesizer: &mut dyn TtsSynthesizer,
) -> io::Result<()> {
    while let Some(line) = read_bounded_line(&mut input, MAX_TTS_LINE_BYTES)? {
        let reply = match line {
            BoundedLine::Line(line) => process_tts_line(&line, cache_dir, synthesizer),
            BoundedLine::Invalid => tts_error_reply(0, TtsErrorCode::InvalidRequest),
        };
        serde_json::to_writer(&mut output, &reply)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}

fn process_tts_line(
    line: &str,
    cache_dir: &Path,
    synthesizer: &mut dyn TtsSynthesizer,
) -> TtsReply {
    let Ok(request) = serde_json::from_str::<TtsRequest>(line) else {
        return tts_error_reply(0, TtsErrorCode::InvalidRequest);
    };
    if validate_tts_request(&request).is_err() {
        return tts_error_reply(request.request_id, TtsErrorCode::InvalidRequest);
    }
    let settings = VoiceSettings {
        speaker_id: request.settings.speaker_id,
        speed: request.settings.speed,
        silence_scale: request.settings.silence_scale,
    };
    match synthesize_to_cache(cache_dir, &request.text, settings, synthesizer) {
        Ok(cached) => TtsReply {
            protocol_version: TTS_PROTOCOL_VERSION,
            request_id: request.request_id,
            outcome: TtsOutcome::Ready {
                cache_key: cached.cache_key,
            },
        },
        Err(_) => tts_error_reply(request.request_id, TtsErrorCode::SynthesisFailed),
    }
}

fn tts_error_reply(request_id: u64, code: TtsErrorCode) -> TtsReply {
    TtsReply {
        protocol_version: TTS_PROTOCOL_VERSION,
        request_id,
        outcome: TtsOutcome::Error { code },
    }
}

#[must_use]
pub fn cache_key(text: &str, settings: VoiceSettings) -> String {
    let mut hash = Sha256::new();
    for field in [
        CACHE_FORMAT_VERSION.as_bytes(),
        KITTEN_MODEL_ID.as_bytes(),
        KITTEN_MODEL_SHA256.as_bytes(),
        KITTEN_VOICES_SHA256.as_bytes(),
        &[settings.speaker_id],
        &settings.speed.to_bits().to_le_bytes(),
        &settings.silence_scale.to_bits().to_le_bytes(),
        text.as_bytes(),
    ] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    format!("{:x}", hash.finalize())
}

fn backend_cache_key(text: &str, settings: VoiceSettings, backend_identity: &str) -> String {
    if backend_identity == KITTEN_MODEL_ID {
        return cache_key(text, settings);
    }
    let mut hash = Sha256::new();
    for field in [
        CACHE_FORMAT_VERSION.as_bytes(),
        backend_identity.as_bytes(),
        &[settings.speaker_id],
        &settings.speed.to_bits().to_le_bytes(),
        &settings.silence_scale.to_bits().to_le_bytes(),
        text.as_bytes(),
    ] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    format!("{:x}", hash.finalize())
}

fn validate_text(text: &str) -> Result<(), TtsError> {
    if text.trim().is_empty() {
        return Err(TtsError::EmptyText);
    }
    if text.len() > MAX_TTS_TEXT_BYTES {
        return Err(TtsError::TextTooLong);
    }
    if text.contains('\0') {
        return Err(TtsError::EmbeddedNul);
    }
    Ok(())
}

fn validate_audio(audio: &SynthesizedAudio) -> Result<(), TtsError> {
    if !(8_000..=192_000).contains(&audio.sample_rate)
        || audio.samples.is_empty()
        || audio.samples.iter().any(|x| !x.is_finite())
    {
        return Err(TtsError::InvalidAudio);
    }
    let maximum_samples = (audio.sample_rate as usize)
        .checked_mul(MAX_TTS_SECONDS)
        .ok_or(TtsError::OutputTooLong)?;
    if audio.samples.len() > maximum_samples {
        return Err(TtsError::OutputTooLong);
    }
    Ok(())
}

fn write_wav_atomic(path: &Path, audio: &SynthesizedAudio) -> Result<(), TtsError> {
    let sample_bytes = audio
        .samples
        .len()
        .checked_mul(2)
        .and_then(|size| u32::try_from(size).ok())
        .ok_or(TtsError::OutputTooLong)?;
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| TtsError::Io(io::Error::other("invalid cache filename")))?;
    let temporary = path.with_file_name(format!(
        ".{file_name}.tmp-{}-{sequence}",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = write_wav(&mut file, audio, sample_bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = result.and_then(|()| fs::rename(&temporary, path));
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    validate_cached_wav(path)
}

fn validate_cached_wav(path: &Path) -> Result<(), TtsError> {
    let metadata = fs::metadata(path)?;
    if !(44..=MAX_WAV_BYTES).contains(&metadata.len()) {
        return Err(TtsError::InvalidAudio);
    }
    let mut header = [0_u8; 44];
    File::open(path)?.read_exact(&mut header)?;
    let channels = u16::from_le_bytes([header[22], header[23]]);
    let audio_format = u16::from_le_bytes([header[20], header[21]]);
    let sample_rate = u32::from_le_bytes(header[24..28].try_into().unwrap());
    let bits = u16::from_le_bytes([header[34], header[35]]);
    let data_bytes = u32::from_le_bytes(header[40..44].try_into().unwrap());
    let fmt_chunk_size = u32::from_le_bytes(header[16..20].try_into().unwrap());
    let byte_rate = u32::from_le_bytes(header[28..32].try_into().unwrap());
    let block_align = u16::from_le_bytes([header[32], header[33]]);
    if &header[0..4] != b"RIFF"
        || &header[8..12] != b"WAVE"
        || &header[12..16] != b"fmt "
        || &header[36..40] != b"data"
        || u64::from(u32::from_le_bytes(header[4..8].try_into().unwrap())) + 8 != metadata.len()
        || audio_format != 1
        || fmt_chunk_size != 16
        || channels != 1
        || bits != 16
        || !(8_000..=192_000).contains(&sample_rate)
        || byte_rate != sample_rate * 2
        || block_align != 2
        || data_bytes == 0
        || data_bytes % 2 != 0
        || u64::from(data_bytes) + 44 != metadata.len()
        || u64::from(data_bytes) > u64::from(sample_rate) * MAX_TTS_SECONDS as u64 * 2
    {
        return Err(TtsError::InvalidAudio);
    }
    Ok(())
}

/// Offline TTS through a separately installed eSpeak NG executable.
///
/// Beastie exchanges only bounded text and a temporary WAV with this process.
/// The executable and its data remain separately distributed GPLv3 components;
/// no eSpeak code is linked into the MIT-licensed Rust binaries.
#[derive(Debug, Clone)]
pub struct EspeakNgSynthesizer {
    executable: PathBuf,
    voice: String,
    cache_identity: String,
    data_parent: Option<PathBuf>,
}

impl EspeakNgSynthesizer {
    pub fn new(
        executable: PathBuf,
        voice: String,
        data_dir: Option<PathBuf>,
    ) -> Result<Self, TtsError> {
        if voice.trim().is_empty() || voice.len() > 64 || voice.contains('\0') {
            return Err(TtsError::Backend(
                "eSpeak NG voice must contain 1..=64 UTF-8 bytes and no NUL".to_owned(),
            ));
        }
        let cache_identity = format!("espeak-ng-process-v1:{voice}");
        let data_parent = data_dir
            .map(|path| {
                if !path.is_dir() || path.file_name().is_none_or(|name| name != "espeak-ng-data") {
                    return Err(TtsError::Backend(format!(
                        "eSpeak NG data directory must exist and be named espeak-ng-data: {}",
                        path.display()
                    )));
                }
                path.parent().map(Path::to_path_buf).ok_or_else(|| {
                    TtsError::Backend("eSpeak NG data directory has no parent".to_owned())
                })
            })
            .transpose()?;
        Ok(Self {
            executable,
            voice,
            cache_identity,
            data_parent,
        })
    }

    fn output_path(&self) -> Result<(PathBuf, PathBuf), TtsError> {
        for _ in 0..16 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir()
                .join(format!("beastie-espeak-{}-{sequence}", std::process::id()));
            match fs::create_dir(&directory) {
                Ok(()) => return Ok((directory.join("speech.wav"), directory)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(TtsError::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not reserve an eSpeak NG temporary directory",
        )))
    }
}

impl TtsSynthesizer for EspeakNgSynthesizer {
    fn cache_identity(&self) -> &str {
        &self.cache_identity
    }

    fn synthesize(
        &mut self,
        text: &str,
        settings: VoiceSettings,
    ) -> Result<SynthesizedAudio, TtsError> {
        validate_text(text)?;
        let settings = settings.validate()?;
        let speaker_voice = if settings.speaker_id == 0 {
            self.voice.clone()
        } else {
            format!("{}+m{}", self.voice, settings.speaker_id)
        };
        let words_per_minute = (175.0 * settings.speed).round() as u16;
        let (output_path, temporary_dir) = self.output_path()?;
        let result = (|| {
            let mut command = Command::new(&self.executable);
            if let Some(parent) = &self.data_parent {
                let mut argument = std::ffi::OsString::from("--path=");
                argument.push(parent);
                command.arg(argument);
            }
            command
                .arg("--stdin")
                .arg("-w")
                .arg(&output_path)
                .arg("-v")
                .arg(&speaker_voice)
                .arg("-s")
                .arg(words_per_minute.to_string())
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let mut child = ContainedChild::spawn(&mut command, UnixProcessGroup::Inherit)
                .map_err(|error| {
                    TtsError::Backend(format!(
                        "could not start {}: {error}",
                        self.executable.display()
                    ))
                })?;
            let write_result = child
                .stdin
                .take()
                .ok_or_else(|| TtsError::Backend("eSpeak NG stdin was unavailable".to_owned()))
                .and_then(|mut stdin| {
                    stdin.write_all(text.as_bytes()).map_err(|error| {
                        TtsError::Backend(format!("could not send text to eSpeak NG: {error}"))
                    })
                });
            if let Err(error) = write_result {
                child.terminate_tree();
                return Err(error);
            }
            let status = child.wait().map_err(|error| {
                TtsError::Backend(format!("could not wait for eSpeak NG: {error}"))
            })?;
            child.close_descendants();
            if !status.success() {
                return Err(TtsError::Backend(format!("eSpeak NG exited with {status}")));
            }
            read_pcm_wav(&output_path)
        })();
        let _ = fs::remove_dir_all(temporary_dir);
        result
    }
}

fn read_pcm_wav(path: &Path) -> Result<SynthesizedAudio, TtsError> {
    validate_cached_wav(path)?;
    let mut file = File::open(path)?;
    let mut header = [0_u8; 44];
    file.read_exact(&mut header)?;
    let sample_rate = u32::from_le_bytes(header[24..28].try_into().unwrap());
    let data_bytes = u32::from_le_bytes(header[40..44].try_into().unwrap()) as usize;
    let mut bytes = vec![0_u8; data_bytes];
    file.read_exact(&mut bytes)?;
    let samples = bytes
        .chunks_exact(2)
        .map(|sample| f32::from(i16::from_le_bytes([sample[0], sample[1]])) / 32_768.0)
        .collect();
    Ok(SynthesizedAudio {
        samples,
        sample_rate,
    })
}

#[cfg(feature = "experimental-gpl-tts")]
fn verify_file_sha256(path: &Path, expected: &str) -> Result<(), TtsError> {
    let mut file = File::open(path).map_err(|error| {
        TtsError::Backend(format!(
            "cannot open pinned artifact {}: {error}",
            path.display()
        ))
    })?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            TtsError::Backend(format!(
                "cannot hash pinned artifact {}: {error}",
                path.display()
            ))
        })?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    if format!("{:x}", hash.finalize()) != expected {
        return Err(TtsError::Backend(format!(
            "pinned artifact checksum mismatch: {}",
            path.display()
        )));
    }
    Ok(())
}

fn write_wav(file: &mut File, audio: &SynthesizedAudio, sample_bytes: u32) -> io::Result<()> {
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + sample_bytes).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16_u32.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&audio.sample_rate.to_le_bytes())?;
    file.write_all(&(audio.sample_rate * 2).to_le_bytes())?;
    file.write_all(&2_u16.to_le_bytes())?;
    file.write_all(&16_u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&sample_bytes.to_le_bytes())?;
    for sample in &audio.samples {
        let pcm = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        file.write_all(&pcm.to_le_bytes())?;
    }
    file.flush()
}

#[cfg(feature = "experimental-gpl-tts")]
mod sherpa {
    use std::path::Path;

    use sherpa_onnx::{
        GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKittenModelConfig,
        OfflineTtsModelConfig,
    };

    use super::{
        KITTEN_MODEL_SHA256, KITTEN_VOICES_SHA256, SynthesizedAudio, TtsError, TtsSynthesizer,
        VoiceSettings, verify_file_sha256,
    };

    pub struct SherpaKittenSynthesizer {
        engine: OfflineTts,
    }

    impl SherpaKittenSynthesizer {
        pub fn load(model_dir: &Path, threads: i32) -> Result<Self, TtsError> {
            if threads < 1 {
                return Err(TtsError::Backend(
                    "thread count must be positive".to_owned(),
                ));
            }
            verify_file_sha256(&model_dir.join("model.int8.onnx"), KITTEN_MODEL_SHA256)?;
            verify_file_sha256(&model_dir.join("voices.bin"), KITTEN_VOICES_SHA256)?;
            let path = |relative: &str| -> Result<String, TtsError> {
                let value = model_dir.join(relative);
                if !value.exists() {
                    return Err(TtsError::Backend(format!(
                        "required model path does not exist: {}",
                        value.display()
                    )));
                }
                value
                    .to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| TtsError::Backend("model path is not valid UTF-8".to_owned()))
            };
            let config = OfflineTtsConfig {
                model: OfflineTtsModelConfig {
                    kitten: OfflineTtsKittenModelConfig {
                        model: Some(path("model.int8.onnx")?),
                        voices: Some(path("voices.bin")?),
                        tokens: Some(path("tokens.txt")?),
                        data_dir: Some(path("espeak-ng-data")?),
                        length_scale: 1.0,
                    },
                    num_threads: threads,
                    provider: Some("cpu".to_owned()),
                    ..Default::default()
                },
                max_num_sentences: 1,
                silence_scale: 0.2,
                ..Default::default()
            };
            OfflineTts::create(&config)
                .map(|engine| Self { engine })
                .ok_or_else(|| {
                    TtsError::Backend("sherpa-onnx rejected the Kitten model".to_owned())
                })
        }
    }

    impl TtsSynthesizer for SherpaKittenSynthesizer {
        fn cache_identity(&self) -> &str {
            super::KITTEN_MODEL_ID
        }

        fn synthesize(
            &mut self,
            text: &str,
            settings: VoiceSettings,
        ) -> Result<SynthesizedAudio, TtsError> {
            // Validate before sherpa's CString conversion, which otherwise panics on NUL.
            super::validate_text(text)?;
            let settings = settings.validate()?;
            let config = GenerationConfig {
                sid: i32::from(settings.speaker_id),
                speed: settings.speed,
                silence_scale: settings.silence_scale,
                ..Default::default()
            };
            let generated = self
                .engine
                .generate_with_config(text, &config, None::<fn(&[f32], f32) -> bool>)
                .ok_or_else(|| TtsError::Backend("sherpa-onnx returned no audio".to_owned()))?;
            let sample_rate =
                u32::try_from(generated.sample_rate()).map_err(|_| TtsError::InvalidAudio)?;
            if !(8_000..=192_000).contains(&sample_rate)
                || generated.samples().is_empty()
                || generated.samples().iter().any(|sample| !sample.is_finite())
            {
                return Err(TtsError::InvalidAudio);
            }
            if generated.samples().len() > sample_rate as usize * super::MAX_TTS_SECONDS {
                return Err(TtsError::OutputTooLong);
            }
            // Copy while the native GeneratedAudio handle still owns the sample buffer.
            let samples = generated.samples().to_vec();
            Ok(SynthesizedAudio {
                samples,
                sample_rate,
            })
        }
    }
}

#[cfg(feature = "experimental-gpl-tts")]
pub use sherpa::SherpaKittenSynthesizer;
