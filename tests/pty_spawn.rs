#![cfg(target_os = "linux")]

use lifelinetty::negotiation::{Capabilities, ControlCaps, ControlFrame, Role};
use lifelinetty::payload::{
    decode_command_frame, decode_tunnel_frame, encode_command_frame, encode_tunnel_msg,
    CommandMessage, TunnelMsgOwned,
};
use rustix::pty::OpenptFlags;
use serde_json::Value;
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn temp_home(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("lifelinetty_pty_home_{label}_{}", stamp()))
}

fn write_default_test_config(home: &PathBuf, extra: &str) {
    let dir = home.join(".serial_lcd");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    let base = "lcd_present = false\n";
    fs::write(path, format!("{base}{extra}")).unwrap();
}

fn open_pty_pair() -> Option<(File, String)> {
    let master = match rustix::pty::openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY) {
        Ok(master) => master,
        Err(err) => {
            eprintln!("skipping PTY test: openpt failed ({err})");
            return None;
        }
    };
    rustix::pty::grantpt(&master).unwrap();
    rustix::pty::unlockpt(&master).unwrap();

    let slave_name: CString = rustix::pty::ptsname(&master, Vec::with_capacity(64)).unwrap();
    let slave_path = slave_name.to_string_lossy().to_string();

    let raw = master.into_raw_fd();
    let master_file = unsafe { File::from_raw_fd(raw) };

    Some((master_file, slave_path))
}

fn spawn_line_reader(mut master: File) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        let mut buf = [0u8; 1024];
        let mut pending = Vec::<u8>::new();
        loop {
            match master.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    while let Some(pos) = pending.iter().position(|b| *b == b'\n') {
                        let line = pending.drain(..=pos).collect::<Vec<u8>>();
                        let line = String::from_utf8_lossy(&line)
                            .trim_end_matches(&['\r', '\n'][..])
                            .trim()
                            .to_string();
                        if !line.is_empty() {
                            let _ = tx.send(line);
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });
    rx
}

fn write_line(mut master: &File, line: &str) {
    let _ = master.write_all(line.as_bytes());
    let _ = master.write_all(b"\n");
    let _ = master.flush();
}

fn wait_for_child_exit(child: &mut Child, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn spawn_binary_exposes_lcd_output_via_stderr_observer() {
    let home = temp_home("lcd_observe");
    write_default_test_config(&home, "lcd_present = false\n");

    let payload_path = home.join("payload.json");
    fs::write(
        &payload_path,
        r#"{"schema_version":1,"line1":"Hello","line2":"LCD"}"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lifelinetty"))
        .args([
            "run",
            "--payload-file",
            payload_path.to_string_lossy().as_ref(),
            "--cols",
            "16",
            "--rows",
            "2",
            "--log-level",
            "error",
        ])
        .env("HOME", &home)
        .env("LIFELINETTY_LCD_OBSERVE", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();

    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("LIFELINETTY_LCD"),
        "expected LCD observer snapshots in stderr; got: {stderr}"
    );
    assert!(
        stderr.contains("Hello"),
        "expected line1 to appear in snapshots"
    );
    assert!(
        stderr.contains("LCD"),
        "expected line2 to appear in snapshots"
    );
}

#[test]
#[ignore]
fn playback_sample_jsons_to_lcd_observer_plain_text() {
    // Roadmap alignment: test harness utility for validating LCD render output
    // end-to-end via a spawned binary (no hardware).
    //
    // Usage:
    //   cargo test playback_sample_jsons_to_lcd_observer_plain_text -- --ignored --nocapture

    fn extract_payloads(raw: &str) -> Vec<String> {
        // 1) JSON object / array (including legacy wrapper files)
        if let Ok(value) = serde_json::from_str::<Value>(raw) {
            match value {
                Value::Object(mut map) => {
                    // Legacy sample format: { "examples": [ { "payload": {..} }, ... ] }
                    if let Some(Value::Array(examples)) = map.remove("examples") {
                        let mut out = Vec::new();
                        for example in examples {
                            if let Value::Object(mut ex) = example {
                                if let Some(payload) = ex.remove("payload") {
                                    if let Ok(payload_json) = serde_json::to_string(&payload) {
                                        out.push(payload_json);
                                    }
                                }
                            }
                        }
                        return out;
                    }

                    // Plain payload object
                    return vec![raw.trim().to_string()];
                }
                Value::Array(items) => {
                    return items
                        .into_iter()
                        .filter_map(|item| serde_json::to_string(&item).ok())
                        .collect();
                }
                _ => return Vec::new(),
            }
        }

        // 2) Fallback: treat as newline-delimited JSON (samples/payload_examples.json).
        raw.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter(|line| serde_json::from_str::<Value>(line).is_ok())
            .map(|line| line.to_string())
            .collect()
    }

    fn to_plain_text_payload(raw_payload: &str) -> Option<String> {
        let value = serde_json::from_str::<Value>(raw_payload).ok()?;
        let obj = value.as_object()?;

        let schema_version = obj
            .get("schema_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        let line1 = obj
            .get("line1")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .chars()
            .take(40)
            .collect::<String>();

        let line2 = obj
            .get("line2")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .chars()
            .take(40)
            .collect::<String>();

        let mut out = serde_json::Map::new();
        out.insert(
            "schema_version".to_string(),
            Value::Number(serde_json::Number::from(schema_version)),
        );
        out.insert("line1".to_string(), Value::String(line1));
        out.insert("line2".to_string(), Value::String(line2));

        serde_json::to_string(&Value::Object(out)).ok()
    }

    let home = temp_home("lcd_demo_playback");
    write_default_test_config(&home, "lcd_present = false\n");

    let sample_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples");
    let mut sample_files = fs::read_dir(&sample_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    sample_files.sort();

    assert!(
        !sample_files.is_empty(),
        "expected at least one JSON file under samples/"
    );

    for sample_path in sample_files {
        let raw = fs::read_to_string(&sample_path).unwrap();
        let payloads = extract_payloads(&raw);
        assert!(
            !payloads.is_empty(),
            "no JSON payloads found in {}",
            sample_path.display()
        );

        for (idx, payload) in payloads.into_iter().enumerate() {
            let payload = to_plain_text_payload(&payload).unwrap_or_else(|| {
                // If the sample isn't a payload object, still feed something safe.
                r#"{"schema_version":1,"line1":"(invalid payload)","line2":""}"#.to_string()
            });
            let tmp_payload = home.join(format!(
                "payload_{}_{}.json",
                sample_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("sample"),
                idx
            ));
            fs::write(&tmp_payload, payload).unwrap();

            let output = Command::new(env!("CARGO_BIN_EXE_lifelinetty"))
                .args([
                    "run",
                    "--payload-file",
                    tmp_payload.to_string_lossy().as_ref(),
                    "--cols",
                    "16",
                    "--rows",
                    "2",
                    "--log-level",
                    "error",
                ])
                .env("HOME", &home)
                .env("LIFELINETTY_LCD_OBSERVE", "1")
                .stdin(Stdio::null())
                .output()
                .unwrap();

            let stderr = String::from_utf8_lossy(&output.stderr);
            println!(
                "\n=== LCD PLAYBACK: {} (payload #{}) ===",
                sample_path.display(),
                idx
            );
            print!("{stderr}");

            assert!(
                output.status.success(),
                "lifelinetty exited non-zero; stderr: {stderr}"
            );
            assert!(
                stderr.contains("LIFELINETTY_LCD"),
                "expected LCD observer snapshots"
            );
        }
    }
}

#[test]
fn pty_spawn_serialsh_tunnel_round_trip() {
    // Roadmap alignment: extends test coverage of real IO over a pseudo-serial link
    // (helps validate tunnel framing end-to-end).

    let Some((master, slave_path)) = open_pty_pair() else {
        return;
    };
    let rx = spawn_line_reader(master.try_clone().unwrap());

    let mut child = Command::new(env!("CARGO_BIN_EXE_lifelinetty"))
        .args([
            "run",
            "--serialsh",
            "--device",
            &slave_path,
            "--serial-timeout-ms",
            "50",
            "--log-level",
            "debug",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // Send one command and exit.
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"echo hello\nexit\n").unwrap();
        stdin.flush().unwrap();
    }

    // Act as the remote peer for the serial shell: respond to tunnel CmdRequest.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut saw_init = false;
    let mut replied = false;

    while Instant::now() < deadline {
        let line = match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => line,
            Err(_) => continue,
        };

        if line == "INIT" {
            saw_init = true;
            continue;
        }

        // Serial shell only emits tunnel frames after INIT.
        if let Ok(TunnelMsgOwned::CmdRequest { cmd }) = decode_tunnel_frame(&line) {
            // Respond with a deterministic stdout+exit sequence.
            let stdout = if cmd == "echo hello" {
                b"hello\n".to_vec()
            } else {
                format!("unhandled: {cmd}\n").into_bytes()
            };
            let out_frame = encode_tunnel_msg(&TunnelMsgOwned::Stdout { chunk: stdout }).unwrap();
            write_line(&master, &out_frame);
            let exit_frame = encode_tunnel_msg(&TunnelMsgOwned::Exit { code: 0 }).unwrap();
            write_line(&master, &exit_frame);
            replied = true;
            break;
        }
    }

    wait_for_child_exit(&mut child, Duration::from_secs(2));

    let mut stdout = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut stdout);
    }
    let mut stderr = String::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
    }

    assert!(saw_init, "expected serialsh to emit INIT over serial");
    assert!(
        replied,
        "expected to receive CmdRequest and reply over tunnel"
    );
    assert!(
        stderr.contains("serialsh>"),
        "expected shell prompt in stderr"
    );
    assert!(stdout.contains("hello"), "expected tunnel stdout to print");
}

#[test]
fn pty_spawn_daemon_handshake_payload_and_command_frames() {
    // Roadmap alignment: IO correctness regression coverage for daemon loop.

    let home = temp_home("daemon");
    write_default_test_config(
        &home,
        "command_allowlist = [\"true\"]\npolling_enabled = false\n",
    );

    let Some((master, slave_path)) = open_pty_pair() else {
        let _ = fs::remove_dir_all(&home);
        return;
    };
    let rx = spawn_line_reader(master.try_clone().unwrap());

    let mut child = Command::new(env!("CARGO_BIN_EXE_lifelinetty"))
        .args([
            "run",
            "--device",
            &slave_path,
            "--serial-timeout-ms",
            "50",
            "--log-level",
            "debug",
        ])
        .env("HOME", &home)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(4);
    let mut saw_init = false;
    let mut saw_hello = false;
    let mut saw_ack = false;
    let mut saw_exit = false;

    while Instant::now() < deadline {
        let line = match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => line,
            Err(_) => continue,
        };

        if line == "INIT" {
            saw_init = true;
            continue;
        }

        if !saw_hello {
            if let Ok(ControlFrame::Hello { .. }) = serde_json::from_str::<ControlFrame>(&line) {
                saw_hello = true;
                let ack = ControlFrame::HelloAck {
                    chosen_role: Role::Server.as_str().to_string(),
                    peer_caps: ControlCaps {
                        bits: Capabilities::default().bits(),
                    },
                };
                let encoded = serde_json::to_string(&ack).unwrap();
                write_line(&master, &encoded);

                // Immediately send a valid payload frame after handshake.
                write_line(
                    &master,
                    r#"{"schema_version":1,"line1":"Test","line2":"Payload"}"#,
                );

                // And a command request frame to validate daemon command routing + responses.
                let req = CommandMessage::Request {
                    request_id: 1,
                    cmd: "true".to_string(),
                    scratch_path: None,
                };
                let frame = encode_command_frame(&req).unwrap();
                write_line(&master, &frame);
                continue;
            }
        }

        if let Ok(msg) = decode_command_frame(&line) {
            match msg {
                CommandMessage::Ack { request_id } => {
                    if request_id == 1 {
                        saw_ack = true;
                    }
                }
                CommandMessage::Exit { request_id, .. } => {
                    if request_id == 1 {
                        saw_exit = true;
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(saw_init, "expected daemon to emit INIT over serial");
    assert!(saw_hello, "expected daemon to emit negotiation hello frame");
    assert!(saw_ack, "expected daemon to ACK the command request");
    assert!(
        saw_exit,
        "expected daemon to emit Exit for the command request"
    );
}
