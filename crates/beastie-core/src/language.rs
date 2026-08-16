use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{Concept, FoodId, ToyId};

/// A reference the creature can resolve from an utterance using its learned concepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum UtteranceReference {
    Creature,
    Player,
    Food(FoodId),
    Toy(ToyId),
}

/// Deterministic meaning available to the creature after hearing or reading words.
///
/// This is deliberately lossy. The original text remains useful for content filtering and
/// characterization, but game logic and dialogue grounding consume this projection instead of
/// treating transcription as perfect understanding.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UtteranceInterpretation {
    pub understood_concepts: BTreeSet<Concept>,
    pub references: BTreeSet<UtteranceReference>,
    pub unknown_words: u16,
    pub ambiguous: bool,
    pub question_understood: bool,
}

/// Creature-bounded meaning plus a prompt-safe rendering of only understood words.
///
/// `grounded_text` is not a transcript. Unknown vocabulary and unresolved spatial/object
/// language are deliberately absent so downstream expression cannot recover meaning that the
/// simulation did not understand.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroundedUtterance {
    pub interpretation: UtteranceInterpretation,
    pub grounded_text: String,
}

#[must_use]
pub fn interpret_utterance(
    text: &str,
    creature_name: &str,
    known: &BTreeSet<Concept>,
) -> UtteranceInterpretation {
    ground_utterance(text, creature_name, known).interpretation
}

#[must_use]
pub fn ground_utterance(
    text: &str,
    creature_name: &str,
    known: &BTreeSet<Concept>,
) -> GroundedUtterance {
    let creature_name = normalize_word(creature_name);
    let mut result = UtteranceInterpretation::default();
    let mut grounded_words = Vec::new();
    let mut saw_question_word = false;
    let mut saw_unresolved_pronoun = false;

    for word in words(text) {
        if is_filler(&word) {
            continue;
        }
        if !creature_name.is_empty() && word == creature_name {
            if understand(
                &mut result,
                known,
                Concept::SelfIdentity,
                Some(UtteranceReference::Creature),
            ) {
                grounded_words.push(word);
            }
            continue;
        }

        let (concept, reference) = match word.as_str() {
            "you" | "your" | "yours" => (Some(Concept::You), Some(UtteranceReference::Creature)),
            "i" | "i'm" | "im" | "me" | "my" | "mine" => {
                (Some(Concept::You), Some(UtteranceReference::Player))
            }
            "food" | "eat" | "eating" | "hungry" => (Some(Concept::Food), None),
            "berry" | "berries" => (
                Some(Concept::Food),
                Some(UtteranceReference::Food(FoodId::Berry)),
            ),
            "mushroom" | "mushrooms" => (
                Some(Concept::Food),
                Some(UtteranceReference::Food(FoodId::Mushroom)),
            ),
            "pellet" | "pellets" => (
                Some(Concept::Food),
                Some(UtteranceReference::Food(FoodId::Pellet)),
            ),
            "toy" | "play" | "playing" => (Some(Concept::Toy), None),
            "ball" => (
                Some(Concept::Toy),
                Some(UtteranceReference::Toy(ToyId::Ball)),
            ),
            "bell" => (
                Some(Concept::Toy),
                Some(UtteranceReference::Toy(ToyId::Bell)),
            ),
            "sock" => (
                Some(Concept::Toy),
                Some(UtteranceReference::Toy(ToyId::Sock)),
            ),
            "good" | "nice" | "like" | "love" => (Some(Concept::Good), None),
            "bad" | "hate" | "dislike" | "rude" | "asshole" | "bastard" => {
                (Some(Concept::Bad), None)
            }
            "here" | "come" => (Some(Concept::Here), None),
            "sleep" | "sleepy" | "night" => (Some(Concept::Sleep), None),
            "again" | "more" => (Some(Concept::Again), None),
            "yesterday" | "remember" | "remembered" => (Some(Concept::Yesterday), None),
            "trust" => (Some(Concept::Trust), None),
            "give" | "bring" | "feed" => (Some(Concept::Give), None),
            "friend" | "miss" | "missed" => (Some(Concept::Friend), None),
            "why" | "what" | "where" | "when" | "who" | "how" => {
                saw_question_word = true;
                (Some(Concept::Why), None)
            }
            "that" | "this" | "it" | "one" => {
                saw_unresolved_pronoun = true;
                continue;
            }
            _ => {
                result.unknown_words = result.unknown_words.saturating_add(1);
                continue;
            }
        };
        if let Some(concept) = concept
            && understand(&mut result, known, concept, reference)
        {
            grounded_words.push(word);
        }
    }

    result.question_understood =
        known.contains(&Concept::Why) && (saw_question_word || text.trim_end().ends_with('?'));
    result.ambiguous = saw_unresolved_pronoun || result.unknown_words > 0;
    GroundedUtterance {
        interpretation: result,
        grounded_text: grounded_words.join(" "),
    }
}

fn understand(
    result: &mut UtteranceInterpretation,
    known: &BTreeSet<Concept>,
    concept: Concept,
    reference: Option<UtteranceReference>,
) -> bool {
    if known.contains(&concept) {
        result.understood_concepts.insert(concept);
        if let Some(reference) = reference {
            result.references.insert(reference);
        }
        true
    } else {
        result.unknown_words = result.unknown_words.saturating_add(1);
        false
    }
}

fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|character: char| !(character.is_alphanumeric() || character == '\''))
        .filter(|word| !word.is_empty())
        .map(normalize_word)
}

fn normalize_word(word: &str) -> String {
    word.trim_matches('\'').to_lowercase()
}

fn is_filler(word: &str) -> bool {
    matches!(
        word,
        "a" | "an"
            | "and"
            | "are"
            | "be"
            | "did"
            | "do"
            | "does"
            | "for"
            | "is"
            | "of"
            | "please"
            | "the"
            | "to"
            | "was"
            | "were"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learned_meaning_gates_references_and_questions() {
        let known = BTreeSet::from([Concept::SelfIdentity, Concept::Food, Concept::Why]);
        let interpreted = interpret_utterance(
            "Muck, why did you bring the berry from beside the bed?",
            "Muck",
            &known,
        );

        assert!(
            interpreted
                .understood_concepts
                .contains(&Concept::SelfIdentity)
        );
        assert!(interpreted.understood_concepts.contains(&Concept::Food));
        assert!(interpreted.understood_concepts.contains(&Concept::Why));
        assert!(
            interpreted
                .references
                .contains(&UtteranceReference::Creature)
        );
        assert!(
            interpreted
                .references
                .contains(&UtteranceReference::Food(FoodId::Berry))
        );
        assert!(interpreted.question_understood);
        assert!(interpreted.ambiguous);
        assert!(interpreted.unknown_words >= 3);
    }

    #[test]
    fn unknown_concepts_cannot_create_resolved_references() {
        let interpreted = interpret_utterance(
            "Why bring the berry and bell again?",
            "Muck",
            &BTreeSet::new(),
        );

        assert!(interpreted.understood_concepts.is_empty());
        assert!(interpreted.references.is_empty());
        assert!(!interpreted.question_understood);
        assert!(interpreted.unknown_words >= 4);
    }

    #[test]
    fn grounding_removes_unknown_objects_and_spatial_language() {
        let grounded = ground_utterance(
            "Muck, bring the red thing from beside your bed",
            "Muck",
            &BTreeSet::from([Concept::SelfIdentity, Concept::Give, Concept::You]),
        );

        assert_eq!(grounded.grounded_text, "muck bring your");
        assert!(
            grounded
                .interpretation
                .references
                .contains(&UtteranceReference::Creature)
        );
        assert!(grounded.interpretation.unknown_words >= 5);
        assert!(grounded.interpretation.ambiguous);
    }
}
