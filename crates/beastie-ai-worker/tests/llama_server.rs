#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime};

use beastie_ai_worker::{DialogueBackend, LlamaServerBackend, LlamaServerConfig, process_line};
use beastie_protocol::DialogueRequest;

const BERRY_MEMORY: &str = include_str!("../../../fixtures/dialogue/berry-memory.json");

fn fixture_program() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-llama-server.py")
}

fn temporary_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "beastie-server-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn backend(mode: &str, record: &Path, maximum: usize) -> LlamaServerBackend {
    LlamaServerBackend::new(LlamaServerConfig {
        executable: PathBuf::from("python3"),
        model: PathBuf::from("ignored-test-model.gguf"),
        timeout: Duration::from_millis(300),
        max_output_bytes: maximum,
        cpu_only: true,
        extra_args: vec![
            fixture_program().into_os_string(),
            OsString::from("--fake-mode"),
            OsString::from(mode),
            OsString::from("--fake-record"),
            record.to_path_buf().into_os_string(),
        ],
        supervisor: None,
    })
}

fn record_lines(record: &PathBuf) -> Vec<String> {
    fs::read_to_string(record)
        .expect("server record should exist")
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn warm_server_reuses_one_pid_and_receives_flags_auth_and_bounded_payload() {
    let record = temporary_path("warm");
    let request: DialogueRequest =
        serde_json::from_str(BERRY_MEMORY).expect("request should parse");
    let mut server = backend("valid", &record, 8 * 1024);
    server
        .generate(&request)
        .expect("first response should work");
    server
        .generate(&request)
        .expect("second response should reuse server");

    let lines = record_lines(&record);
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("start "))
            .count(),
        1
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("POST /v1/chat/completions"))
            .count(),
        2
    );
    let start = lines
        .iter()
        .find(|line| line.starts_with("start "))
        .expect("start record");
    assert!(start.contains("--host 127.0.0.1"));
    assert!(start.contains("--device none --no-op-offload -ngl 0"));
    let post = lines
        .iter()
        .find(|line| line.contains("POST /v1/chat/completions"))
        .expect("post record");
    assert!(post.contains("auth=True"));
    assert!(post.contains("\"enable_thinking\":false"));
    assert!(post.contains("\"reasoning_effort\":\"none\""));
    fs::remove_file(record).expect("record should be removable");
}

#[test]
fn malformed_timeout_and_oversized_responses_restart_once_then_fallback() {
    for mode in ["malformed", "timeout", "oversized"] {
        let record = temporary_path(mode);
        let mut server = backend(mode, &record, 512);
        let reply = process_line(BERRY_MEMORY, &mut server);
        assert_eq!(reply.say, "berry remains bad.", "{mode}");
        assert_eq!(reply.recalled_memory.map(|id| id.0), Some(41));
        let starts = record_lines(&record)
            .iter()
            .filter(|line| line.starts_with("start "))
            .count();
        assert_eq!(starts, 2, "{mode} should replace the failed sidecar once");
        fs::remove_file(record).expect("record should be removable");
    }
}

#[test]
fn dropping_backend_reaps_server_process() {
    let record = temporary_path("drop");
    {
        let request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY).expect("request should parse");
        let mut server = backend("valid", &record, 8 * 1024);
        server
            .generate(&request)
            .expect("response should start server");
    }
    let start = record_lines(&record)
        .into_iter()
        .find(|line| line.starts_with("start "))
        .expect("start record");
    let pid = start
        .split_whitespace()
        .find_map(|part| part.strip_prefix("pid="))
        .expect("pid should be recorded");
    for _ in 0..20 {
        let alive = Command::new("kill")
            .args(["-0", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("kill should run")
            .success();
        if !alive {
            fs::remove_file(record).expect("record should be removable");
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("server PID {pid} remained after backend drop");
}

#[test]
fn supervisor_reaps_server_when_worker_is_killed() {
    let record = temporary_path("crash");
    let mut child = Command::new(env!("CARGO_BIN_EXE_beastie-ai-worker"))
        .env("BEASTIE_AI_BACKEND", "llama-server")
        .env("BEASTIE_AI_MODEL", "ignored-test-model.gguf")
        .env("BEASTIE_LLAMA_SERVER", "python3")
        .env("BEASTIE_AI_TIMEOUT_MS", "1000")
        .arg("--llama-server-arg")
        .arg(fixture_program())
        .arg("--llama-server-arg")
        .arg("--fake-record")
        .arg("--llama-server-arg")
        .arg(&record)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("worker should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(BERRY_MEMORY.as_bytes())
        .expect("request should be written");
    let start = (0..50).find_map(|_| {
        let start = fs::read_to_string(&record).ok().and_then(|contents| {
            contents
                .lines()
                .find(|line| line.starts_with("start "))
                .map(str::to_owned)
        });
        if start.is_none() {
            thread::sleep(Duration::from_millis(10));
        }
        start
    });
    let start = start.expect("server should have started before worker is killed");
    child.kill().expect("worker should be killable");
    child.wait().expect("worker should exit");
    let pid = start
        .split_whitespace()
        .find_map(|part| part.strip_prefix("pid="))
        .expect("pid should be recorded");
    for _ in 0..50 {
        let alive = Command::new("kill")
            .args(["-0", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("kill should run")
            .success();
        if !alive {
            fs::remove_file(record).expect("record should be removable");
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("server PID {pid} remained after worker crash");
}

#[test]
fn environment_selects_warm_server_without_changing_jsonl_transport() {
    let record = temporary_path("env");
    let mut child = Command::new(env!("CARGO_BIN_EXE_beastie-ai-worker"))
        .env("BEASTIE_AI_BACKEND", "llama-server")
        .env("BEASTIE_AI_MODEL", "ignored-test-model.gguf")
        .env("BEASTIE_LLAMA_SERVER", "python3")
        .env("BEASTIE_AI_TIMEOUT_MS", "1000")
        .arg("--llama-server-arg")
        .arg(fixture_program())
        .arg("--llama-server-arg")
        .arg("--fake-record")
        .arg("--llama-server-arg")
        .arg(&record)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("worker should start");
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(BERRY_MEMORY.as_bytes())
        .expect("request should be written");
    let output = child.wait_with_output().expect("worker should finish");
    assert!(output.status.success());
    let reply: beastie_protocol::DialogueReply =
        serde_json::from_slice(&output.stdout).expect("worker should emit one JSONL reply");
    assert_eq!(reply.say, "berry remains bad.");
    fs::remove_file(record).expect("record should be removable");
}
