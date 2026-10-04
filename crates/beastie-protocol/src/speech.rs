//! What the creature can say, and how it says it without a model.
//!
//! The creature's words are the player's words: it can only use vocabulary the simulation says
//! it has learned, plus a small stage-dependent set of glue words and its own creature sounds.
//! `compose_line` turns a speech intent into a line deterministically. It is the complete
//! no-model voice, not an error message. When a local model is present it may phrase the same
//! intent differently, but `uses_only_known_words` holds it to the same vocabulary.

use serde::{Deserialize, Serialize};

pub use beastie_core::{ActWord, Meaning, RequestResponse};
use beastie_core::{Lexicon, WorldState};

use crate::DialogueRequest;

/// A learned word and what it means to the creature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VocabularyWord {
    pub word: String,
    pub meaning: Meaning,
}

/// Why the creature is speaking now. The simulation decides this; expression only phrases it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpeechIntent {
    /// It just learned a word and says it, proud of it.
    NewWord { word: String, meaning: Meaning },
    /// It tries to repeat a word it does not know yet.
    Echo { attempt: String },
    /// It answers a word it understood.
    Answer {
        word: String,
        meaning: Meaning,
        response: RequestResponse,
    },
    /// It asks for something it wants.
    Want { meaning: Meaning },
    /// It names something it is attending to, in its own words.
    Remark { meaning: Meaning },
    /// It heard talk it did not understand and babbles back.
    Babble,
}

impl DialogueRequest {
    /// Why the creature speaks now. Requests without an intent, from older fixtures and saves,
    /// babble.
    #[must_use]
    pub fn intent(&self) -> &SpeechIntent {
        self.speech_intent.as_ref().unwrap_or(&SpeechIntent::Babble)
    }
}

/// Most words the creature may say in one line, by vocabulary size.
#[must_use]
pub fn line_word_limit(vocabulary: usize) -> usize {
    match vocabulary {
        0..=2 => 3,
        3..=7 => 5,
        _ => 9,
    }
}

const GLUE_WORDS: &[&str] = &["no", "yes", "more", "want", "go", "me", "you", "know"];
const PHRASE_GLUE_WORDS: &[&str] = &[
    "a", "the", "is", "it", "and", "not", "now", "again", "here", "too", "my", "your", "i", "with",
    "like", "that", "this",
];

/// Creature sounds are always available, from the first minute.
const SOUNDS: &[&str] = &["mrp", "prr", "bweep", "eep", "hm", "oo", "nnn", "mm", "zz"];

/// Learned words in a stable order, for requests and prompts.
#[must_use]
pub fn vocabulary(lexicon: &Lexicon) -> Vec<VocabularyWord> {
    lexicon
        .learned_words()
        .into_iter()
        .map(|(word, meaning)| VocabularyWord { word, meaning })
        .collect()
}

/// The creature's vocabulary as a request field.
#[must_use]
pub fn world_vocabulary(world: &WorldState) -> Vec<VocabularyWord> {
    vocabulary(&world.creature.lexicon)
}

fn word_for(vocabulary: &[VocabularyWord], meaning: Meaning) -> Option<&str> {
    vocabulary
        .iter()
        .rev()
        .find(|entry| entry.meaning == meaning)
        .map(|entry| entry.word.as_str())
}

/// A small, stable variation index for one request (splitmix64 over id, salt and name).
fn variant(request: &DialogueRequest, salt: u64, choices: usize) -> usize {
    let mut hash = request
        .request_id
        .wrapping_add(salt.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    for byte in request.creature_name.bytes() {
        hash = hash.rotate_left(7) ^ u64::from(byte);
    }
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    hash ^= hash >> 31;
    (hash % choices.max(1) as u64) as usize
}

fn sound(request: &DialogueRequest, salt: u64) -> &'static str {
    SOUNDS[variant(request, salt, SOUNDS.len())]
}

/// Compose the creature's line for this request without a model.
#[must_use]
pub fn compose_line(request: &DialogueRequest) -> String {
    let words = &request.vocabulary;
    let stage = words.len();
    let me = word_for(words, Meaning::Creature);
    let line = match request.intent() {
        SpeechIntent::NewWord { word, .. } => match (stage, variant(request, 1, 3)) {
            (0..=2, 0) => format!("{word}!"),
            (0..=2, 1) => format!("{word}? {word}!"),
            (0..=2, _) => format!("{word}! {}!", sound(request, 2)),
            (_, 0) => format!("{word}! {word}!"),
            (_, 1) => match me {
                Some(me) => format!("{me} know {word}!"),
                None => format!("{word}! me know!"),
            },
            _ => format!("{word}? yes! {word}!"),
        },
        SpeechIntent::Echo { attempt } => attempt.clone(),
        SpeechIntent::Answer { word, response, .. } => {
            answer_line(request, word, *response, stage, me)
        }
        SpeechIntent::Want { meaning } => match word_for(words, *meaning) {
            Some(word) if stage >= 3 => match (me, variant(request, 3, 2)) {
                (Some(me), 0) => format!("{me} want {word}."),
                _ => format!("want {word}! {word}?"),
            },
            Some(word) => format!("{word}? {word}!"),
            None => format!("{}? {}!", sound(request, 4), sound(request, 5)),
        },
        SpeechIntent::Remark { meaning } => match word_for(words, *meaning) {
            Some(word) => {
                let me = me.unwrap_or("me");
                match (stage >= 3, variant(request, 17, 6)) {
                    (true, 0) => format!("{me}... {word}."),
                    (true, 1) => format!("{word}! {me} {word}!"),
                    (true, 2) => format!("{}. {word} {word}.", sound(request, 20)),
                    (_, 3) => format!("{word}? {}.", sound(request, 21)),
                    (_, 4) => format!("{word}~ {word}~"),
                    _ => format!("{word}. {}.", sound(request, 18)),
                }
            }
            None => format!("{}?", sound(request, 19)),
        },
        SpeechIntent::Babble => match variant(request, 6, 3) {
            0 => format!("{}?", sound(request, 7)),
            1 => format!("{} {}.", sound(request, 8), sound(request, 9)),
            _ => format!("{}! {}?", sound(request, 10), sound(request, 11)),
        },
    };
    line.split_whitespace()
        .take(request.constraints.max_words.max(1))
        .collect::<Vec<_>>()
        .join(" ")
}

fn answer_line(
    request: &DialogueRequest,
    word: &str,
    response: RequestResponse,
    stage: usize,
    me: Option<&str>,
) -> String {
    let pick = variant(request, 12, 2);
    match response {
        RequestResponse::Comply if stage < 3 => format!("{word}!"),
        RequestResponse::Comply => match (me, pick) {
            (Some(me), 0) => format!("{me} go {word}!"),
            _ => format!("yes! {word}!"),
        },
        RequestResponse::Refuse if stage < 3 => {
            format!("{}! {word}... {}.", sound(request, 13), "nnn")
        }
        RequestResponse::Refuse => format!("no {word}. no!"),
        RequestResponse::Delight => match pick {
            0 => format!("{word}! {}!", sound(request, 14)),
            _ => format!("{}! {word}!", sound(request, 15)),
        },
        RequestResponse::Sulk => format!("{}...", sound(request, 16)),
        RequestResponse::Look => format!("{word}?"),
    }
}

/// Whether a token is a creature sound: a short run of hums, purrs, peeps and buzzes, however
/// many letters it is stretched to ("mm", "mmm", "mrrp", "ooo", "zzz").
#[must_use]
pub fn is_creature_sound(word: &str) -> bool {
    if word.is_empty() || word.chars().count() > 6 {
        return false;
    }
    let mut collapsed = String::new();
    for character in word.chars() {
        if !collapsed.ends_with(character) {
            collapsed.push(character);
        }
    }
    matches!(
        collapsed.as_str(),
        "m" | "mr"
            | "mrp"
            | "pr"
            | "prp"
            | "bwep"
            | "bwe"
            | "ep"
            | "e"
            | "hm"
            | "o"
            | "n"
            | "z"
            | "r"
            | "br"
            | "mp"
            | "u"
            | "uh"
            | "hmp"
            | "nm"
    )
}

/// Whether a line uses only words the creature may say at its stage.
#[must_use]
pub fn uses_only_known_words(request: &DialogueRequest, say: &str) -> bool {
    let stage = request.vocabulary.len();
    say.split(|c: char| !(c.is_alphabetic() || c == '\''))
        .filter(|word| !word.is_empty())
        .map(|word| word.trim_matches('\'').to_lowercase())
        .all(|word| {
            request.vocabulary.iter().any(|entry| entry.word == word)
                || is_creature_sound(&word)
                || (stage >= 3 && GLUE_WORDS.contains(&word.as_str()))
                || (stage >= 8 && PHRASE_GLUE_WORDS.contains(&word.as_str()))
                || matches!(
                    request.intent(),
                    SpeechIntent::Echo { attempt } if attempt.trim_end_matches('?') == word
                )
        })
}

/// Glue words the model may use at this vocabulary size.
#[must_use]
pub fn allowed_glue(vocabulary: usize) -> Vec<&'static str> {
    let mut glue = Vec::new();
    if vocabulary >= 3 {
        glue.extend_from_slice(GLUE_WORDS);
    }
    if vocabulary >= 8 {
        glue.extend_from_slice(PHRASE_GLUE_WORDS);
    }
    glue
}

/// Creature sounds the model may use.
#[must_use]
pub const fn creature_sounds() -> &'static [&'static str] {
    SOUNDS
}
