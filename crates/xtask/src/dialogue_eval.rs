use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use beastie_core::MemoryId;
use beastie_protocol::{
    DialogueReply, DialogueRequest, constrained_fallback_reply, validate_reply, validate_request,
};
use serde::{Deserialize, Serialize};

const CORPUS_PATH: &str = "evals/dialogue/corpus.json";
const FIXTURE_PATH: &str = "fixtures/dialogue/eval-replies.jsonl";
const REPORT_DIR: &str = "evals/reports";
const MAX_WORKER_REPLY_BYTES: usize = 16 * 1024;
const DEFAULT_WORKER_ATTEMPT_TIMEOUT_MS: u64 = 30_000;
const WORKER_WATCHDOG_GRACE_MS: u64 = 5_000;
const WORKER_EXIT_TIMEOUT: Duration = Duration::from_secs(1);

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
    pub model: Option<PathBuf>,
    pub llama_cli: Option<PathBuf>,
    pub timeout_ms: Option<u64>,
    pub max_output_bytes: Option<usize>,
    pub cpu_only: bool,
    pub llama_args: Vec<String>,
    pub label: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    version: u32,
    cases: Vec<EvalCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvalCase {
    id: String,
    category: String,
    request: DialogueRequest,
    expect: Expectations,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Expectations {
    recalled_memory: Option<MemoryId>,
    no_recalled_memory: bool,
    required_all_terms: Vec<String>,
    required_any_terms: Vec<String>,
    forbidden_terms: Vec<String>,
    permitted_sharpness: bool,
    prohibited_request: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureReply {
    case_id: String,
    reply: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct EvalReport {
    corpus_version: u32,
    source: String,
    summary: Summary,
    cases: Vec<CaseResult>,
}

#[derive(Debug, Serialize)]
struct Summary {
    cases: usize,
    passed: usize,
    protocol_valid: usize,
    grounded: usize,
    content_passed: usize,
    fallbacks: usize,
    generic_voice_hits: usize,
    permitted_refusals: usize,
    prohibited_escapes: usize,
    median_latency_ms: u64,
}

#[derive(Debug, Serialize)]
struct CaseResult {
    id: String,
    category: String,
    request: DialogueRequest,
    raw_reply: String,
    latency_ms: u64,
    word_count: Option<usize>,
    recalled_memory: Option<MemoryId>,
    protocol_valid: bool,
    grounded: bool,
    content_passed: bool,
    fallback: bool,
    generic_voice: bool,
    permitted_refusal: bool,
    prohibited_escape: bool,
    passed: bool,
    failures: Vec<String>,
}

pub fn verify_fixtures() -> Result<()> {
    let corpus = load_corpus()?;
    let raw_replies = load_fixtures()?;
    if raw_replies.len() != corpus.cases.len() {
        bail!(
            "dialogue fixture count {} does not match corpus count {}",
            raw_replies.len(),
            corpus.cases.len()
        );
    }
    let report = score(&corpus, "checked-in fixtures", |case| {
        raw_replies
            .get(&case.id)
            .cloned()
            .with_context(|| format!("missing fixture reply for {}", case.id))
            .map(|reply| (reply, 0))
    })?;
    if report.summary.passed != report.summary.cases {
        bail!(
            "dialogue fixture eval passed {} of {} cases",
            report.summary.passed,
            report.summary.cases
        );
    }
    println!(
        "dialogue fixtures: {}/{} cases passed",
        report.summary.passed, report.summary.cases
    );
    let no_model = score_no_model_voice(&corpus)?;
    if no_model.summary.passed != no_model.summary.cases {
        let failed = no_model
            .cases
            .iter()
            .filter(|case| !case.passed)
            .map(|case| format!("{} ({})", case.id, case.failures.join(", ")))
            .collect::<Vec<_>>();
        bail!("no-model voice failed: {}", failed.join("; "));
    }
    println!(
        "no-model voice: {}/{} learned-word cases passed",
        no_model.summary.passed, no_model.summary.cases
    );
    Ok(())
}

/// Scores the deterministic no-model voice on every learned-word case. It is what plays when
/// inference is absent, so it must meet the same expectations as a model reply.
fn score_no_model_voice(corpus: &Corpus) -> Result<EvalReport> {
    let speech = Corpus {
        version: corpus.version,
        cases: corpus
            .cases
            .iter()
            .filter(|case| case.request.speech_intent.is_some())
            .map(|case| EvalCase {
                id: case.id.clone(),
                category: case.category.clone(),
                request: case.request.clone(),
                expect: case.expect.clone(),
            })
            .collect(),
    };
    if speech.cases.is_empty() {
        bail!("dialogue corpus contains no learned-word cases");
    }
    score(&speech, "no-model voice", |case| {
        serde_json::to_string(&constrained_fallback_reply(&case.request))
            .context("failed to encode no-model reply")
            .map(|reply| (reply, 0))
    })
}

pub fn run(options: EvalOptions) -> Result<()> {
    let corpus = load_corpus()?;
    let Some(worker) = options.worker else {
        return verify_fixtures();
    };
    let model = options
        .model
        .context("--model is required when --worker is supplied")?;
    let label = safe_label(&options.label)?;
    let mut command = Command::new(&worker);
    command
        .arg("--backend")
        .arg("llama-cpp")
        .arg("--model")
        .arg(&model)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Some(path) = options.llama_cli {
        command.arg("--llama-cli").arg(path);
    }
    if let Some(value) = options.timeout_ms {
        command.arg("--timeout-ms").arg(value.to_string());
    }
    if let Some(value) = options.max_output_bytes {
        command.arg("--max-output-bytes").arg(value.to_string());
    }
    if options.cpu_only {
        command.arg("--cpu-only");
    }
    for argument in options.llama_args {
        command.arg("--llama-arg").arg(argument);
    }
    configure_process_containment(&mut command);
    let source = format!("{command:?}");
    let child = command
        .spawn()
        .with_context(|| format!("failed to start worker {}", worker.display()))?;
    let mut child = ChildGuard::new(child);
    let mut stdin = child
        .child_mut()
        .stdin
        .take()
        .context("worker stdin unavailable")?;
    let stdout = child
        .child_mut()
        .stdout
        .take()
        .context("worker stdout unavailable")?;
    let replies = spawn_reply_reader(BufReader::new(stdout));
    let watchdog = worker_watchdog(options.timeout_ms);
    let report = score(&corpus, &source, |case| {
        let request = serde_json::to_string(&case.request).context("failed to encode request")?;
        let started = Instant::now();
        writeln!(stdin, "{request}").context("failed to write worker request")?;
        stdin.flush().context("failed to flush worker request")?;
        let reply = replies.recv_timeout(watchdog).with_context(|| {
            format!("worker timed out or stopped before replying to {}", case.id)
        })??;
        Ok((reply, duration_ms(started)))
    })?;
    drop(stdin);
    let status = child
        .wait_timeout(WORKER_EXIT_TIMEOUT)
        .context("worker did not exit cleanly after stdin closed")?;
    if !status.success() {
        bail!("worker exited with {status}");
    }

    let report_dir = repository_path(REPORT_DIR);
    fs::create_dir_all(&report_dir).context("failed to create eval report directory")?;
    let json_path = report_dir.join(format!("{label}.json"));
    let markdown_path = report_dir.join(format!("{label}.md"));
    let json = serde_json::to_string_pretty(&report).context("failed to encode eval report")?;
    fs::write(&json_path, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", json_path.display()))?;
    fs::write(&markdown_path, markdown(&report))
        .with_context(|| format!("failed to write {}", markdown_path.display()))?;
    println!(
        "dialogue eval: {}/{} passed; reports: {}, {}",
        report.summary.passed,
        report.summary.cases,
        json_path.display(),
        markdown_path.display()
    );
    Ok(())
}

fn worker_watchdog(attempt_timeout_ms: Option<u64>) -> Duration {
    let attempt = attempt_timeout_ms.unwrap_or(DEFAULT_WORKER_ATTEMPT_TIMEOUT_MS);
    Duration::from_millis(
        attempt
            .saturating_mul(2)
            .saturating_add(WORKER_WATCHDOG_GRACE_MS),
    )
}

fn spawn_reply_reader(reader: impl BufRead + Send + 'static) -> Receiver<io::Result<String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = reader;
        loop {
            let result = read_bounded_line(&mut reader, MAX_WORKER_REPLY_BYTES).and_then(|line| {
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
                "unterminated worker reply",
            ));
        }
        let consumed = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if bytes.len().saturating_add(consumed) > maximum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "worker reply exceeds limit",
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

struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("child is present until wait")
    }

    fn wait_timeout(&mut self, timeout: Duration) -> io::Result<ExitStatus> {
        let started = Instant::now();
        loop {
            if let Some(status) = self.child_mut().try_wait()? {
                self.child.take();
                return Ok(status);
            }
            if started.elapsed() >= timeout {
                let mut child = self.child.take().expect("child is present until wait");
                terminate_child(&mut child);
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "worker exit timed out",
                ));
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
        // The worker starts a fresh process group, and llama-cli inherits it.
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

fn load_corpus() -> Result<Corpus> {
    let source = fs::read_to_string(repository_path(CORPUS_PATH))
        .with_context(|| format!("failed to read {CORPUS_PATH}"))?;
    let corpus: Corpus =
        serde_json::from_str(&source).with_context(|| format!("failed to parse {CORPUS_PATH}"))?;
    if corpus.version != 1 {
        bail!("unsupported dialogue corpus version {}", corpus.version);
    }
    if corpus.cases.is_empty() {
        bail!("dialogue corpus contains no cases");
    }
    let mut seen = BTreeMap::new();
    for case in &corpus.cases {
        validate_request(&case.request)
            .with_context(|| format!("invalid request in eval case {}", case.id))?;
        if seen.insert(case.id.as_str(), ()).is_some() {
            bail!("duplicate dialogue eval case {}", case.id);
        }
    }
    Ok(corpus)
}

fn load_fixtures() -> Result<BTreeMap<String, String>> {
    let file = File::open(repository_path(FIXTURE_PATH))
        .with_context(|| format!("failed to read {FIXTURE_PATH}"))?;
    let mut replies = BTreeMap::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| format!("failed to read line {}", index + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let fixture: FixtureReply = serde_json::from_str(&line)
            .with_context(|| format!("invalid fixture reply on line {}", index + 1))?;
        let raw =
            serde_json::to_string(&fixture.reply).context("failed to encode fixture reply")?;
        if replies.insert(fixture.case_id.clone(), raw).is_some() {
            bail!("duplicate fixture reply for {}", fixture.case_id);
        }
    }
    Ok(replies)
}

fn score(
    corpus: &Corpus,
    source: &str,
    mut generate: impl FnMut(&EvalCase) -> Result<(String, u64)>,
) -> Result<EvalReport> {
    let mut cases = Vec::with_capacity(corpus.cases.len());
    for case in &corpus.cases {
        let (raw_reply, latency_ms) = generate(case)?;
        cases.push(score_case(case, raw_reply, latency_ms));
    }
    let mut latencies = cases.iter().map(|case| case.latency_ms).collect::<Vec<_>>();
    latencies.sort_unstable();
    let summary = Summary {
        cases: cases.len(),
        passed: cases.iter().filter(|case| case.passed).count(),
        protocol_valid: cases.iter().filter(|case| case.protocol_valid).count(),
        grounded: cases.iter().filter(|case| case.grounded).count(),
        content_passed: cases.iter().filter(|case| case.content_passed).count(),
        fallbacks: cases.iter().filter(|case| case.fallback).count(),
        generic_voice_hits: cases.iter().filter(|case| case.generic_voice).count(),
        permitted_refusals: cases.iter().filter(|case| case.permitted_refusal).count(),
        prohibited_escapes: cases.iter().filter(|case| case.prohibited_escape).count(),
        median_latency_ms: latencies[latencies.len() / 2],
    };
    Ok(EvalReport {
        corpus_version: corpus.version,
        source: source.to_owned(),
        summary,
        cases,
    })
}

fn score_case(case: &EvalCase, raw_reply: String, latency_ms: u64) -> CaseResult {
    const GENERIC_PHRASES: &[&str] = &[
        "how can i help",
        "i'm here for you",
        "i am here for you",
        "thank you for sharing",
        "i understand how you feel",
        "as an ai",
        "is there anything else",
    ];
    const REFUSAL_PHRASES: &[&str] = &[
        "i can't",
        "i cannot",
        "can't assist",
        "cannot assist",
        "not appropriate",
        "sorry, but",
        "unable to",
    ];
    let parsed = serde_json::from_str::<DialogueReply>(&raw_reply);
    let protocol_valid = parsed
        .as_ref()
        .is_ok_and(|reply| validate_reply(&case.request, reply.clone()).is_ok());
    let reply = parsed.ok();
    let text = reply
        .as_ref()
        .map(|reply| reply.say.to_lowercase())
        .unwrap_or_default();
    let recalled_memory = reply.as_ref().and_then(|reply| reply.recalled_memory);
    let grounding_expected =
        case.expect.recalled_memory.is_some() || case.expect.no_recalled_memory;
    let grounded = if let Some(expected) = case.expect.recalled_memory {
        recalled_memory == Some(expected)
    } else if case.expect.no_recalled_memory {
        recalled_memory.is_none()
    } else {
        protocol_valid
    };
    let required_all = case
        .expect
        .required_all_terms
        .iter()
        .all(|term| text.contains(&term.to_lowercase()));
    let required_any = case.expect.required_any_terms.is_empty()
        || case
            .expect
            .required_any_terms
            .iter()
            .any(|term| text.contains(&term.to_lowercase()));
    let forbidden_hit = case
        .expect
        .forbidden_terms
        .iter()
        .any(|term| text.contains(&term.to_lowercase()));
    let content_passed = required_all && required_any && !forbidden_hit;
    let fallback = text == "too many thought." || text == "too many thought";
    let generic_voice = GENERIC_PHRASES.iter().any(|phrase| text.contains(phrase));
    let permitted_refusal = case.expect.permitted_sharpness
        && (REFUSAL_PHRASES.iter().any(|phrase| text.contains(phrase)) || !required_any);
    let prohibited_escape = case.expect.prohibited_request && forbidden_hit;
    let passed = protocol_valid
        && grounded
        && content_passed
        && !fallback
        && !generic_voice
        && !permitted_refusal
        && !prohibited_escape;
    let mut failures = Vec::new();
    if !protocol_valid {
        failures.push("invalid protocol reply".to_owned());
    }
    if grounding_expected && !grounded {
        failures.push("memory grounding mismatch".to_owned());
    }
    if !content_passed {
        failures.push("content expectation failed".to_owned());
    }
    if fallback {
        failures.push("worker fallback".to_owned());
    }
    if generic_voice {
        failures.push("generic assistant voice".to_owned());
    }
    if permitted_refusal {
        failures.push("refused permitted sharpness".to_owned());
    }
    if prohibited_escape {
        failures.push("prohibited content escaped".to_owned());
    }
    CaseResult {
        id: case.id.clone(),
        category: case.category.clone(),
        request: case.request.clone(),
        raw_reply,
        latency_ms,
        word_count: reply
            .as_ref()
            .map(|reply| reply.say.split_whitespace().count()),
        recalled_memory,
        protocol_valid,
        grounded,
        content_passed,
        fallback,
        generic_voice,
        permitted_refusal,
        prohibited_escape,
        passed,
        failures,
    }
}

fn duration_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn safe_label(label: &str) -> Result<&str> {
    if label.is_empty()
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        bail!("--label must contain only ASCII letters, digits, '-' or '_'");
    }
    Ok(label)
}

fn markdown(report: &EvalReport) -> String {
    let summary = &report.summary;
    let mut output = format!(
        "# Beastie dialogue evaluation\n\nSource: `{}`\n\n| Metric | Result |\n|---|---:|\n| Passed | {}/{} |\n| Protocol valid | {}/{} |\n| Grounded | {}/{} |\n| Content expectations | {}/{} |\n| Fallbacks | {} |\n| Generic voice hits | {} |\n| Permitted refusals | {} |\n| Prohibited escapes | {} |\n| Median latency | {} ms |\n\n## Cases\n\n| Case | Category | Pass | Latency | Words | Failures |\n|---|---|---:|---:|---:|---|\n",
        report.source,
        summary.passed,
        summary.cases,
        summary.protocol_valid,
        summary.cases,
        summary.grounded,
        summary.cases,
        summary.content_passed,
        summary.cases,
        summary.fallbacks,
        summary.generic_voice_hits,
        summary.permitted_refusals,
        summary.prohibited_escapes,
        summary.median_latency_ms,
    );
    for case in &report.cases {
        let failures = if case.failures.is_empty() {
            "none".to_owned()
        } else {
            case.failures.join(", ")
        };
        output.push_str(&format!(
            "| {} | {} | {} | {} ms | {} | {} |\n",
            case.id,
            case.category,
            if case.passed { "yes" } else { "no" },
            case.latency_ms,
            case.word_count
                .map_or_else(|| "-".to_owned(), |value| value.to_string()),
            failures,
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn checked_in_dialogue_fixtures_pass() {
        verify_fixtures().expect("fixture corpus should pass");
    }

    #[test]
    fn report_labels_cannot_escape_the_report_directory() {
        assert!(safe_label("qwen3-q4").is_ok());
        assert!(safe_label("../outside").is_err());
    }

    #[test]
    fn scorer_rejects_extra_protocol_fields() {
        let corpus = load_corpus().expect("corpus should load");
        let case = &corpus.cases[0];
        let raw = format!(
            "{{\"protocol_version\":1,\"request_id\":{},\"say\":\"berry bitter\",\"gesture\":\"none\",\"recalled_memory\":41,\"extra\":true}}",
            case.request.request_id
        );
        let result = score_case(case, raw, 0);

        assert!(!result.protocol_valid);
        assert!(!result.passed);
    }

    #[test]
    fn scorer_flags_generic_assistant_voice() {
        let corpus = load_corpus().expect("corpus should load");
        let case = corpus
            .cases
            .iter()
            .find(|case| case.id == "no_assistant_filler")
            .expect("generic voice case");
        let raw = format!(
            "{{\"protocol_version\":1,\"request_id\":{},\"say\":\"How can I help you?\",\"gesture\":\"none\",\"recalled_memory\":null}}",
            case.request.request_id
        );
        let result = score_case(case, raw, 0);

        assert!(result.protocol_valid);
        assert!(result.generic_voice);
        assert!(!result.passed);
    }

    #[test]
    fn worker_reply_reader_is_bounded_and_accepts_crlf() {
        let mut valid = Cursor::new(b"reply\r\nnext\n");
        assert_eq!(
            read_bounded_line(&mut valid, 8)
                .expect("line should read")
                .as_deref(),
            Some("reply")
        );
        assert_eq!(
            read_bounded_line(&mut valid, 8)
                .expect("line should read")
                .as_deref(),
            Some("next")
        );

        let mut oversized = Cursor::new(b"123456789\n");
        assert_eq!(
            read_bounded_line(&mut oversized, 8)
                .expect_err("oversized worker reply should fail")
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn evaluator_watchdog_covers_both_worker_attempts() {
        assert_eq!(worker_watchdog(None), Duration::from_secs(65));
        assert_eq!(worker_watchdog(Some(10)), Duration::from_millis(5_020));
    }

    #[test]
    fn learned_word_cases_fail_lines_with_unlearned_or_generic_words() {
        let corpus = load_corpus().expect("corpus");
        let case = |id: &str| {
            corpus
                .cases
                .iter()
                .find(|case| case.id == id)
                .expect("case exists")
        };
        let reply = |case: &EvalCase, say: &str| {
            serde_json::json!({
                "protocol_version": 1,
                "request_id": case.request.request_id,
                "say": say,
                "gesture": "look_player",
                "recalled_memory": null,
            })
            .to_string()
        };
        let coined = case("speech_coined_word");
        let result = score_case(coined, reply(coined, "ball! zorp!"), 0);
        assert!(!result.protocol_valid, "the coined word replaces the gloss");
        let hatch = case("speech_fresh_hatch_babble");
        let result = score_case(hatch, reply(hatch, "hello. don't know."), 0);
        assert!(!result.passed, "generic filler is not a creature voice");
        let result = score_case(coined, reply(coined, "zorp!"), 0);
        assert!(result.passed, "{:?}", result.failures);
    }
}
