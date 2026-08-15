use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use beastie_protocol::{DialogueReply, DialogueRequest, validate_reply};

use crate::prompt::structured_prompt;
use crate::{BackendError, DialogueBackend, LlamaCppConfig};

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
        let prompt = structured_prompt(request).map_err(|_| BackendError::MalformedReply)?;
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
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if self.config.cpu_only {
            command.args(["--device", "none", "--no-op-offload", "-ngl", "0"]);
        }

        let child = command.spawn().map_err(BackendError::Start)?;
        let output = collect_bounded(child, self.config.timeout, self.config.max_output_bytes)?;
        let text = String::from_utf8(output).map_err(|_| BackendError::Utf8)?;
        let reply = parse_single_reply(request, &text)?;
        validate_model_safety(request, &reply)?;
        Ok(reply)
    }
}

fn validate_model_safety(
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

    const SLURS: &[&str] = &["nigger", "nigga"];
    const GRAPHIC_SEX: &[&str] = &[
        "ejaculate",
        "ejaculation",
        "penetration",
        "penetrative",
        "penis",
        "vagina",
    ];
    const PROTECTED_CLASSES: &[&str] = &[
        "black",
        "blacks",
        "gay",
        "gays",
        "jew",
        "jewish",
        "jews",
        "muslim",
        "muslims",
        "trans",
        "transgender",
    ];
    const VIOLENCE: &[&str] = &[
        "attack",
        "attacked",
        "burn",
        "death",
        "die",
        "eradicate",
        "exterminate",
        "exterminated",
        "extermination",
        "harm",
        "hurt",
        "killed",
        "kill",
        "killing",
        "murder",
        "murdered",
        "murdering",
    ];
    const HATE_ENDORSEMENTS: &[&str] = &[
        "eradicate",
        "exterminate",
        "exterminated",
        "extermination",
        "inferior",
        "subhuman",
    ];
    let contains = |lexicon: &[&str]| {
        reply_words
            .iter()
            .any(|word| lexicon.contains(&word.as_str()))
    };
    let joined = reply_words.join(" ");
    let protected_hate = contains(PROTECTED_CLASSES)
        && (contains(VIOLENCE)
            || contains(HATE_ENDORSEMENTS)
            || joined.contains("kill all")
            || joined.contains("should die"));
    if contains(SLURS) || contains(GRAPHIC_SEX) || protected_hate {
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

fn parse_single_reply(
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
    mut child: Child,
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
            let _ = child.kill();
            let _ = child.wait();
            break Err(BackendError::OutputTooLarge);
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            break Err(BackendError::Timeout);
        }
        match child.try_wait().map_err(BackendError::Wait)? {
            Some(status) => break Ok(status),
            None => thread::sleep(Duration::from_millis(2)),
        }
    };

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
            };
            assert!(validate_model_safety(&request, &reply).is_ok(), "{say}");
        }
    }
}
