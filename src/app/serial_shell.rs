use crate::payload::{decode_tunnel_frame, encode_tunnel_msg};
use crate::{
    app::AppConfig, cli::RunOptions, config::Config, payload::TunnelMsgOwned, serial::SerialPort,
    Result,
};
use std::io::{self, BufRead, Write};

/// Abstraction over the serial port used by the serial shell loop.
pub trait SerialShellTransport {
    fn send_command_line(&mut self, line: &str) -> Result<()>;
    fn read_message_line(&mut self, buf: &mut String) -> Result<usize>;
}

impl SerialShellTransport for SerialPort {
    fn send_command_line(&mut self, line: &str) -> Result<()> {
        SerialPort::send_command_line(self, line)
    }

    fn read_message_line(&mut self, buf: &mut String) -> Result<usize> {
        SerialPort::read_message_line(self, buf)
    }
}

/// Run the serial shell with stdin/stdout/stderr connected to the current process.
pub fn run_serial_shell(opts: RunOptions) -> Result<()> {
    let cfg = Config::load_or_default()?;
    let merged = AppConfig::from_sources(cfg, opts);
    let mut serial = SerialPort::connect(&merged.device, merged.serial_options())?;
    let stdin = io::stdin();
    let mut stdin_lock = stdin.lock();
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    let exit_code =
        drive_serial_shell_loop(&mut serial, &mut stdin_lock, &mut stdout, &mut stderr)?;
    std::process::exit(exit_code);
}

/// Core loop used by `run_serial_shell`. Accepts injectable transports + IO for easier testing.
pub fn drive_serial_shell_loop<T, I, O, E>(
    serial: &mut T,
    input: &mut I,
    stdout: &mut O,
    stderr: &mut E,
) -> Result<i32>
where
    T: SerialShellTransport,
    I: BufRead,
    O: Write,
    E: Write,
{
    serial.send_command_line("INIT")?;
    let mut buffer = String::new();
    let mut last_exit = 0;

    loop {
        buffer.clear();
        write_prompt(stdout)?;
        let bytes = input.read_line(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        let command = buffer.trim();
        if command.is_empty() {
            continue;
        }
        if command.eq_ignore_ascii_case("exit") {
            break;
        }
        send_serial_command(serial, command)?;
        last_exit = wait_for_exit(serial, stdout, stderr)?;
    }

    Ok(last_exit)
}

fn write_prompt<W: Write>(stdout: &mut W) -> Result<()> {
    stdout.write_all(b"serialsh> ")?;
    stdout.flush()?;
    Ok(())
}

fn send_serial_command<T: SerialShellTransport>(serial: &mut T, command: &str) -> Result<()> {
    let msg = TunnelMsgOwned::CmdRequest {
        cmd: command.to_string(),
    };
    let encoded = encode_tunnel_msg(&msg)?;
    serial.send_command_line(&encoded)
}

fn wait_for_exit<T, O, E>(serial: &mut T, stdout: &mut O, stderr: &mut E) -> Result<i32>
where
    T: SerialShellTransport,
    O: Write,
    E: Write,
{
    let mut line = String::new();
    loop {
        line.clear();
        if serial.read_message_line(&mut line)? == 0 {
            continue;
        }
        let trimmed = line.trim_end_matches(&['\r', '\n'][..]).trim();
        if trimmed.is_empty() || !is_tunnel_line(trimmed) {
            continue;
        }
        match decode_tunnel_frame(trimmed)? {
            TunnelMsgOwned::Stdout { chunk } => {
                write_chunk(&chunk, stdout)?;
            }
            TunnelMsgOwned::Stderr { chunk } => {
                write_chunk(&chunk, stderr)?;
            }
            TunnelMsgOwned::Exit { code } => return Ok(code),
            TunnelMsgOwned::Busy => {
                writeln!(stderr, "remote busy")?;
                return Ok(1);
            }
            TunnelMsgOwned::Heartbeat => {}
            _ => {}
        }
    }
}

fn write_chunk<W: Write>(chunk: &[u8], target: &mut W) -> Result<()> {
    target.write_all(chunk)?;
    target.flush()?;
    Ok(())
}

fn is_tunnel_line(line: &str) -> bool {
    line.contains("\"msg\"") && line.contains("\"crc32\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::encode_tunnel_msg;
    use crate::serial::fake::FakeSerialPort;
    use std::io::Cursor;

    impl SerialShellTransport for FakeSerialPort {
        fn send_command_line(&mut self, line: &str) -> Result<()> {
            FakeSerialPort::send_command_line(self, line)
        }

        fn read_message_line(&mut self, buf: &mut String) -> Result<usize> {
            FakeSerialPort::read_message_line(self, buf)
        }
    }

    fn encoded(msg: TunnelMsgOwned) -> String {
        encode_tunnel_msg(&msg).expect("failed to encode tunnel frame")
    }

    #[test]
    fn loop_tracks_exit_code_and_prompts() {
        let mut serial = FakeSerialPort::new(vec![
            Ok(encoded(TunnelMsgOwned::Stdout {
                chunk: b"hello".to_vec(),
            })),
            Ok(encoded(TunnelMsgOwned::Exit { code: 42 })),
        ]);
        let mut input = Cursor::new("echo hi\nexit\n");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = drive_serial_shell_loop(&mut serial, &mut input, &mut stdout, &mut stderr)
            .expect("failed to drive loop");

        assert_eq!(exit_code, 42);
        assert!(stderr.is_empty(), "stderr got data: {:?}", stderr);
        let output = String::from_utf8_lossy(&stdout);
        assert!(output.matches("serialsh> ").count() >= 2);
        assert!(output.contains("hello"));
        assert_eq!(
            serial.writes(),
            &[
                "INIT".to_string(),
                encoded(TunnelMsgOwned::CmdRequest {
                    cmd: "echo hi".into(),
                }),
            ]
        );
    }

    #[test]
    fn busy_response_returns_one() {
        let mut serial = FakeSerialPort::new(vec![Ok(encoded(TunnelMsgOwned::Busy))]);
        let mut input = Cursor::new("list\nexit\n");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = drive_serial_shell_loop(&mut serial, &mut input, &mut stdout, &mut stderr)
            .expect("loop failed");

        assert_eq!(exit_code, 1);
        assert!(String::from_utf8_lossy(&stderr).contains("remote busy"));
    }
}
