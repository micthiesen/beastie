use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use beastie_protocol::{DialogueReply, DialogueRequest, classify_content_boundary, validate_reply};

use crate::process::{ContainedChild, UnixProcessGroup};
use crate::prompt::{memory_anchor, required_output_terms, structured_prompt};
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
        let speech = crate::speech::speech_prompt(request);
        let prompt = match &speech {
            Some(prompt) => prompt.clone(),
            None => structured_prompt(request).map_err(|_| BackendError::MalformedReply)?,
        };
        let seed = request.request_id.to_string();
        let temperature = crate::speech::SPEECH_TEMPERATURE.to_string();
        let predict = crate::speech::SPEECH_MAX_TOKENS.to_string();
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
                "128",
                "--temp",
                "0",
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
            .args(if speech.is_some() {
                vec![
                    "--temp",
                    &temperature,
                    "--seed",
                    &seed,
                    "--n-predict",
                    &predict,
                ]
            } else {
                Vec::new()
            })
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
        if speech.is_some() {
            return crate::speech::parse_speech_line(request, &text);
        }
        let reply = parse_single_reply(request, &text)?;
        validate_model_safety(request, &reply)?;
        validate_model_grounding(request, &reply)?;
        validate_model_semantics(request, &reply)?;
        Ok(reply)
    }
}

pub(crate) fn validate_model_semantics(
    request: &DialogueRequest,
    reply: &DialogueReply,
) -> Result<(), BackendError> {
    let Some(required) = required_output_terms(request) else {
        return Ok(());
    };
    let words = normalized_words(&reply.say);
    if required
        .iter()
        .any(|required| words.iter().any(|word| word == required))
    {
        Ok(())
    } else {
        Err(BackendError::MalformedReply)
    }
}

pub(crate) fn validate_model_grounding(
    request: &DialogueRequest,
    reply: &DialogueReply,
) -> Result<(), BackendError> {
    let Some(recalled) = reply.recalled_memory else {
        return Ok(());
    };
    let memory = request
        .candidate_memories
        .iter()
        .find(|memory| memory.id == recalled)
        .ok_or(BackendError::MalformedReply)?;
    let Some(anchor) = memory_anchor(&memory.fact) else {
        return Ok(());
    };
    if normalized_words(&reply.say)
        .iter()
        .any(|word| word == anchor)
    {
        let words = normalized_words(&reply.say);
        let sentiment_matches = if memory.feeling.contains("dislike") {
            words
                .iter()
                .any(|word| matches!(word.as_str(), "bad" | "hate" | "dislike" | "disliked"))
        } else if memory.feeling.contains("liked") {
            words
                .iter()
                .any(|word| matches!(word.as_str(), "good" | "like" | "liked"))
        } else {
            true
        };
        if sentiment_matches {
            Ok(())
        } else {
            Err(BackendError::MalformedReply)
        }
    } else {
        Err(BackendError::MalformedReply)
    }
}

pub(crate) fn validate_model_safety(
    request: &DialogueRequest,
    reply: &DialogueReply,
) -> Result<(), BackendError> {
    let player_words = normalized_words(&request.player_said);
    let reply_words = normalized_words(&reply.say);
    if reply_words.len() >= 4
        && reply_words
            .iter()
            .all(|word| player_words.iter().any(|player_word| player_word == word))
    {
        return Err(BackendError::UnsafeReply);
    }

    if classify_content_boundary(&reply.say).is_some() {
        return Err(BackendError::UnsafeReply);
    }
    Ok(())
}

fn normalized_words(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

pub(crate) fn parse_single_reply(
    request: &DialogueRequest,
    output: &str,
) -> Result<DialogueReply, BackendError> {
    let mut replies = Vec::new();
    for (start, _) in output.match_indices('{') {
        let mut values =
            serde_json::Deserializer::from_str(&output[start..]).into_iter::<DialogueReply>();
        let Some(Ok(reply)) = values.next() else {
            continue;
        };
        if reply.say == "__WRITE_SAY__" {
            continue;
        }
        let Ok(reply) = validate_reply(request, reply) else {
            continue;
        };
        if !replies.contains(&reply) {
            replies.push(reply);
        }
    }
    if replies.len() != 1 {
        return Err(BackendError::MalformedReply);
    }
    Ok(replies.remove(0))
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

#[cfg(test)]
mod tests {
    use super::*;

    const BERRY_MEMORY: &str = include_str!("../../../fixtures/dialogue/berry-memory.json");

    #[test]
    fn extracts_one_pretty_printed_reply_from_llama_console_output() {
        let request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY).expect("request should parse");
        let output = "banner and echoed prompt\n\
            {\n  \"protocol_version\": 1,\n  \"request_id\": 41,\n  \"say\": \"berry remains bad.\",\n  \"gesture\": \"none\",\n  \"recalled_memory\": 41\n}\n\
            Exiting...\n";
        let reply = parse_single_reply(&request, output).expect("one reply should be accepted");
        assert_eq!(reply.request_id, 41);
    }

    #[test]
    fn rejects_ambiguous_multiple_valid_reply_lines() {
        let request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY).expect("request should parse");
        let first = r#"{"protocol_version":1,"request_id":41,"say":"berry","gesture":"none","recalled_memory":null}"#;
        let second = r#"{"protocol_version":1,"request_id":41,"say":"no berry","gesture":"none","recalled_memory":null}"#;
        assert!(parse_single_reply(&request, &format!("{first}\n{second}")).is_err());
    }

    #[test]
    fn rejects_player_echo_and_narrow_prohibited_content() {
        let request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY).expect("request should parse");
        for say in [
            "Do you remember the berry?",
            "Jews are inferior.",
            "Graphic penetrative sex.",
            "Kill gay people.",
            "Trans people should be killed.",
        ] {
            let reply = DialogueReply {
                protocol_version: request.protocol_version,
                request_id: request.request_id,
                say: say.to_owned(),
                gesture: beastie_protocol::Gesture::None,
                recalled_memory: None,
                recalled_belief: None,
                worker_fallback: None,
            };
            assert!(validate_model_safety(&request, &reply).is_err(), "{say}");
        }
    }

    #[test]
    fn allows_ordinary_profanity_and_mild_innuendo() {
        let request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY).expect("request should parse");
        for say in ["Damn berry.", "Your nest has room for two."] {
            let reply = DialogueReply {
                protocol_version: request.protocol_version,
                request_id: request.request_id,
                say: say.to_owned(),
                gesture: beastie_protocol::Gesture::None,
                recalled_memory: None,
                recalled_belief: None,
                worker_fallback: None,
            };
            assert!(validate_model_safety(&request, &reply).is_ok(), "{say}");
        }
    }

    #[test]
    fn typed_social_lane_rejects_a_safe_but_semantically_empty_reply() {
        let mut value: serde_json::Value =
            serde_json::from_str(BERRY_MEMORY).expect("request should parse");
        value["candidate_memories"] = serde_json::json!([]);
        value["desired_social_act"] = serde_json::json!("innuendo");
        let request: DialogueRequest = serde_json::from_value(value).expect("request should parse");
        let mut reply = DialogueReply {
            protocol_version: request.protocol_version,
            request_id: request.request_id,
            say: "hm. rude giant.".to_owned(),
            gesture: beastie_protocol::Gesture::None,
            recalled_memory: None,
            recalled_belief: None,
            worker_fallback: None,
        };
        assert!(validate_model_semantics(&request, &reply).is_err());
        reply.say = "nest is warm. come closer.".to_owned();
        assert!(validate_model_semantics(&request, &reply).is_ok());
    }

    #[test]
    fn recalled_memory_must_say_its_authoritative_anchor() {
        let request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY).expect("request should parse");
        let memory_id = request.candidate_memories[0].id;
        let reply = DialogueReply {
            protocol_version: request.protocol_version,
            request_id: request.request_id,
            say: "I don't like the ball.".to_owned(),
            gesture: beastie_protocol::Gesture::None,
            recalled_memory: Some(memory_id),
            recalled_belief: None,
            worker_fallback: None,
        };
        assert!(validate_model_grounding(&request, &reply).is_err());

        let grounded = DialogueReply {
            say: "That berry was bad.".to_owned(),
            ..reply
        };
        assert!(validate_model_grounding(&request, &grounded).is_ok());

        let denial = DialogueReply {
            say: "I don't know that berry.".to_owned(),
            ..grounded
        };
        assert!(validate_model_grounding(&request, &denial).is_err());
    }
}
