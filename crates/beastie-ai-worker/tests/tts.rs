use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use beastie_ai_worker::tts::{
    EspeakNgSynthesizer, KITTEN_MODEL_ID, KITTEN_MODEL_SHA256, KITTEN_VOICES_SHA256,
    MAX_TTS_LINE_BYTES, MAX_TTS_TEXT_BYTES, SynthesizedAudio, TtsError, TtsSynthesizer,
    VoiceSettings, cache_key, run_tts_jsonl, synthesize_to_cache,
};
use beastie_protocol::{TTS_PROTOCOL_VERSION, TtsOutcome, TtsReply, TtsRequest, TtsVoiceSettings};
use sha2::{Digest, Sha256};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct FakeSynthesizer {
    calls: usize,
}

impl TtsSynthesizer for FakeSynthesizer {
    fn cache_identity(&self) -> &str {
        KITTEN_MODEL_ID
    }

    fn synthesize(
        &mut self,
        _text: &str,
        _settings: VoiceSettings,
    ) -> Result<SynthesizedAudio, TtsError> {
        self.calls += 1;
        Ok(SynthesizedAudio {
            samples: vec![-1.0, -0.5, 0.0, 0.5, 1.0],
            sample_rate: 24_000,
        })
    }
}

fn temporary_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "beastie-tts-{}-{nonce}-{sequence}",
        std::process::id()
    ))
}

#[test]
fn fake_synthesis_writes_pcm_wav_once_and_reuses_hash_cache() {
    let cache_dir = temporary_dir();
    let settings = VoiceSettings::default();
    let mut synthesizer = FakeSynthesizer { calls: 0 };

    let first = synthesize_to_cache(&cache_dir, "red shit again.", settings, &mut synthesizer)
        .expect("first synthesis should succeed");
    let second = synthesize_to_cache(&cache_dir, "red shit again.", settings, &mut synthesizer)
        .expect("cached synthesis should succeed");

    assert!(!first.cache_hit);
    assert!(second.cache_hit);
    assert_eq!(synthesizer.calls, 1);
    assert_eq!(first.path, second.path);
    assert_eq!(
        first.path.file_name().and_then(|name| name.to_str()),
        Some(format!("{}.wav", cache_key("red shit again.", settings)).as_str())
    );
    let bytes = fs::read(&first.path).expect("WAV should exist");
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(
        u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
        24_000
    );
    assert_eq!(bytes.len(), 44 + 10);

    fs::remove_dir_all(cache_dir).expect("temporary cache should be removable");
}

#[test]
fn invalid_inputs_are_rejected_before_calling_backend() {
    let cache_dir = temporary_dir();
    let mut synthesizer = FakeSynthesizer { calls: 0 };
    for (text, settings) in [
        ("", VoiceSettings::default()),
        ("bad\0text", VoiceSettings::default()),
        (
            "hello",
            VoiceSettings {
                speaker_id: 8,
                ..VoiceSettings::default()
            },
        ),
    ] {
        assert!(synthesize_to_cache(&cache_dir, text, settings, &mut synthesizer).is_err());
    }
    assert_eq!(synthesizer.calls, 0);
    assert!(!cache_dir.exists());
}

#[test]
fn cache_key_changes_with_text_and_every_voice_setting() {
    let defaults = VoiceSettings::default();
    let original = cache_key("hello", defaults);
    assert_ne!(original, cache_key("goodbye", defaults));
    assert_ne!(
        original,
        cache_key(
            "hello",
            VoiceSettings {
                speaker_id: 1,
                ..defaults
            }
        )
    );
    assert_ne!(
        original,
        cache_key(
            "hello",
            VoiceSettings {
                speed: 0.9,
                ..defaults
            }
        )
    );
    assert_ne!(
        original,
        cache_key(
            "hello",
            VoiceSettings {
                silence_scale: 0.1,
                ..defaults
            }
        )
    );
}

#[test]
fn cache_key_binds_both_pinned_voice_artifacts() {
    let settings = VoiceSettings::default();
    let mut expected = Sha256::new();
    for field in [
        b"beastie-tts-cache-v1".as_slice(),
        KITTEN_MODEL_ID.as_bytes(),
        KITTEN_MODEL_SHA256.as_bytes(),
        KITTEN_VOICES_SHA256.as_bytes(),
        &[settings.speaker_id],
        &settings.speed.to_bits().to_le_bytes(),
        &settings.silence_scale.to_bits().to_le_bytes(),
        b"hello".as_slice(),
    ] {
        expected.update((field.len() as u64).to_le_bytes());
        expected.update(field);
    }
    assert_eq!(
        cache_key("hello", settings),
        format!("{:x}", expected.finalize())
    );
}

#[test]
fn partial_cache_entry_is_replaced_before_it_can_be_returned() {
    let cache_dir = temporary_dir();
    fs::create_dir_all(&cache_dir).expect("cache should be created");
    let settings = VoiceSettings::default();
    let path = cache_dir.join(format!("{}.wav", cache_key("hello", settings)));
    fs::write(&path, b"RIFF partial").expect("partial entry should be written");
    let mut synthesizer = FakeSynthesizer { calls: 0 };

    let cached = synthesize_to_cache(&cache_dir, "hello", settings, &mut synthesizer)
        .expect("invalid cache should be regenerated");

    assert!(!cached.cache_hit);
    assert_eq!(synthesizer.calls, 1);
    assert_eq!(fs::read(&path).unwrap().len(), 54);
    fs::remove_dir_all(cache_dir).expect("temporary cache should be removable");
}

#[test]
fn versioned_jsonl_worker_stays_alive_and_never_returns_a_path() {
    let cache_dir = temporary_dir();
    let request = |request_id, text: &str| TtsRequest {
        protocol_version: TTS_PROTOCOL_VERSION,
        request_id,
        text: text.to_owned(),
        settings: TtsVoiceSettings::default(),
    };
    let input = [request(1, "hello"), request(2, "goodbye")]
        .into_iter()
        .map(|request| serde_json::to_string(&request).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let mut output = Vec::new();
    let mut synthesizer = FakeSynthesizer { calls: 0 };

    run_tts_jsonl(
        Cursor::new(input),
        &mut output,
        &cache_dir,
        &mut synthesizer,
    )
    .expect("worker stream should run");

    let output = String::from_utf8(output).unwrap();
    let replies = output
        .lines()
        .map(|line| serde_json::from_str::<TtsReply>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(synthesizer.calls, 2);
    assert_eq!(replies.len(), 2);
    assert!(matches!(replies[0].outcome, TtsOutcome::Ready { .. }));
    assert!(!output.contains(&cache_dir.display().to_string()));
    fs::remove_dir_all(cache_dir).expect("temporary cache should be removable");
}

#[test]
fn oversized_jsonl_request_is_rejected_and_next_request_runs() {
    let cache_dir = temporary_dir();
    let valid = serde_json::to_string(&TtsRequest {
        protocol_version: TTS_PROTOCOL_VERSION,
        request_id: 7,
        text: "hello".to_owned(),
        settings: TtsVoiceSettings::default(),
    })
    .unwrap();
    let input = format!("{}\n{valid}\n", "x".repeat(MAX_TTS_LINE_BYTES + 1));
    let mut output = Vec::new();
    let mut synthesizer = FakeSynthesizer { calls: 0 };
    run_tts_jsonl(
        Cursor::new(input),
        &mut output,
        &cache_dir,
        &mut synthesizer,
    )
    .unwrap();
    let output = String::from_utf8(output).unwrap();
    let replies = output
        .lines()
        .map(|line| serde_json::from_str::<TtsReply>(line).unwrap())
        .collect::<Vec<_>>();
    assert!(matches!(replies[0].outcome, TtsOutcome::Error { .. }));
    assert_eq!(replies[1].request_id, 7);
    assert_eq!(synthesizer.calls, 1);
    fs::remove_dir_all(cache_dir).expect("temporary cache should be removable");
}

#[test]
fn espeak_process_receives_bounded_stdin_and_produces_validated_pcm() {
    let (source, executable) = compile_fake_espeak("valid");
    let record = executable.with_extension("input");
    let arguments_record = executable.with_extension("args");
    let cache_dir = temporary_dir();
    let data_parent = temporary_dir();
    let data_dir = data_parent.join("espeak-ng-data");
    fs::create_dir_all(&data_dir).unwrap();
    let text = "x".repeat(MAX_TTS_TEXT_BYTES);
    let mut synthesizer =
        EspeakNgSynthesizer::new(executable.clone(), "en-us".to_owned(), Some(data_dir)).unwrap();

    let cached = synthesize_to_cache(
        &cache_dir,
        &text,
        VoiceSettings::default(),
        &mut synthesizer,
    )
    .expect("valid fake eSpeak output should be cached");

    assert_eq!(fs::read_to_string(&record).unwrap(), text);
    assert!(
        fs::read_to_string(&arguments_record)
            .unwrap()
            .lines()
            .any(|argument| argument == format!("--path={}", data_parent.display()))
    );
    assert_eq!(&fs::read(cached.path).unwrap()[0..12], b"RIFF&\0\0\0WAVE");
    fs::remove_dir_all(cache_dir).unwrap();
    fs::remove_file(record).unwrap();
    fs::remove_file(arguments_record).unwrap();
    fs::remove_dir_all(data_parent).unwrap();
    fs::remove_file(source).unwrap();
    fs::remove_file(executable).unwrap();
}

#[test]
fn oversized_text_never_reaches_espeak_process() {
    let (source, executable) = compile_fake_espeak("valid");
    let record = executable.with_extension("input");
    let cache_dir = temporary_dir();
    let mut synthesizer =
        EspeakNgSynthesizer::new(executable.clone(), "en-us".to_owned(), None).unwrap();

    let error = synthesize_to_cache(
        &cache_dir,
        &"x".repeat(MAX_TTS_TEXT_BYTES + 1),
        VoiceSettings::default(),
        &mut synthesizer,
    )
    .expect_err("oversized text should be rejected before process launch");

    assert!(matches!(error, TtsError::TextTooLong));
    assert!(!record.exists());
    assert!(!executable.with_extension("args").exists());
    fs::remove_file(source).unwrap();
    fs::remove_file(executable).unwrap();
}

#[test]
fn espeak_process_failure_and_invalid_output_fail_closed() {
    for mode in ["failure", "invalid", "oversized"] {
        let (source, executable) = compile_fake_espeak(mode);
        let cache_dir = temporary_dir();
        let mut synthesizer =
            EspeakNgSynthesizer::new(executable.clone(), "en-us".to_owned(), None).unwrap();

        assert!(
            synthesize_to_cache(
                &cache_dir,
                "hello",
                VoiceSettings::default(),
                &mut synthesizer
            )
            .is_err(),
            "{mode} output must not enter the cache"
        );
        assert_eq!(fs::read_dir(&cache_dir).unwrap().count(), 0);
        fs::remove_dir_all(cache_dir).unwrap();
        let record = executable.with_extension("input");
        if record.exists() {
            fs::remove_file(record).unwrap();
        }
        fs::remove_file(executable.with_extension("args")).unwrap();
        fs::remove_file(source).unwrap();
        fs::remove_file(executable).unwrap();
    }
}

fn compile_fake_espeak(mode: &str) -> (PathBuf, PathBuf) {
    let source = temporary_dir().with_extension("rs");
    let executable = temporary_dir().with_file_name(format!(
        "beastie-fake-espeak-{mode}-{}{}",
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        std::env::consts::EXE_SUFFIX
    ));
    fs::write(
        &source,
        r#"
use std::io::{self, Read};
use std::path::PathBuf;

fn main() {
    let executable = std::env::current_exe().unwrap();
    let mode = executable.file_stem().unwrap().to_string_lossy();
    let arguments = std::env::args_os().collect::<Vec<_>>();
    std::fs::write(executable.with_extension("args"), arguments.iter().skip(1).map(|value| value.to_string_lossy()).collect::<Vec<_>>().join("\n")).unwrap();
    let output = arguments.windows(2).find(|pair| pair[0] == "-w").map(|pair| PathBuf::from(&pair[1])).unwrap();
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    std::fs::write(executable.with_extension("input"), input).unwrap();
    if mode.contains("failure") { std::process::exit(7); }
    if mode.contains("invalid") { std::fs::write(output, b"not a wav").unwrap(); return; }
    let sample_rate = 22_050_u32;
    let samples = if mode.contains("oversized") { sample_rate as usize * 31 } else { 1 };
    let data_bytes = u32::try_from(samples * 2).unwrap();
    let mut wav = Vec::with_capacity(44 + data_bytes as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    wav.resize(44 + data_bytes as usize, 0);
    std::fs::write(output, wav).unwrap();
}
"#,
    )
    .unwrap();
    let status = Command::new("rustc")
        .args(["--edition=2024", "-o"])
        .arg(&executable)
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success());
    (source, executable)
}
