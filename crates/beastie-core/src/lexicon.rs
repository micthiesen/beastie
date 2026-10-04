//! Words the creature learns from the player.
//!
//! The creature starts without vocabulary. Each content word it hears collects evidence for
//! whatever is salient at that moment: the toy it is playing with, the food it is eating, itself
//! when it is being addressed. Once one meaning clearly dominates a word's evidence, the word is
//! learned. Learned words drive understanding, requests and the creature's own speech. The model
//! never declares that anything was learned; only these rules do.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Concept, FoodId, ToyId};

/// What a learned word refers to. Deliberately small: things that exist in the aquarium,
/// the two participants, a few actions the creature can perform and the player's verdicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Meaning {
    /// The creature itself: usually its name.
    Creature,
    /// The player.
    Player,
    Toy(ToyId),
    Food(FoodId),
    Act(ActWord),
    /// Approval: "good", "yay", "clever".
    Praise,
    /// Disapproval: "no", "bad", "stop".
    Scold,
    /// A greeting heard when the player arrives.
    Greeting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActWord {
    Play,
    Eat,
    Come,
    Sleep,
}

impl Meaning {
    /// A short English gloss for prompts, inspection and tests.
    #[must_use]
    pub const fn gloss(self) -> &'static str {
        match self {
            Self::Creature => "me (my name)",
            Self::Player => "you",
            Self::Toy(ToyId::Ball) => "the ball",
            Self::Toy(ToyId::Bell) => "the bell",
            Self::Toy(ToyId::Sock) => "the sock",
            Self::Food(FoodId::Berry) => "the berry",
            Self::Food(FoodId::Mushroom) => "the mushroom",
            Self::Food(FoodId::Pellet) => "the pellet",
            Self::Act(ActWord::Play) => "playing",
            Self::Act(ActWord::Eat) => "eating",
            Self::Act(ActWord::Come) => "coming over",
            Self::Act(ActWord::Sleep) => "sleeping",
            Self::Praise => "good, well done",
            Self::Scold => "no, bad",
            Self::Greeting => "hello",
        }
    }

    /// The coarse concept this meaning contributes to dialogue grounding.
    #[must_use]
    pub const fn concept(self) -> Concept {
        match self {
            Self::Creature => Concept::SelfIdentity,
            Self::Player => Concept::You,
            Self::Toy(_) | Self::Act(ActWord::Play) => Concept::Toy,
            Self::Food(_) | Self::Act(ActWord::Eat) => Concept::Food,
            Self::Act(ActWord::Come) => Concept::Here,
            Self::Act(ActWord::Sleep) => Concept::Sleep,
            Self::Praise | Self::Greeting => Concept::Good,
            Self::Scold => Concept::Bad,
        }
    }
}

/// Everything the creature knows about one heard word.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WordKnowledge {
    /// How many separate utterances contained this word.
    pub heard: u32,
    /// Accumulated situational evidence for each candidate meaning, in meaning order.
    pub evidence: Vec<Evidence>,
    pub learned: Option<Meaning>,
    pub learned_at_ms: Option<u64>,
    pub last_heard_ms: u64,
    /// How often the creature has said this word itself.
    pub spoken: u32,
}

/// Votes for one candidate meaning of a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub meaning: Meaning,
    pub votes: u16,
}

impl WordKnowledge {
    fn add_evidence(&mut self, meaning: Meaning, weight: u16) {
        match self
            .evidence
            .binary_search_by(|entry| entry.meaning.cmp(&meaning))
        {
            Ok(index) => {
                self.evidence[index].votes = self.evidence[index].votes.saturating_add(weight);
            }
            Err(index) => self.evidence.insert(
                index,
                Evidence {
                    meaning,
                    votes: weight,
                },
            ),
        }
    }
}

/// The creature's vocabulary and the evidence behind it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Lexicon {
    pub words: BTreeMap<String, WordKnowledge>,
}

/// A salient meaning at the moment a word was heard, with its strength (1 weak to 3 strong).
pub type Salience = (Meaning, u16);

/// A recent moment that makes a meaning salient until `until_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FocusMark {
    pub meaning: Meaning,
    pub weight: u16,
    pub until_ms: u64,
}

/// Record a focus mark, keeping the strongest live mark per meaning and a bounded list.
pub fn mark_focus(marks: &mut Vec<FocusMark>, mark: FocusMark, now_ms: u64) {
    marks.retain(|existing| existing.until_ms > now_ms);
    if let Some(existing) = marks
        .iter_mut()
        .find(|existing| existing.meaning == mark.meaning)
    {
        existing.weight = existing.weight.max(mark.weight);
        existing.until_ms = existing.until_ms.max(mark.until_ms);
    } else {
        marks.push(mark);
    }
    if marks.len() > 12 {
        marks.remove(0);
    }
}

/// The result of hearing one utterance.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hearing {
    /// Words learned by this utterance.
    pub learned: Vec<(String, Meaning)>,
    /// Every learned word in the utterance, in order, including those just learned.
    pub understood: Vec<(String, Meaning)>,
    /// The unknown word the creature found most interesting, for a curious echo.
    pub curious_about: Option<String>,
    /// Content words heard, in order.
    pub content_words: Vec<String>,
}

/// Utterances that only differ in function words carry no teachable evidence.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "is", "are", "am", "was", "be", "this", "that", "these", "those", "it",
    "it's", "its", "is'nt", "isn't", "i", "i'm", "im", "me", "my", "your", "you're", "youre", "of",
    "to", "and", "or", "but", "so", "on", "in", "at", "with", "for", "from", "up", "down", "here",
    "there", "what", "whats", "what's", "who", "how", "why", "where", "when", "do", "does", "did",
    "can", "will", "just", "very", "really", "oh", "ok", "okay", "um", "uh", "hmm", "look", "see",
    "say", "says", "said", "called", "named", "name", "word", "means", "mean", "like", "have",
    "has", "had", "get", "got", "let's", "lets", "let", "we", "us", "our", "they", "them", "he",
    "she", "him", "her", "his", "hers", "one", "some", "thing", "things", "now", "then", "too",
    "also", "not", "don't", "dont", "yes", "yeah", "please", "thank", "thanks", "if", "into",
    "out", "about", "again", "more", "little", "big",
];

const MAX_WORD_LEN: usize = 14;
const MAX_CONTENT_WORDS: usize = 4;
const MAX_TRACKED_WORDS: usize = 96;
/// Evidence needed before a word can be learned.
pub const LEARN_EVIDENCE: u16 = 4;
/// A word must be heard in at least this many utterances: one mention is never a lesson.
pub const LEARN_HEARINGS: u32 = 2;

/// Lowercased content words of an utterance, in order and without duplicates.
#[must_use]
pub fn content_words(text: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for raw in text.split(|c: char| !(c.is_alphabetic() || c == '\'')) {
        let word = raw.trim_matches('\'').to_lowercase();
        let length = word.chars().count();
        if !(2..=MAX_WORD_LEN).contains(&length) && word != "no" {
            continue;
        }
        if STOPWORDS.contains(&word.as_str()) || words.contains(&word) {
            continue;
        }
        words.push(word);
        if words.len() == MAX_CONTENT_WORDS {
            break;
        }
    }
    words
}

impl Lexicon {
    /// Number of learned words.
    #[must_use]
    pub fn learned_count(&self) -> usize {
        self.words
            .values()
            .filter(|word| word.learned.is_some())
            .count()
    }

    /// The meaning of a learned word.
    #[must_use]
    pub fn meaning_of(&self, word: &str) -> Option<Meaning> {
        self.words.get(word).and_then(|knowledge| knowledge.learned)
    }

    /// The creature's preferred word for a meaning: the most heard learned word.
    #[must_use]
    pub fn word_for(&self, meaning: Meaning) -> Option<&str> {
        self.words
            .iter()
            .filter(|(_, knowledge)| knowledge.learned == Some(meaning))
            .max_by_key(|(word, knowledge)| (knowledge.heard, std::cmp::Reverse(word.len())))
            .map(|(word, _)| word.as_str())
    }

    /// Learned words with their meanings, most recently learned last.
    #[must_use]
    pub fn learned_words(&self) -> Vec<(String, Meaning)> {
        let mut learned = self
            .words
            .iter()
            .filter_map(|(word, knowledge)| {
                knowledge
                    .learned
                    .map(|meaning| (knowledge.learned_at_ms.unwrap_or(0), word.clone(), meaning))
            })
            .collect::<Vec<_>>();
        learned.sort();
        learned
            .into_iter()
            .map(|(_, word, meaning)| (word, meaning))
            .collect()
    }

    /// Hear an utterance in a situation. Evidence only accrues for meanings that are salient now.
    ///
    /// A meaning already named by a known word in the same utterance does not also become the
    /// meaning of the other words: in "good ball" with "ball" known, "good" is not the ball.
    pub fn hear(&mut self, text: &str, salient: &[Salience], now_ms: u64) -> Hearing {
        let content = content_words(text);
        let named: Vec<Meaning> = content
            .iter()
            .filter_map(|word| self.meaning_of(word))
            .collect();
        // Mutual exclusivity: an unfamiliar word is less likely to name something that already
        // has a name. Synonyms remain learnable, just more slowly.
        let has_name: Vec<Meaning> = self
            .words
            .values()
            .filter_map(|knowledge| knowledge.learned)
            .collect();
        let mut hearing = Hearing {
            content_words: content.clone(),
            ..Hearing::default()
        };
        let mut best_unknown: Option<(u16, String)> = None;
        for word in &content {
            let already = self.meaning_of(word);
            let knowledge = self.words.entry(word.clone()).or_default();
            knowledge.heard = knowledge.heard.saturating_add(1);
            knowledge.last_heard_ms = now_ms;
            for &(meaning, weight) in salient {
                if named.contains(&meaning) && already != Some(meaning) {
                    continue;
                }
                let weight = if already.is_none() && has_name.contains(&meaning) {
                    (weight / 3).max(1)
                } else {
                    weight
                };
                knowledge.add_evidence(meaning, weight);
            }
            let leader = leading_meaning(knowledge);
            match (already, leader) {
                (None, Some(meaning)) => {
                    knowledge.learned = Some(meaning);
                    knowledge.learned_at_ms = Some(now_ms);
                    hearing.learned.push((word.clone(), meaning));
                }
                // Consistent contrary evidence can correct an earlier mistake.
                (Some(old), Some(meaning)) if old != meaning => {
                    knowledge.learned = Some(meaning);
                    knowledge.learned_at_ms = Some(now_ms);
                    hearing.learned.push((word.clone(), meaning));
                }
                _ => {}
            }
            if let Some(meaning) = knowledge.learned {
                hearing.understood.push((word.clone(), meaning));
            } else {
                let interest = knowledge
                    .evidence
                    .iter()
                    .map(|entry| entry.votes)
                    .max()
                    .unwrap_or(0);
                if interest > 0
                    && best_unknown
                        .as_ref()
                        .is_none_or(|(best, _)| interest > *best)
                {
                    best_unknown = Some((interest, word.clone()));
                }
            }
        }
        hearing.curious_about = best_unknown.map(|(_, word)| word);
        self.evict();
        hearing
    }

    /// Record that the creature said a learned word.
    pub fn note_spoken(&mut self, word: &str) {
        if let Some(knowledge) = self.words.get_mut(word) {
            knowledge.spoken = knowledge.spoken.saturating_add(1);
        }
    }

    fn evict(&mut self) {
        while self.words.len() > MAX_TRACKED_WORDS {
            let Some(victim) = self
                .words
                .iter()
                .filter(|(_, knowledge)| knowledge.learned.is_none())
                .min_by_key(|(word, knowledge)| {
                    (knowledge.heard, knowledge.last_heard_ms, (*word).clone())
                })
                .map(|(word, _)| word.clone())
            else {
                return;
            };
            self.words.remove(&victim);
        }
    }
}

/// The meaning a word's evidence clearly supports, if any.
fn leading_meaning(knowledge: &WordKnowledge) -> Option<Meaning> {
    if knowledge.heard < LEARN_HEARINGS {
        return None;
    }
    let mut ranked = knowledge
        .evidence
        .iter()
        .map(|entry| (entry.votes, entry.meaning))
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.cmp(left));
    let (top, meaning) = *ranked.first()?;
    let second = ranked.get(1).map_or(0, |(votes, _)| *votes);
    // A clear lead (one and a half times the runner-up) settles the meaning.
    (top >= LEARN_EVIDENCE && u32::from(top) * 2 >= u32::from(second) * 3).then_some(meaning)
}

/// A creature's attempt to repeat a word it does not know yet: a soft first syllable.
#[must_use]
pub fn echo_attempt(word: &str) -> String {
    let mut out = String::new();
    let mut seen_vowel = false;
    for character in word.chars() {
        let vowel = "aeiouy".contains(character);
        if seen_vowel && !vowel {
            // A following liquid softens into a "w", the way small mouths manage it.
            if matches!(character, 'l' | 'r') {
                out.push('w');
            }
            break;
        }
        seen_vowel |= vowel;
        out.push(character);
        if out.chars().count() >= 4 {
            break;
        }
    }
    if out.is_empty() {
        out = word.chars().take(3).collect();
    }
    format!("{out}?")
}

#[cfg(test)]
mod tests {
    use super::*;

    const BALL: Salience = (Meaning::Toy(ToyId::Ball), 3);

    #[test]
    fn content_words_drop_function_words_and_duplicates() {
        assert_eq!(
            content_words("This is the BALL, the ball! Ball?"),
            vec!["ball".to_owned()]
        );
        assert_eq!(
            content_words("good job mop"),
            vec!["good".to_owned(), "job".to_owned(), "mop".to_owned()]
        );
        assert_eq!(content_words("no"), vec!["no".to_owned()]);
    }

    #[test]
    fn one_mention_is_never_a_lesson_but_two_in_context_teach() {
        let mut lexicon = Lexicon::default();
        let first = lexicon.hear("ball", &[BALL], 1_000);
        assert!(first.learned.is_empty());
        assert_eq!(first.curious_about.as_deref(), Some("ball"));
        let second = lexicon.hear("this is a ball", &[BALL], 4_000);
        assert_eq!(
            second.learned,
            vec![("ball".to_owned(), Meaning::Toy(ToyId::Ball))]
        );
        assert_eq!(lexicon.word_for(Meaning::Toy(ToyId::Ball)), Some("ball"));
    }

    #[test]
    fn invented_words_are_learned_like_any_other() {
        let mut lexicon = Lexicon::default();
        lexicon.hear("zorp", &[BALL], 0);
        let hearing = lexicon.hear("zorp!", &[BALL], 1);
        assert_eq!(hearing.understood[0].0, "zorp");
    }

    #[test]
    fn ambiguous_situations_need_more_evidence() {
        let mut lexicon = Lexicon::default();
        let mixed = [
            (Meaning::Toy(ToyId::Ball), 2),
            (Meaning::Act(ActWord::Play), 2),
        ];
        lexicon.hear("ball", &mixed, 0);
        assert!(lexicon.hear("ball", &mixed, 1).learned.is_empty());
        // Later lessons in a clearer situation resolve the ambiguity.
        let learned = (2..6).find(|time| !lexicon.hear("ball", &[BALL], *time).learned.is_empty());
        assert!(learned.is_some());
        assert_eq!(lexicon.meaning_of("ball"), Some(Meaning::Toy(ToyId::Ball)));
    }

    #[test]
    fn praise_emerges_across_different_situations_once_objects_have_names() {
        let mut lexicon = Lexicon::default();
        let praise = (Meaning::Praise, 2);
        for (word, toy) in [
            ("ball", ToyId::Ball),
            ("bell", ToyId::Bell),
            ("sock", ToyId::Sock),
        ] {
            lexicon.hear(word, &[(Meaning::Toy(toy), 3)], 0);
            lexicon.hear(word, &[(Meaning::Toy(toy), 3)], 1);
        }
        let mut learned = None;
        for (time, toy) in [ToyId::Ball, ToyId::Bell, ToyId::Sock, ToyId::Ball]
            .into_iter()
            .enumerate()
        {
            let hearing = lexicon.hear("yay", &[(Meaning::Toy(toy), 3), praise], time as u64);
            if let Some(found) = hearing.learned.first() {
                learned = Some(found.1);
            }
        }
        assert_eq!(learned, Some(Meaning::Praise));
    }

    #[test]
    fn known_words_claim_their_meaning_in_mixed_utterances() {
        let mut lexicon = Lexicon::default();
        lexicon.hear("ball", &[BALL], 0);
        lexicon.hear("ball", &[BALL], 1);
        let praise = (Meaning::Praise, 3);
        lexicon.hear("good ball", &[BALL, praise], 2);
        let hearing = lexicon.hear("good ball", &[BALL, praise], 3);
        assert!(
            hearing
                .learned
                .contains(&("good".to_owned(), Meaning::Praise))
        );
    }

    #[test]
    fn nothing_salient_means_nothing_learned() {
        let mut lexicon = Lexicon::default();
        for time in 0..5 {
            let hearing = lexicon.hear("blorb", &[], time);
            assert!(hearing.learned.is_empty());
            assert!(hearing.curious_about.is_none());
        }
    }

    #[test]
    fn echo_attempts_are_soft_first_syllables() {
        assert_eq!(echo_attempt("ball"), "baw?");
        assert_eq!(echo_attempt("sock"), "so?");
        assert_eq!(echo_attempt("zorp"), "zow?");
        assert_eq!(echo_attempt("bell"), "bew?");
        assert_eq!(echo_attempt("mushroom"), "mu?");
    }

    #[test]
    fn tracked_vocabulary_is_bounded_without_forgetting_learned_words() {
        let mut lexicon = Lexicon::default();
        lexicon.hear("ball", &[BALL], 0);
        lexicon.hear("ball", &[BALL], 1);
        for index in 0..200_u64 {
            let word: String = [index % 26, index / 26 % 26, 7]
                .iter()
                .map(|offset| char::from(b'a' + *offset as u8))
                .collect();
            lexicon.hear(&format!("q{word}"), &[], index);
        }
        assert!(lexicon.words.len() <= MAX_TRACKED_WORDS);
        assert_eq!(lexicon.meaning_of("ball"), Some(Meaning::Toy(ToyId::Ball)));
    }

    #[test]
    fn vocabulary_round_trips_through_json() {
        let mut lexicon = Lexicon::default();
        lexicon.hear("ball", &[BALL, (Meaning::Act(ActWord::Play), 1)], 0);
        lexicon.hear("ball", &[BALL], 1);
        let json = serde_json::to_string(&lexicon).unwrap();
        assert_eq!(serde_json::from_str::<Lexicon>(&json).unwrap(), lexicon);
    }
}
