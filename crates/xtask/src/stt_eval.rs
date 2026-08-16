use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use beastie_protocol::{
    RecognitionLanguage, RecognitionOutcome, RecognitionReply, RecognitionRequest,
    STT_PROTOCOL_VERSION, validate_recognition_reply, validate_recognition_request,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CORPUS_PATH: &str = "evals/stt/corpus.json";
const FIXTURE_REPLIES_PATH: &str = "fixtures/stt/eval-replies.jsonl";
const MODEL_MANIFEST_PATH: &str = "models/manifest.toml";
const REPORT_DIR: &str = "evals/reports";
const MAX_REPLY_BYTES: usize = 4 * 1024;
const WORKER_REPLY_TIMEOUT: Duration = Duration::from_secs(30);
const WORKER_EXIT_TIMEOUT: Duration = Duration::from_secs(2);

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask must live two directories below the repository root")
        .join(relative)
}

#[derive(Debug)]
pub struct EvalOptions {
    pub worker: Option<PathBuf>,
    pub model_dir: Option<PathBuf>,
    pub moonshine_engine: Option<PathBuf>,
    pub worker_args: Vec<String>,
    pub label: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    version: u32,
    audio_format: AudioFormat,
    provenance: String,
    cases: Vec<EvalCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AudioFormat {
    container: String,
    encoding: String,
    sample_rate_hz: u32,
    channels: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExpectedOutcome {
    Speech,
    NoSpeech,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvalCase {
    id: String,
    audio: String,
    reference: String,
    keywords: Vec<String>,
    expected: ExpectedOutcome,
    voice: String,
    condition: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureResult {
    case_id: String,
    latency_ms: u64,
    reply: RecognitionReply,
}

#[derive(Debug, Serialize)]
struct EvalReport {
    corpus_version: u32,
    source: String,
    corpus_provenance: String,
    synthetic_only: bool,
    summary: Summary,
    cases: Vec<CaseResult>,
}

#[derive(Debug, Serialize)]
struct Summary {
    cases: usize,
    passed: usize,
    expected_speech: usize,
    expected_no_speech: usize,
    exact_transcripts: usize,
    no_speech_correct: usize,
    classification_correct: usize,
    reference_words: usize,
    word_errors: usize,
    normalized_wer: f64,
    keywords: usize,
    keywords_recalled: usize,
    keyword_recall: f64,
    cold_latency_ms: u64,
    warm_median_latency_ms: u64,
    peak_rss_bytes: Option<u64>,
    confidence_calibration: Vec<ConfidenceBucket>,
}

#[derive(Debug, Serialize)]
struct ConfidenceBucket {
    range: String,
    cases: usize,
    exact: usize,
    exact_rate: f64,
    mean_confidence: u16,
}

#[derive(Debug, Serialize)]
struct CaseResult {
    id: String,
    voice: String,
    condition: String,
    reference: String,
    transcript: Option<String>,
    outcome: String,
    confidence: Option<u16>,
    normalized_reference: String,
    normalized_transcript: String,
    word_errors: usize,
    reference_words: usize,
    wer: f64,
    keywords: usize,
    keywords_recalled: usize,
    exact: bool,
    classification_correct: bool,
    passed: bool,
    latency_ms: u64,
    peak_rss_bytes: Option<u64>,
}

#[derive(Debug)]
struct Observation {
    reply: RecognitionReply,
    latency_ms: u64,
    peak_rss_bytes: Option<u64>,
}

pub fn verify_fixtures() -> Result<()> {
    validate_selection_manifest()?;
    let corpus = load_corpus()?;
    let fixtures = load_fixture_results()?;
    validate_fixture_identity(&corpus, &fixtures)?;
    let report = score(
        &corpus,
        "checked-in deterministic replies",
        fixtures
            .into_iter()
            .map(|fixture| {
                (
                    fixture.case_id,
                    Observation {
                        reply: fixture.reply,
                        latency_ms: fixture.latency_ms,
                        peak_rss_bytes: None,
                    },
                )
            })
            .collect(),
    )?;
    ensure!(
        report.summary.passed == report.summary.cases,
        "STT fixture eval passed {} of {} cases",
        report.summary.passed,
        report.summary.cases
    );
    println!(
        "STT fixtures: {}/{} passed, WER {:.3}, keyword recall {:.3}",
        report.summary.passed,
        report.summary.cases,
        report.summary.normalized_wer,
        report.summary.keyword_recall
    );
    Ok(())
}

pub fn run(options: EvalOptions) -> Result<()> {
    let corpus = load_corpus()?;
    let Some(worker) = options.worker else {
        return verify_fixtures();
    };
    validate_selection_manifest()?;
    let model_dir = options
        .model_dir
        .context("--model-dir is required when --worker is supplied")?;
    let moonshine_engine = options
        .moonshine_engine
        .context("--moonshine-engine is required when --worker is supplied")?;
    let label = safe_label(&options.label)?;
    let audio_root = ScopedAudioRoot::create(&corpus)?;

    let mut command = Command::new(&worker);
    command
        .arg("--backend")
        .arg("moonshine")
        .arg("--audio-root")
        .arg(audio_root.path())
        .arg("--model-dir")
        .arg(&model_dir)
        .arg("--moonshine-engine")
        .arg(&moonshine_engine)
        .args(&options.worker_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    configure_process_containment(&mut command);
    let source = format!("{command:?}");
    let child = command
        .spawn()
        .with_context(|| format!("failed to start STT worker {}", worker.display()))?;
    let mut child = ChildGuard::new(child);
    let child_id = child.id();
    let mut stdin = child
        .child_mut()
        .stdin
        .take()
        .context("STT worker stdin unavailable")?;
    let stdout = child
        .child_mut()
        .stdout
        .take()
        .context("STT worker stdout unavailable")?;
    let replies = spawn_reply_reader(BufReader::new(stdout));
    let mut observations = BTreeMap::new();

    for (index, case) in corpus.cases.iter().enumerate() {
        let request = RecognitionRequest {
            protocol_version: STT_PROTOCOL_VERSION,
            request_id: u64::try_from(index + 1).expect("corpus length fits u64"),
            audio_key: case.sha256.clone(),
            language: RecognitionLanguage::English,
        };
        validate_recognition_request(&request).context("invalid generated STT request")?;
        let encoded = serde_json::to_string(&request).context("failed to encode STT request")?;
        let started = Instant::now();
        writeln!(stdin, "{encoded}").context("failed to write STT request")?;
        stdin.flush().context("failed to flush STT request")?;
        let reply_line = replies
            .recv_timeout(WORKER_REPLY_TIMEOUT)
            .with_context(|| format!("STT worker timed out before replying to {}", case.id))??;
        let latency_ms = duration_ms(started);
        let reply: RecognitionReply = serde_json::from_str(&reply_line)
            .with_context(|| format!("malformed STT reply for {}", case.id))?;
        let reply = validate_recognition_reply(&request, reply)
            .with_context(|| format!("invalid STT reply for {}", case.id))?;
        observations.insert(
            case.id.clone(),
            Observation {
                reply,
                latency_ms,
                peak_rss_bytes: process_rss_bytes(child_id),
            },
        );
    }
    drop(stdin);
    child.wait_timeout(WORKER_EXIT_TIMEOUT)?;

    let report = score(&corpus, &source, observations)?;
    let report_dir = repository_path(REPORT_DIR);
    fs::create_dir_all(&report_dir).context("failed to create STT report directory")?;
    let json_path = report_dir.join(format!("stt-{label}.json"));
    let markdown_path = report_dir.join(format!("stt-{label}.md"));
    let json = serde_json::to_string_pretty(&report).context("failed to encode STT report")?;
    fs::write(&json_path, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", json_path.display()))?;
    fs::write(&markdown_path, markdown(&report))
        .with_context(|| format!("failed to write {}", markdown_path.display()))?;
    println!(
        "STT eval: {}/{} passed, WER {:.3}, keyword recall {:.3}; reports: {}, {}",
        report.summary.passed,
        report.summary.cases,
        report.summary.normalized_wer,
        report.summary.keyword_recall,
        json_path.display(),
        markdown_path.display()
    );
    Ok(())
}

fn load_corpus() -> Result<Corpus> {
    let source = fs::read_to_string(repository_path(CORPUS_PATH))
        .with_context(|| format!("failed to read {CORPUS_PATH}"))?;
    let corpus: Corpus =
        serde_json::from_str(&source).with_context(|| format!("failed to parse {CORPUS_PATH}"))?;
    ensure!(corpus.version == 1, "unsupported STT corpus version");
    ensure!(!corpus.cases.is_empty(), "STT corpus is empty");
    ensure!(
        corpus.audio_format.container == "wav"
            && corpus.audio_format.encoding == "pcm_s16le"
            && corpus.audio_format.sample_rate_hz == 16_000
            && corpus.audio_format.channels == 1,
        "STT corpus audio format must be mono PCM16 WAV at 16 kHz"
    );
    let mut ids = BTreeSet::new();
    let mut voices = BTreeSet::new();
    let mut has_noise = false;
    let mut has_silence = false;
    for case in &corpus.cases {
        ensure!(!case.id.is_empty(), "STT case id is empty");
        ensure!(ids.insert(&case.id), "duplicate STT case id {}", case.id);
        ensure!(
            case.sha256.len() == 64
                && case
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "invalid SHA-256 for STT case {}",
            case.id
        );
        if case.voice != "none" {
            voices.insert(&case.voice);
        }
        has_noise |= case.condition.contains("noise");
        has_silence |= case.condition.contains("silence");
        match case.expected {
            ExpectedOutcome::Speech => {
                ensure!(
                    !case.reference.trim().is_empty(),
                    "speech case has no reference"
                );
                ensure!(!case.keywords.is_empty(), "speech case has no keywords");
            }
            ExpectedOutcome::NoSpeech => {
                ensure!(case.reference.is_empty(), "no-speech case has a reference");
                ensure!(case.keywords.is_empty(), "no-speech case has keywords");
            }
        }
        validate_audio(case)?;
    }
    ensure!(voices.len() >= 2, "STT corpus requires at least two voices");
    ensure!(
        has_noise && has_silence,
        "STT corpus requires noise and silence cases"
    );
    Ok(corpus)
}

fn validate_audio(case: &EvalCase) -> Result<()> {
    let path = repository_path(&case.audio);
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    ensure!(
        u64::try_from(bytes.len()).expect("usize fits u64") == case.bytes,
        "STT audio byte count changed for {}",
        case.id
    );
    let digest = format!("{:x}", Sha256::digest(&bytes));
    ensure!(
        digest == case.sha256,
        "STT audio SHA-256 changed for {}",
        case.id
    );
    validate_pcm16_wav(&bytes).with_context(|| format!("invalid WAV for {}", case.id))
}

fn validate_pcm16_wav(bytes: &[u8]) -> Result<()> {
    ensure!(bytes.len() >= 44, "WAV is shorter than its minimum header");
    ensure!(
        &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE",
        "not RIFF/WAVE"
    );
    let mut cursor = 12_usize;
    let mut format_seen = false;
    let mut data_seen = false;
    while cursor.saturating_add(8) <= bytes.len() {
        let id = &bytes[cursor..cursor + 4];
        let size = u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into()?) as usize;
        let start = cursor + 8;
        let end = start.checked_add(size).context("WAV chunk size overflow")?;
        ensure!(end <= bytes.len(), "WAV chunk exceeds file");
        if id == b"fmt " {
            ensure!(size >= 16, "WAV fmt chunk is too short");
            let audio_format = u16::from_le_bytes(bytes[start..start + 2].try_into()?);
            let channels = u16::from_le_bytes(bytes[start + 2..start + 4].try_into()?);
            let sample_rate = u32::from_le_bytes(bytes[start + 4..start + 8].try_into()?);
            let bits = u16::from_le_bytes(bytes[start + 14..start + 16].try_into()?);
            ensure!(audio_format == 1, "WAV is not integer PCM");
            ensure!(channels == 1, "WAV is not mono");
            ensure!(sample_rate == 16_000, "WAV is not 16 kHz");
            ensure!(bits == 16, "WAV is not PCM16");
            format_seen = true;
        } else if id == b"data" {
            ensure!(
                size > 0 && size.is_multiple_of(2),
                "WAV data is empty or not PCM16 aligned"
            );
            data_seen = true;
        }
        cursor = end + usize::from(!size.is_multiple_of(2));
    }
    ensure!(format_seen && data_seen, "WAV lacks fmt or data chunk");
    Ok(())
}

fn load_fixture_results() -> Result<Vec<FixtureResult>> {
    let path = repository_path(FIXTURE_REPLIES_PATH);
    let file = File::open(&path).with_context(|| format!("failed to read {}", path.display()))?;
    BufReader::new(file)
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let line =
                line.with_context(|| format!("failed to read fixture line {}", index + 1))?;
            serde_json::from_str(&line)
                .with_context(|| format!("failed to parse fixture line {}", index + 1))
        })
        .collect()
}

fn validate_fixture_identity(corpus: &Corpus, fixtures: &[FixtureResult]) -> Result<()> {
    ensure!(
        fixtures.len() == corpus.cases.len(),
        "STT fixture count does not match corpus"
    );
    let expected: BTreeSet<_> = corpus.cases.iter().map(|case| case.id.as_str()).collect();
    let actual: BTreeSet<_> = fixtures
        .iter()
        .map(|fixture| fixture.case_id.as_str())
        .collect();
    ensure!(
        actual.len() == fixtures.len(),
        "duplicate STT fixture case id"
    );
    ensure!(
        actual == expected,
        "STT fixture case ids do not match corpus"
    );
    Ok(())
}

fn score(
    corpus: &Corpus,
    source: &str,
    mut observations: BTreeMap<String, Observation>,
) -> Result<EvalReport> {
    let mut cases = Vec::with_capacity(corpus.cases.len());
    for (index, case) in corpus.cases.iter().enumerate() {
        let observation = observations
            .remove(&case.id)
            .with_context(|| format!("missing STT result for {}", case.id))?;
        let request = RecognitionRequest {
            protocol_version: STT_PROTOCOL_VERSION,
            request_id: u64::try_from(index + 1).expect("corpus length fits u64"),
            audio_key: case.sha256.clone(),
            language: RecognitionLanguage::English,
        };
        let Observation {
            reply,
            latency_ms,
            peak_rss_bytes,
        } = observation;
        let reply = validate_recognition_reply(&request, reply)
            .with_context(|| format!("invalid STT result for {}", case.id))?;
        cases.push(score_case(case, reply, latency_ms, peak_rss_bytes));
    }
    ensure!(
        observations.is_empty(),
        "STT results contain unknown case ids"
    );

    let expected_speech = corpus
        .cases
        .iter()
        .filter(|case| case.expected == ExpectedOutcome::Speech)
        .count();
    let expected_no_speech = corpus.cases.len() - expected_speech;
    let reference_words = cases.iter().map(|case| case.reference_words).sum();
    let word_errors = cases.iter().map(|case| case.word_errors).sum();
    let keywords = cases.iter().map(|case| case.keywords).sum();
    let keywords_recalled = cases.iter().map(|case| case.keywords_recalled).sum();
    let cold_latency_ms = cases.first().map_or(0, |case| case.latency_ms);
    let warm_median_latency_ms = median(cases.iter().skip(1).map(|case| case.latency_ms).collect());
    let peak_rss_bytes = cases.iter().filter_map(|case| case.peak_rss_bytes).max();
    let summary = Summary {
        cases: cases.len(),
        passed: cases.iter().filter(|case| case.passed).count(),
        expected_speech,
        expected_no_speech,
        exact_transcripts: cases
            .iter()
            .filter(|case| case.exact && !case.reference.is_empty())
            .count(),
        no_speech_correct: cases
            .iter()
            .filter(|case| case.reference.is_empty() && case.classification_correct)
            .count(),
        classification_correct: cases
            .iter()
            .filter(|case| case.classification_correct)
            .count(),
        reference_words,
        word_errors,
        normalized_wer: ratio(word_errors, reference_words),
        keywords,
        keywords_recalled,
        keyword_recall: ratio(keywords_recalled, keywords),
        cold_latency_ms,
        warm_median_latency_ms,
        peak_rss_bytes,
        confidence_calibration: confidence_buckets(&cases),
    };
    Ok(EvalReport {
        corpus_version: corpus.version,
        source: source.to_owned(),
        corpus_provenance: corpus.provenance.clone(),
        synthetic_only: true,
        summary,
        cases,
    })
}

fn score_case(
    case: &EvalCase,
    reply: RecognitionReply,
    latency_ms: u64,
    peak_rss_bytes: Option<u64>,
) -> CaseResult {
    let normalized_reference = normalize(&case.reference);
    let reference_tokens = tokens(&case.reference);
    let (outcome, transcript, confidence) = match reply.outcome {
        RecognitionOutcome::Recognized { text, confidence } => (
            "recognized".to_owned(),
            Some(text),
            Some(confidence.parts_per_thousand()),
        ),
        RecognitionOutcome::NoSpeech {} => ("no_speech".to_owned(), None, None),
        RecognitionOutcome::Error { code } => (format!("error:{code:?}"), None, None),
    };
    let normalized_transcript = transcript.as_deref().map(normalize).unwrap_or_default();
    let transcript_tokens = transcript.as_deref().map(tokens).unwrap_or_default();
    let word_errors = edit_distance(&reference_tokens, &transcript_tokens);
    let reference_words = reference_tokens.len();
    let exact = normalized_reference == normalized_transcript;
    let classification_correct = matches!(
        (case.expected, transcript.as_ref(), outcome.as_str()),
        (ExpectedOutcome::Speech, Some(_), "recognized")
            | (ExpectedOutcome::NoSpeech, None, "no_speech")
    );
    let keywords_recalled = case
        .keywords
        .iter()
        .filter(|keyword| contains_phrase(&transcript_tokens, &tokens(keyword)))
        .count();
    let passed = classification_correct
        && match case.expected {
            ExpectedOutcome::Speech => exact && keywords_recalled == case.keywords.len(),
            ExpectedOutcome::NoSpeech => true,
        };
    CaseResult {
        id: case.id.clone(),
        voice: case.voice.clone(),
        condition: case.condition.clone(),
        reference: case.reference.clone(),
        transcript,
        outcome,
        confidence,
        normalized_reference,
        normalized_transcript,
        word_errors,
        reference_words,
        wer: ratio(word_errors, reference_words),
        keywords: case.keywords.len(),
        keywords_recalled,
        exact,
        classification_correct,
        passed,
        latency_ms,
        peak_rss_bytes,
    }
}

fn normalize(text: &str) -> String {
    tokens(text).join(" ")
}

fn tokens(text: &str) -> Vec<String> {
    let mut normalized = String::with_capacity(text.len());
    for character in text.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            normalized.push(character);
        } else if !matches!(character, '\'' | '’') {
            normalized.push(' ');
        }
    }
    normalized
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect()
}

fn edit_distance(reference: &[String], hypothesis: &[String]) -> usize {
    let mut previous: Vec<usize> = (0..=hypothesis.len()).collect();
    let mut current = vec![0; hypothesis.len() + 1];
    for (reference_index, reference_word) in reference.iter().enumerate() {
        current[0] = reference_index + 1;
        for (hypothesis_index, hypothesis_word) in hypothesis.iter().enumerate() {
            current[hypothesis_index + 1] = if reference_word == hypothesis_word {
                previous[hypothesis_index]
            } else {
                (previous[hypothesis_index] + 1)
                    .min(previous[hypothesis_index + 1] + 1)
                    .min(current[hypothesis_index] + 1)
            };
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[hypothesis.len()]
}

fn contains_phrase(haystack: &[String], needle: &[String]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn median(mut values: Vec<u64>) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    values[values.len() / 2]
}

fn confidence_buckets(cases: &[CaseResult]) -> Vec<ConfidenceBucket> {
    [
        (0_u16, 249_u16),
        (250, 499),
        (500, 649),
        (650, 799),
        (800, 1_000),
    ]
    .into_iter()
    .map(|(minimum, maximum)| {
        let members: Vec<_> = cases
            .iter()
            .filter(|case| {
                case.confidence
                    .is_some_and(|value| value >= minimum && value <= maximum)
            })
            .collect();
        let exact = members.iter().filter(|case| case.exact).count();
        let total_confidence: u64 = members
            .iter()
            .filter_map(|case| case.confidence.map(u64::from))
            .sum();
        ConfidenceBucket {
            range: format!("{minimum}-{maximum}"),
            cases: members.len(),
            exact,
            exact_rate: ratio(exact, members.len()),
            mean_confidence: if members.is_empty() {
                0
            } else {
                u16::try_from(total_confidence / members.len() as u64)
                    .expect("mean bounded by confidence maximum")
            },
        }
    })
    .collect()
}

fn spawn_reply_reader(reader: impl BufRead + Send + 'static) -> Receiver<io::Result<String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = reader;
        loop {
            let result = read_bounded_line(&mut reader, MAX_REPLY_BYTES).and_then(|line| {
                line.ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "worker exited"))
            });
            let finished = result.is_err();
            if sender.send(result).is_err() || finished {
                break;
            }
        }
    });
    receiver
}

fn read_bounded_line(reader: &mut impl BufRead, maximum: usize) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if bytes.is_empty() {
                return Ok(None);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "unterminated STT worker reply",
            ));
        }
        let consumed = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if bytes.len().saturating_add(consumed) > maximum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "STT worker reply exceeds byte limit",
            ));
        }
        bytes.extend_from_slice(&available[..consumed]);
        reader.consume(consumed);
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
            return String::from_utf8(bytes)
                .map(Some)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
        }
    }
}

fn duration_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn safe_label(label: &str) -> Result<&str> {
    ensure!(
        !label.is_empty()
            && label.len() <= 64
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "STT report label must contain only letters, numbers, '-' or '_'"
    );
    Ok(label)
}

fn markdown(report: &EvalReport) -> String {
    let summary = &report.summary;
    let rss = summary
        .peak_rss_bytes
        .map(|bytes| format!("{} MiB", bytes / 1024 / 1024))
        .unwrap_or_else(|| "unavailable on this host".to_owned());
    let mut output = format!(
        "# STT evaluation: {}\n\nSynthetic fixtures only: **yes**. This report is not real-human acceptance evidence.\n\n- Passed: {}/{}\n- Normalized WER: {:.3} ({} errors / {} reference words)\n- Keyword recall: {:.3} ({}/{})\n- Exact transcripts: {}/{} speech cases\n- No-speech: {}/{} correct\n- Cold latency: {} ms\n- Warm median latency: {} ms\n- Peak process-tree RSS: {}\n\n## Cases\n\n| Case | Condition | Outcome | WER | Keywords | Confidence | Latency |\n|---|---|---:|---:|---:|---:|---:|\n",
        report.source,
        summary.passed,
        summary.cases,
        summary.normalized_wer,
        summary.word_errors,
        summary.reference_words,
        summary.keyword_recall,
        summary.keywords_recalled,
        summary.keywords,
        summary.exact_transcripts,
        summary.expected_speech,
        summary.no_speech_correct,
        summary.expected_no_speech,
        summary.cold_latency_ms,
        summary.warm_median_latency_ms,
        rss,
    );
    for case in &report.cases {
        let confidence = case
            .confidence
            .map_or_else(|| "n/a".to_owned(), |value| value.to_string());
        output.push_str(&format!(
            "| {} | {} | {} | {:.3} | {}/{} | {} | {} ms |\n",
            case.id,
            case.condition,
            case.outcome,
            case.wer,
            case.keywords_recalled,
            case.keywords,
            confidence,
            case.latency_ms
        ));
    }
    output.push_str("\n## Confidence calibration\n\n| Confidence | Cases | Exact | Exact rate | Mean confidence |\n|---|---:|---:|---:|---:|\n");
    for bucket in &summary.confidence_calibration {
        output.push_str(&format!(
            "| {} | {} | {} | {:.3} | {} |\n",
            bucket.range, bucket.cases, bucket.exact, bucket.exact_rate, bucket.mean_confidence
        ));
    }
    output
}

struct ScopedAudioRoot {
    path: PathBuf,
}

impl ScopedAudioRoot {
    fn create(corpus: &Corpus) -> Result<Self> {
        let path = repository_path("target").join(format!("stt-eval-audio-{}", std::process::id()));
        if path.exists() {
            fs::remove_dir_all(&path).context("failed to clear stale scoped STT audio root")?;
        }
        fs::create_dir_all(&path).context("failed to create scoped STT audio root")?;
        for case in &corpus.cases {
            let destination = path.join(format!("{}.wav", case.sha256));
            fs::copy(repository_path(&case.audio), &destination)
                .with_context(|| format!("failed to stage STT audio case {}", case.id))?;
        }
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScopedAudioRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn id(&self) -> u32 {
        self.child.as_ref().expect("worker is present").id()
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("worker is present")
    }

    fn wait_timeout(&mut self, timeout: Duration) -> Result<()> {
        let started = Instant::now();
        loop {
            if let Some(status) = self.child_mut().try_wait()? {
                self.child.take();
                ensure!(status.success(), "STT worker exited with {status}");
                return Ok(());
            }
            if started.elapsed() >= timeout {
                bail!("STT worker did not exit after stdin closed");
            }
            thread::sleep(Duration::from_millis(2));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            terminate_child(&mut child);
        }
    }
}

fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    if let Ok(process_group) = i32::try_from(child.id()) {
        unsafe {
            libc::kill(-process_group, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn configure_process_containment(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
}

#[cfg(target_os = "macos")]
fn process_rss_bytes(process_id: u32) -> Option<u64> {
    let output = Command::new("ps")
        .args(["-axo", "pid=,ppid=,rss="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let rows: Vec<(u32, u32, u64)> = String::from_utf8(output.stdout)
        .ok()?
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some((
                fields.next()?.parse().ok()?,
                fields.next()?.parse().ok()?,
                fields.next()?.parse().ok()?,
            ))
        })
        .collect();
    let mut process_tree = BTreeSet::from([process_id]);
    loop {
        let previous = process_tree.len();
        for (pid, parent, _) in &rows {
            if process_tree.contains(parent) {
                process_tree.insert(*pid);
            }
        }
        if process_tree.len() == previous {
            break;
        }
    }
    let kibibytes = rows
        .iter()
        .filter(|(pid, _, _)| process_tree.contains(pid))
        .map(|(_, _, rss)| *rss)
        .sum::<u64>();
    (kibibytes > 0).then(|| kibibytes.saturating_mul(1024))
}

#[cfg(not(target_os = "macos"))]
fn process_rss_bytes(_process_id: u32) -> Option<u64> {
    None
}

#[derive(Debug, Deserialize)]
struct ModelManifest {
    selection: SttSelectionContainer,
    stt_candidate: Vec<SttCandidate>,
}

#[derive(Debug, Deserialize)]
struct SttSelectionContainer {
    stt: SttSelection,
}

#[derive(Debug, Deserialize)]
struct SttSelection {
    status: String,
    preferred_candidate: String,
    requires_real_human_acceptance: bool,
    corpus: String,
    results: Vec<SttCandidateResult>,
}

#[derive(Debug, Deserialize)]
struct SttCandidateResult {
    candidate: String,
    runtime_revision: String,
    model_bytes: u64,
    license: String,
}

#[derive(Debug, Deserialize)]
struct SttCandidate {
    id: String,
    runtime_revision: String,
    bytes: u64,
    sha256: Option<String>,
    architecture: Option<u32>,
    components: Option<Vec<SttComponent>>,
}

#[derive(Debug, Deserialize)]
struct SttComponent {
    file: String,
    bytes: u64,
    sha256: String,
}

fn validate_selection_manifest() -> Result<()> {
    let source = fs::read_to_string(repository_path(MODEL_MANIFEST_PATH))
        .with_context(|| format!("failed to read {MODEL_MANIFEST_PATH}"))?;
    let manifest: ModelManifest = toml::from_str(&source)
        .with_context(|| format!("failed to parse STT selection in {MODEL_MANIFEST_PATH}"))?;
    let selection = manifest.selection.stt;
    ensure!(
        selection.status == "provisional",
        "STT selection must remain provisional"
    );
    ensure!(
        selection.preferred_candidate == "moonshine-tiny-streaming-en",
        "unexpected provisional STT candidate"
    );
    ensure!(
        selection.requires_real_human_acceptance,
        "STT selection must require real-human acceptance"
    );
    ensure!(
        selection.corpus == "evals/stt/corpus.json version 1",
        "STT selection references the wrong corpus"
    );
    let required = [
        "moonshine-tiny-streaming-en",
        "whisper-cpp-tiny-en-q5_1",
        "whisper-cpp-base-en-q5_1",
    ];
    let ids: BTreeSet<_> = selection
        .results
        .iter()
        .map(|result| result.candidate.as_str())
        .collect();
    ensure!(
        ids.len() == selection.results.len(),
        "duplicate STT selection result"
    );
    ensure!(
        required.iter().all(|id| ids.contains(id)),
        "STT selection comparison is incomplete"
    );
    for result in &selection.results {
        ensure!(
            !result.runtime_revision.is_empty(),
            "STT runtime revision is empty"
        );
        ensure!(result.model_bytes > 0, "STT model byte count is empty");
        ensure!(!result.license.is_empty(), "STT license is empty");
    }
    let candidates: BTreeMap<_, _> = manifest
        .stt_candidate
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect();
    ensure!(
        candidates.len() == manifest.stt_candidate.len(),
        "duplicate STT candidate metadata"
    );
    let moonshine = candidates
        .get("moonshine-tiny-streaming-en")
        .context("Moonshine STT candidate metadata is missing")?;
    ensure!(
        moonshine.runtime_revision == "07648e45e0b1daf1923ce325cd61d624a407e615"
            && moonshine.architecture == Some(2)
            && moonshine.bytes == 51_441_771,
        "Moonshine STT revision, architecture, or total bytes drifted"
    );
    let components = moonshine
        .components
        .as_ref()
        .context("Moonshine model components are missing")?;
    ensure!(
        components.len() == 7,
        "Moonshine model component set is incomplete"
    );
    ensure!(
        components
            .iter()
            .map(|component| component.bytes)
            .sum::<u64>()
            == moonshine.bytes,
        "Moonshine model component byte total drifted"
    );
    ensure!(
        components.iter().all(|component| {
            !component.file.is_empty()
                && component.sha256.len() == 64
                && component
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }),
        "Moonshine model component identity is incomplete"
    );
    for (id, bytes, sha256) in [
        (
            "whisper-cpp-tiny-en-q5_1",
            32_166_155,
            "c77c5766f1cef09b6b7d47f21b546cbddd4157886b3b5d6d4f709e91e66c7c2b",
        ),
        (
            "whisper-cpp-base-en-q5_1",
            59_721_011,
            "4baf70dd0d7c4247ba2b81fafd9c01005ac77c2f9ef064e00dcf195d0e2fdd2f",
        ),
    ] {
        let candidate = candidates
            .get(id)
            .with_context(|| format!("{id} candidate metadata is missing"))?;
        ensure!(
            candidate.runtime_revision == "306c88f4d1286aec1bf96e544632897886af5501"
                && candidate.bytes == bytes
                && candidate.sha256.as_deref() == Some(sha256),
            "{id} candidate metadata drifted"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_is_case_and_punctuation_insensitive() {
        assert_eq!(normalize("Muck?  You're HERE!"), "muck youre here");
        assert_eq!(normalize("aquarium\n yesterday"), "aquarium yesterday");
    }

    #[test]
    fn word_error_rate_counts_insertions_deletions_and_substitutions() {
        let reference = tokens("muck likes the berry");
        assert_eq!(edit_distance(&reference, &tokens("muck likes berry")), 1);
        assert_eq!(
            edit_distance(&reference, &tokens("muck hates the berry now")),
            2
        );
        assert_eq!(edit_distance(&reference, &reference), 0);
    }

    #[test]
    fn checked_in_corpus_and_replies_are_complete_and_immutable() {
        let corpus = load_corpus().expect("checked-in corpus is valid");
        let fixtures = load_fixture_results().expect("checked-in replies are valid");
        validate_fixture_identity(&corpus, &fixtures).expect("fixture identity matches corpus");
        assert!(
            corpus
                .cases
                .iter()
                .any(|case| case.condition.contains("noise"))
        );
        assert!(
            corpus
                .cases
                .iter()
                .any(|case| case.condition.contains("silence"))
        );
    }

    #[test]
    fn selection_schema_keeps_acceptance_provisional() {
        validate_selection_manifest().expect("checked-in STT selection is strict and complete");
    }

    #[test]
    fn worker_reply_reader_is_bounded_and_accepts_crlf() {
        let mut valid = BufReader::new(&b"reply\r\n"[..]);
        assert_eq!(
            read_bounded_line(&mut valid, 16).expect("valid line"),
            Some("reply".to_owned())
        );
        let mut oversized = BufReader::new(&b"too long\n"[..]);
        assert!(read_bounded_line(&mut oversized, 4).is_err());
    }
}
