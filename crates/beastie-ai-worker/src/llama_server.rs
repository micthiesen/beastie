use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use beastie_protocol::{DialogueReply, DialogueRequest};
use serde::Deserialize;
use serde_json::json;

use crate::llama_cpp::{parse_single_reply, validate_model_grounding, validate_model_safety};
use crate::prompt::structured_prompt;
use crate::{BackendError, DialogueBackend, LlamaServerConfig};

const LOOPBACK_HOST: &str = "127.0.0.1";
const MAX_HTTP_HEADER_BYTES: usize = 8 * 1024;
const MAX_HTTP_BODY_BYTES: usize = 1024 * 1024;
const STARTUP_ATTEMPTS: usize = 3;
const STARTUP_POLL: Duration = Duration::from_millis(20);

#[derive(Debug)]
pub struct LlamaServerBackend {
    config: LlamaServerConfig,
    api_key: String,
    server: Option<RunningServer>,
}

#[derive(Debug)]
struct RunningServer {
    child: Child,
    shutdown: Option<std::process::ChildStdin>,
    address: SocketAddr,
}

impl LlamaServerBackend {
    #[must_use]
    pub fn new(mut config: LlamaServerConfig) -> Self {
        config.max_output_bytes = config.max_output_bytes.min(MAX_HTTP_BODY_BYTES);
        Self {
            config,
            api_key: random_api_key(),
            server: None,
        }
    }

    fn attempt(&mut self, request: &DialogueRequest) -> Result<DialogueReply, BackendError> {
        self.ensure_server()?;
        let prompt = server_prompt(request)?;
        let body = serde_json::to_vec(&json!({
            "messages": [{ "role": "user", "content": prompt }],
            "temperature": 0,
            "top_p": 1,
            "top_k": 1,
            "min_p": 0,
            "seed": 1,
            "max_tokens": 128,
            "stream": false,
            "reasoning_effort": "none",
            "chat_template_kwargs": { "enable_thinking": false },
        }))
        .map_err(|_| BackendError::MalformedReply)?;
        if body.len() > self.config.max_output_bytes {
            return Err(BackendError::OutputTooLarge);
        }

        let address = self
            .server
            .as_ref()
            .map(|server| server.address)
            .ok_or(BackendError::ExitFailure)?;
        let response = http_request(
            address,
            "POST",
            "/v1/chat/completions",
            Some(&body),
            &self.api_key,
            self.config.timeout,
            self.config.max_output_bytes,
        )?;
        if response.status != 200 {
            return Err(BackendError::ExitFailure);
        }
        let response: ChatResponse =
            serde_json::from_slice(&response.body).map_err(|_| BackendError::MalformedReply)?;
        let [choice] = response.choices.as_slice() else {
            return Err(BackendError::MalformedReply);
        };
        let reply = parse_single_reply(request, &choice.message.content)?;
        validate_model_safety(request, &reply)?;
        validate_model_grounding(request, &reply)?;
        Ok(reply)
    }

    fn ensure_server(&mut self) -> Result<(), BackendError> {
        if let Some(server) = self.server.as_mut() {
            if server
                .child
                .try_wait()
                .map_err(BackendError::Wait)?
                .is_none()
            {
                return Ok(());
            }
            self.server = None;
        }

        let mut last_error = BackendError::ExitFailure;
        for _ in 0..STARTUP_ATTEMPTS {
            match self.start_server() {
                Ok(server) => {
                    self.server = Some(server);
                    return Ok(());
                }
                Err(error) => last_error = error,
            }
        }
        Err(last_error)
    }

    fn start_server(&self) -> Result<RunningServer, BackendError> {
        let listener = TcpListener::bind((LOOPBACK_HOST, 0)).map_err(BackendError::Start)?;
        let address = listener.local_addr().map_err(BackendError::Start)?;
        drop(listener);

        let mut server_args = self.config.extra_args.clone();
        server_args.extend([
            OsString::from("--model"),
            self.config.model.clone().into_os_string(),
            OsString::from("--host"),
            OsString::from(LOOPBACK_HOST),
            OsString::from("--port"),
            OsString::from(address.port().to_string()),
            OsString::from("--api-key"),
            OsString::from(&self.api_key),
            OsString::from("--ctx-size"),
            OsString::from("2048"),
            OsString::from("--parallel"),
            OsString::from("1"),
            OsString::from("--no-warmup"),
            OsString::from("--offline"),
            OsString::from("--log-disable"),
        ]);
        if self.config.cpu_only {
            server_args.extend([
                OsString::from("--device"),
                OsString::from("none"),
                OsString::from("--no-op-offload"),
                OsString::from("-ngl"),
                OsString::from("0"),
            ]);
        }

        let mut command = if let Some(supervisor) = &self.config.supervisor {
            let mut command = Command::new(supervisor);
            command
                .arg("--llama-server-supervisor")
                .arg(&self.config.executable)
                .args(&server_args)
                .stdin(Stdio::piped());
            command
        } else {
            let mut command = Command::new(&self.config.executable);
            command.args(&server_args).stdin(Stdio::null());
            command
        };
        command.stdout(Stdio::null()).stderr(Stdio::null());
        configure_process_containment(&mut command);
        let mut child = command.spawn().map_err(BackendError::Start)?;
        let shutdown = if self.config.supervisor.is_some() {
            Some(child.stdin.take().ok_or(BackendError::ExitFailure)?)
        } else {
            None
        };
        let mut server = RunningServer {
            child,
            shutdown,
            address,
        };
        if let Err(error) = wait_for_health(
            server.address,
            &self.api_key,
            self.config.timeout,
            self.config.max_output_bytes,
            &mut server.child,
        ) {
            server.terminate();
            return Err(error);
        }
        Ok(server)
    }

    fn stop_server(&mut self) {
        if let Some(mut server) = self.server.take() {
            server.terminate();
        }
    }
}

fn server_prompt(request: &DialogueRequest) -> Result<String, BackendError> {
    let prompt = structured_prompt(request).map_err(|_| BackendError::MalformedReply)?;
    let (prefix, scaffold) = prompt
        .rsplit_once('\n')
        .ok_or(BackendError::MalformedReply)?;
    let scaffold: serde_json::Value =
        serde_json::from_str(scaffold).map_err(|_| BackendError::MalformedReply)?;
    let gesture = scaffold
        .get("gesture")
        .ok_or(BackendError::MalformedReply)?;
    let recalled_memory = scaffold
        .get("recalled_memory")
        .ok_or(BackendError::MalformedReply)?;
    let gesture = serde_json::to_string(gesture).map_err(|_| BackendError::MalformedReply)?;
    let recalled_memory =
        serde_json::to_string(recalled_memory).map_err(|_| BackendError::MalformedReply)?;
    Ok(format!(
        "{prefix}\n{{\"protocol_version\":{},\"request_id\":{},\"say\":\"__WRITE_SAY__\",\"gesture\":{gesture},\"recalled_memory\":{recalled_memory}}}",
        request.protocol_version, request.request_id
    ))
}

impl DialogueBackend for LlamaServerBackend {
    fn generate(&mut self, request: &DialogueRequest) -> Result<DialogueReply, BackendError> {
        let first = self.attempt(request);
        if first.is_ok() {
            return first;
        }
        self.stop_server();
        self.attempt(request)
    }
}

impl Drop for LlamaServerBackend {
    fn drop(&mut self) {
        self.stop_server();
    }
}

impl RunningServer {
    fn terminate(&mut self) {
        if self.shutdown.take().is_some() {
            let deadline = Instant::now() + Duration::from_millis(300);
            while Instant::now() < deadline {
                match self.child.try_wait() {
                    Ok(Some(_)) => return,
                    Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                    Err(_) => break,
                }
            }
        }
        terminate_child(&mut self.child);
    }
}

/// Runs the hidden parent-death supervisor used by the production worker binary.
///
/// `arguments` starts with the llama-server executable followed by its arguments. The supervisor
/// exits and reaps the server group when its stdin pipe closes, including when the parent worker
/// crashes or is killed.
pub fn run_llama_server_supervisor(arguments: Vec<OsString>) -> io::Result<()> {
    let Some((executable, server_args)) = arguments.split_first() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "llama-server supervisor requires an executable",
        ));
    };
    let mut command = Command::new(executable);
    command
        .args(server_args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    configure_process_containment(&mut command);
    let mut child = command.spawn()?;
    let (closed_sender, closed_receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut stdin = io::stdin().lock();
        let mut buffer = [0_u8; 64];
        while stdin.read(&mut buffer).unwrap_or(0) != 0 {}
        let _ = closed_sender.send(());
    });

    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if closed_receiver
            .recv_timeout(Duration::from_millis(20))
            .is_ok()
        {
            terminate_child(&mut child);
            return Ok(());
        }
    }
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: String,
}

fn wait_for_health(
    address: SocketAddr,
    api_key: &str,
    timeout: Duration,
    maximum: usize,
    child: &mut Child,
) -> Result<(), BackendError> {
    let started = Instant::now();
    loop {
        if child.try_wait().map_err(BackendError::Wait)?.is_some() {
            return Err(BackendError::ExitFailure);
        }
        let remaining = timeout
            .checked_sub(started.elapsed())
            .ok_or(BackendError::Timeout)?;
        let response = http_request(
            address,
            "GET",
            "/health",
            None,
            api_key,
            remaining.min(Duration::from_millis(250)),
            maximum,
        );
        if matches!(response, Ok(HttpResponse { status: 200, .. })) {
            return Ok(());
        }
        std::thread::sleep(STARTUP_POLL.min(remaining));
    }
}

fn http_request(
    address: SocketAddr,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    api_key: &str,
    timeout: Duration,
    maximum: usize,
) -> Result<HttpResponse, BackendError> {
    let started = Instant::now();
    let mut stream = TcpStream::connect_timeout(&address, timeout).map_err(BackendError::Start)?;
    let body = body.unwrap_or_default();
    let headers = format!(
        "{method} {path} HTTP/1.1\r\nHost: {LOOPBACK_HOST}:{}\r\nAuthorization: Bearer {api_key}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        address.port(),
        body.len()
    );
    write_bounded(&mut stream, headers.as_bytes(), timeout, started)?;
    write_bounded(&mut stream, body, timeout, started)?;
    stream.flush().map_err(BackendError::Read)?;
    read_http_response(&mut stream, maximum, timeout, started)
}

fn write_bounded(
    stream: &mut TcpStream,
    mut bytes: &[u8],
    timeout: Duration,
    started: Instant,
) -> Result<(), BackendError> {
    while !bytes.is_empty() {
        stream
            .set_write_timeout(Some(remaining_timeout(timeout, started)?))
            .map_err(BackendError::Read)?;
        let written = stream.write(bytes).map_err(map_http_read_error)?;
        if written == 0 {
            return Err(BackendError::ExitFailure);
        }
        bytes = &bytes[written..];
    }
    Ok(())
}

fn read_http_response(
    stream: &mut TcpStream,
    maximum: usize,
    timeout: Duration,
    started: Instant,
) -> Result<HttpResponse, BackendError> {
    let maximum = maximum.min(MAX_HTTP_BODY_BYTES);
    let mut received = Vec::with_capacity(MAX_HTTP_HEADER_BYTES.min(maximum));
    let header_end = loop {
        if received.len() >= MAX_HTTP_HEADER_BYTES {
            return Err(BackendError::OutputTooLarge);
        }
        let mut byte = [0_u8; 1];
        stream
            .set_read_timeout(Some(remaining_timeout(timeout, started)?))
            .map_err(BackendError::Read)?;
        let count = stream.read(&mut byte).map_err(map_http_read_error)?;
        if count == 0 {
            return Err(BackendError::ExitFailure);
        }
        received.push(byte[0]);
        if received.ends_with(b"\r\n\r\n") {
            break received.len();
        }
    };
    let headers = std::str::from_utf8(&received[..header_end]).map_err(|_| BackendError::Utf8)?;
    let mut lines = headers.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or(BackendError::MalformedReply)?;
    let length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .ok_or(BackendError::MalformedReply)?;
    if length > maximum {
        return Err(BackendError::OutputTooLarge);
    }
    let mut body = vec![0_u8; length];
    let mut read = 0;
    while read < body.len() {
        stream
            .set_read_timeout(Some(remaining_timeout(timeout, started)?))
            .map_err(BackendError::Read)?;
        let count = stream
            .read(&mut body[read..])
            .map_err(map_http_read_error)?;
        if count == 0 {
            return Err(BackendError::ExitFailure);
        }
        read += count;
    }
    Ok(HttpResponse { status, body })
}

fn remaining_timeout(timeout: Duration, started: Instant) -> Result<Duration, BackendError> {
    timeout
        .checked_sub(started.elapsed())
        .ok_or(BackendError::Timeout)
}

fn map_http_read_error(error: io::Error) -> BackendError {
    if matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ) {
        BackendError::Timeout
    } else {
        BackendError::Read(error)
    }
}

fn random_api_key() -> String {
    let mut random = [0_u8; 32];
    getrandom::fill(&mut random).expect("operating system randomness is required for local auth");
    hex_key(random)
}

fn hex_key(bytes: impl IntoIterator<Item = u8>) -> String {
    let mut key = String::with_capacity(64);
    for byte in bytes {
        write!(&mut key, "{byte:02x}").expect("writing to String cannot fail");
    }
    key
}

fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    if let Ok(process_group) = i32::try_from(child.id()) {
        // The server has its own group, so its descendants cannot outlive a restart or drop.
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        const SIGKILL: i32 = 9;
        unsafe {
            kill(-process_group, SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn configure_process_containment(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        command.process_group(0);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use beastie_protocol::DialogueRequest;

    use super::{LlamaServerBackend, LlamaServerConfig, MAX_HTTP_BODY_BYTES, server_prompt};

    const BERRY_MEMORY: &str = include_str!("../../../fixtures/dialogue/berry-memory.json");

    #[test]
    fn server_prompt_puts_say_before_the_remaining_reply_fields() {
        let request: DialogueRequest = serde_json::from_str(BERRY_MEMORY).expect("valid request");
        let prompt = server_prompt(&request).expect("prompt should build");
        assert!(prompt.ends_with(
            "{\"protocol_version\":1,\"request_id\":41,\"say\":\"__WRITE_SAY__\",\"gesture\":\"none\",\"recalled_memory\":41}"
        ));
    }

    #[test]
    fn configured_output_limit_cannot_exceed_the_hard_http_cap() {
        let backend = LlamaServerBackend::new(LlamaServerConfig {
            executable: "llama-server".into(),
            model: "model.gguf".into(),
            timeout: Duration::from_secs(1),
            max_output_bytes: usize::MAX,
            cpu_only: true,
            extra_args: Vec::new(),
            supervisor: None,
        });
        assert_eq!(backend.config.max_output_bytes, MAX_HTTP_BODY_BYTES);
    }
}
