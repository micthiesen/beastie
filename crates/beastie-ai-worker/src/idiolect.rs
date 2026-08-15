use beastie_protocol::{DialogueReply, DialogueRequest, IdiolectQuirk, MAX_DIALOGUE_REPLY_BYTES};

/// Applies only semantics-preserving surface quirks.  New words are either a pause marker or
/// an existing word, so the worker cannot invent a fact while making a save's voice recognizable.
pub(crate) fn apply(request: &DialogueRequest, reply: DialogueReply) -> DialogueReply {
    let quirk = request.idiolect.quirk;
    if quirk == IdiolectQuirk::Plain {
        return reply;
    }

    let original = reply.say.clone();
    let mut words = original.split_whitespace().collect::<Vec<_>>();
    let styled = match quirk {
        IdiolectQuirk::Plain => original,
        IdiolectQuirk::ArticleDrop => {
            words.retain(|word| {
                !matches!(
                    word.trim_matches(|character: char| !character.is_alphanumeric())
                        .to_ascii_lowercase()
                        .as_str(),
                    "a" | "an" | "the"
                )
            });
            if words.is_empty() {
                original
            } else {
                words.join(" ")
            }
        }
        IdiolectQuirk::Echo => {
            if words.len() >= request.constraints.max_words {
                original
            } else if let Some(last) = words.last().copied() {
                words.push(last);
                words.join(" ")
            } else {
                original
            }
        }
        IdiolectQuirk::HmPrefix => {
            if words.len() >= request.constraints.max_words {
                original
            } else {
                format!("hm, {original}")
            }
        }
    };

    let styled = if request.context.repetition_count > 0 {
        let ending = match request.mood.to_ascii_lowercase().as_str() {
            "sleepy" | "lonely" => "...",
            "resentful" => "!",
            _ => "",
        };
        if ending.is_empty() {
            styled
        } else {
            format!("{}{}", styled.trim_end_matches(['.', '!', '?']), ending)
        }
    } else {
        styled
    };

    if styled.is_empty() || styled.len() > MAX_DIALOGUE_REPLY_BYTES {
        return reply;
    }
    let mut reply = reply;
    reply.say = styled;
    reply
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use beastie_protocol::{
        DialogueConstraints, DialogueReply, DialogueRequest, Gesture, PROTOCOL_VERSION,
        validate_reply,
    };

    use super::*;

    fn request(quirk: IdiolectQuirk, max_words: usize) -> DialogueRequest {
        DialogueRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: 1,
            creature_name: "Mop".to_owned(),
            mood: "content".to_owned(),
            known_concepts: BTreeSet::new(),
            candidate_memories: Vec::new(),
            candidate_beliefs: Vec::new(),
            idiolect: beastie_protocol::Idiolect { quirk },
            desired_social_act: None,
            input_rejection: None,
            context: beastie_protocol::DialogueContext::default(),
            player_said: "hello".to_owned(),
            constraints: DialogueConstraints {
                max_words,
                allowed_gestures: BTreeSet::from([Gesture::None]),
            },
        }
    }

    fn reply(say: &str) -> DialogueReply {
        DialogueReply {
            protocol_version: PROTOCOL_VERSION,
            request_id: 1,
            say: say.to_owned(),
            gesture: Gesture::None,
            recalled_memory: None,
            recalled_belief: None,
        }
    }

    #[test]
    fn quirks_are_stable_and_bounded() {
        let request = request(IdiolectQuirk::Echo, 5);
        let styled = apply(&request, reply("old berry remains."));
        assert_eq!(styled.say, "old berry remains. remains.");
        assert_eq!(apply(&request, reply("old berry remains.")), styled);
        validate_reply(&request, styled).expect("styled reply should remain valid");
    }

    #[test]
    fn article_drop_does_not_erase_an_entire_reply() {
        let request = request(IdiolectQuirk::ArticleDrop, 5);
        let styled = apply(&request, reply("the"));
        assert_eq!(styled.say, "the");
    }
}
