use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
use std::thread;
use std::time::Duration;

use beastie_protocol::{
    DialogueReply, DialogueRequest, constrained_fallback_reply, validate_reply, validate_request,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::process::ContainedChild;

const MAX_REPLY_BYTES: usize = 16 * 1024;
const MAX_MODEL_MANIFEST_BYTES: u64 = 256 * 1024;

#[derive(Debug, Error)]
enum PackageDiscoveryError {
    #[error("models/manifest.toml is missing or unreadable: {0}")]
    ManifestIo(io::Error),
    #[error("models/manifest.toml is malformed: {0}")]
    Manifest(&'static str),
    #[error("selected dialogue model `{0}` is absent from models/manifest.toml")]
    Selection(String),
    #[error("selected dialogue model file is missing: {0}")]
    ModelMissing(PathBuf),
    #[error("selected dialogue model cannot be read ({path}): {source}")]
    ModelIo { path: PathBuf, source: io::Error },
    #[error("selected dialogue model byte count mismatch: expected {expected}, found {actual}")]
    ModelBytes { expected: u64, actual: u64 },
    #[error("selected dialogue model SHA-256 mismatch: expected {expected}, found {actual}")]
    ModelSha256 { expected: String, actual: String },
}

#[derive(Debug, Default)]
struct ManifestCandidate {
    id: Option<String>,
    file: Option<String>,
    bytes: Option<u64>,
    sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    executable: PathBuf,
    arguments: Vec<OsString>,
    reply_timeout: Duration,
}

impl WorkerConfig {
    const DEFAULT_INNER_TIMEOUT_MS: u64 = 30_000;
    const WATCHDOG_GRACE_MS: u64 = 5_000;

    #[must_use]
    pub fn environment_reply_timeout() -> Duration {
        let inner_timeout = std::env::var("BEASTIE_AI_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(Self::DEFAULT_INNER_TIMEOUT_MS);
        Duration::from_millis(
            inner_timeout
                .saturating_mul(2)
                .saturating_add(Self::WATCHDOG_GRACE_MS),
        )
    }

    #[must_use]
    pub fn from_environment(fake_ai: bool, reply_timeout: Duration) -> Option<Self> {
        if let Some(executable) = std::env::var_os("BEASTIE_AI_WORKER").map(PathBuf::from) {
            let arguments = fake_ai
                .then(|| [OsString::from("--backend"), OsString::from("fixture")])
                .into_iter()
                .flatten()
                .collect();
            return Some(Self {
                executable,
                arguments,
                reply_timeout,
            });
        }
        if fake_ai {
            return None;
        }
        let executable = std::env::current_exe().ok()?;
        Self::from_package_root_with_fallback(executable.parent()?, reply_timeout)
    }

    fn from_package_root_with_fallback(root: &Path, reply_timeout: Duration) -> Option<Self> {
        match Self::from_package_root(root, reply_timeout) {
            Ok(config) => config,
            Err(error) => {
                eprintln!(
                    "Beastie warning: packaged dialogue disabled ({error}); using authored fallback."
                );
                None
            }
        }
    }

    fn from_package_root(
        root: &Path,
        reply_timeout: Duration,
    ) -> Result<Option<Self>, PackageDiscoveryError> {
        let executable = root.join(format!("beastie-ai-worker{}", std::env::consts::EXE_SUFFIX));
        let server = root
            .join("runtime")
            .join(format!("llama-server{}", std::env::consts::EXE_SUFFIX));
        if !executable.is_file() || !server.is_file() {
            return Ok(None);
        }
        let selected = selected_dialogue_model(&root.join("models/manifest.toml"))?;
        let model = root.join("models").join(&selected.file);
        verify_model(&model, selected.bytes, &selected.sha256)?;
        Ok(Some(Self {
            executable,
            arguments: vec![
                OsString::from("--backend"),
                OsString::from("llama-server"),
                OsString::from("--model"),
                model.into_os_string(),
                OsString::from("--llama-server"),
                server.into_os_string(),
            ],
            reply_timeout,
        }))
    }

    #[cfg(test)]
    fn new(executable: PathBuf, arguments: Vec<OsString>, reply_timeout: Duration) -> Self {
        Self {
            executable,
            arguments,
            reply_timeout,
        }
    }
}

#[derive(Debug)]
struct SelectedDialogueModel {
    file: String,
    bytes: u64,
    sha256: String,
}

fn selected_dialogue_model(path: &Path) -> Result<SelectedDialogueModel, PackageDiscoveryError> {
    let metadata = fs::metadata(path).map_err(PackageDiscoveryError::ManifestIo)?;
    if metadata.len() > MAX_MODEL_MANIFEST_BYTES {
        return Err(PackageDiscoveryError::Manifest("file exceeds size limit"));
    }
    let source = fs::read_to_string(path).map_err(PackageDiscoveryError::ManifestIo)?;
    let mut in_dialogue_selection = false;
    let mut preferred = None;
    for line in source.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dialogue_selection = line == "[selection.dialogue]";
        } else if in_dialogue_selection
            && let Some(value) = string_assignment(line, "preferred_candidate")
        {
            preferred = Some(value);
            break;
        }
    }
    let preferred = preferred.ok_or(PackageDiscoveryError::Manifest(
        "selection.dialogue.preferred_candidate is missing",
    ))?;
    let mut candidates = Vec::new();
    let mut current = None;
    for line in source.lines().map(str::trim) {
        if line == "[[candidate]]" {
            if let Some(candidate) = current.take() {
                candidates.push(candidate);
            }
            current = Some(ManifestCandidate::default());
            continue;
        }
        let Some(candidate) = current.as_mut() else {
            continue;
        };
        if line.starts_with('[') {
            candidates.push(current.take().expect("candidate exists"));
            continue;
        }
        if let Some(value) = string_assignment(line, "id") {
            candidate.id = Some(value);
        } else if let Some(value) = string_assignment(line, "file") {
            candidate.file = Some(value);
        } else if let Some(value) = integer_assignment(line, "bytes") {
            candidate.bytes = Some(value);
        } else if let Some(value) = string_assignment(line, "sha256") {
            candidate.sha256 = Some(value);
        }
    }
    if let Some(candidate) = current {
        candidates.push(candidate);
    }
    let candidate = candidates
        .into_iter()
        .find(|candidate| candidate.id.as_deref() == Some(preferred.as_str()))
        .ok_or_else(|| PackageDiscoveryError::Selection(preferred.clone()))?;
    let file = candidate.file.ok_or(PackageDiscoveryError::Manifest(
        "selected candidate.file is missing",
    ))?;
    if Path::new(&file).file_name().and_then(|name| name.to_str()) != Some(file.as_str()) {
        return Err(PackageDiscoveryError::Manifest(
            "selected candidate.file must be a plain file name",
        ));
    }
    let bytes = candidate.bytes.ok_or(PackageDiscoveryError::Manifest(
        "selected candidate.bytes is missing",
    ))?;
    if bytes == 0 {
        return Err(PackageDiscoveryError::Manifest(
            "selected candidate.bytes must be positive",
        ));
    }
    let sha256 = candidate.sha256.ok_or(PackageDiscoveryError::Manifest(
        "selected candidate.sha256 is missing",
    ))?;
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(PackageDiscoveryError::Manifest(
            "selected candidate.sha256 must be 64 lowercase hexadecimal characters",
        ));
    }
    Ok(SelectedDialogueModel {
        file,
        bytes,
        sha256,
    })
}

fn string_assignment(line: &str, key: &str) -> Option<String> {
    let (candidate, value) = line.trim().split_once('=')?;
    if candidate.trim() != key {
        return None;
    }
    let value = value.trim().strip_prefix('"')?.strip_suffix('"')?;
    (!value.is_empty() && !value.contains('"')).then(|| value.to_owned())
}

fn integer_assignment(line: &str, key: &str) -> Option<u64> {
    let (candidate, value) = line.trim().split_once('=')?;
    (candidate.trim() == key)
        .then(|| value.trim().parse::<u64>().ok())
        .flatten()
}

fn verify_model(
    path: &Path,
    expected_bytes: u64,
    expected_sha256: &str,
) -> Result<(), PackageDiscoveryError> {
    let actual_bytes = fs::metadata(path)
        .map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                PackageDiscoveryError::ModelMissing(path.to_path_buf())
            } else {
                PackageDiscoveryError::ModelIo {
                    path: path.to_path_buf(),
                    source,
                }
            }
        })?
        .len();
    if actual_bytes != expected_bytes {
        return Err(PackageDiscoveryError::ModelBytes {
            expected: expected_bytes,
            actual: actual_bytes,
        });
    }
    let mut file = File::open(path).map_err(|source| PackageDiscoveryError::ModelIo {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| PackageDiscoveryError::ModelIo {
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected_sha256 {
        return Err(PackageDiscoveryError::ModelSha256 {
            expected: expected_sha256.to_owned(),
            actual,
        });
    }
    Ok(())
}

enum ManagerCommand {
    Request(DialogueRequest),
    Shutdown,
}

/// Owns a single long-lived worker process on a dedicated manager thread.
///
/// The game loop only sends requests and polls replies. A failed exchange is
/// converted to the authored constrained fallback; the next request starts a
/// fresh worker process.
pub struct DialogueManager {
    commands: SyncSender<ManagerCommand>,
    replies: Receiver<DialogueReply>,
    pending: bool,
    cancelled: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl DialogueManager {
    #[must_use]
    pub fn new(config: Option<WorkerConfig>) -> Self {
        let (commands, command_receiver) = mpsc::sync_channel(1);
        let (reply_sender, replies) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let manager_cancelled = Arc::clone(&cancelled);
        let thread = thread::spawn(move || {
            run_manager(config, command_receiver, reply_sender, &manager_cancelled);
        });
        Self {
            commands,
            replies,
            pending: false,
            cancelled,
            thread: Some(thread),
        }
    }

    /// Queue one dialogue request. Returns false while another talk is pending.
    pub fn request(&mut self, request: DialogueRequest) -> bool {
        if self.pending || validate_request(&request).is_err() {
            return false;
        }
        if self
            .commands
            .send(ManagerCommand::Request(request))
            .is_err()
        {
            return false;
        }
        self.pending = true;
        true
    }

    pub fn try_recv(&mut self) -> Result<DialogueReply, TryRecvError> {
        match self.replies.try_recv() {
            Ok(reply) => {
                self.pending = false;
                Ok(reply)
            }
            Err(TryRecvError::Disconnected) => {
                self.pending = false;
                Err(TryRecvError::Disconnected)
            }
            Err(TryRecvError::Empty) => Err(TryRecvError::Empty),
        }
    }

    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.pending
    }

    #[cfg(test)]
    fn shutdown(mut self) {
        let _ = self.commands.send(ManagerCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            thread.join().expect("manager should stop cleanly");
        }
    }

    #[cfg(test)]
    fn recv_timeout(&mut self, timeout: Duration) -> DialogueReply {
        let reply = self
            .replies
            .recv_timeout(timeout)
            .expect("reply should arrive");
        self.pending = false;
        reply
    }
}

impl Drop for DialogueManager {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.commands.send(ManagerCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_manager(
    config: Option<WorkerConfig>,
    commands: Receiver<ManagerCommand>,
    replies: Sender<DialogueReply>,
    cancelled: &AtomicBool,
) {
    let mut worker = None;
    while let Ok(command) = commands.recv() {
        match command {
            ManagerCommand::Request(request) => {
                let reply = config
                    .as_ref()
                    .and_then(|config| {
                        exchange_with_recovery(config, &mut worker, &request, cancelled)
                    })
                    .unwrap_or_else(|| constrained_fallback_reply(&request));
                if cancelled.load(Ordering::Acquire) {
                    break;
                }
                if replies.send(reply).is_err() {
                    break;
                }
            }
            ManagerCommand::Shutdown => break,
        }
    }
    if let Some(mut worker) = worker {
        worker.terminate();
    }
}

fn exchange_with_recovery(
    config: &WorkerConfig,
    worker: &mut Option<WorkerSession>,
    request: &DialogueRequest,
    cancelled: &AtomicBool,
) -> Option<DialogueReply> {
    if worker.is_none() {
        *worker = WorkerSession::spawn(config).ok();
    }

    let reply = worker.as_mut().and_then(|session| {
        session
            .exchange(request, config.reply_timeout, cancelled)
            .ok()
    });
    if reply.is_some() {
        return reply;
    }

    if let Some(mut failed) = worker.take() {
        failed.terminate();
    }
    None
}

struct WorkerSession {
    child: ContainedChild,
    stdin: ChildStdin,
    lines: Receiver<io::Result<String>>,
    exchanged: bool,
}

impl WorkerSession {
    fn spawn(config: &WorkerConfig) -> io::Result<Self> {
        let mut command = Command::new(&config.executable);
        command
            .args(&config.arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = ContainedChild::spawn(&mut command)?;
        let Some(stdin) = child.stdin.take() else {
            child.terminate_tree();
            return Err(io::Error::other("worker stdin unavailable"));
        };
        let Some(stdout) = child.stdout.take() else {
            child.terminate_tree();
            return Err(io::Error::other("worker stdout unavailable"));
        };
        let (line_sender, lines) = mpsc::sync_channel(1);
        thread::spawn(move || read_worker_lines(BufReader::new(stdout), line_sender));
        Ok(Self {
            child,
            stdin,
            lines,
            exchanged: false,
        })
    }

    fn exchange(
        &mut self,
        request: &DialogueRequest,
        timeout: Duration,
        cancelled: &AtomicBool,
    ) -> io::Result<DialogueReply> {
        if self.exchanged {
            match self.lines.try_recv() {
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "worker emitted an unsolicited reply",
                    ));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "worker reply reader disconnected",
                    ));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        serde_json::to_writer(&mut self.stdin, request)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        self.exchanged = true;
        let started = std::time::Instant::now();
        let line = loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "dialogue manager is shutting down",
                ));
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "worker reply timed out",
                ));
            }
            match self
                .lines
                .recv_timeout(remaining.min(Duration::from_millis(10)))
            {
                Ok(line) => break line?,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error @ mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::new(io::ErrorKind::BrokenPipe, error));
                }
            }
        };
        let reply = serde_json::from_str::<DialogueReply>(&line)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        validate_reply(request, reply)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn terminate(&mut self) {
        self.child.terminate_tree();
    }
}

fn read_worker_lines(mut reader: impl BufRead, sender: SyncSender<io::Result<String>>) {
    loop {
        let line = read_bounded_line(&mut reader, MAX_REPLY_BYTES);
        let finished = matches!(&line, Ok(None) | Err(_));
        let result = line.and_then(|line| {
            line.ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "worker exited"))
        });
        if sender.send(result).is_err() || finished {
            break;
        }
    }
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::io::Cursor;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use beastie_protocol::{
        DialogueConstraints, Gesture, Idiolect, PROTOCOL_VERSION, validate_reply,
    };

    use super::*;

    static PACKAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn packaged_runtime_fixture() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "beastie-package-discovery-{}-{}",
            std::process::id(),
            PACKAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let worker = root.join(format!("beastie-ai-worker{}", std::env::consts::EXE_SUFFIX));
        let server = root
            .join("runtime")
            .join(format!("llama-server{}", std::env::consts::EXE_SUFFIX));
        let model = root.join("models/Qwen3.5-0.8B-Q4_0.gguf");
        fs::create_dir_all(server.parent().expect("server parent")).unwrap();
        fs::create_dir_all(model.parent().expect("model parent")).unwrap();
        fs::write(&worker, b"fixture").unwrap();
        fs::write(&server, b"fixture").unwrap();
        fs::write(&model, b"fixture").unwrap();
        let sha256 = format!("{:x}", Sha256::digest(b"fixture"));
        fs::write(
            root.join("models/manifest.toml"),
            format!(
                "[selection.dialogue]\npreferred_candidate = \"selected\"\n\n[[candidate]]\nid = \"other\"\nfile = \"other.gguf\"\nbytes = 1\nsha256 = \"{}\"\n\n[[candidate]]\nid = \"selected\"\nfile = \"Qwen3.5-0.8B-Q4_0.gguf\"\nbytes = 7\nsha256 = \"{sha256}\"\n",
                "0".repeat(64)
            ),
        )
        .unwrap();
        (root, worker, server, model)
    }

    fn request() -> DialogueRequest {
        DialogueRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: 7,
            creature_name: "Mop".to_owned(),
            mood: "wary".to_owned(),
            known_concepts: BTreeSet::new(),
            candidate_memories: Vec::new(),
            candidate_beliefs: Vec::new(),
            idiolect: Idiolect::default(),
            desired_social_act: None,
            input_rejection: None,
            player_said: "hello".to_owned(),
            constraints: DialogueConstraints {
                max_words: 3,
                allowed_gestures: BTreeSet::from([Gesture::None]),
            },
        }
    }

    #[test]
    fn missing_worker_returns_a_bounded_fallback() {
        let dialogue = request();
        let mut manager = DialogueManager::new(None);
        assert!(manager.request(dialogue.clone()));
        let reply = loop {
            match manager.try_recv() {
                Ok(reply) => break reply,
                Err(TryRecvError::Empty) => thread::yield_now(),
                Err(TryRecvError::Disconnected) => panic!("manager disconnected"),
            }
        };
        assert_eq!(validate_reply(&dialogue, reply.clone()), Ok(reply));
    }

    #[test]
    fn one_outstanding_talk_is_enforced() {
        let mut manager = DialogueManager::new(None);
        assert!(manager.request(request()));
        assert!(!manager.request(request()));
    }

    #[test]
    fn missing_executable_recovers_to_fallback() {
        let dialogue = request();
        let config = WorkerConfig::new(
            PathBuf::from("beastie-worker-that-does-not-exist"),
            Vec::new(),
            Duration::from_millis(10),
        );
        let mut manager = DialogueManager::new(Some(config));
        assert!(manager.request(dialogue.clone()));
        let reply = manager
            .replies
            .recv_timeout(Duration::from_secs(1))
            .expect("fallback should arrive");
        assert_eq!(validate_reply(&dialogue, reply.clone()), Ok(reply));
    }

    #[test]
    fn bounded_reader_accepts_crlf_and_rejects_oversized_lines() {
        let mut valid = Cursor::new(b"ok\r\nnext\n");
        assert_eq!(
            read_bounded_line(&mut valid, 8).unwrap().as_deref(),
            Some("ok")
        );
        assert_eq!(
            read_bounded_line(&mut valid, 8).unwrap().as_deref(),
            Some("next")
        );
        let mut oversized = Cursor::new(b"123456789\n");
        assert_eq!(
            read_bounded_line(&mut oversized, 8)
                .expect_err("line should be rejected")
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn default_watchdog_covers_both_inner_attempts_and_grace() {
        if std::env::var_os("BEASTIE_AI_TIMEOUT_MS").is_none() {
            assert_eq!(
                WorkerConfig::environment_reply_timeout(),
                Duration::from_secs(65)
            );
        }
    }

    #[test]
    fn packaged_runtime_is_discovered_without_environment_configuration() {
        let (root, worker, server, model) = packaged_runtime_fixture();

        let config = WorkerConfig::from_package_root(&root, Duration::from_secs(5))
            .expect("valid package should pass integrity")
            .expect("complete package should be discovered");
        assert_eq!(config.executable, worker);
        assert!(
            config
                .arguments
                .iter()
                .any(|argument| argument == server.as_os_str())
        );
        assert!(
            config
                .arguments
                .iter()
                .any(|argument| argument == model.as_os_str())
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn packaged_runtime_rejects_selected_model_byte_count_mismatch() {
        let (root, _, _, model) = packaged_runtime_fixture();
        fs::write(&model, b"truncated").unwrap();
        let error = WorkerConfig::from_package_root(&root, Duration::from_secs(5))
            .expect_err("wrong model bytes must fail closed");
        assert!(matches!(error, PackageDiscoveryError::ModelBytes { .. }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn packaged_runtime_sha_mismatch_uses_authored_dialogue_fallback() {
        let (root, _, _, model) = packaged_runtime_fixture();
        fs::write(&model, b"fIxture").unwrap();
        let config = WorkerConfig::from_package_root_with_fallback(&root, Duration::from_secs(5));
        assert!(config.is_none(), "wrong model digest must fail closed");

        let dialogue = request();
        let mut manager = DialogueManager::new(config);
        assert!(manager.request(dialogue.clone()));
        let fallback = manager.recv_timeout(Duration::from_secs(1));
        assert_eq!(
            fallback,
            beastie_protocol::constrained_fallback_reply(&dialogue)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn worker_process_is_reused_across_requests() {
        let worker = compile_worker("counting");
        let config = WorkerConfig::new(worker, Vec::new(), Duration::from_secs(1));
        let mut manager = DialogueManager::new(Some(config));

        let first_request = request();
        assert!(manager.request(first_request));
        let first = manager.recv_timeout(Duration::from_secs(1));

        let second_request = request();
        assert!(manager.request(second_request));
        let second = manager.recv_timeout(Duration::from_secs(1));

        assert_eq!(first.say, "reply 1");
        assert_eq!(second.say, "reply 2");
        manager.shutdown();
    }

    #[test]
    fn unsolicited_worker_output_restarts_the_worker() {
        let worker = compile_worker("extra-output");
        let config = WorkerConfig::new(
            worker,
            vec![OsString::from("extra")],
            Duration::from_secs(1),
        );
        let mut manager = DialogueManager::new(Some(config));

        assert!(manager.request(request()));
        assert_eq!(manager.recv_timeout(Duration::from_secs(1)).say, "reply 1");
        assert!(manager.request(request()));
        assert_eq!(
            manager.recv_timeout(Duration::from_secs(1)).say,
            "too many thought."
        );
        assert!(manager.request(request()));
        assert_eq!(manager.recv_timeout(Duration::from_secs(1)).say, "reply 1");
        manager.shutdown();
    }

    #[test]
    fn watchdog_falls_back_then_restarts_for_the_next_request() {
        let worker = compile_worker("recovery");
        let marker = std::env::temp_dir().join(format!(
            "beastie-worker-recovery-marker-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&marker);
        let config = WorkerConfig::new(
            worker,
            vec![OsString::from("hang-once"), marker.into_os_string()],
            Duration::from_millis(50),
        );
        let mut manager = DialogueManager::new(Some(config));

        assert!(manager.request(request()));
        let fallback = manager.recv_timeout(Duration::from_secs(1));
        assert_ne!(fallback.say, "reply 1");

        assert!(manager.request(request()));
        let recovered = manager.recv_timeout(Duration::from_secs(1));
        assert_eq!(recovered.say, "reply 1");
        manager.shutdown();
    }

    #[test]
    fn dropping_manager_interrupts_and_reaps_a_pending_worker() {
        let worker = compile_worker("drop-cleanup");
        let marker =
            std::env::temp_dir().join(format!("beastie-worker-drop-marker-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let config = WorkerConfig::new(
            worker,
            vec![OsString::from("hang-once"), marker.clone().into_os_string()],
            Duration::from_secs(2),
        );
        let mut manager = DialogueManager::new(Some(config));
        assert!(manager.request(request()));
        for _ in 0..100 {
            if marker.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(marker.exists(), "worker should enter its hanging request");

        let started = std::time::Instant::now();
        drop(manager);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "shutdown should interrupt the reply watchdog"
        );
        let _ = std::fs::remove_file(marker);
    }

    #[cfg(unix)]
    #[test]
    fn dropping_manager_kills_the_worker_process_group() {
        let worker = compile_worker("tree-cleanup");
        let marker =
            std::env::temp_dir().join(format!("beastie-worker-tree-marker-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let config = WorkerConfig::new(
            worker,
            vec![OsString::from("hang-tree"), marker.clone().into_os_string()],
            Duration::from_secs(2),
        );
        let mut manager = DialogueManager::new(Some(config));
        assert!(manager.request(request()));
        for _ in 0..100 {
            if marker.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let descendant: i32 = std::fs::read_to_string(&marker)
            .expect("worker should record its descendant")
            .parse()
            .expect("descendant PID should parse");
        assert!(process_exists(descendant));

        drop(manager);
        for _ in 0..200 {
            if !process_exists(descendant) {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(!process_exists(descendant), "descendant should be killed");
        let _ = std::fs::remove_file(marker);
    }

    #[cfg(unix)]
    fn process_exists(pid: i32) -> bool {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;

        kill(Pid::from_raw(pid), None).is_ok()
    }

    fn compile_worker(label: &str) -> PathBuf {
        let stem = format!("beastie-{label}-worker-{}", std::process::id());
        let source = std::env::temp_dir().join(format!("{stem}.rs"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(
            &source,
            r#"
	use std::io::{self, BufRead, Write};
	use std::path::Path;
	use std::process::Command;
	use std::time::Duration;

	fn main() {
	    let args: Vec<String> = std::env::args().collect();
	    if args.get(1).is_some_and(|value| value == "grandchild") {
	        std::thread::sleep(Duration::from_secs(60));
	        return;
	    }
	    let tree = args.get(1).is_some_and(|value| value == "hang-tree");
    let extra = args.get(1).is_some_and(|value| value == "extra");
	    if tree {
	        let descendant = Command::new(std::env::current_exe().unwrap())
	            .arg("grandchild")
	            .spawn()
	            .unwrap();
	        std::fs::write(&args[2], descendant.id().to_string()).unwrap();
	    }
	    let hang = tree
	        || (args.get(1).is_some_and(|value| value == "hang-once")
	            && args.get(2).is_some_and(|marker| !Path::new(marker).exists()));
	    if hang && !tree {
	        std::fs::write(&args[2], "started").unwrap();
	    }
    let mut count = 0;
    for line in io::stdin().lock().lines() {
        if line.is_err() {
            break;
        }
        if hang {
            std::thread::sleep(Duration::from_secs(60));
        }
        count += 1;
        println!(
            "{{\"protocol_version\":1,\"request_id\":7,\"say\":\"reply {count}\",\"gesture\":\"none\",\"recalled_memory\":null}}"
        );
        if extra {
            println!(
                "{{\"protocol_version\":1,\"request_id\":7,\"say\":\"reply {count}\",\"gesture\":\"none\",\"recalled_memory\":null}}"
            );
        }
        io::stdout().flush().unwrap();
    }
}
"#,
        )
        .expect("helper source should be writable");
        let status = Command::new("rustc")
            .args(["--edition=2024", "-o"])
            .arg(&executable)
            .arg(&source)
            .status()
            .expect("rustc should start");
        assert!(status.success(), "helper worker should compile");
        executable
    }
}
