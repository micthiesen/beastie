use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use beastie_ai_worker::stt::{
    FixtureSttBackend, MAX_STT_LINE_BYTES, MoonshineBackend, MoonshineConfig, SttBackend,
    SttBackendError, ValidatedAudio, process_stt_line, run_stt_jsonl,
};
use beastie_protocol::{
    AcousticConfidence, RecognitionErrorCode, RecognitionLanguage, RecognitionOutcome,
    RecognitionReply, RecognitionRequest, STT_PROTOCOL_VERSION,
};
use sha2::{Digest, Sha256};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temporary_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "beastie-stt-{label}-{}-{nonce}-{sequence}",
        std::process::id()
    ))
}

fn wav(samples: &[i16], sample_rate: u32, channels: u16, bits: u16) -> Vec<u8> {
    let data_bytes = samples.len() as u32 * 2;
    let block_align = channels * (bits / 8);
    let mut bytes = Vec::with_capacity(44 + data_bytes as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * u32::from(block_align)).to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&bits.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

fn store_audio(root: &Path, bytes: &[u8]) -> String {
    fs::create_dir_all(root).expect("audio root should be created");
    let key = format!("{:x}", Sha256::digest(bytes));
    fs::write(root.join(format!("{key}.wav")), bytes).expect("audio should be written");
    key
}

fn request(request_id: u64, audio_key: String) -> RecognitionRequest {
    RecognitionRequest {
        protocol_version: STT_PROTOCOL_VERSION,
        request_id,
        audio_key,
        language: RecognitionLanguage::English,
    }
}

fn process(
    root: &Path,
    request: &RecognitionRequest,
    backend: &mut dyn SttBackend,
) -> RecognitionReply {
    process_stt_line(
        &serde_json::to_string(request).expect("request should serialize"),
        root,
        backend,
    )
}

#[test]
fn fixture_recognizes_signal_and_reports_silence_without_a_model() {
    let root = temporary_dir("fixture");
    let signal = store_audio(&root, &wav(&[0, 20, -20, 0], 16_000, 1, 16));
    let silence = store_audio(&root, &wav(&[0; 40], 16_000, 1, 16));
    let mut backend = FixtureSttBackend;

    let recognized = process(&root, &request(1, signal), &mut backend);
    let no_speech = process(&root, &request(2, silence), &mut backend);

    assert!(matches!(
        recognized.outcome,
        RecognitionOutcome::Recognized {
            ref text,
            confidence
        } if text == "hello beastie" && confidence == AcousticConfidence::new(900).unwrap()
    ));
    assert!(matches!(no_speech.outcome, RecognitionOutcome::NoSpeech {}));
    fs::remove_dir_all(root).expect("temporary directory should be removable");
}

#[test]
fn missing_mismatched_and_non_pcm_audio_have_distinct_typed_failures() {
    let root = temporary_dir("invalid-audio");
    fs::create_dir_all(&root).unwrap();
    let mut backend = FixtureSttBackend;

    let missing = process(&root, &request(1, "a".repeat(64)), &mut backend);
    assert!(matches!(
        missing.outcome,
        RecognitionOutcome::Error {
            code: RecognitionErrorCode::AudioUnavailable
        }
    ));

    let valid = wav(&[1, 2], 16_000, 1, 16);
    let key = format!("{:x}", Sha256::digest(&valid));
    fs::write(root.join(format!("{key}.wav")), b"tampered").unwrap();
    let mismatched = process(&root, &request(2, key), &mut backend);
    assert!(matches!(
        mismatched.outcome,
        RecognitionOutcome::Error {
            code: RecognitionErrorCode::InvalidAudio
        }
    ));

    for (id, invalid) in [
        (3, wav(&[1, 2], 48_000, 1, 16)),
        (4, wav(&[1, 2], 16_000, 2, 16)),
        (5, b"not a wav".to_vec()),
    ] {
        let key = store_audio(&root, &invalid);
        let reply = process(&root, &request(id, key), &mut backend);
        assert!(matches!(
            reply.outcome,
            RecognitionOutcome::Error {
                code: RecognitionErrorCode::InvalidAudio
            }
        ));
    }
    fs::remove_dir_all(root).expect("temporary directory should be removable");
}

#[cfg(unix)]
#[test]
fn content_addressed_audio_must_not_be_a_symlink() {
    use std::os::unix::fs::symlink;

    let root = temporary_dir("symlink");
    fs::create_dir_all(&root).unwrap();
    let bytes = wav(&[1, 2], 16_000, 1, 16);
    let target = root.join("target.wav");
    fs::write(&target, &bytes).unwrap();
    let key = format!("{:x}", Sha256::digest(&bytes));
    symlink(&target, root.join(format!("{key}.wav"))).unwrap();
    let reply = process(&root, &request(1, key), &mut FixtureSttBackend);
    assert!(matches!(
        reply.outcome,
        RecognitionOutcome::Error {
            code: RecognitionErrorCode::InvalidAudio
        }
    ));
    fs::remove_dir_all(root).expect("temporary directory should be removable");
}

struct InvalidTextBackend;

impl SttBackend for InvalidTextBackend {
    fn recognize(
        &mut self,
        _request: &RecognitionRequest,
        _audio: &ValidatedAudio,
    ) -> Result<RecognitionOutcome, SttBackendError> {
        Ok(RecognitionOutcome::Recognized {
            text: "bad\ntext".to_owned(),
            confidence: AcousticConfidence::new(700).unwrap(),
        })
    }
}

#[test]
fn malformed_and_oversized_lines_do_not_kill_the_persistent_worker() {
    let root = temporary_dir("recovery");
    let key = store_audio(&root, &wav(&[1, 2], 16_000, 1, 16));
    let valid = serde_json::to_string(&request(7, key)).unwrap();
    let input = format!(
        "{{bad json}}\n{}\n{valid}\n",
        "x".repeat(MAX_STT_LINE_BYTES + 1)
    );
    let mut output = Vec::new();
    run_stt_jsonl(
        Cursor::new(input),
        &mut output,
        &root,
        &mut FixtureSttBackend,
    )
    .unwrap();
    let replies = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<RecognitionReply>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(replies.len(), 3);
    assert!(matches!(
        replies[0].outcome,
        RecognitionOutcome::Error {
            code: RecognitionErrorCode::InvalidRequest
        }
    ));
    assert_eq!(replies[2].request_id, 7);

    let invalid_backend_reply = process(
        &root,
        &request(8, store_audio(&root, &wav(&[3], 16_000, 1, 16))),
        &mut InvalidTextBackend,
    );
    assert!(matches!(
        invalid_backend_reply.outcome,
        RecognitionOutcome::Error {
            code: RecognitionErrorCode::RecognitionFailed
        }
    ));
    fs::remove_dir_all(root).expect("temporary directory should be removable");
}

#[test]
fn absent_moonshine_engine_is_a_typed_technical_failure() {
    let root = temporary_dir("missing-engine");
    let key = store_audio(&root, &wav(&[1, 2], 16_000, 1, 16));
    let mut backend = MoonshineBackend::new(MoonshineConfig {
        executable: root.join("does-not-exist"),
        model_dir: root.join("model"),
        audio_root: root.clone(),
        timeout: Duration::from_millis(50),
    });
    let reply = process(&root, &request(1, key), &mut backend);
    assert!(matches!(
        reply.outcome,
        RecognitionOutcome::Error {
            code: RecognitionErrorCode::BackendUnavailable
        }
    ));
    fs::remove_dir_all(root).expect("temporary directory should be removable");
}

#[test]
fn moonshine_engine_is_persistent_and_replies_are_correlated() {
    let root = temporary_dir("persistent-engine");
    let model_dir = root.join("model");
    fs::create_dir_all(&model_dir).unwrap();
    let executable = compile_fake_moonshine_engine(&root);
    let first_key = store_audio(&root, &wav(&[1, 2], 16_000, 1, 16));
    let second_key = store_audio(&root, &wav(&[3, 4], 16_000, 1, 16));
    let mut backend = MoonshineBackend::new(MoonshineConfig {
        executable,
        model_dir: model_dir.clone(),
        audio_root: root.clone(),
        timeout: Duration::from_secs(2),
    });

    let first = process(&root, &request(41, first_key), &mut backend);
    let second = process(&root, &request(42, second_key), &mut backend);

    assert_eq!(first.request_id, 41);
    assert_eq!(second.request_id, 42);
    assert!(matches!(
        second.outcome,
        RecognitionOutcome::Recognized {
            ref text,
            confidence
        } if text == "engine words" && confidence == AcousticConfidence::new(777).unwrap()
    ));
    assert_eq!(fs::read_to_string(model_dir.join("starts")).unwrap(), "1");
    drop(backend);
    fs::remove_dir_all(root).expect("temporary directory should be removable");
}

fn compile_fake_moonshine_engine(root: &Path) -> PathBuf {
    let source = root.join("fake-moonshine.rs");
    let executable = root.join(if cfg!(windows) {
        "fake-moonshine.exe"
    } else {
        "fake-moonshine"
    });
    fs::write(
        &source,
        r#"
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

fn main() {
    let args = std::env::args_os().collect::<Vec<_>>();
    let model_index = args.iter().position(|arg| arg == "--model-dir").unwrap();
    let model_dir = PathBuf::from(&args[model_index + 1]);
    let mut starts = std::fs::OpenOptions::new().create(true).append(true).open(model_dir.join("starts")).unwrap();
    starts.write_all(b"1").unwrap();
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let marker = "\"request_id\":";
        let start = line.find(marker).unwrap() + marker.len();
        let id = line[start..].chars().take_while(char::is_ascii_digit).collect::<String>();
        writeln!(stdout, "{{\"protocol_version\":1,\"request_id\":{id},\"outcome\":{{\"status\":\"recognized\",\"text\":\"engine words\",\"confidence\":777}}}}").unwrap();
        stdout.flush().unwrap();
    }
}
"#,
    )
    .unwrap();
    let status = Command::new("rustc")
        .arg("--edition=2024")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .status()
        .expect("rustc should compile fake Moonshine engine");
    assert!(status.success());
    executable
}
