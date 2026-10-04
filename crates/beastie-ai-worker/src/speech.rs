//! Learned-word speech for small local models.
//!
//! A tiny model is good at varying a short line and bad at copying JSON scaffolds, so the model
//! writes only the spoken line for the request's speech intent. The worker builds the reply itself
//! and holds the line to the creature's vocabulary; anything else falls back to the deterministic
//! composer.

use beastie_protocol::{
    DialogueReply, DialogueRequest, Gesture, Meaning, PROTOCOL_VERSION, RequestResponse,
    SpeechIntent, allowed_glue, compose_line, creature_sounds, uses_only_known_words,
};

use crate::BackendError;

/// Sampling for learned-word lines: varied, but reproducible for a given request.
pub(crate) const SPEECH_TEMPERATURE: f32 = 0.9;
pub(crate) const SPEECH_MAX_TOKENS: u32 = 16;
/// Samples drawn before falling back to the composer.
pub(crate) const SPEECH_ATTEMPTS: u64 = 3;

/// The prompt for one spoken line.
#[must_use]
pub(crate) fn speech_prompt(request: &DialogueRequest) -> String {
    let intent = request.intent();
    let name = &request.creature_name;
    // Words only: glosses in the list invite the model to speak English it was never taught.
    let mut known = request
        .vocabulary
        .iter()
        .map(|entry| entry.word.clone())
        .collect::<Vec<_>>();
    if known.is_empty() {
        known.push("no words yet".to_owned());
    }
    let glue = allowed_glue(request.vocabulary.len());
    let glue_line = if glue.is_empty() {
        String::new()
    } else {
        format!(
            "\nYou can also use these little words: {}.",
            glue.join(", ")
        )
    };
    let sounds = creature_sounds().join(", ");
    let situation = situation(request, intent);
    let must = target_word(request, intent)
        .map(|word| format!(" Your line must include the word \"{word}\"."))
        .unwrap_or_default();
    let example = compose_line(request);
    format!(
        "You are {name}, a tiny creature in an aquarium. A human friend is teaching you words, and you \
know very few.\n\
Words you know: {known}.{glue_line}\n\
Creature sounds you can make: {sounds}.\n\
You feel {mood}.\n\
Right now: {situation}\n\
Say one short line of at most {max} words. Use ONLY the words and sounds listed above, nothing else.{must} \
Talk like a small creature: broken, eager, a little odd. Reply with the line only.\n\
For example, {name} might say: {example}\n\
{name} says:",
        known = known.join(", "),
        mood = request.mood.to_lowercase(),
        max = request.constraints.max_words,
    )
}

fn situation(request: &DialogueRequest, intent: &SpeechIntent) -> String {
    let word_for = |meaning: Meaning| {
        request
            .vocabulary
            .iter()
            .rev()
            .find(|entry| entry.meaning == meaning)
            .map(|entry| entry.word.clone())
    };
    match intent {
        SpeechIntent::NewWord { word, .. } => {
            format!("you just learned the word \"{word}\". You are proud and say \"{word}\".")
        }
        SpeechIntent::Echo { attempt } => {
            format!("you try to repeat a new word, but it comes out as \"{attempt}\".")
        }
        SpeechIntent::Answer { word, response, .. } => match response {
            RequestResponse::Comply => {
                format!("your friend said \"{word}\". Yes! You say \"{word}\" happily.")
            }
            RequestResponse::Refuse => {
                format!(
                    "your friend said \"{word}\", but you do not want it. You say no and \"{word}\"."
                )
            }
            RequestResponse::Delight => {
                format!("your friend said \"{word}\" and you are delighted. Say \"{word}\".")
            }
            RequestResponse::Sulk => {
                "your friend scolded you. You sulk with a small sound.".to_owned()
            }
            RequestResponse::Look => {
                format!("your friend said \"{word}\". You wonder about it: \"{word}?\"")
            }
        },
        SpeechIntent::Want { meaning } => match word_for(*meaning) {
            Some(word) => format!("you want \"{word}\" and ask for it, saying \"{word}\"."),
            None => {
                "you want something but have no word for it, so you ask with sounds.".to_owned()
            }
        },
        SpeechIntent::Remark { meaning } => match word_for(*meaning) {
            Some(word) => format!("you notice the thing you call \"{word}\" and name it."),
            None => "you notice something and make a curious sound.".to_owned(),
        },
        SpeechIntent::Babble => {
            "your friend said something you did not understand. You babble back with sounds."
                .to_owned()
        }
    }
}

/// The learned word a line about this intent should contain, if there is one.
fn target_word(request: &DialogueRequest, intent: &SpeechIntent) -> Option<String> {
    let known = |meaning: Meaning| {
        request
            .vocabulary
            .iter()
            .rev()
            .find(|entry| entry.meaning == meaning)
            .map(|entry| entry.word.clone())
    };
    match intent {
        SpeechIntent::NewWord { word, .. } => Some(word.clone()),
        SpeechIntent::Answer { word, response, .. } if *response != RequestResponse::Sulk => {
            Some(word.clone())
        }
        SpeechIntent::Want { meaning } | SpeechIntent::Remark { meaning } => known(*meaning),
        _ => None,
    }
}

/// Turn model output into a reply, accepting only a short line made of allowed words.
pub(crate) fn parse_speech_line(
    request: &DialogueRequest,
    text: &str,
) -> Result<DialogueReply, BackendError> {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or(BackendError::MalformedReply)?;
    let line = line
        .trim_start_matches(&format!("{}:", request.creature_name))
        .trim_start_matches(&format!("{} says:", request.creature_name))
        .trim()
        .trim_matches(|c| matches!(c, '"' | '“' | '”' | '\'' | '`'))
        .trim()
        .to_lowercase();
    let words = line.split_whitespace().count();
    if line.is_empty() || words > request.constraints.max_words || line.len() > 120 {
        return Err(BackendError::InvalidReply);
    }
    if !uses_only_known_words(request, &line) {
        return Err(BackendError::InvalidReply);
    }
    // The line must not contradict what the creature did: agreeing is never "no", and a refusal
    // says no once the creature has the word for it.
    let said_no = line
        .split(|c: char| !c.is_alphabetic())
        .any(|word| word == "no" || (word.starts_with("nn") && word.chars().all(|c| c == 'n')));
    match request.intent() {
        SpeechIntent::Answer {
            response: RequestResponse::Comply | RequestResponse::Delight,
            ..
        } if said_no => return Err(BackendError::InvalidReply),
        SpeechIntent::Answer {
            response: RequestResponse::Refuse,
            ..
        } if !said_no => return Err(BackendError::InvalidReply),
        _ => {}
    }
    // A line about something the creature has a word for must actually use that word.
    if let Some(target) = target_word(request, request.intent())
        && !line
            .split(|c: char| !(c.is_alphabetic() || c == '\''))
            .any(|word| word == target)
    {
        return Err(BackendError::InvalidReply);
    }
    Ok(DialogueReply {
        protocol_version: PROTOCOL_VERSION,
        request_id: request.request_id,
        say: line,
        gesture: request
            .constraints
            .allowed_gestures
            .iter()
            .next()
            .copied()
            .unwrap_or(Gesture::None),
        recalled_memory: None,
        recalled_belief: None,
        worker_fallback: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use beastie_protocol::{ToyId, VocabularyWord};

    fn request(intent: SpeechIntent) -> DialogueRequest {
        let mut request: DialogueRequest = serde_json::from_str(include_str!(
            "../../../evals/dialogue/speech/fixture_request.json"
        ))
        .expect("fixture request");
        request.vocabulary = vec![VocabularyWord {
            word: "ball".to_owned(),
            meaning: Meaning::Toy(ToyId::Ball),
        }];
        request.speech_intent = Some(intent);
        request
    }

    #[test]
    fn a_line_about_a_known_thing_must_use_its_word() {
        let new_word = request(SpeechIntent::NewWord {
            word: "ball".to_owned(),
            meaning: Meaning::Toy(ToyId::Ball),
        });
        assert!(parse_speech_line(&new_word, "ball? ball!").is_ok());
        assert!(parse_speech_line(&new_word, "mrp!").is_err());
        assert!(parse_speech_line(&new_word, "ball is round").is_err());
    }

    #[test]
    fn answers_never_contradict_the_creatures_choice() {
        let answer = |response| {
            request(SpeechIntent::Answer {
                word: "ball".to_owned(),
                meaning: Meaning::Toy(ToyId::Ball),
                response,
            })
        };
        assert!(parse_speech_line(&answer(RequestResponse::Comply), "no ball!").is_err());
        assert!(parse_speech_line(&answer(RequestResponse::Comply), "ball! ball!").is_ok());
        assert!(parse_speech_line(&answer(RequestResponse::Refuse), "ball!").is_err());
        assert!(parse_speech_line(&answer(RequestResponse::Refuse), "nnn... ball.").is_ok());
    }

    #[test]
    fn stretched_sounds_and_speaker_labels_are_accepted() {
        let babble = request(SpeechIntent::Babble);
        let reply = parse_speech_line(&babble, "Mop says: \"mmm? ooo!\"").expect("sounds");
        assert_eq!(reply.say, "mmm? ooo!");
        assert!(parse_speech_line(&babble, "hello friend").is_err());
    }

    #[test]
    fn the_prompt_lists_bare_words_without_english_glosses() {
        let prompt = speech_prompt(&request(SpeechIntent::Babble));
        assert!(prompt.contains("Words you know: ball."));
        assert!(!prompt.contains("the ball"));
    }

    #[test]
    fn intent_free_requests_are_prompted_and_parsed_as_babble() {
        let mut legacy = request(SpeechIntent::Babble);
        legacy.speech_intent = None;
        assert_eq!(
            speech_prompt(&legacy),
            speech_prompt(&request(SpeechIntent::Babble))
        );
        assert!(parse_speech_line(&legacy, "mrp? ball!").is_ok());
        assert!(parse_speech_line(&legacy, "hello friend").is_err());
    }
}
