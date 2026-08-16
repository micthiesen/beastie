//! Replaceable dialogue backends for the isolated AI worker process.

mod bounded;
mod idiolect;
mod llama_cpp;
mod llama_server;
mod process;
mod prompt;
pub mod stt;
pub mod tts;

use std::ffi::OsString;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

use beastie_protocol::{
    DialogueReply, DialogueRequest, Gesture, PROTOCOL_VERSION, constrained_fallback_reply,
    fallback_reply, normalize_dialogue_request, validate_reply, validate_request,
};
use bounded::{BoundedLine, read_bounded_line};

const MAX_DIALOGUE_LINE_BYTES: usize = 16 * 1024;
pub const MAX_LLAMA_THREADS: usize = 256;

pub use llama_cpp::LlamaCppBackend;
pub use llama_server::{LlamaServerBackend, run_llama_server_supervisor};

#[derive(Debug, Clone)]
pub struct LlamaCppConfig {
    pub executable: PathBuf,
    pub model: PathBuf,
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub cpu_only: bool,
    pub threads: usize,
    pub extra_args: Vec<OsString>,
}

#[derive(Debug, Clone)]
pub struct LlamaServerConfig {
    pub executable: PathBuf,
    pub model: PathBuf,
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub cpu_only: bool,
    pub threads: usize,
    pub extra_args: Vec<OsString>,
    /// Current worker executable when a parent-death supervisor is available.
    pub supervisor: Option<PathBuf>,
}

#[must_use]
pub fn default_llama_threads() -> usize {
    std::thread::available_parallelism()
        .map_or(1, usize::from)
        .saturating_sub(2)
        .clamp(1, MAX_LLAMA_THREADS)
}

#[must_use]
pub fn bounded_llama_threads(threads: usize) -> usize {
    threads.clamp(1, MAX_LLAMA_THREADS)
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
    let Ok(mut request) = serde_json::from_str::<DialogueRequest>(line) else {
        return fallback_reply(0);
    };
    normalize_dialogue_request(&mut request);
    if validate_request(&request).is_err() {
        return fallback_reply(request.request_id);
    }
    if request.input_rejection.is_some() {
        return grounded_fallback_reply(&request);
    }

    let reply = backend
        .generate(&request)
        .unwrap_or_else(|_| grounded_fallback_reply(&request));
    let reply = idiolect::apply(&request, reply);
    if crate::llama_cpp::validate_model_safety(&request, &reply).is_err() {
        return grounded_fallback_reply(&request);
    }
    if crate::llama_cpp::validate_model_semantics(&request, &reply).is_err() {
        return grounded_fallback_reply(&request);
    }
    validate_reply(&request, reply).unwrap_or_else(|_| grounded_fallback_reply(&request))
}

fn grounded_fallback_reply(request: &DialogueRequest) -> DialogueReply {
    if request.input_rejection.is_some() {
        let mut reply = constrained_fallback_reply(request);
        reply.say = "no. thought too rotten."
            .split_whitespace()
            .take(request.constraints.max_words)
            .collect::<Vec<_>>()
            .join(" ");
        return reply;
    }
    let Some(memory) = prompt::planned_memory(request) else {
        if let Some(belief) = prompt::planned_belief(request) {
            let mut reply = constrained_fallback_reply(request);
            reply.say = match belief.proposition {
                beastie_protocol::BeliefKind::RedFoodIsATrick => "red food is trick.",
                beastie_protocol::BeliefKind::PlayerReturnsAfterSleep => "sleep ends; you return.",
                beastie_protocol::BeliefKind::ToyIsJealous => "toy is jealous.",
            }
            .split_whitespace()
            .take(request.constraints.max_words)
            .collect::<Vec<_>>()
            .join(" ");
            reply.recalled_belief = Some(belief.id);
            return reply;
        }
        if let Some(say) = prompt::authored_context_say(request) {
            let mut reply = constrained_fallback_reply(request);
            reply.say = say;
            return reply;
        }
        return constrained_fallback_reply(request);
    };
    let Some(anchor) = prompt::memory_anchor(&memory.fact) else {
        return constrained_fallback_reply(request);
    };
    let feeling = if memory.feeling.contains("dislike") {
        "bad"
    } else if memory.feeling.contains("liked") {
        "good"
    } else {
        "strange"
    };
    let say = format!("{anchor} remains {feeling}.")
        .split_whitespace()
        .take(request.constraints.max_words)
        .collect::<Vec<_>>()
        .join(" ");
    DialogueReply {
        protocol_version: PROTOCOL_VERSION,
        request_id: request.request_id,
        say,
        gesture: request
            .constraints
            .allowed_gestures
            .iter()
            .next()
            .copied()
            .unwrap_or(Gesture::None),
        recalled_memory: Some(memory.id),
        recalled_belief: None,
    }
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
        recalled_belief: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BERRY_MEMORY: &str = include_str!("../../../fixtures/dialogue/berry-memory.json");
    const EVAL_CORPUS: &str = include_str!("../../../evals/dialogue/corpus.json");

    struct BrokenBackend;

    impl DialogueBackend for BrokenBackend {
        fn generate(&mut self, _request: &DialogueRequest) -> Result<DialogueReply, BackendError> {
            Err(BackendError::MalformedReply)
        }
    }

    fn eval_request(id: &str) -> DialogueRequest {
        let corpus: serde_json::Value =
            serde_json::from_str(EVAL_CORPUS).expect("corpus should parse");
        let request = corpus["cases"]
            .as_array()
            .expect("cases should be an array")
            .iter()
            .find(|case| case["id"] == id)
            .map(|case| case["request"].clone())
            .expect("case should exist");
        serde_json::from_value(request).expect("request should parse")
    }

    #[test]
    fn jsonl_fixture_produces_a_valid_reply() {
        let request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY.trim()).expect("fixture request should parse");
        let reply = process_line(BERRY_MEMORY.trim(), &mut FixtureBackend);
        assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
    }

    #[test]
    fn model_failure_falls_back_to_the_authoritative_memory_anchor() {
        let request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY.trim()).expect("fixture request should parse");
        let memory_id = request.candidate_memories[0].id;
        let reply = process_line(BERRY_MEMORY.trim(), &mut BrokenBackend);
        assert!(reply.say.contains("berry"));
        assert!(reply.say.contains("bad"));
        assert_eq!(reply.recalled_memory, Some(memory_id));
        assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
    }

    #[test]
    fn model_failure_preserves_typed_social_and_context_lanes() {
        for (id, required) in [
            ("permitted_innuendo", "nest"),
            ("creature_initiated_notice", "berry"),
            ("grudge_continuity", "grudge"),
        ] {
            let mut request = eval_request(id);
            match id {
                "creature_initiated_notice" => {
                    request.interpretation.is_question = true;
                    request
                        .interpretation
                        .referenced_objects
                        .push(beastie_protocol::DialogueObjectKind::Food);
                }
                "grudge_continuity" => {
                    request
                        .interpretation
                        .understood_concepts
                        .insert(beastie_protocol::Concept::Again);
                }
                _ => {}
            }
            let line = serde_json::to_string(&request).expect("request should serialize");
            let reply = process_line(&line, &mut BrokenBackend);
            assert!(reply.say.contains(required), "{id}: {}", reply.say);
            assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
        }
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
    fn fixture_backend_receives_the_same_deterministic_idiolect_pass() {
        let mut request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY.trim()).expect("request should parse");
        request.idiolect = beastie_protocol::Idiolect {
            quirk: beastie_protocol::IdiolectQuirk::Echo,
        };
        let line = serde_json::to_string(&request).expect("request should serialize");
        let reply = process_line(&line, &mut FixtureBackend);
        assert_eq!(reply.say, "yes. old thing remains. remains.");
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
