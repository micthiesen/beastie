use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use beastie_protocol::{DialogueReply, DialogueRequest};

use crate::process::{ContainedChild, UnixProcessGroup};
use crate::speech::{SPEECH_MAX_TOKENS, SPEECH_TEMPERATURE, parse_speech_line, speech_prompt};
use crate::{BackendError, DialogueBackend, LlamaCppConfig, bounded_llama_threads};

#[derive(Debug)]
pub struct LlamaCppBackend {
    config: LlamaCppConfig,
}

impl LlamaCppBackend {
    #[must_use]
    pub fn new(config: LlamaCppConfig) -> Self {
        Self { config }
    }

    fn attempt(&self, request: &DialogueRequest) -> Result<DialogueReply, BackendError> {
        let prompt = speech_prompt(request);
        let seed = request.request_id.to_string();
        let temperature = SPEECH_TEMPERATURE.to_string();
        let predict = SPEECH_MAX_TOKENS.to_string();
        let threads = bounded_llama_threads(self.config.threads).to_string();
        let mut command = Command::new(&self.config.executable);
        command
            .args(&self.config.extra_args)
            .arg("--model")
            .arg(&self.config.model)
            .args([
                "--prompt",
                &prompt,
                "--n-predict",
                &predict,
                "--temp",
                &temperature,
                "--seed",
                &seed,
                "--no-display-prompt",
                "--simple-io",
                "--no-warmup",
                "--no-show-timings",
                "--reasoning",
                "off",
                "--reasoning-budget",
                "0",
                "--offline",
                "--log-disable",
                "--single-turn",
                "--no-conversation",
            ])
            .args(["--threads", &threads, "--threads-batch", &threads])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if self.config.cpu_only {
            command.args(["--device", "none", "--no-op-offload", "-ngl", "0"]);
        }

        let child = ContainedChild::spawn(&mut command, UnixProcessGroup::New)
            .map_err(BackendError::Start)?;
        let output = collect_bounded(child, self.config.timeout, self.config.max_output_bytes)?;
        let text = String::from_utf8(output).map_err(|_| BackendError::Utf8)?;
        parse_speech_line(request, &text)
    }
}

impl DialogueBackend for LlamaCppBackend {
    fn generate(&mut self, request: &DialogueRequest) -> Result<DialogueReply, BackendError> {
        self.attempt(request).or_else(|_| self.attempt(request))
    }
}

fn collect_bounded(
    mut child: ContainedChild,
    timeout: Duration,
    maximum: usize,
) -> Result<Vec<u8>, BackendError> {
    let stdout = child
        .stdout
        .take()
        .ok_or(BackendError::Read(std::io::Error::other(
            "child stdout was not piped",
        )))?;
    let oversized = Arc::new(AtomicBool::new(false));
    let reader_oversized = Arc::clone(&oversized);
    let reader = thread::spawn(move || read_bounded(stdout, maximum, &reader_oversized));
    let started = Instant::now();

    let status = loop {
        if oversized.load(Ordering::Relaxed) {
            child.terminate_tree();
            break Err(BackendError::OutputTooLarge);
        }
        if started.elapsed() >= timeout {
            child.terminate_tree();
            break Err(BackendError::Timeout);
        }
        match child.try_wait().map_err(BackendError::Wait)? {
            Some(status) => break Ok(status),
            None => thread::sleep(Duration::from_millis(2)),
        }
    };

    child.close_descendants();

    let output = reader
        .join()
        .map_err(|_| BackendError::Read(std::io::Error::other("stdout reader panicked")))??;
    let status = status?;
    if oversized.load(Ordering::Relaxed) {
        return Err(BackendError::OutputTooLarge);
    }
    if !status.success() {
        return Err(BackendError::ExitFailure);
    }
    Ok(output)
}

fn read_bounded(
    mut input: impl Read,
    maximum: usize,
    oversized: &AtomicBool,
) -> Result<Vec<u8>, BackendError> {
    let mut output = Vec::with_capacity(maximum.min(4096));
    let mut buffer = [0_u8; 1024];
    loop {
        let read = input.read(&mut buffer).map_err(BackendError::Read)?;
        if read == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(read) > maximum {
            oversized.store(true, Ordering::Relaxed);
        } else {
            output.extend_from_slice(&buffer[..read]);
        }
    }
}
