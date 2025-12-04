use crate::{
    payload::{decode_command_frame, CommandMessage, CommandStream},
    Result,
};
use serde_bytes::ByteBuf;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc,
};
use std::thread;

/// Stores scroll offsets for the two LCD lines to avoid ad-hoc tuples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollOffsets {
    pub top: usize,
    pub bottom: usize,
}

impl ScrollOffsets {
    pub fn zero() -> Self {
        Self { top: 0, bottom: 0 }
    }

    pub fn update(self, top: usize, bottom: usize) -> Self {
        Self { top, bottom }
    }
}

/// Lightweight event type so Milestone A can hook specific handlers later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandEvent {
    Request {
        request_id: u32,
        cmd: String,
        scratch_path: Option<String>,
    },
    Chunk {
        request_id: u32,
        stream: CommandStream,
        seq: u32,
        len: usize,
    },
    Exit {
        request_id: u32,
        code: i32,
    },
    Ack {
        request_id: u32,
    },
    Busy {
        request_id: u32,
    },
    Error {
        request_id: Option<u32>,
        message: String,
    },
    Heartbeat {
        request_id: Option<u32>,
    },
}

impl CommandEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            CommandEvent::Request { .. } => "request",
            CommandEvent::Chunk { stream, .. } => match stream {
                CommandStream::Stdout => "stdout",
                CommandStream::Stderr => "stderr",
            },
            CommandEvent::Exit { .. } => "exit",
            CommandEvent::Ack { .. } => "ack",
            CommandEvent::Busy { .. } => "busy",
            CommandEvent::Error { .. } => "error",
            CommandEvent::Heartbeat { .. } => "heartbeat",
        }
    }
}

impl From<CommandMessage> for CommandEvent {
    fn from(msg: CommandMessage) -> Self {
        match msg {
            CommandMessage::Request {
                request_id,
                cmd,
                scratch_path,
            } => CommandEvent::Request {
                request_id,
                cmd,
                scratch_path,
            },
            CommandMessage::Chunk {
                request_id,
                stream,
                seq,
                data,
            } => CommandEvent::Chunk {
                request_id,
                stream,
                seq,
                len: data.len(),
            },
            CommandMessage::Exit { request_id, code } => CommandEvent::Exit { request_id, code },
            CommandMessage::Ack { request_id } => CommandEvent::Ack { request_id },
            CommandMessage::Busy { request_id } => CommandEvent::Busy { request_id },
            CommandMessage::Error {
                request_id,
                message,
            } => CommandEvent::Error {
                request_id,
                message,
            },
            CommandMessage::Heartbeat { request_id } => CommandEvent::Heartbeat { request_id },
        }
    }
}

/// CommandBridge ingests newline-delimited JSON command frames and emits structured events.
#[derive(Default)]
pub struct CommandBridge {
    last_seen_request: Option<u32>,
}

impl CommandBridge {
    pub fn new() -> Self {
        Self {
            last_seen_request: None,
        }
    }

    pub fn ingest_line(&mut self, raw: &str) -> Result<Option<CommandEvent>> {
        let message = decode_command_frame(raw)?;
        if let Some(request_id) = message_request_id(&message) {
            self.last_seen_request = Some(request_id);
        }
        let event = CommandEvent::from(message);
        Ok(Some(event))
    }

    pub fn last_request_id(&self) -> Option<u32> {
        self.last_seen_request
    }
}

fn message_request_id(msg: &CommandMessage) -> Option<u32> {
    match msg {
        CommandMessage::Request { request_id, .. }
        | CommandMessage::Chunk { request_id, .. }
        | CommandMessage::Exit { request_id, .. }
        | CommandMessage::Ack { request_id }
        | CommandMessage::Busy { request_id }
        | CommandMessage::Heartbeat {
            request_id: Some(request_id),
        } => Some(*request_id),
        CommandMessage::Error { request_id, .. } => *request_id,
        CommandMessage::Heartbeat { request_id: None } => None,
    }
}

const COMMAND_STREAM_CHUNK_SIZE: usize = 512;

pub struct CommandExecutor {
    allowlist: Vec<String>,
    session_active: bool,
    current_request: Option<u32>,
    outgoing_tx: Sender<CommandMessage>,
    outgoing_rx: Receiver<CommandMessage>,
}

impl CommandExecutor {
    pub fn new(allowlist: Vec<String>) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            allowlist,
            session_active: false,
            current_request: None,
            outgoing_tx: tx,
            outgoing_rx: rx,
        }
    }

    pub fn handle_event(&mut self, event: CommandEvent) -> Option<CommandMessage> {
        match event {
            CommandEvent::Request {
                request_id,
                cmd,
                scratch_path: _,
            } => {
                if self.session_active {
                    return Some(CommandMessage::Busy { request_id });
                }
                let tokens = match split_command_line(&cmd) {
                    Ok(tokens) => tokens,
                    Err(err) => {
                        let msg = format!("command parse error: {err}");
                        self.queue(CommandMessage::Error {
                            request_id: Some(request_id),
                            message: msg.clone(),
                        });
                        self.queue(CommandMessage::Exit {
                            request_id,
                            code: 1,
                        });
                        return Some(CommandMessage::Error {
                            request_id: Some(request_id),
                            message: msg,
                        });
                    }
                };
                let program = tokens[0].clone();
                if !command_allowed(&program, &self.allowlist) {
                    let msg = format!("command not allowed: {program}");
                    self.queue(CommandMessage::Error {
                        request_id: Some(request_id),
                        message: msg.clone(),
                    });
                    self.queue(CommandMessage::Exit {
                        request_id,
                        code: 1,
                    });
                    return Some(CommandMessage::Error {
                        request_id: Some(request_id),
                        message: msg,
                    });
                }
                match Command::new(&program)
                    .args(&tokens[1..])
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                {
                    Ok(mut child) => {
                        self.session_active = true;
                        self.current_request = Some(request_id);
                        let tx = self.outgoing_tx.clone();
                        let stdout_seq = Arc::new(AtomicU32::new(0));
                        let stderr_seq = Arc::new(AtomicU32::new(0));
                        if let Some(stdout) = child.stdout.take() {
                            spawn_stream_reader(
                                stdout,
                                CommandStream::Stdout,
                                request_id,
                                stdout_seq,
                                tx.clone(),
                            );
                        }
                        if let Some(stderr) = child.stderr.take() {
                            spawn_stream_reader(
                                stderr,
                                CommandStream::Stderr,
                                request_id,
                                stderr_seq,
                                tx.clone(),
                            );
                        }
                        let tx_exit = self.outgoing_tx.clone();
                        thread::spawn(move || {
                            let code = match child.wait() {
                                Ok(status) => status.code().unwrap_or(-1),
                                Err(_) => -1,
                            };
                            let _ = tx_exit.send(CommandMessage::Exit { request_id, code });
                        });
                        Some(CommandMessage::Ack { request_id })
                    }
                    Err(err) => {
                        let msg = format!("failed to spawn '{program}': {err}");
                        self.queue(CommandMessage::Error {
                            request_id: Some(request_id),
                            message: msg.clone(),
                        });
                        self.queue(CommandMessage::Exit {
                            request_id,
                            code: 1,
                        });
                        Some(CommandMessage::Error {
                            request_id: Some(request_id),
                            message: msg,
                        })
                    }
                }
            }
            _ => None,
        }
    }

    pub fn next_outgoing(&mut self) -> Option<CommandMessage> {
        match self.outgoing_rx.try_recv() {
            Ok(msg) => {
                if matches!(msg, CommandMessage::Exit { .. }) {
                    self.session_active = false;
                    self.current_request = None;
                }
                Some(msg)
            }
            Err(_) => None,
        }
    }

    fn queue(&self, msg: CommandMessage) {
        let _ = self.outgoing_tx.send(msg);
    }
}

fn spawn_stream_reader<R>(
    mut reader: R,
    stream: CommandStream,
    request_id: u32,
    seq_counter: Arc<AtomicU32>,
    tx: Sender<CommandMessage>,
) where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buf = [0u8; COMMAND_STREAM_CHUNK_SIZE];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let seq = seq_counter.fetch_add(1, Ordering::SeqCst);
                    let data = ByteBuf::from(buf[..n].to_vec());
                    let msg = CommandMessage::Chunk {
                        request_id,
                        stream,
                        seq,
                        data,
                    };
                    if tx.send(msg).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn split_command_line(line: &str) -> std::result::Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escape = false;
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return Err("empty command".into());
    }

    for ch in trimmed.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }
        match ch {
            '\\' => {
                escape = true;
            }
            '\'' | '"' => {
                if let Some(marker) = quote {
                    if marker == ch {
                        quote = None;
                    } else {
                        current.push(ch);
                    }
                } else {
                    quote = Some(ch);
                }
            }
            c if c.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            c => {
                current.push(c);
            }
        }
    }

    if escape {
        return Err("unterminated escape".into());
    }
    if quote.is_some() {
        return Err("unterminated quote".into());
    }
    if !current.is_empty() {
        args.push(current);
    }
    if args.is_empty() {
        return Err("empty command".into());
    }
    Ok(args)
}

fn command_allowed(program: &str, allowlist: &[String]) -> bool {
    if allowlist.is_empty() {
        return true;
    }
    let candidate = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program);
    allowlist
        .iter()
        .any(|entry| entry == program || entry == candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::encode_command_frame;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn bridge_parses_request() {
        let msg = CommandMessage::Request {
            request_id: 42,
            cmd: "uptime".into(),
            scratch_path: Some(format!("{}/tunnel/req42", crate::CACHE_DIR)),
        };
        let encoded = encode_command_frame(&msg).unwrap();
        let mut bridge = CommandBridge::default();
        let event = bridge.ingest_line(&encoded).unwrap().unwrap();
        assert!(matches!(
            event,
            CommandEvent::Request { request_id: 42, .. }
        ));
        assert_eq!(bridge.last_request_id(), Some(42));
    }

    #[test]
    fn command_executor_rejects_disallowed() {
        let mut executor = CommandExecutor::new(vec!["true".into()]);
        let response = executor.handle_event(CommandEvent::Request {
            request_id: 5,
            cmd: "whoami".into(),
            scratch_path: None,
        });
        assert!(matches!(
            response,
            Some(CommandMessage::Error {
                request_id: Some(5),
                ..
            })
        ));
        let mut saw_exit = false;
        while let Some(msg) = executor.next_outgoing() {
            if let CommandMessage::Exit { request_id, code } = msg {
                assert_eq!(request_id, 5);
                assert_eq!(code, 1);
                saw_exit = true;
                break;
            }
        }
        assert!(saw_exit);
    }

    #[cfg(unix)]
    #[test]
    fn command_executor_emits_exit_for_true() {
        let mut executor = CommandExecutor::new(Vec::new());
        let response = executor.handle_event(CommandEvent::Request {
            request_id: 7,
            cmd: "true".into(),
            scratch_path: None,
        });
        assert!(matches!(
            response,
            Some(CommandMessage::Ack { request_id: 7 })
        ));
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut exit_seen = false;
        while Instant::now() < deadline {
            if let Some(msg) = executor.next_outgoing() {
                if let CommandMessage::Exit { request_id, code } = msg {
                    assert_eq!(request_id, 7);
                    assert_eq!(code, 0);
                    exit_seen = true;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(exit_seen, "expected exit message");
    }

    #[cfg(unix)]
    #[test]
    fn command_executor_returns_busy_when_active() {
        let mut executor = CommandExecutor::new(Vec::new());
        let _ = executor.handle_event(CommandEvent::Request {
            request_id: 8,
            cmd: "sleep 1".into(),
            scratch_path: None,
        });
        let busy = executor.handle_event(CommandEvent::Request {
            request_id: 9,
            cmd: "true".into(),
            scratch_path: None,
        });
        assert!(matches!(busy, Some(CommandMessage::Busy { request_id: 9 })));
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if let Some(msg) = executor.next_outgoing() {
                if matches!(msg, CommandMessage::Exit { .. }) {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
