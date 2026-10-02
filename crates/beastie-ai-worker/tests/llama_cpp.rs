#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use beastie_ai_worker::{DialogueBackend, LlamaCppBackend, LlamaCppConfig, process_line};
use beastie_protocol::{DialogueRequest, validate_reply};

const BERRY_MEMORY: &str = include_str!("../../../fixtures/dialogue/berry-memory.json");

fn fixture_program() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-llama.sh")
}

fn temporary_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "beastie-worker-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn backend(mode: &str, extra: Vec<OsString>) -> LlamaCppBackend {
    let mut args = vec![OsString::from("--fake-mode"), OsString::from(mode)];
    args.extend(extra);
    LlamaCppBackend::new(LlamaCppConfig {
        executable: fixture_program(),
        model: PathBuf::from("ignored-test-model.gguf"),
        // Success cases test the protocol, not the host's shell-startup latency.
        // Keep the deliberately stalled fixture's short deadline as the timeout oracle.
        timeout: if mode == "timeout" {
            Duration::from_millis(40)
        } else {
            Duration::from_secs(2)
        },
        max_output_bytes: 512,
        cpu_only: true,
        threads: 3,
        extra_args: args,
    })
}

#[test]
fn fake_executable_reply_passes_strict_protocol_validation() {
    let request: DialogueRequest =
        serde_json::from_str(BERRY_MEMORY).expect("request should parse");
    let reply = backend("valid", Vec::new())
        .generate(&request)
        .expect("fake generation should pass");
    assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
}

#[test]
fn malformed_first_output_is_retried_once() {
    let state = temporary_path("retry");
    let request: DialogueRequest =
        serde_json::from_str(BERRY_MEMORY).expect("request should parse");
    let reply = backend(
        "retry",
        vec![
            OsString::from("--fake-state"),
            state.clone().into_os_string(),
        ],
    )
    .generate(&request)
    .expect("second fake generation should pass");
    assert_eq!(reply.request_id, request.request_id);
    fs::remove_file(state).expect("retry state should exist");
}

#[test]
fn timeout_and_oversized_output_use_authored_fallback() {
    for mode in ["timeout", "oversized"] {
        let reply = process_line(BERRY_MEMORY, &mut backend(mode, Vec::new()));
        assert_eq!(reply.say, "berry remains bad.");
        assert_eq!(reply.request_id, 41);
        assert_eq!(reply.recalled_memory.map(|id| id.0), Some(41));
    }
}

#[test]
fn prohibited_echo_falls_back_but_ordinary_profanity_passes() {
    let echo = process_line(BERRY_MEMORY, &mut backend("echo", Vec::new()));
    assert_eq!(echo.say, "berry remains bad.");

    let profanity = process_line(BERRY_MEMORY, &mut backend("profanity", Vec::new()));
    assert_eq!(profanity.say, "Damn berry.");
}

#[test]
fn cpu_only_flags_reach_the_replaceable_process() {
    let record = temporary_path("args");
    let request: DialogueRequest =
        serde_json::from_str(BERRY_MEMORY).expect("request should parse");
    backend(
        "valid",
        vec![
            OsString::from("--fake-record"),
            record.clone().into_os_string(),
        ],
    )
    .generate(&request)
    .expect("fake generation should pass");
    let arguments = fs::read_to_string(&record).expect("arguments should be recorded");
    assert!(arguments.lines().any(|argument| argument == "--device"));
    assert!(
        arguments
            .lines()
            .any(|argument| argument == "--no-op-offload")
    );
    assert!(arguments.lines().any(|argument| argument == "-ngl"));
    let arguments = arguments.lines().collect::<Vec<_>>();
    assert!(arguments.windows(2).any(|pair| pair == ["--threads", "3"]));
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["--threads-batch", "3"])
    );
    fs::remove_file(record).expect("argument record should exist");
}

#[test]
fn environment_selects_llama_backend_without_changing_jsonl_transport() {
    let record = temporary_path("env-threads");
    let mut child = Command::new(env!("CARGO_BIN_EXE_beastie-ai-worker"))
        .env("BEASTIE_AI_BACKEND", "llama-cpp")
        .env("BEASTIE_AI_MODEL", "ignored-test-model.gguf")
        .env("BEASTIE_LLAMA_CLI", fixture_program())
        .env("BEASTIE_AI_TIMEOUT_MS", "1000")
        .env("BEASTIE_AI_THREADS", "4")
        .arg("--llama-arg")
        .arg("--fake-record")
        .arg("--llama-arg")
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
    let arguments = fs::read_to_string(&record).expect("arguments should be recorded");
    let arguments = arguments.lines().collect::<Vec<_>>();
    assert!(arguments.windows(2).any(|pair| pair == ["--threads", "4"]));
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["--threads-batch", "4"])
    );
    fs::remove_file(record).expect("argument record should exist");
}

#[test]
fn cli_and_environment_thread_overrides_are_bounded() {
    for mut command in [
        {
            let mut command = Command::new(env!("CARGO_BIN_EXE_beastie-ai-worker"));
            command.args(["--threads", "0"]);
            command
        },
        {
            let mut command = Command::new(env!("CARGO_BIN_EXE_beastie-ai-worker"));
            command.env("BEASTIE_AI_THREADS", "257");
            command
        },
    ] {
        assert!(
            !command
                .output()
                .expect("worker should execute")
                .status
                .success()
        );
    }
}
