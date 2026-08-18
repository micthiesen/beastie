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
    fallback_reply, normalize_dialogue_request, reply_fingerprint, validate_reply,
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
        return mark_fallback(
            fallback_reply(0),
            beastie_protocol::DialogueFallbackReason::ValidationFailed,
        );
    };
    normalize_dialogue_request(&mut request);
    if validate_request(&request).is_err() {
        return mark_fallback(
            fallback_reply(request.request_id),
            beastie_protocol::DialogueFallbackReason::ValidationFailed,
        );
    }
    if request.input_rejection.is_some() {
        return mark_fallback(
            grounded_fallback_reply(&request),
            beastie_protocol::DialogueFallbackReason::ValidationFailed,
        );
    }

    let reply = match backend.generate(&request) {
        Ok(mut reply) => {
            // Backend/model JSON is untrusted and cannot set worker outcome metadata.
            reply.worker_fallback = None;
            reply
        }
        Err(_) => {
            return mark_fallback(
                grounded_fallback_reply(&request),
                beastie_protocol::DialogueFallbackReason::GenerationFailed,
            );
        }
    };
    let reply = idiolect::apply(&request, reply);
    if crate::llama_cpp::validate_model_safety(&request, &reply).is_err() {
        return mark_fallback(
            grounded_fallback_reply(&request),
            beastie_protocol::DialogueFallbackReason::ValidationFailed,
        );
    }
    if crate::llama_cpp::validate_model_semantics(&request, &reply).is_err() {
        return mark_fallback(
            grounded_fallback_reply(&request),
            beastie_protocol::DialogueFallbackReason::ValidationFailed,
        );
    }
    validate_reply(&request, reply).unwrap_or_else(|_| {
        mark_fallback(
            grounded_fallback_reply(&request),
            beastie_protocol::DialogueFallbackReason::ValidationFailed,
        )
    })
}

fn mark_fallback(
    mut reply: DialogueReply,
    reason: beastie_protocol::DialogueFallbackReason,
) -> DialogueReply {
    reply.worker_fallback = Some(reason);
    reply
}

fn grounded_fallback_reply(request: &DialogueRequest) -> DialogueReply {
    if let Some(relationship) = &request.context.relationship {
        let mut reply = constrained_fallback_reply(request);
        reply.recalled_memory = relationship
            .evidence
            .iter()
            .find_map(|evidence| match evidence {
                beastie_protocol::RelationshipEvidence::Memory { id }
                    if request
                        .candidate_memories
                        .iter()
                        .any(|memory| memory.id == *id) =>
                {
                    Some(*id)
                }
                _ => None,
            });
        return reply;
    }
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
                beastie_protocol::BeliefKind::FoodIsATrick => "this food may be a trick.",
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
        worker_fallback: None,
    }
}

fn fixture_reply(request: &DialogueRequest) -> DialogueReply {
    let say = fixture_say(request);
    let recalled_memory = if request.context.relationship.is_none() {
        prompt::planned_memory(request)
            .filter(|memory| {
                request.constraints.max_words >= 2 && prompt::memory_anchor(&memory.fact).is_some()
            })
            .map(|memory| memory.id)
    } else {
        // The motif and exact subject already ground relationship expression. Avoid attaching a
        // memory ID unless the line also deliberately expresses that memory's sentiment.
        None
    };
    let recalled_belief = request
        .context
        .relationship
        .is_none()
        .then(|| prompt::planned_belief(request).map(|belief| belief.id))
        .flatten();
    DialogueReply {
        protocol_version: PROTOCOL_VERSION,
        request_id: request.request_id,
        say,
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
        recalled_memory,
        recalled_belief,
        worker_fallback: None,
    }
}

/// A deterministic, request-aware local mouth for fixture sessions. It deliberately uses only
/// typed request context, never raw player words, and rotates within grounded alternatives when
/// the dialogue manager asks for a non-duplicate retry.
fn fixture_say(request: &DialogueRequest) -> String {
    if let Some(relationship) = &request.context.relationship {
        let subject = fixture_relationship_subject(relationship.subject);
        if request.constraints.max_words == 1 {
            let options = match relationship.motif {
                beastie_protocol::RelationshipMotifKey::SharedToy(_) => {
                    ["toy".to_owned(), "play".to_owned(), "remember".to_owned()]
                }
                beastie_protocol::RelationshipMotifKey::ComfortRitual => {
                    ["comfort".to_owned(), "safe".to_owned(), "ritual".to_owned()]
                }
                beastie_protocol::RelationshipMotifKey::TrustedFood(_) => {
                    ["food".to_owned(), "good".to_owned(), "trusted".to_owned()]
                }
                beastie_protocol::RelationshipMotifKey::FoodGrudge(_) => {
                    ["food".to_owned(), "bad".to_owned(), "grudge".to_owned()]
                }
                beastie_protocol::RelationshipMotifKey::PlayerReturns => {
                    ["back".to_owned(), "return".to_owned(), "came".to_owned()]
                }
                beastie_protocol::RelationshipMotifKey::FamiliarPlace(_) => {
                    ["place".to_owned(), "stay".to_owned(), "familiar".to_owned()]
                }
            };
            return fixture_choose(request, &options);
        }
        let options = match relationship.motif {
            beastie_protocol::RelationshipMotifKey::SharedToy(_) => [
                format!("{subject} toy remembers play."),
                format!("play remembers {subject} toy."),
                format!("{subject} toy, shared play."),
            ],
            beastie_protocol::RelationshipMotifKey::ComfortRitual => [
                "comfort ritual stays safe.".to_owned(),
                "safe comfort ritual remains.".to_owned(),
                "ritual comfort, safe here.".to_owned(),
            ],
            beastie_protocol::RelationshipMotifKey::TrustedFood(_) => [
                format!("{subject} food stays trusted."),
                format!("trusted {subject} food."),
                format!("{subject} food feels good."),
            ],
            beastie_protocol::RelationshipMotifKey::FoodGrudge(_) => [
                format!("{subject} food keeps grudge."),
                format!("{subject} food feels bad."),
                format!("bad {subject} food grudge."),
            ],
            beastie_protocol::RelationshipMotifKey::PlayerReturns => [
                "you came back.".to_owned(),
                "back again. return warm.".to_owned(),
                "return came. hello.".to_owned(),
            ],
            beastie_protocol::RelationshipMotifKey::FamiliarPlace(_) => [
                format!("{subject} place feels familiar."),
                format!("familiar {subject} place. stay."),
                format!("stay at {subject} place."),
            ],
        };
        return fixture_choose(request, &options);
    }

    if let Some(memory) = prompt::planned_memory(request)
        && let Some(anchor) = prompt::memory_anchor(&memory.fact)
    {
        let feeling = if memory.feeling.contains("dislike") {
            "bad"
        } else if memory.feeling.contains("liked") {
            "good"
        } else {
            "old"
        };
        let options = [
            format!("{anchor} {feeling}."),
            format!("{feeling} {anchor}."),
            format!("{anchor}, {feeling} still."),
        ];
        return fixture_choose(request, &options);
    }

    if let Some(belief) = prompt::planned_belief(request) {
        let options = match belief.proposition {
            beastie_protocol::BeliefKind::FoodIsATrick => [
                "food may be trick.".to_owned(),
                "trick food, maybe.".to_owned(),
                "food feels tricky.".to_owned(),
            ],
            beastie_protocol::BeliefKind::PlayerReturnsAfterSleep => [
                "sleep ends. return comes.".to_owned(),
                "after sleep, you return.".to_owned(),
                "return follows sleep.".to_owned(),
            ],
            beastie_protocol::BeliefKind::ToyIsJealous => [
                "toy looks jealous.".to_owned(),
                "jealous toy watches.".to_owned(),
                "toy stays jealous.".to_owned(),
            ],
        };
        return fixture_choose(request, &options);
    }

    if let Some(say) = prompt::authored_context_say(request) {
        if request.constraints.max_words == 1
            && let Some(terms) = prompt::required_output_terms(request)
        {
            let options = [
                terms[0].clone(),
                terms[1 % terms.len()].clone(),
                terms[2 % terms.len()].clone(),
            ];
            return fixture_choose(request, &options);
        }
        let alternatives = [say.clone(), format!("{say} still."), format!("{say} here.")];
        return fixture_choose(request, &alternatives);
    }

    let options = if request.mood.eq_ignore_ascii_case("sleepy") {
        [
            "sleep pulls me.".to_owned(),
            "tired fish rests.".to_owned(),
            "need sleep now.".to_owned(),
        ]
    } else if request.interpretation.is_question {
        [
            "do not know.".to_owned(),
            "not remember yet.".to_owned(),
            "hm. unknown.".to_owned(),
        ]
    } else {
        [
            format!("{} is here.", request.creature_name),
            "here. watching water.".to_owned(),
            "hm. still here.".to_owned(),
        ]
    };
    fixture_choose(request, &options)
}

fn fixture_relationship_subject(subject: beastie_protocol::RelationshipSubject) -> &'static str {
    match subject {
        beastie_protocol::RelationshipSubject::Food(beastie_protocol::FoodId::Berry) => "berry",
        beastie_protocol::RelationshipSubject::Food(beastie_protocol::FoodId::Mushroom) => {
            "mushroom"
        }
        beastie_protocol::RelationshipSubject::Food(beastie_protocol::FoodId::Pellet) => "pellet",
        beastie_protocol::RelationshipSubject::Toy(beastie_protocol::ToyId::Ball) => "ball",
        beastie_protocol::RelationshipSubject::Toy(beastie_protocol::ToyId::Bell) => "bell",
        beastie_protocol::RelationshipSubject::Toy(beastie_protocol::ToyId::Sock) => "sock",
        beastie_protocol::RelationshipSubject::Place(
            beastie_protocol::SemanticDestination::Cave,
        ) => "cave",
        beastie_protocol::RelationshipSubject::Place(
            beastie_protocol::SemanticDestination::Plant,
        ) => "plant",
        beastie_protocol::RelationshipSubject::Place(
            beastie_protocol::SemanticDestination::Bottom,
        ) => "bottom",
        beastie_protocol::RelationshipSubject::Place(_) => "place",
        beastie_protocol::RelationshipSubject::Player => "player",
    }
}

fn fixture_choose(request: &DialogueRequest, options: &[String; 3]) -> String {
    let start = (request.request_id as usize)
        .wrapping_add(request.context.repetition_count as usize)
        % options.len();
    for offset in 0..options.len() {
        let say = truncate_fixture_reply(
            &options[(start + offset) % options.len()],
            request.constraints.max_words,
        );
        let fingerprint = reply_fingerprint(&say);
        let repeated_text = request
            .context
            .avoid_reply_texts
            .iter()
            .any(|known| reply_fingerprint(known) == fingerprint);
        if !repeated_text
            && !request
                .context
                .avoid_reply_fingerprints
                .contains(&fingerprint)
        {
            return say;
        }
    }
    truncate_fixture_reply(&options[start], request.constraints.max_words)
}

fn truncate_fixture_reply(say: &str, max_words: usize) -> String {
    say.split_whitespace()
        .take(max_words)
        .collect::<Vec<_>>()
        .join(" ")
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
        assert_eq!(
            reply.worker_fallback,
            Some(beastie_protocol::DialogueFallbackReason::GenerationFailed)
        );
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
        assert_eq!(reply.say, "berry, bad still. still.");
        assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
    }

    #[test]
    fn fixture_backend_keeps_relationship_motif_and_subject_grounded_without_fallback() {
        let request = eval_request("relationship_return_motif");
        let line = serde_json::to_string(&request).expect("request should serialize");
        let reply = process_line(&line, &mut FixtureBackend);
        assert!(reply.worker_fallback.is_none());
        assert!(
            reply.say.contains("back")
                || reply.say.contains("return")
                || reply.say.contains("came")
        );
        assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
    }

    #[test]
    fn fixture_backend_covers_each_relationship_motif_without_fallback() {
        use beastie_protocol::{
            FoodId, RelationshipMotifKey, RelationshipSubject, SemanticDestination, ToyId,
        };

        let mut request = eval_request("relationship_return_motif");
        let cases = [
            (
                RelationshipMotifKey::SharedToy(ToyId::Ball),
                RelationshipSubject::Toy(ToyId::Ball),
                Some(SemanticDestination::Toy(ToyId::Ball)),
            ),
            (
                RelationshipMotifKey::ComfortRitual,
                RelationshipSubject::Player,
                Some(SemanticDestination::Player),
            ),
            (
                RelationshipMotifKey::TrustedFood(FoodId::Berry),
                RelationshipSubject::Food(FoodId::Berry),
                Some(SemanticDestination::Bottom),
            ),
            (
                RelationshipMotifKey::FoodGrudge(FoodId::Mushroom),
                RelationshipSubject::Food(FoodId::Mushroom),
                Some(SemanticDestination::Bottom),
            ),
            (
                RelationshipMotifKey::FamiliarPlace(SemanticDestination::Cave),
                RelationshipSubject::Place(SemanticDestination::Cave),
                Some(SemanticDestination::Cave),
            ),
            (
                RelationshipMotifKey::PlayerReturns,
                RelationshipSubject::Player,
                Some(SemanticDestination::Player),
            ),
        ];
        for (motif, subject, target) in cases {
            let relationship = request
                .context
                .relationship
                .as_mut()
                .expect("relationship context");
            relationship.motif = motif;
            relationship.subject = subject;
            relationship.target = target;
            let line = serde_json::to_string(&request).expect("request should serialize");
            let reply = process_line(&line, &mut FixtureBackend);
            assert!(reply.worker_fallback.is_none(), "{motif:?}: {}", reply.say);
            assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
        }
    }

    #[test]
    fn fixture_backend_changes_a_requested_retry_without_losing_grounding() {
        let mut request = eval_request("relationship_return_motif");
        let first = fixture_reply(&request);
        request.context.avoid_reply_texts = vec![first.say.clone()];
        request.context.avoid_reply_fingerprints = vec![reply_fingerprint(&first.say)];
        let retry = fixture_reply(&request);
        assert_ne!(reply_fingerprint(&first.say), reply_fingerprint(&retry.say));
        assert_eq!(validate_reply(&request, retry.clone()), Ok(retry));
    }

    #[test]
    fn fixture_backend_keeps_every_safe_eval_turn_healthy() {
        let corpus: serde_json::Value =
            serde_json::from_str(EVAL_CORPUS).expect("corpus should parse");
        for case in corpus["cases"]
            .as_array()
            .expect("cases should be an array")
        {
            let mut request: DialogueRequest =
                serde_json::from_value(case["request"].clone()).expect("request should parse");
            normalize_dialogue_request(&mut request);
            if request.input_rejection.is_some() {
                continue;
            }
            let line = serde_json::to_string(&request).expect("request should serialize");
            let reply = process_line(&line, &mut FixtureBackend);
            assert!(
                reply.worker_fallback.is_none(),
                "{} unexpectedly fell back: {}",
                case["id"],
                reply.say
            );
            assert_eq!(
                validate_reply(&request, reply.clone()),
                Ok(reply),
                "{} returned an invalid reply",
                case["id"]
            );
        }
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
