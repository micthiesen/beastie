use std::io::{self, BufRead, Write};

use beastie_protocol::{
    DialogueReply, DialogueRequest, Gesture, PROTOCOL_VERSION, constrained_fallback_reply,
    fallback_reply, validate_reply, validate_request,
};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        let reply = process_line(&line);
        serde_json::to_writer(&mut stdout, &reply)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}

fn process_line(line: &str) -> DialogueReply {
    let Ok(request) = serde_json::from_str::<DialogueRequest>(line) else {
        return fallback_reply(0);
    };
    if validate_request(&request).is_err() {
        return fallback_reply(request.request_id);
    }
    let reply = fixture_reply(&request);
    validate_reply(&request, reply).unwrap_or_else(|_| constrained_fallback_reply(&request))
}

fn fixture_reply(request: &DialogueRequest) -> DialogueReply {
    let memory = request.candidate_memories.first();
    DialogueReply {
        protocol_version: PROTOCOL_VERSION,
        request_id: request.request_id,
        say: memory.map_or_else(
            || "hm. no old thought.".to_owned(),
            |_| "yes. old thing remains.".to_owned(),
        ),
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
        recalled_memory: memory.map(|candidate| candidate.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BERRY_MEMORY: &str = include_str!("../../../fixtures/dialogue/berry-memory.json");

    #[test]
    fn jsonl_fixture_produces_a_valid_reply() {
        let request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY.trim()).expect("fixture request should parse");
        let reply = process_line(BERRY_MEMORY.trim());
        assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
    }

    #[test]
    fn generated_reply_falls_back_to_requested_word_limit() {
        let mut request: DialogueRequest =
            serde_json::from_str(BERRY_MEMORY.trim()).expect("fixture request should parse");
        request.constraints.max_words = 1;
        let line = serde_json::to_string(&request).expect("request should serialize");
        let reply = process_line(&line);
        assert_eq!(validate_reply(&request, reply.clone()), Ok(reply));
    }
}
