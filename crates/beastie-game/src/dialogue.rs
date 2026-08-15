use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use beastie_protocol::{
    DialogueReply, DialogueRequest, constrained_fallback_reply, validate_reply, validate_request,
};

pub fn request(worker: Option<PathBuf>, dialogue: DialogueRequest) -> Receiver<DialogueReply> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let fallback = || constrained_fallback_reply(&dialogue);
        let reply = worker
            .and_then(|worker| run_worker(worker, &dialogue))
            .unwrap_or_else(fallback);
        let _ = sender.send(reply);
    });
    receiver
}

fn run_worker(worker: PathBuf, request: &DialogueRequest) -> Option<DialogueReply> {
    validate_request(request).ok()?;
    let mut child = Command::new(worker)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let request_line = serde_json::to_string(request).ok()?;
    let mut stdin = child.stdin.take()?;
    writeln!(stdin, "{request_line}").ok()?;
    drop(stdin);

    let stdout = child.stdout.take()?;
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout)
            .read_line(&mut line)
            .ok()
            .map(|_| line);
        let _ = sender.send(result);
    });
    let line = match receiver.recv_timeout(Duration::from_secs(2)) {
        Ok(Some(line)) => line,
        Ok(None) | Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return None;
        }
    };
    let status = child.wait().ok()?;
    reader.join().ok()?;
    if !status.success() {
        return None;
    }
    let reply = serde_json::from_str::<DialogueReply>(&line).ok()?;
    validate_reply(request, reply).ok()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use beastie_protocol::{DialogueConstraints, Gesture, PROTOCOL_VERSION};

    use super::*;

    #[test]
    fn missing_worker_returns_a_bounded_fallback() {
        let dialogue_request = DialogueRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: 7,
            creature_name: "Mop".to_owned(),
            mood: "wary".to_owned(),
            known_concepts: BTreeSet::new(),
            candidate_memories: Vec::new(),
            desired_social_act: None,
            player_said: "hello".to_owned(),
            constraints: DialogueConstraints {
                max_words: 3,
                allowed_gestures: BTreeSet::from([Gesture::None]),
            },
        };
        let reply = request(None, dialogue_request.clone())
            .recv_timeout(Duration::from_secs(1))
            .expect("fallback should arrive");
        assert_eq!(validate_reply(&dialogue_request, reply.clone()), Ok(reply));
    }
}
