use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread;
use std::time::Duration;

use beastie_protocol::{
    MAX_RECOGNITION_TEXT_BYTES, RecognitionErrorCode, RecognitionLanguage, RecognitionOutcome,
    RecognitionReply, RecognitionRequest, STT_PROTOCOL_VERSION, SpeechInputFailure,
    validate_recognition_reply,
};
use sha2::{Digest, Sha256};

use crate::microphone::CapturedAudio;
use crate::process::JsonlWorkerSession;

const MAX_REPLY_BYTES: usize = 4_096;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);
const SELECTED_MODEL_ID: &str = "parakeet-tdt-0.6b-v3-int8";

#[derive(Debug, serde::Deserialize)]
struct PackageIntegrityManifest {
    files: BTreeMap<String, PackageIntegrityRecord>,
}

#[derive(Debug, serde::Deserialize)]
struct PackageIntegrityRecord {
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone)]
pub struct RecognitionWorkerConfig {
    executable: PathBuf,
    arguments: Vec<OsString>,
    timeout: Duration,
}

impl RecognitionWorkerConfig {
    #[must_use]
    pub fn discover(
        fake: bool,
        audio_root: &Path,
        worker_override: Option<&Path>,
        backend_override: &str,
        model_override: Option<&Path>,
        engine_override: Option<&Path>,
        timeout_override_ms: Option<u64>,
    ) -> Option<Self> {
        let timeout = timeout_override_ms
            .or_else(|| {
                std::env::var("BEASTIE_STT_TIMEOUT_MS")
                    .ok()
                    .and_then(|value| value.parse::<u64>().ok())
            })
            .map_or(DEFAULT_TIMEOUT, Duration::from_millis);
        let executable = worker_override
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("BEASTIE_STT_WORKER").map(PathBuf::from));
        if let Some(executable) = executable {
            return Some(Self::from_overrides(
                executable,
                fake,
                audio_root,
                backend_override,
                model_override,
                engine_override,
                timeout,
            ));
        }
        let current = std::env::current_exe().ok()?;
        let root = current.parent()?;
        let worker = root.join(format!("beastie-stt{}", std::env::consts::EXE_SUFFIX));
        if fake {
            return worker.is_file().then(|| Self {
                executable: worker,
                arguments: vec![
                    "--backend".into(),
                    "fixture".into(),
                    "--audio-root".into(),
                    audio_root.as_os_str().to_owned(),
                ],
                timeout,
            });
        }
        Self::from_package_root(root, audio_root, timeout)
            .ok()
            .flatten()
    }

    fn from_overrides(
        executable: PathBuf,
        fake: bool,
        audio_root: &Path,
        backend: &str,
        model_override: Option<&Path>,
        engine_override: Option<&Path>,
        timeout: Duration,
    ) -> Self {
        let backend = if fake { "fixture" } else { backend };
        let mut arguments = vec![
            "--backend".into(),
            backend.into(),
            "--audio-root".into(),
            audio_root.as_os_str().to_owned(),
        ];
        if !fake {
            if let Some(model) = model_override
                .map(|path| path.as_os_str().to_owned())
                .or_else(|| std::env::var_os("BEASTIE_STT_MODEL_DIR"))
            {
                arguments.extend(["--model-dir".into(), model]);
            }
            if backend == "moonshine"
                && let Some(engine) = engine_override
                    .map(|path| path.as_os_str().to_owned())
                    .or_else(|| std::env::var_os("BEASTIE_MOONSHINE_ENGINE"))
            {
                arguments.extend(["--moonshine-engine".into(), engine]);
            }
        }
        Self {
            executable,
            arguments,
            timeout,
        }
    }

    fn from_package_root(
        root: &Path,
        audio_root: &Path,
        timeout: Duration,
    ) -> io::Result<Option<Self>> {
        let worker = root.join(format!("beastie-stt{}", std::env::consts::EXE_SUFFIX));
        let manifest = root.join("models/manifest.toml");
        let model = root.join("models").join(SELECTED_MODEL_ID);
        if !worker.is_file() || !manifest.is_file() || !model.is_dir() {
            return Ok(None);
        }
        let package_manifest =
            load_package_integrity_manifest(&root.join("package-manifest.json"))?;
        verify_packaged_file(
            root,
            &package_manifest,
            &format!("beastie-stt{}", std::env::consts::EXE_SUFFIX),
        )?;
        verify_packaged_file(root, &package_manifest, "models/manifest.toml")?;
        verify_selected_model(&manifest, &model)?;
        Ok(Some(Self {
            executable: worker,
            arguments: vec![
                "--backend".into(),
                "parakeet".into(),
                "--audio-root".into(),
                audio_root.as_os_str().to_owned(),
                "--model-dir".into(),
                model.into_os_string(),
            ],
            timeout,
        }))
    }
}

fn load_package_integrity_manifest(path: &Path) -> io::Result<PackageIntegrityManifest> {
    let source = fs::read(path)?;
    serde_json::from_slice(&source)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn verify_packaged_file(
    root: &Path,
    manifest: &PackageIntegrityManifest,
    relative: &str,
) -> io::Result<()> {
    let record = manifest.files.get(relative).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("packaged STT file is absent from package manifest: {relative}"),
        )
    })?;
    verify_file(&root.join(relative), record.bytes, &record.sha256)
}

fn verify_selected_model(manifest_path: &Path, model_root: &Path) -> io::Result<()> {
    let source = fs::read_to_string(manifest_path)?;
    let manifest = toml::from_str::<toml::Value>(&source)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let selected = manifest
        .get("selection")
        .and_then(|value| value.get("stt"))
        .and_then(|value| value.get("preferred_candidate"))
        .and_then(toml::Value::as_str);
    if selected != Some(SELECTED_MODEL_ID) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "packaged STT selection does not match the supported model",
        ));
    }
    let candidate = manifest
        .get("stt_candidate")
        .and_then(toml::Value::as_array)
        .and_then(|candidates| {
            candidates.iter().find(|candidate| {
                candidate.get("id").and_then(toml::Value::as_str) == Some(SELECTED_MODEL_ID)
            })
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "STT model is not declared"))?;
    let components = candidate
        .get("components")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "STT components are missing"))?;
    for component in components {
        let file = component
            .get("file")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "STT file is missing"))?;
        if Path::new(file).file_name().and_then(|name| name.to_str()) != Some(file) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "STT component path is not a plain filename",
            ));
        }
        let bytes = component
            .get("bytes")
            .and_then(toml::Value::as_integer)
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "STT size is missing"))?;
        let sha256 = component
            .get("sha256")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "STT hash is missing"))?;
        verify_file(&model_root.join(file), bytes, sha256)?;
    }
    Ok(())
}

fn verify_file(path: &Path, expected_bytes: u64, expected_sha256: &str) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() != expected_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("STT file size or type mismatch: {}", path.display()),
        ));
    }
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    if format!("{:x}", hasher.finalize()) != expected_sha256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("STT file hash mismatch: {}", path.display()),
        ));
    }
    Ok(())
}

enum ManagerCommand {
    Recognize {
        request_id: u64,
        audio: CapturedAudio,
    },
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecognitionCompletion {
    pub request_id: u64,
    pub outcome: RecognitionOutcome,
}

pub struct RecognitionManager {
    config: Option<RecognitionWorkerConfig>,
    commands: SyncSender<ManagerCommand>,
    completions: Receiver<RecognitionCompletion>,
    cancelled: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    pending: bool,
}

impl RecognitionManager {
    #[must_use]
    pub fn new(config: Option<RecognitionWorkerConfig>) -> Self {
        let restart_config = config.clone();
        let (commands, command_receiver) = mpsc::sync_channel(1);
        let (completion_sender, completions) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let manager_cancelled = Arc::clone(&cancelled);
        let thread = thread::spawn(move || {
            run_manager(
                config,
                command_receiver,
                completion_sender,
                &manager_cancelled,
            );
        });
        Self {
            config: restart_config,
            commands,
            completions,
            cancelled,
            thread: Some(thread),
            pending: false,
        }
    }

    pub fn request(&mut self, request_id: u64, audio: CapturedAudio) -> bool {
        if self.pending || request_id == 0 {
            return false;
        }
        match self
            .commands
            .try_send(ManagerCommand::Recognize { request_id, audio })
        {
            Ok(()) => {
                self.pending = true;
                true
            }
            Err(
                TrySendError::Full(ManagerCommand::Recognize { audio, .. })
                | TrySendError::Disconnected(ManagerCommand::Recognize { audio, .. }),
            ) => {
                let _ = fs::remove_file(audio.path);
                false
            }
            Err(
                TrySendError::Full(ManagerCommand::Shutdown)
                | TrySendError::Disconnected(ManagerCommand::Shutdown),
            ) => false,
        }
    }

    pub fn try_recv(&mut self) -> Result<RecognitionCompletion, TryRecvError> {
        match self.completions.try_recv() {
            Ok(completion) => {
                self.pending = false;
                Ok(completion)
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

    /// Discard a displaced creature's request and reclaim its private recording. The old
    /// manager's cancellation path terminates the worker before a fresh owner can submit.
    pub fn cancel_pending(&mut self) {
        if self.pending {
            *self = Self::new(self.config.clone());
        }
    }

    #[cfg(test)]
    fn recv_timeout(&mut self, timeout: Duration) -> RecognitionCompletion {
        let completion = self
            .completions
            .recv_timeout(timeout)
            .expect("recognition completion should arrive");
        self.pending = false;
        completion
    }
}

impl Drop for RecognitionManager {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.commands.try_send(ManagerCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_manager(
    config: Option<RecognitionWorkerConfig>,
    commands: Receiver<ManagerCommand>,
    completions: mpsc::Sender<RecognitionCompletion>,
    cancelled: &AtomicBool,
) {
    let mut worker = None;
    while let Ok(command) = commands.recv() {
        match command {
            ManagerCommand::Recognize { request_id, audio } => {
                let outcome =
                    recognize(config.as_ref(), &mut worker, request_id, &audio, cancelled);
                let _ = fs::remove_file(&audio.path);
                if cancelled.load(Ordering::Acquire) {
                    break;
                }
                if completions
                    .send(RecognitionCompletion {
                        request_id,
                        outcome,
                    })
                    .is_err()
                {
                    break;
                }
            }
            ManagerCommand::Shutdown => break,
        }
    }
    if let Some(mut session) = worker {
        session.terminate();
    }
}

fn recognize(
    config: Option<&RecognitionWorkerConfig>,
    worker: &mut Option<JsonlWorkerSession>,
    request_id: u64,
    audio: &CapturedAudio,
    cancelled: &AtomicBool,
) -> RecognitionOutcome {
    let Some(config) = config else {
        return RecognitionOutcome::Error {
            code: RecognitionErrorCode::BackendUnavailable,
        };
    };
    let request = RecognitionRequest {
        protocol_version: STT_PROTOCOL_VERSION,
        request_id,
        audio_key: audio.key.clone(),
        language: RecognitionLanguage::English,
    };
    if worker.is_none() {
        *worker =
            JsonlWorkerSession::spawn(&config.executable, &config.arguments, MAX_REPLY_BYTES).ok();
    }
    let Some(session) = worker.as_mut() else {
        return RecognitionOutcome::Error {
            code: RecognitionErrorCode::BackendUnavailable,
        };
    };
    let result = session
        .exchange(&request, config.timeout, cancelled)
        .ok()
        .and_then(|line| serde_json::from_str::<RecognitionReply>(&line).ok())
        .and_then(|reply| validate_recognition_reply(&request, reply).ok())
        .map(|reply| reply.outcome);
    if let Some(outcome) = result {
        return outcome;
    }
    if let Some(mut failed) = worker.take() {
        failed.terminate();
    }
    RecognitionOutcome::Error {
        code: RecognitionErrorCode::RecognitionFailed,
    }
}

#[must_use]
pub const fn speech_failure(code: RecognitionErrorCode) -> SpeechInputFailure {
    match code {
        RecognitionErrorCode::UnsupportedLanguage => SpeechInputFailure::UnsupportedLanguage,
        RecognitionErrorCode::BackendUnavailable => SpeechInputFailure::RecognizerUnavailable,
        RecognitionErrorCode::InvalidRequest
        | RecognitionErrorCode::AudioUnavailable
        | RecognitionErrorCode::InvalidAudio
        | RecognitionErrorCode::RecognitionFailed
        | RecognitionErrorCode::Timeout => SpeechInputFailure::RecognitionFailed,
    }
}

const _: () = assert!(MAX_RECOGNITION_TEXT_BYTES <= MAX_REPLY_BYTES);

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    use super::*;

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "beastie-stt-{label}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn audio(root: &Path, fill: u8) -> CapturedAudio {
        fs::create_dir_all(root).unwrap();
        let key = format!("{fill:064x}");
        let path = root.join(format!("{key}.wav"));
        fs::write(&path, b"fixture wav").unwrap();
        CapturedAudio { key, path }
    }

    fn compile_worker(label: &str) -> (PathBuf, PathBuf) {
        let stem = format!(
            "beastie-stt-{label}-worker-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let source = std::env::temp_dir().join(format!("{stem}.rs"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        fs::write(
            &source,
            r#"
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::Duration;

fn request_id(line: &str) -> u64 {
    line.split("\"request_id\":").nth(1)
        .and_then(|tail| tail.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|digits| digits.parse().ok()).unwrap_or(0)
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let mode = args.get(1).map(String::as_str).unwrap_or("normal");
    let marker = args.get(2).map(String::as_str).unwrap_or("");
    let malformed = mode == "malformed-once" && !Path::new(marker).exists();
    if malformed { std::fs::write(marker, "started").unwrap(); }
    let mut count = 0;
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        if mode == "hang" || (mode == "hang-once" && !Path::new(marker).exists()) { std::fs::write(marker, "started").unwrap(); std::thread::sleep(Duration::from_secs(60)); }
        if malformed { println!("not json"); io::stdout().flush().unwrap(); break; }
        count += 1;
        println!("{{\"protocol_version\":1,\"request_id\":{},\"outcome\":{{\"status\":\"recognized\",\"text\":\"reply {}\",\"confidence\":900}}}}", request_id(&line), count);
        io::stdout().flush().unwrap();
    }
}
"#,
        )
        .unwrap();
        let status = Command::new("rustc")
            .args(["--edition", "2024", "-o"])
            .arg(&executable)
            .arg(&source)
            .status()
            .expect("rustc should run");
        assert!(status.success());
        (source, executable)
    }

    fn packaged_runtime_fixture() -> PathBuf {
        let root = temporary_path("package");
        let worker = root.join(format!("beastie-stt{}", std::env::consts::EXE_SUFFIX));
        let model = root.join("models").join(SELECTED_MODEL_ID);
        fs::create_dir_all(&model).unwrap();
        fs::write(&worker, b"worker").unwrap();
        fs::write(model.join("encoder.ort"), b"model").unwrap();
        let digest = format!("{:x}", Sha256::digest(b"model"));
        let model_manifest = format!(
            "[selection.stt]\npreferred_candidate = \"{SELECTED_MODEL_ID}\"\n\n[[stt_candidate]]\nid = \"{SELECTED_MODEL_ID}\"\ncomponents = [{{ file = \"encoder.ort\", bytes = 5, sha256 = \"{digest}\" }}]\n"
        );
        fs::write(root.join("models/manifest.toml"), &model_manifest).unwrap();
        let mut files = BTreeMap::new();
        let contents = b"worker".as_slice();
        files.insert(
            format!("beastie-stt{}", std::env::consts::EXE_SUFFIX),
            serde_json::json!({
                "bytes": contents.len(),
                "sha256": format!("{:x}", Sha256::digest(contents)),
            }),
        );
        files.insert(
            "models/manifest.toml".to_owned(),
            serde_json::json!({
                "bytes": model_manifest.len(),
                "sha256": format!("{:x}", Sha256::digest(model_manifest.as_bytes())),
            }),
        );
        fs::write(
            root.join("package-manifest.json"),
            serde_json::to_vec(&serde_json::json!({ "files": files })).unwrap(),
        )
        .unwrap();
        root
    }

    fn config(
        executable: PathBuf,
        arguments: Vec<OsString>,
        timeout: Duration,
    ) -> RecognitionWorkerConfig {
        RecognitionWorkerConfig {
            executable,
            arguments,
            timeout,
        }
    }

    #[test]
    fn recognition_errors_remain_typed() {
        assert_eq!(
            speech_failure(RecognitionErrorCode::BackendUnavailable),
            SpeechInputFailure::RecognizerUnavailable
        );
        assert_eq!(
            speech_failure(RecognitionErrorCode::UnsupportedLanguage),
            SpeechInputFailure::UnsupportedLanguage
        );
        assert_eq!(
            speech_failure(RecognitionErrorCode::Timeout),
            SpeechInputFailure::RecognitionFailed
        );
    }

    #[test]
    fn packaged_runtime_is_discovered_only_after_model_integrity_passes() {
        let root = packaged_runtime_fixture();
        let audio_root = root.join("private-audio");
        let config =
            RecognitionWorkerConfig::from_package_root(&root, &audio_root, Duration::from_secs(1))
                .expect("valid package")
                .expect("complete package");
        assert!(
            config
                .arguments
                .iter()
                .any(|value| value == audio_root.as_os_str())
        );

        fs::write(
            root.join("models")
                .join(SELECTED_MODEL_ID)
                .join("encoder.ort"),
            b"wrong",
        )
        .unwrap();
        assert!(
            RecognitionWorkerConfig::from_package_root(&root, &audio_root, Duration::from_secs(1))
                .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn packaged_runtime_rejects_tampered_worker() {
        let root = packaged_runtime_fixture();
        fs::write(
            root.join(format!("beastie-stt{}", std::env::consts::EXE_SUFFIX)),
            b"tampered executable",
        )
        .unwrap();
        assert!(
            RecognitionWorkerConfig::from_package_root(
                &root,
                &root.join("private-audio"),
                Duration::from_secs(1)
            )
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn packaged_runtime_rejects_tampered_model_manifest() {
        let root = packaged_runtime_fixture();
        fs::write(root.join("models/manifest.toml"), b"tampered manifest").unwrap();
        assert!(
            RecognitionWorkerConfig::from_package_root(
                &root,
                &root.join("private-audio"),
                Duration::from_secs(1)
            )
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fake_worker_is_persistent_and_audio_is_deleted_after_each_request() {
        let root = temporary_path("reuse-audio");
        let (source, executable) = compile_worker("reuse");
        let mut manager = RecognitionManager::new(Some(config(
            executable.clone(),
            Vec::new(),
            Duration::from_secs(1),
        )));
        let first_audio = audio(&root, 1);
        let first_path = first_audio.path.clone();
        assert!(manager.request(1, first_audio));
        let first = manager.recv_timeout(Duration::from_secs(2));
        assert!(matches!(
            first.outcome,
            RecognitionOutcome::Recognized { ref text, .. } if text == "reply 1"
        ));
        assert!(!first_path.exists());

        let second_audio = audio(&root, 2);
        let second_path = second_audio.path.clone();
        assert!(manager.request(2, second_audio));
        let second = manager.recv_timeout(Duration::from_secs(2));
        assert!(matches!(
            second.outcome,
            RecognitionOutcome::Recognized { ref text, .. } if text == "reply 2"
        ));
        assert!(!second_path.exists());
        drop(manager);
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(source).unwrap();
        fs::remove_file(executable).unwrap();
    }

    #[test]
    fn malformed_worker_restarts_for_the_next_request() {
        let root = temporary_path("restart-audio");
        let marker = temporary_path("restart-marker");
        let (source, executable) = compile_worker("restart");
        let mut manager = RecognitionManager::new(Some(config(
            executable.clone(),
            vec!["malformed-once".into(), marker.clone().into_os_string()],
            Duration::from_secs(1),
        )));
        assert!(manager.request(1, audio(&root, 3)));
        assert!(matches!(
            manager.recv_timeout(Duration::from_secs(2)).outcome,
            RecognitionOutcome::Error {
                code: RecognitionErrorCode::RecognitionFailed
            }
        ));
        assert!(manager.request(2, audio(&root, 4)));
        assert!(matches!(
            manager.recv_timeout(Duration::from_secs(2)).outcome,
            RecognitionOutcome::Recognized { .. }
        ));
        drop(manager);
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(marker).unwrap();
        fs::remove_file(source).unwrap();
        fs::remove_file(executable).unwrap();
    }

    #[test]
    fn drop_interrupts_a_slow_worker_and_removes_private_audio() {
        let root = temporary_path("drop-audio");
        let marker = temporary_path("drop-marker");
        let (source, executable) = compile_worker("drop");
        let mut manager = RecognitionManager::new(Some(config(
            executable.clone(),
            vec!["hang".into(), marker.clone().into_os_string()],
            Duration::from_secs(10),
        )));
        let captured = audio(&root, 5);
        let audio_path = captured.path.clone();
        assert!(manager.request(1, captured));
        for _ in 0..100 {
            if marker.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(marker.exists());
        let started = Instant::now();
        drop(manager);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(!audio_path.exists());
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(marker).unwrap();
        fs::remove_file(source).unwrap();
        fs::remove_file(executable).unwrap();
    }

    #[test]
    fn cancelled_request_releases_its_audio_and_accepts_a_new_owner() {
        let root = temporary_path("cancel-audio");
        let marker = temporary_path("cancel-marker");
        let (source, executable) = compile_worker("cancel");
        let mut manager = RecognitionManager::new(Some(config(
            executable.clone(),
            vec!["hang-once".into(), marker.clone().into_os_string()],
            Duration::from_secs(30),
        )));
        let first = audio(&root, 10);
        let first_path = first.path.clone();
        assert!(manager.request(41, first));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !marker.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(marker.exists());
        manager.cancel_pending();
        assert!(!manager.is_pending());
        assert!(!first_path.exists());
        assert!(matches!(manager.try_recv(), Err(TryRecvError::Empty)));
        assert!(manager.request(42, audio(&root, 11)));
        let completion = manager.recv_timeout(Duration::from_secs(5));
        assert_eq!(completion.request_id, 42);
        assert!(matches!(
            completion.outcome,
            RecognitionOutcome::Recognized { .. }
        ));
        drop(manager);
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(marker).unwrap();
        fs::remove_file(source).unwrap();
        fs::remove_file(executable).unwrap();
    }

    #[test]
    fn slow_recognition_does_not_block_simulation_ticks_or_world_actions() {
        use beastie_session::{
            CommandEnvelope, GameSession, SESSION_PROTOCOL_VERSION, SessionCommand,
        };

        let root = temporary_path("nonblocking-audio");
        let marker = temporary_path("nonblocking-marker");
        let (source, executable) = compile_worker("nonblocking");
        let mut manager = RecognitionManager::new(Some(config(
            executable.clone(),
            vec!["hang".into(), marker.clone().into_os_string()],
            Duration::from_secs(10),
        )));
        assert!(manager.request(1, audio(&root, 6)));

        let mut session = GameSession::new(42, "Mop");
        let started = Instant::now();
        for _ in 0..100 {
            session
                .apply(CommandEnvelope {
                    version: SESSION_PROTOCOL_VERSION,
                    command: SessionCommand::Tick { milliseconds: 16 },
                })
                .unwrap();
        }
        session
            .apply(CommandEnvelope {
                version: SESSION_PROTOCOL_VERSION,
                command: SessionCommand::Comfort,
            })
            .unwrap();
        assert!(started.elapsed() < Duration::from_millis(100));
        assert!(manager.is_pending());

        drop(manager);
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(marker).ok();
        fs::remove_file(source).unwrap();
        fs::remove_file(executable).unwrap();
    }
}
