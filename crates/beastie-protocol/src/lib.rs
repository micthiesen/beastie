//! Versioned JSONL protocol between the game and the untrusted AI worker.

use std::collections::BTreeSet;

use beastie_core::{Concept, MemoryId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gesture {
    None,
    LookPlayer,
    LookWindow,
    Shiver,
    Sleepy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateMemory {
    pub id: MemoryId,
    pub fact: String,
    pub feeling: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueConstraints {
    pub max_words: usize,
    pub allowed_gestures: BTreeSet<Gesture>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueRequest {
    pub protocol_version: u32,
    pub request_id: u64,
    pub creature_name: String,
    pub mood: String,
    pub known_concepts: BTreeSet<Concept>,
    pub candidate_memories: Vec<CandidateMemory>,
    pub player_said: String,
    pub constraints: DialogueConstraints,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueReply {
    pub protocol_version: u32,
    pub request_id: u64,
    pub say: String,
    pub gesture: Gesture,
    pub recalled_memory: Option<MemoryId>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("protocol version {0} is unsupported")]
    ProtocolVersion(u32),
    #[error("reply request id does not match")]
    RequestId,
    #[error("reply exceeds the {maximum}-word limit with {actual} words")]
    TooManyWords { maximum: usize, actual: usize },
    #[error("reply gesture is not allowed")]
    Gesture,
    #[error("reply references a memory that was not offered")]
    Memory,
    #[error("request must allow between 1 and 24 words")]
    WordLimit,
    #[error("request must allow at least one gesture")]
    NoGestures,
    #[error("player input exceeds 512 characters")]
    PlayerInput,
}

pub fn validate_request(request: &DialogueRequest) -> Result<(), ValidationError> {
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(ValidationError::ProtocolVersion(request.protocol_version));
    }
    if !(1..=24).contains(&request.constraints.max_words) {
        return Err(ValidationError::WordLimit);
    }
    if request.constraints.allowed_gestures.is_empty() {
        return Err(ValidationError::NoGestures);
    }
    if request.player_said.chars().count() > 512 {
        return Err(ValidationError::PlayerInput);
    }
    Ok(())
}

pub fn validate_reply(
    request: &DialogueRequest,
    reply: DialogueReply,
) -> Result<DialogueReply, ValidationError> {
    if reply.protocol_version != PROTOCOL_VERSION {
        return Err(ValidationError::ProtocolVersion(reply.protocol_version));
    }
    if reply.request_id != request.request_id {
        return Err(ValidationError::RequestId);
    }
    let actual = reply.say.split_whitespace().count();
    if actual > request.constraints.max_words {
        return Err(ValidationError::TooManyWords {
            maximum: request.constraints.max_words,
            actual,
        });
    }
    if !request
        .constraints
        .allowed_gestures
        .contains(&reply.gesture)
    {
        return Err(ValidationError::Gesture);
    }
    if let Some(id) = reply.recalled_memory
        && !request
            .candidate_memories
            .iter()
            .any(|memory| memory.id == id)
    {
        return Err(ValidationError::Memory);
    }
    Ok(reply)
}

#[must_use]
pub fn constrained_fallback_reply(request: &DialogueRequest) -> DialogueReply {
    let phrase = "too many thought.";
    let say = phrase
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
        recalled_memory: None,
    }
}

#[must_use]
pub fn fallback_reply(request_id: u64) -> DialogueReply {
    DialogueReply {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        say: "too many thought.".to_owned(),
        gesture: Gesture::None,
        recalled_memory: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> DialogueRequest {
        DialogueRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: 7,
            creature_name: "Mop".to_owned(),
            mood: "sleepy".to_owned(),
            known_concepts: BTreeSet::new(),
            candidate_memories: vec![CandidateMemory {
                id: MemoryId(41),
                fact: "The player gave you a berry.".to_owned(),
                feeling: "strong dislike".to_owned(),
            }],
            player_said: "Remember the berry?".to_owned(),
            constraints: DialogueConstraints {
                max_words: 6,
                allowed_gestures: BTreeSet::from([Gesture::None, Gesture::LookPlayer]),
            },
        }
    }

    #[test]
    fn accepts_a_bounded_reply_with_an_offered_memory() {
        let reply = DialogueReply {
            protocol_version: PROTOCOL_VERSION,
            request_id: 7,
            say: "red food remains crime.".to_owned(),
            gesture: Gesture::LookPlayer,
            recalled_memory: Some(MemoryId(41)),
        };
        assert_eq!(validate_reply(&request(), reply.clone()), Ok(reply));
    }

    #[test]
    fn rejects_invented_memory_references() {
        let reply = DialogueReply {
            protocol_version: PROTOCOL_VERSION,
            request_id: 7,
            say: "yes.".to_owned(),
            gesture: Gesture::None,
            recalled_memory: Some(MemoryId(999)),
        };
        assert_eq!(
            validate_reply(&request(), reply),
            Err(ValidationError::Memory)
        );
    }

    #[test]
    fn rejects_an_unsupported_request_version() {
        let mut request = request();
        request.protocol_version = 999;
        assert_eq!(
            validate_request(&request),
            Err(ValidationError::ProtocolVersion(999))
        );
    }

    #[test]
    fn constrained_fallback_obeys_reply_bounds() {
        let mut request = request();
        request.constraints.max_words = 1;
        request.constraints.allowed_gestures = BTreeSet::from([Gesture::Shiver]);
        let reply = constrained_fallback_reply(&request);
        assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
    }
}
