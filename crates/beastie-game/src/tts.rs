use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use beastie_protocol::{
    TTS_PROTOCOL_VERSION, TtsOutcome, TtsReply, TtsRequest, TtsVoiceSettings, validate_tts_reply,
    validate_tts_request,
};

const MAX_REPLY_BYTES: usize = 1_024;
const MAX_WAV_BYTES: u64 = 44 + 192_000 * 30 * 2;

#[derive(Debug, Clone)]
pub struct TtsWorkerConfig {
    executable: PathBuf,
    arguments: Vec<OsString>,
    cache_root: PathBuf,
    reply_timeout: Duration,
}

impl TtsWorkerConfig {
    #[must_use]
    pub fn discover(explicit: bool, default_cache_root: PathBuf) -> Option<Self> {
        if explicit && let Some(config) = Self::from_environment(default_cache_root.clone()) {
            return Some(config);
        }
        let executable = std::env::current_exe().ok()?;
        Self::from_package_root(executable.parent()?, default_cache_root)
    }

    fn from_environment(default_cache_root: PathBuf) -> Option<Self> {
        let executable = PathBuf::from(std::env::var_os("BEASTIE_TTS_WORKER")?);
        let cache_root = std::env::var_os("BEASTIE_TTS_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or(default_cache_root);
        let reply_timeout = std::env::var("BEASTIE_TTS_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map_or(Duration::from_secs(60), Duration::from_millis);
        let mut arguments = vec![
            OsString::from("--cache-dir"),
            cache_root.clone().into_os_string(),
        ];
        for (environment, argument) in [
            ("BEASTIE_TTS_BACKEND", "--backend"),
            ("BEASTIE_ESPEAK_NG", "--espeak"),
            ("BEASTIE_ESPEAK_DATA", "--espeak-data"),
            ("BEASTIE_ESPEAK_VOICE", "--voice"),
            ("BEASTIE_TTS_MODEL_DIR", "--model-dir"),
        ] {
            if let Some(value) = std::env::var_os(environment) {
                arguments.push(OsString::from(argument));
                arguments.push(value);
            }
        }
        Some(Self {
            executable,
            arguments,
            cache_root,
            reply_timeout,
        })
    }

    fn from_package_root(root: &Path, cache_root: PathBuf) -> Option<Self> {
        let executable = root.join(format!("beastie-tts{}", std::env::consts::EXE_SUFFIX));
        let espeak = root
            .join("runtime")
            .join(format!("espeak-ng{}", std::env::consts::EXE_SUFFIX));
        let data = root.join("runtime/espeak-ng-data");
        let license = root.join("runtime/espeak-ng-COPYING");
        if !executable.is_file()
            || !espeak.is_file()
            || !data.is_dir()
            || !license.is_file()
            || fs::read_dir(&data).ok()?.next().is_none()
        {
            return None;
        }
        Some(Self {
            executable,
            arguments: vec![
                OsString::from("--backend"),
                OsString::from("espeak"),
                OsString::from("--espeak"),
                espeak.into_os_string(),
                OsString::from("--espeak-data"),
                data.into_os_string(),
                OsString::from("--cache-dir"),
                cache_root.clone().into_os_string(),
            ],
            cache_root,
            reply_timeout: Duration::from_secs(60),
        })
    }

    #[cfg(test)]
    fn new(executable: PathBuf, arguments: Vec<OsString>, cache_root: PathBuf) -> Self {
        Self {
            executable,
            arguments,
            cache_root,
            reply_timeout: Duration::from_secs(1),
        }
    }
}

enum ManagerCommand {
    Request(TtsRequest),
    Shutdown,
}

#[derive(Debug)]
pub struct TtsCompletion {
    pub request_id: u64,
    pub wav: Option<Arc<[u8]>>,
}

pub struct TtsManager {
    commands: SyncSender<ManagerCommand>,
    completions: Receiver<TtsCompletion>,
    pending: bool,
    latest_request_id: u64,
    enabled: bool,
    next_request_id: u64,
    cancelled: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl TtsManager {
    #[must_use]
    pub fn new(config: Option<TtsWorkerConfig>) -> Self {
        let enabled = config.is_some();
        let (commands, command_receiver) = mpsc::sync_channel(1);
        let (completion_sender, completions) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let thread_cancelled = Arc::clone(&cancelled);
        let thread = thread::spawn(move || {
            run_manager(
                config,
                command_receiver,
                completion_sender,
                &thread_cancelled,
            );
        });
        Self {
            commands,
            completions,
            pending: false,
            latest_request_id: 0,
            enabled,
            next_request_id: 1,
            cancelled,
            thread: Some(thread),
        }
    }

    pub fn request(&mut self, text: String) -> bool {
        if !self.enabled {
            return false;
        }
        let request = TtsRequest {
            protocol_version: TTS_PROTOCOL_VERSION,
            request_id: self.next_request_id,
            text,
            settings: TtsVoiceSettings::default(),
        };
        if validate_tts_request(&request).is_err() {
            return false;
        }
        let request_id = request.request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.latest_request_id = request_id;
        if self
            .commands
            .try_send(ManagerCommand::Request(request))
            .is_err()
        {
            self.pending = false;
            return false;
        }
        self.pending = true;
        true
    }

    pub fn try_recv(&mut self) -> Result<TtsCompletion, TryRecvError> {
        loop {
            match self.completions.try_recv() {
                Ok(completion) if completion.request_id < self.latest_request_id => continue,
                Ok(completion) => {
                    self.pending = false;
                    return Ok(completion);
                }
                Err(TryRecvError::Disconnected) => {
                    self.pending = false;
                    return Err(TryRecvError::Disconnected);
                }
                Err(TryRecvError::Empty) => return Err(TryRecvError::Empty),
            }
        }
    }

    #[cfg(test)]
    fn recv_timeout(&mut self, timeout: Duration) -> TtsCompletion {
        let completion = self
            .completions
            .recv_timeout(timeout)
            .expect("TTS completion should arrive");
        self.pending = false;
        completion
    }
}

impl Drop for TtsManager {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.commands.send(ManagerCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_manager(
    config: Option<TtsWorkerConfig>,
    commands: Receiver<ManagerCommand>,
    completions: Sender<TtsCompletion>,
    cancelled: &AtomicBool,
) {
    let mut worker = None;
    while let Ok(command) = commands.recv() {
        match command {
            ManagerCommand::Request(request) => {
                let request_id = request.request_id;
                let wav = config.as_ref().and_then(|config| {
                    exchange_with_recovery(config, &mut worker, &request, cancelled)
                });
                if completions.send(TtsCompletion { request_id, wav }).is_err() {
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
    config: &TtsWorkerConfig,
    worker: &mut Option<WorkerSession>,
    request: &TtsRequest,
    cancelled: &AtomicBool,
) -> Option<Arc<[u8]>> {
    if worker.is_none() {
        *worker = WorkerSession::spawn(config).ok();
    }
    let cache_key = worker.as_mut().and_then(|session| {
        session
            .exchange(request, config.reply_timeout, cancelled)
            .ok()
    });
    let Some(cache_key) = cache_key else {
        if let Some(mut failed) = worker.take() {
            failed.terminate();
        }
        return None;
    };
    read_scoped_cache_file(&config.cache_root, &cache_key).ok()
}

fn read_scoped_cache_file(cache_root: &Path, cache_key: &str) -> io::Result<Arc<[u8]>> {
    let root = fs::canonicalize(cache_root)?;
    let path = fs::canonicalize(cache_root.join(format!("{cache_key}.wav")))?;
    if path.parent() != Some(root.as_path()) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "TTS cache response escaped its configured root",
        ));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(&path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || !(44..=MAX_WAV_BYTES).contains(&metadata.len()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "TTS cache response has an invalid size",
        ));
    }
    let mut header = [0_u8; 44];
    file.read_exact(&mut header)?;
    let data_size = u32::from_le_bytes(header[40..44].try_into().unwrap()) as u64;
    let sample_rate = u32::from_le_bytes(header[24..28].try_into().unwrap());
    let valid_header = &header[0..4] == b"RIFF"
        && &header[8..12] == b"WAVE"
        && &header[12..16] == b"fmt "
        && u32::from_le_bytes(header[16..20].try_into().unwrap()) == 16
        && u16::from_le_bytes(header[20..22].try_into().unwrap()) == 1
        && u16::from_le_bytes(header[22..24].try_into().unwrap()) == 1
        && (8_000..=192_000).contains(&sample_rate)
        && u32::from_le_bytes(header[28..32].try_into().unwrap()) == sample_rate * 2
        && u16::from_le_bytes(header[32..34].try_into().unwrap()) == 2
        && u16::from_le_bytes(header[34..36].try_into().unwrap()) == 16
        && &header[36..40] == b"data"
        && data_size == metadata.len().saturating_sub(44)
        && data_size > 0
        && data_size <= u64::from(sample_rate) * 30 * 2;
    if !valid_header {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "TTS cache response is not a supported PCM WAV",
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.rewind()?;
    file.read_to_end(&mut bytes)?;
    Ok(Arc::from(bytes))
}

struct WorkerSession {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<io::Result<String>>,
    exchanged: bool,
}

impl WorkerSession {
    fn spawn(config: &TtsWorkerConfig) -> io::Result<Self> {
        let mut command = Command::new(&config.executable);
        command
            .args(&config.arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        configure_process_containment(&mut command);
        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("TTS worker stdin unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("TTS worker stdout unavailable"))?;
        let (sender, lines) = mpsc::sync_channel(1);
        thread::spawn(move || read_worker_lines(BufReader::new(stdout), sender));
        Ok(Self {
            child,
            stdin,
            lines,
            exchanged: false,
        })
    }

    fn exchange(
        &mut self,
        request: &TtsRequest,
        timeout: Duration,
        cancelled: &AtomicBool,
    ) -> io::Result<String> {
        if self.exchanged {
            match self.lines.try_recv() {
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "TTS worker emitted an unsolicited reply",
                    ));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "TTS worker reply reader disconnected",
                    ));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        serde_json::to_writer(&mut self.stdin, request)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        self.exchanged = true;
        let started = Instant::now();
        let line = loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "TTS shutting down",
                ));
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "TTS reply timed out",
                ));
            }
            match self
                .lines
                .recv_timeout(remaining.min(Duration::from_millis(10)))
            {
                Ok(line) => break line?,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error) => return Err(io::Error::new(io::ErrorKind::BrokenPipe, error)),
            }
        };
        let reply = serde_json::from_str::<TtsReply>(&line)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let reply = validate_tts_reply(request, reply)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        match reply.outcome {
            TtsOutcome::Ready { cache_key } => Ok(cache_key),
            TtsOutcome::Error { code } => Err(io::Error::other(format!("TTS failed: {code:?}"))),
        }
    }

    fn terminate(&mut self) {
        terminate_child(&mut self.child);
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

fn read_worker_lines(mut reader: impl BufRead, sender: SyncSender<io::Result<String>>) {
    loop {
        let result = read_reply_line(&mut reader);
        let finished = result.is_err() || matches!(result, Ok(None));
        let line = result.and_then(|line| {
            line.ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "TTS worker exited"))
        });
        if sender.send(line).is_err() || finished {
            break;
        }
    }
}

fn read_reply_line(reader: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "unterminated TTS reply",
                ))
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |index| index + 1);
        if bytes.len().saturating_add(consumed) > MAX_REPLY_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "TTS reply too large",
            ));
        }
        bytes.extend_from_slice(&available[..consumed]);
        reader.consume(consumed);
        if newline.is_some() {
            bytes.pop();
            return String::from_utf8(bytes)
                .map(Some)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn fake_worker_is_reused_and_wav_bytes_are_copied_from_scoped_cache() {
        let root = temporary_path("cache");
        fs::create_dir_all(&root).unwrap();
        let (source, executable) = compile_worker();
        let config = TtsWorkerConfig::new(
            executable.clone(),
            vec![root.clone().into_os_string()],
            root.clone(),
        );
        let mut manager = TtsManager::new(Some(config));

        assert!(manager.request("hello".to_owned()));
        let first = manager.recv_timeout(Duration::from_secs(2));
        assert_eq!(first.request_id, 1);
        assert_eq!(first.wav.as_deref().map(<[u8]>::len), Some(46));

        assert!(manager.request("again".to_owned()));
        let second = manager.recv_timeout(Duration::from_secs(2));
        assert_eq!(second.request_id, 2);
        assert_eq!(second.wav.as_deref().map(<[u8]>::len), Some(46));
        assert!(root.join(format!("{}.wav", "a".repeat(64))).is_file());
        assert!(root.join(format!("{}.wav", "b".repeat(64))).is_file());

        drop(manager);
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(source).unwrap();
        fs::remove_file(executable).unwrap();
    }

    #[test]
    fn superseding_request_discards_the_older_completion() {
        let root = temporary_path("supersede-cache");
        fs::create_dir_all(&root).unwrap();
        let (source, executable) = compile_worker();
        let config = TtsWorkerConfig::new(
            executable.clone(),
            vec![root.clone().into_os_string()],
            root.clone(),
        );
        let mut manager = TtsManager::new(Some(config));
        assert!(manager.request("old".to_owned()));

        let enqueue_started = Instant::now();
        while !manager.request("new".to_owned()) {
            assert!(enqueue_started.elapsed() < Duration::from_secs(2));
            thread::sleep(Duration::from_millis(5));
        }
        let expected_request_id = manager.latest_request_id;

        let started = Instant::now();
        let completion = loop {
            match manager.try_recv() {
                Ok(completion) => break completion,
                Err(TryRecvError::Empty) if started.elapsed() < Duration::from_secs(2) => {
                    thread::sleep(Duration::from_millis(5));
                }
                result => panic!("latest completion did not arrive: {result:?}"),
            }
        };
        assert_eq!(completion.request_id, expected_request_id);

        drop(manager);
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(source).unwrap();
        fs::remove_file(executable).unwrap();
    }

    #[test]
    fn unsolicited_worker_output_restarts_the_worker() {
        let root = temporary_path("extra-output-cache");
        fs::create_dir_all(&root).unwrap();
        let (source, executable) = compile_worker_with_mode("extra");
        let config = TtsWorkerConfig::new(
            executable.clone(),
            vec![root.clone().into_os_string(), OsString::from("extra")],
            root.clone(),
        );
        let mut manager = TtsManager::new(Some(config));

        assert!(manager.request("hello".to_owned()));
        assert_eq!(manager.recv_timeout(Duration::from_secs(2)).request_id, 1);
        assert!(manager.request("again".to_owned()));
        let second = manager.recv_timeout(Duration::from_secs(2));
        assert_eq!(second.request_id, 2);
        assert!(second.wav.is_none());
        assert!(manager.request("recovered".to_owned()));
        assert!(manager.recv_timeout(Duration::from_secs(2)).wav.is_some());

        drop(manager);
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(source).unwrap();
        fs::remove_file(executable).unwrap();
    }

    #[test]
    fn rejected_newer_request_invalidates_older_completion() {
        let root = temporary_path("queue-cache");
        fs::create_dir_all(&root).unwrap();
        let (source, executable) = compile_worker();
        let config = TtsWorkerConfig::new(
            executable.clone(),
            vec![root.clone().into_os_string()],
            root.clone(),
        );
        let mut manager = TtsManager::new(Some(config));
        assert!(manager.request("one".to_owned()));
        let mut rejected = false;
        for index in 0..100 {
            if !manager.request(format!("queued-{index}")) {
                rejected = true;
                break;
            }
        }
        assert!(rejected, "bounded queue should reject a newer request");
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(1) {
            assert!(matches!(manager.try_recv(), Err(TryRecvError::Empty)));
            thread::sleep(Duration::from_millis(5));
        }
        drop(manager);
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(source).unwrap();
        fs::remove_file(executable).unwrap();
    }

    #[test]
    fn cache_reader_rejects_corrupt_wav_header() {
        let root = temporary_path("corrupt-cache");
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("{}.wav", "c".repeat(64)));
        fs::write(&path, vec![0_u8; 46]).unwrap();
        assert!(read_scoped_cache_file(&root, &"c".repeat(64)).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn complete_packaged_espeak_bundle_is_discovered_but_partial_bundle_is_not() {
        let root = temporary_path("package-root");
        let runtime = root.join("runtime");
        let data = runtime.join("espeak-ng-data");
        fs::create_dir_all(&data).unwrap();
        fs::write(
            root.join(format!("beastie-tts{}", std::env::consts::EXE_SUFFIX)),
            b"worker",
        )
        .unwrap();
        fs::write(
            runtime.join(format!("espeak-ng{}", std::env::consts::EXE_SUFFIX)),
            b"runtime",
        )
        .unwrap();
        fs::write(data.join("voices"), b"voice data").unwrap();
        let license = runtime.join("espeak-ng-COPYING");
        fs::write(&license, b"GPLv3").unwrap();
        let cache = temporary_path("package-cache");

        let config = TtsWorkerConfig::from_package_root(&root, cache.clone())
            .expect("complete package should auto-enable TTS");
        assert_eq!(config.cache_root, cache);
        assert!(
            config
                .arguments
                .iter()
                .any(|argument| argument == "--espeak-data")
        );

        fs::remove_file(license).unwrap();
        assert!(TtsWorkerConfig::from_package_root(&root, temporary_path("unused")).is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn cache_reader_rejects_final_symlink() {
        use std::os::unix::fs::symlink;

        let root = temporary_path("symlink-cache");
        let outside = temporary_path("outside.wav");
        fs::create_dir_all(&root).unwrap();
        fs::write(&outside, vec![0_u8; 46]).unwrap();
        symlink(&outside, root.join(format!("{}.wav", "d".repeat(64)))).unwrap();
        assert!(read_scoped_cache_file(&root, &"d".repeat(64)).is_err());
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(outside).unwrap();
    }

    fn temporary_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "beastie-tts-game-{label}-{}-{nonce}-{sequence}",
            std::process::id()
        ))
    }

    fn compile_worker() -> (PathBuf, PathBuf) {
        compile_worker_with_mode("")
    }

    fn compile_worker_with_mode(_mode: &str) -> (PathBuf, PathBuf) {
        let source = temporary_path("worker.rs");
        let executable = temporary_path(&format!("worker{}", std::env::consts::EXE_SUFFIX));
        fs::write(
            &source,
            r#"
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).unwrap());
    let extra = std::env::args().nth(2).is_some_and(|value| value == "extra");
    let mut count = 0_u8;
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let marker = "\"request_id\":";
        let start = line.find(marker).unwrap() + marker.len();
        let id = line[start..].chars().take_while(char::is_ascii_digit).collect::<String>();
        count += 1;
        if count == 1 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let key = if count == 1 { "a".repeat(64) } else { "b".repeat(64) };
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&38_u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&22_050_u32.to_le_bytes());
        wav.extend_from_slice(&44_100_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&2_u32.to_le_bytes());
        wav.extend_from_slice(&0_i16.to_le_bytes());
        std::fs::write(root.join(format!("{key}.wav")), wav).unwrap();
        println!("{{\"protocol_version\":1,\"request_id\":{id},\"status\":\"ready\",\"cache_key\":\"{key}\"}}");
        if extra {
            println!("{{\"protocol_version\":1,\"request_id\":{id},\"status\":\"ready\",\"cache_key\":\"{key}\"}}");
        }
        io::stdout().flush().unwrap();
    }
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
}
