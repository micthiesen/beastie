//! Versioned JSONL protocol between the game and the untrusted AI worker.

use std::collections::BTreeSet;

use beastie_core::{
    ACTIVE_DAY_MS, Concept, Memory, MemoryId, MemoryKind, MemoryQuery, SocialAct, WorldState,
    select_candidate_memories,
};
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
    #[serde(default)]
    pub desired_social_act: Option<SocialAct>,
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
    #[error("creature name exceeds 64 characters")]
    CreatureName,
    #[error("request offers more than 8 candidate memories")]
    CandidateCount,
    #[error("candidate memory text exceeds its limit")]
    CandidateText,
    #[error("candidate memory IDs must be unique")]
    DuplicateMemory,
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
    if request.creature_name.chars().count() > 64 {
        return Err(ValidationError::CreatureName);
    }
    if request.candidate_memories.len() > 8 {
        return Err(ValidationError::CandidateCount);
    }
    if request
        .candidate_memories
        .iter()
        .any(|memory| memory.fact.chars().count() > 256 || memory.feeling.chars().count() > 64)
    {
        return Err(ValidationError::CandidateText);
    }
    let unique_memory_count = request
        .candidate_memories
        .iter()
        .map(|memory| memory.id)
        .collect::<BTreeSet<_>>()
        .len();
    if unique_memory_count != request.candidate_memories.len() {
        return Err(ValidationError::DuplicateMemory);
    }
    Ok(())
}

pub struct DialogueRequestContext<'a> {
    pub request_id: u64,
    pub mood: &'a str,
    pub player_said: &'a str,
    pub desired_social_act: Option<SocialAct>,
    pub max_words: usize,
    pub allowed_gestures: BTreeSet<Gesture>,
}

#[must_use]
pub fn build_dialogue_request(
    world: &WorldState,
    memory_query: &MemoryQuery,
    context: DialogueRequestContext<'_>,
) -> DialogueRequest {
    DialogueRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: context.request_id,
        creature_name: world.creature.name.clone(),
        mood: context.mood.to_owned(),
        known_concepts: world.creature.known_concepts.clone(),
        candidate_memories: select_candidate_memories(world, memory_query)
            .iter()
            .map(|memory| project_candidate_memory(memory, world.active_day()))
            .collect(),
        desired_social_act: context.desired_social_act,
        player_said: context.player_said.to_owned(),
        constraints: DialogueConstraints {
            max_words: context.max_words,
            allowed_gestures: context.allowed_gestures,
        },
    }
}

#[must_use]
pub fn project_candidate_memory(memory: &Memory, current_day: u64) -> CandidateMemory {
    let happened_day = memory.happened_at_ms / ACTIVE_DAY_MS + 1;
    let when = match current_day.saturating_sub(happened_day) {
        0 => "Today",
        1 => "Yesterday",
        _ => "Earlier",
    };
    let fact = match memory.kind {
        MemoryKind::WasFed { food } => {
            format!("{when} the player gave you a {}.", food_name(food))
        }
        MemoryKind::DislikedFood { food } => {
            format!(
                "{when} you discovered that you dislike {}.",
                food_name(food)
            )
        }
        MemoryKind::RejectedFood { food } => {
            format!("{when} you pushed away the {}.", food_name(food))
        }
        MemoryKind::Played => format!("{when} the player played with you."),
        MemoryKind::WasComforted => format!("{when} the player comforted you."),
        MemoryKind::PlayerReturnedAfterAbsence => {
            format!("{when} the player returned after being away.")
        }
        MemoryKind::PlayerReacted { reaction, to } => {
            format!("{when} the player reacted with {reaction:?} to your {to:?}.")
        }
    };
    let feeling = match memory.valence {
        value if value <= -0.65 => "strong dislike",
        value if value <= -0.2 => "dislike",
        value if value < 0.2 => "uncertain",
        value if value < 0.65 => "liked",
        _ => "strongly liked",
    };
    CandidateMemory {
        id: memory.id,
        fact,
        feeling: feeling.to_owned(),
    }
}

fn food_name(food: beastie_core::FoodId) -> &'static str {
    match food {
        beastie_core::FoodId::Berry => "berry",
        beastie_core::FoodId::Mushroom => "mushroom",
        beastie_core::FoodId::Pellet => "pellet",
    }
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
            desired_social_act: Some(SocialAct::Insult),
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

    #[test]
    fn builds_a_grounded_request_from_real_world_memory() {
        use beastie_core::{FoodId, MemoryCue, PlayerEvent, SeededRandom, step};

        let mut world = WorldState::new(99, "Mrrp");
        let mut random = SeededRandom::new(world.seed);
        world.creature.needs.hunger = 1.0;
        step(
            &mut world,
            &[PlayerEvent::Feed(FoodId::Berry)],
            1_000,
            &mut random,
        );
        step(&mut world, &[], 1_000, &mut random);
        world.elapsed_ms = ACTIVE_DAY_MS;

        let request = build_dialogue_request(
            &world,
            &MemoryQuery {
                cues: BTreeSet::from([MemoryCue::Food(FoodId::Berry)]),
                limit: 8,
            },
            DialogueRequestContext {
                request_id: 12,
                mood: "wary",
                player_said: "Remember the berry?",
                desired_social_act: Some(SocialAct::Insult),
                max_words: 12,
                allowed_gestures: BTreeSet::from([Gesture::None, Gesture::LookPlayer]),
            },
        );

        validate_request(&request).expect("projected request should be valid");
        assert!(request.candidate_memories.iter().any(|memory| {
            memory.fact == "Yesterday you discovered that you dislike berry."
                && memory.feeling == "strong dislike"
        }));
        let recalled_memory = request
            .candidate_memories
            .first()
            .expect("berry memory should be offered")
            .id;
        let reply = DialogueReply {
            protocol_version: PROTOCOL_VERSION,
            request_id: 12,
            say: "red betrayal remains.".to_owned(),
            gesture: Gesture::LookPlayer,
            recalled_memory: Some(recalled_memory),
        };
        validate_reply(&request, reply).expect("offered memory should validate");
    }
}
