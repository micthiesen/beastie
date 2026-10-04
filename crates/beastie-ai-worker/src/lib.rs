//! Replaceable dialogue backends for the isolated AI worker process.

mod bounded;
mod llama_cpp;
mod llama_server;
mod process;
mod speech;
pub mod stt;
pub mod tts;

use std::ffi::OsString;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

use beastie_protocol::{
    DialogueFallbackReason, DialogueReply, DialogueRequest, SpeechIntent,
    constrained_fallback_reply, fallback_reply, normalize_dialogue_request, validate_reply,
    validate_request,
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
}

pub trait DialogueBackend {
    fn generate(&mut self, request: &DialogueRequest) -> Result<DialogueReply, BackendError>;
}

#[derive(Debug, Default)]
pub struct FixtureBackend;

/// The deterministic no-model voice, so fixture sessions hear exactly what plays without a model.
impl DialogueBackend for FixtureBackend {
    fn generate(&mut self, request: &DialogueRequest) -> Result<DialogueReply, BackendError> {
        Ok(constrained_fallback_reply(request))
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

/// Answers one request line. The model only ever phrases the simulation's speech intent in the
/// creature's learned words; the composed line is the complete voice whenever it cannot.
#[must_use]
pub fn process_line(line: &str, backend: &mut dyn DialogueBackend) -> DialogueReply {
    let Ok(mut request) = serde_json::from_str::<DialogueRequest>(line) else {
        return mark_fallback(fallback_reply(0), DialogueFallbackReason::ValidationFailed);
    };
    normalize_dialogue_request(&mut request);
    if validate_request(&request).is_err() {
        return mark_fallback(
            fallback_reply(request.request_id),
            DialogueFallbackReason::ValidationFailed,
        );
    }
    // Prohibited input never reaches a model; the creature only babbles back. An echo is the
    // simulation's exact attempt at an unfamiliar word, so there is nothing for a model to phrase.
    if request.input_rejection.is_some() || matches!(request.intent(), SpeechIntent::Echo { .. }) {
        return constrained_fallback_reply(&request);
    }
    match backend.generate(&request) {
        Ok(mut reply) => {
            // Backend output is untrusted and cannot set worker outcome metadata.
            reply.worker_fallback = None;
            validate_reply(&request, reply).unwrap_or_else(|_| {
                mark_fallback(
                    constrained_fallback_reply(&request),
                    DialogueFallbackReason::ValidationFailed,
                )
            })
        }
        // Samples that all missed the creature's words are not a technical failure worth
        // reporting: the composer is a complete voice.
        Err(BackendError::InvalidReply | BackendError::MalformedReply) => {
            constrained_fallback_reply(&request)
        }
        Err(_) => mark_fallback(
            constrained_fallback_reply(&request),
            DialogueFallbackReason::GenerationFailed,
        ),
    }
}

fn mark_fallback(mut reply: DialogueReply, reason: DialogueFallbackReason) -> DialogueReply {
    reply.worker_fallback = Some(reason);
    reply
}

#[cfg(test)]
mod tests {
    use super::*;
    use beastie_protocol::{ContentBoundaryViolation, FoodId, Meaning, ToyId, compose_line};

    const SPEECH_REQUEST: &str = include_str!("../../../fixtures/dialogue/speech-request.json");

    struct BrokenBackend(fn() -> BackendError);

    impl DialogueBackend for BrokenBackend {
        fn generate(&mut self, _request: &DialogueRequest) -> Result<DialogueReply, BackendError> {
            Err((self.0)())
        }
    }

    struct FixedBackend(&'static str);

    impl DialogueBackend for FixedBackend {
        fn generate(&mut self, request: &DialogueRequest) -> Result<DialogueReply, BackendError> {
            let mut reply = constrained_fallback_reply(request);
            reply.say = self.0.to_owned();
            reply.worker_fallback = Some(DialogueFallbackReason::GenerationFailed);
            Ok(reply)
        }
    }

    fn speech_request(intent: SpeechIntent) -> DialogueRequest {
        let mut request: DialogueRequest =
            serde_json::from_str(SPEECH_REQUEST.trim()).expect("fixture should parse");
        request.speech_intent = Some(intent);
        request
    }

    fn line(request: &DialogueRequest) -> String {
        serde_json::to_string(request).expect("request should serialize")
    }

    #[test]
    fn jsonl_fixture_produces_the_composed_line() {
        let request: DialogueRequest =
            serde_json::from_str(SPEECH_REQUEST.trim()).expect("fixture request should parse");
        let reply = process_line(SPEECH_REQUEST.trim(), &mut FixtureBackend);
        assert_eq!(reply.say, compose_line(&request));
        assert!(reply.say.contains("berry"));
        assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
    }

    #[test]
    fn every_intent_gets_the_no_model_voice_from_fixture_and_failing_backends() {
        let intents = [
            SpeechIntent::NewWord {
                word: "ball".to_owned(),
                meaning: Meaning::Toy(ToyId::Ball),
            },
            SpeechIntent::Echo {
                attempt: "baw?".to_owned(),
            },
            SpeechIntent::Want {
                meaning: Meaning::Food(FoodId::Berry),
            },
            SpeechIntent::Babble,
        ];
        for intent in intents {
            let request = speech_request(intent);
            for reply in [
                process_line(&line(&request), &mut FixtureBackend),
                process_line(
                    &line(&request),
                    &mut BrokenBackend(|| BackendError::InvalidReply),
                ),
            ] {
                assert_eq!(reply.say, compose_line(&request));
                assert_eq!(
                    reply.worker_fallback, None,
                    "the composer is the designed voice"
                );
                assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
            }
        }
    }

    #[test]
    fn intent_free_requests_babble() {
        let mut request = speech_request(SpeechIntent::Babble);
        let babble = process_line(&line(&request), &mut FixtureBackend);
        request.speech_intent = None;
        let legacy = process_line(&line(&request), &mut FixtureBackend);
        assert_eq!(legacy.say, babble.say);
    }

    #[test]
    fn a_backend_failure_is_reported_with_the_composed_line() {
        let request = speech_request(SpeechIntent::Babble);
        let reply = process_line(
            &line(&request),
            &mut BrokenBackend(|| BackendError::Timeout),
        );
        assert_eq!(reply.say, compose_line(&request));
        assert_eq!(
            reply.worker_fallback,
            Some(DialogueFallbackReason::GenerationFailed)
        );
    }

    #[test]
    fn backend_replies_cannot_set_fallback_metadata_or_break_the_protocol() {
        let request = speech_request(SpeechIntent::Babble);
        let accepted = process_line(&line(&request), &mut FixedBackend("mrp? prr."));
        assert_eq!(accepted.say, "mrp? prr.");
        assert_eq!(accepted.worker_fallback, None);

        for invalid in ["mrp prr eep oo hm mm zz", "hello friend"] {
            let reply = process_line(&line(&request), &mut FixedBackend(invalid));
            assert_eq!(reply.say, compose_line(&request), "{invalid}");
            assert_eq!(
                reply.worker_fallback,
                Some(DialogueFallbackReason::ValidationFailed)
            );
        }
    }

    #[test]
    fn rejected_input_babbles_without_reaching_the_backend() {
        let mut request = speech_request(SpeechIntent::Babble);
        request.player_said.clear();
        request.input_rejection = Some(ContentBoundaryViolation::SelfHarmEncouragement);
        let reply = process_line(&line(&request), &mut FixedBackend("ball"));
        assert_eq!(reply.say, compose_line(&request));
        assert_eq!(reply.worker_fallback, None);

        // Unscreened prohibited text is normalized before anything else happens.
        let mut raw = speech_request(SpeechIntent::Babble);
        raw.player_said = "go kill yourself".to_owned();
        let reply = process_line(&line(&raw), &mut FixedBackend("ball"));
        assert_eq!(reply.say, compose_line(&raw));
    }

    #[test]
    fn oversized_dialogue_line_falls_back_and_stream_continues() {
        let input = format!(
            "{}\n{}\n",
            "x".repeat(MAX_DIALOGUE_LINE_BYTES + 1),
            SPEECH_REQUEST.trim_end()
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
        run_jsonl(
            SPEECH_REQUEST.trim_end().as_bytes(),
            &mut output,
            &mut FixtureBackend,
        )
        .expect("unterminated final request should run");
        let reply: DialogueReply = serde_json::from_slice(&output).expect("valid reply");
        assert_eq!(reply.request_id, 41);
    }
}
