//! Replaceable dialogue backends for the isolated AI worker process.

mod bounded;
mod llama_cpp;
mod prompt;
pub mod tts;

use std::ffi::OsString;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

use beastie_protocol::{
    DialogueReply, DialogueRequest, Gesture, PROTOCOL_VERSION, constrained_fallback_reply,
    fallback_reply, validate_reply, validate_request,
};
use bounded::{BoundedLine, read_bounded_line};

const MAX_DIALOGUE_LINE_BYTES: usize = 16 * 1024;

pub use llama_cpp::LlamaCppBackend;

#[derive(Debug, Clone)]
pub struct LlamaCppConfig {
    pub executable: PathBuf,
    pub model: PathBuf,
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub cpu_only: bool,
    pub extra_args: Vec<OsString>,
}

#[derive(Debug)]
pub enum BackendError {
    Start(io::Error),
    Wait(io::Error),
    Read(io::Error),
    Timeout,
    OutputTooLarge,
    ExitFailure,
    Utf8,
    MalformedReply,
    InvalidReply,
    UnsafeReply,
}

pub trait DialogueBackend {
    fn generate(&mut self, request: &DialogueRequest) -> Result<DialogueReply, BackendError>;
}

#[derive(Debug, Default)]
pub struct FixtureBackend;

impl DialogueBackend for FixtureBackend {
    fn generate(&mut self, request: &DialogueRequest) -> Result<DialogueReply, BackendError> {
        Ok(fixture_reply(request))
    }
}

pub fn run_jsonl(
    mut input: impl BufRead,
    mut output: impl Write,
    backend: &mut dyn DialogueBackend,
) -> io::Result<()> {
    while let Some(line) = read_bounded_line(&mut input, MAX_DIALOGUE_LINE_BYTES)? {
        let reply = match line {
            BoundedLine::Line(line) => process_line(&line, backend),
            BoundedLine::Invalid => fallback_reply(0),
        };
        serde_json::to_writer(&mut output, &reply)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}

#[must_use]
pub fn process_line(line: &str, backend: &mut dyn DialogueBackend) -> DialogueReply {
    let Ok(request) = serde_json::from_str::<DialogueRequest>(line) else {
        return fallback_reply(0);
    };
    if validate_request(&request).is_err() {
        return fallback_reply(request.request_id);
    }

    backend
        .generate(&request)
        .and_then(|reply| validate_reply(&request, reply).map_err(|_| BackendError::InvalidReply))
        .unwrap_or_else(|_| constrained_fallback_reply(&request))
}

fn fixture_reply(request: &DialogueRequest) -> DialogueReply {
    let memory = request.candidate_memories.first();
    DialogueReply {
        protocol_version: PROTOCOL_VERSION,
        request_id: request.request_id,
        say: memory.map_or_else(
            || "hm. no old thought.".to_owned(),
            |_| "yes. old thing remains.".to_owned(),
        ),
        gesture: if request
            .constraints
            .allowed_gestures
            .contains(&Gesture::LookPlayer)
        {
            Gesture::LookPlayer
        } else {
            request
                .constraints
                .allowed_gestures
                .iter()
                .next()
                .cloned()
                .unwrap_or(Gesture::None)
        },
        recalled_memory: memory.map(|candidate| candidate.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BERRY_MEMORY: &str = include_str!("../../../fixtures/dialogue/berry-memory.json");

    #[test]
    fn jsonl_fixture_produces_a_valid_reply() {
        let request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY.trim()).expect("fixture request should parse");
        let reply = process_line(BERRY_MEMORY.trim(), &mut FixtureBackend);
        assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
    }

    #[test]
    fn generated_reply_falls_back_to_requested_word_limit() {
        let mut request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY.trim()).expect("fixture request should parse");
        request.constraints.max_words = 1;
        let line = serde_json::to_string(&request).expect("request should serialize");
        let reply = process_line(&line, &mut FixtureBackend);
        assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
    }

    #[test]
    fn oversized_dialogue_line_falls_back_and_stream_continues() {
        let input = format!(
            "{}\n{}\n",
            "x".repeat(MAX_DIALOGUE_LINE_BYTES + 1),
            BERRY_MEMORY.trim_end()
        );
        let mut output = Vec::new();
        run_jsonl(input.as_bytes(), &mut output, &mut FixtureBackend).expect("stream should run");
        let replies = String::from_utf8(output).expect("replies should be UTF-8");
        let replies = replies
            .lines()
            .map(|line| serde_json::from_str::<DialogueReply>(line).expect("valid reply"))
            .collect::<Vec<_>>();
        assert_eq!(replies.len(), 2);
        assert_eq!(replies[0], fallback_reply(0));
        assert_eq!(replies[1].request_id, 41);
    }

    #[test]
    fn unterminated_bounded_dialogue_request_is_processed_at_eof() {
        let mut output = Vec::new();
        run_jsonl(BERRY_MEMORY.as_bytes(), &mut output, &mut FixtureBackend)
            .expect("unterminated final request should run");
        let reply: DialogueReply = serde_json::from_slice(&output).expect("valid reply");
        assert_eq!(reply.request_id, 41);
    }
}
