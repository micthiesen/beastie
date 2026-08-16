use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TARGET_SAMPLE_RATE: u32 = 16_000;
const MAX_CAPTURE_SECONDS: usize = 30;

#[derive(Debug, Error)]
pub enum MicrophoneError {
    #[error("no default microphone is available")]
    Unavailable,
    #[error("microphone input format is unsupported")]
    UnsupportedFormat,
    #[error("microphone could not start: {0}")]
    Start(String),
    #[error("captured audio could not be stored: {0}")]
    Storage(#[from] io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedAudio {
    pub key: String,
    pub path: PathBuf,
}

/// Owns the short-lived directory used to hand captured WAVs to the local recognizer.
///
/// Preparing a new session removes remnants from an interrupted prior run. Dropping the guard
/// removes the current session after the recognition manager has shut down.
#[derive(Debug)]
pub struct PrivateAudioRoot {
    path: PathBuf,
}

impl PrivateAudioRoot {
    pub fn prepare(path: PathBuf) -> io::Result<Self> {
        remove_audio_root(&path)?;
        fs::create_dir_all(&path)?;
        set_private_directory_permissions(&path)?;
        Ok(Self { path })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateAudioRoot {
    fn drop(&mut self) {
        let _ = remove_audio_root(&self.path);
    }
}

fn remove_audio_root(path: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(path)
    } else {
        fs::remove_dir_all(path)
    }
}

pub struct MicrophoneCapture {
    stream: Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    failed: Arc<AtomicBool>,
    sample_rate: u32,
}

impl MicrophoneCapture {
    pub fn start() -> Result<Self, MicrophoneError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(MicrophoneError::Unavailable)?;
        let supported = device
            .default_input_config()
            .map_err(|error| MicrophoneError::Start(error.to_string()))?;
        let sample_rate = supported.sample_rate();
        let channels = usize::from(supported.channels());
        if channels == 0 || sample_rate == 0 {
            return Err(MicrophoneError::UnsupportedFormat);
        }
        let maximum = usize::try_from(sample_rate)
            .unwrap_or(usize::MAX)
            .saturating_mul(MAX_CAPTURE_SECONDS);
        let samples = Arc::new(Mutex::new(Vec::with_capacity(maximum.min(1_000_000))));
        let config = supported.config();
        let failed = Arc::new(AtomicBool::new(false));
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_stream::<f32>(
                &device,
                &config,
                Arc::clone(&samples),
                channels,
                maximum,
                Arc::clone(&failed),
            ),
            SampleFormat::I16 => build_stream::<i16>(
                &device,
                &config,
                Arc::clone(&samples),
                channels,
                maximum,
                Arc::clone(&failed),
            ),
            SampleFormat::U16 => build_stream::<u16>(
                &device,
                &config,
                Arc::clone(&samples),
                channels,
                maximum,
                Arc::clone(&failed),
            ),
            _ => return Err(MicrophoneError::UnsupportedFormat),
        }
        .map_err(|error| MicrophoneError::Start(error.to_string()))?;
        stream
            .play()
            .map_err(|error| MicrophoneError::Start(error.to_string()))?;
        Ok(Self {
            stream,
            samples,
            failed,
            sample_rate,
        })
    }

    #[must_use]
    pub fn has_failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }

    pub fn finish(self, audio_root: &Path) -> Result<CapturedAudio, MicrophoneError> {
        drop(self.stream);
        if self.failed.load(Ordering::Acquire) {
            return Err(MicrophoneError::Start(
                "microphone disconnected during capture".to_owned(),
            ));
        }
        let source = self
            .samples
            .lock()
            .map_err(|_| MicrophoneError::Start("capture buffer was poisoned".to_owned()))?;
        let mut pcm = resample_pcm16(&source, self.sample_rate, TARGET_SAMPLE_RATE);
        // A tap released before the first device callback is still a valid no-speech capture.
        // The worker intentionally rejects empty WAV data, so represent it with one silent frame.
        if pcm.is_empty() {
            pcm.push(0);
        }
        let wav = pcm16_mono_wav(&pcm, TARGET_SAMPLE_RATE);
        store_content_addressed(audio_root, &wav).map_err(MicrophoneError::Storage)
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    channels: usize,
    maximum: usize,
    failed: Arc<AtomicBool>,
) -> Result<Stream, cpal::BuildStreamError>
where
    T: SizedSample + Sample,
    f32: FromSample<T>,
{
    device.build_input_stream(
        config,
        move |input: &[T], _| {
            let Ok(mut output) = samples.lock() else {
                return;
            };
            if output.len() >= maximum {
                return;
            }
            for frame in input.chunks_exact(channels) {
                let mono =
                    frame.iter().copied().map(f32::from_sample).sum::<f32>() / channels as f32;
                output.push(mono.clamp(-1.0, 1.0));
                if output.len() >= maximum {
                    break;
                }
            }
        },
        move |_error| failed.store(true, Ordering::Release),
        None,
    )
}

fn resample_pcm16(source: &[f32], source_rate: u32, target_rate: u32) -> Vec<i16> {
    if source.is_empty() || source_rate == 0 || target_rate == 0 {
        return Vec::new();
    }
    let output_len = source
        .len()
        .saturating_mul(target_rate as usize)
        .div_ceil(source_rate as usize)
        .min(target_rate as usize * MAX_CAPTURE_SECONDS);
    (0..output_len)
        .map(|index| {
            let numerator = index as u64 * u64::from(source_rate);
            let base = usize::try_from(numerator / u64::from(target_rate)).unwrap_or(source.len());
            let remainder = (numerator % u64::from(target_rate)) as f32 / target_rate as f32;
            let first = source.get(base).copied().unwrap_or(0.0);
            let second = source.get(base + 1).copied().unwrap_or(first);
            let sample = first + (second - first) * remainder;
            (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
        })
        .collect()
}

fn pcm16_mono_wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_bytes = u32::try_from(samples.len().saturating_mul(2)).unwrap_or(u32::MAX - 1);
    let mut wav = Vec::with_capacity(44 + data_bytes as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36_u32.saturating_add(data_bytes)).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&sample_rate.saturating_mul(2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

fn store_content_addressed(root: &Path, wav: &[u8]) -> io::Result<CapturedAudio> {
    fs::create_dir_all(root)?;
    set_private_directory_permissions(root)?;
    let key = format!("{:x}", Sha256::digest(wav));
    let path = root.join(format!("{key}.wav"));
    if !path.exists() {
        let temporary = root.join(format!(".{key}-{}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(wav)?;
        file.sync_all()?;
        drop(file);
        match fs::rename(&temporary, &path) {
            Ok(()) => {}
            Err(error) if path.exists() => {
                let _ = fs::remove_file(&temporary);
                if fs::read(&path)? != wav {
                    return Err(error);
                }
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                return Err(error);
            }
        }
    }
    Ok(CapturedAudio { key, path })
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "beastie-microphone-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn resampling_is_bounded_and_interpolates() {
        let source = [0.0, 1.0, 0.0];
        let output = resample_pcm16(&source, 2, 4);
        assert_eq!(output.len(), 6);
        assert_eq!(output[0], 0);
        assert!((16_300..=16_500).contains(&output[1]));
        let overlong = vec![0.25; 40 * 48_000];
        assert_eq!(resample_pcm16(&overlong, 48_000, 16_000).len(), 30 * 16_000);
    }

    #[test]
    fn private_audio_root_removes_interrupted_and_current_capture_files() {
        let root = temporary_root();
        fs::create_dir_all(&root).expect("stale root");
        fs::write(root.join("stale.wav"), b"private speech").expect("stale capture");
        fs::write(root.join(".stale.tmp"), b"partial speech").expect("stale temporary");

        {
            let guard = PrivateAudioRoot::prepare(root.clone()).expect("prepare private root");
            assert!(!guard.path().join("stale.wav").exists());
            assert!(!guard.path().join(".stale.tmp").exists());
            fs::write(guard.path().join("current.wav"), b"current speech")
                .expect("current capture");
        }

        assert!(!root.exists());
    }

    #[test]
    fn wav_is_atomic_content_addressed_and_private() {
        let root = temporary_root();
        let wav = pcm16_mono_wav(&[0, i16::MAX, i16::MIN], TARGET_SAMPLE_RATE);
        let stored = store_content_addressed(&root, &wav).expect("store wav");
        assert_eq!(fs::read(&stored.path).expect("read wav"), wav);
        assert_eq!(stored.key, format!("{:x}", Sha256::digest(&wav)));
        assert_eq!(store_content_addressed(&root, &wav).unwrap(), stored);
        assert!(fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
        fs::remove_dir_all(root).expect("cleanup");
    }
}
