use crate::{
    compression::CompressionCodec,
    config::Pcf8574Addr,
    serial::{DtrBehavior, FlowControlMode, ParityMode, StopBitsMode},
    Error, Result,
};

/// Entry mode for the `run` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// Default daemon path that renders onto the LCD.
    Daemon,
    /// P7: CLI integration groundwork for the serial shell preview gate.
    SerialShell,
}

impl Default for RunMode {
    fn default() -> Self {
        RunMode::Daemon
    }
}

/// Options for the `run` command; values are `None` when not provided on CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunOptions {
    pub mode: RunMode,
    pub device: Option<String>,
    pub baud: Option<u32>,
    pub flow_control: Option<FlowControlMode>,
    pub parity: Option<ParityMode>,
    pub stop_bits: Option<StopBitsMode>,
    pub dtr_on_open: Option<DtrBehavior>,
    pub serial_timeout_ms: Option<u64>,
    pub cols: Option<u8>,
    pub rows: Option<u8>,
    pub payload_file: Option<String>,
    pub backoff_initial_ms: Option<u64>,
    pub backoff_max_ms: Option<u64>,
    pub pcf8574_addr: Option<Pcf8574Addr>,
    pub log_level: Option<String>,
    pub log_file: Option<String>,
    pub compression_enabled: Option<bool>,
    pub compression_codec: Option<CompressionCodec>,
    pub demo: bool,
    pub polling_enabled: Option<bool>,
    pub poll_interval_ms: Option<u64>,
}

/// Parsed command-line intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run(RunOptions),
    ShowHelp,
    ShowVersion,
}

impl Command {
    pub fn parse(args: &[String]) -> Result<Self> {
        if args.is_empty() {
            return Ok(Command::Run(RunOptions::default()));
        }

        let mut iter = args.iter();
        match iter.next().map(|s| s.as_str()) {
            Some("run") => Ok(Command::Run(parse_run_options(&mut iter)?)),
            Some("--help") | Some("-h") => Ok(Command::ShowHelp),
            Some("--version") | Some("-V") => Ok(Command::ShowVersion),
            Some(flag) if flag.starts_with('-') => {
                // Allow omitting the explicit `run` subcommand: pass the consumed flag plus the
                // remaining args into the run parser.
                let mut flags: Vec<String> = Vec::with_capacity(args.len());
                flags.push(flag.to_string());
                flags.extend(iter.map(|s| s.to_string()));
                let mut iter = flags.iter();
                Ok(Command::Run(parse_run_options(&mut iter)?))
            }
            Some(cmd) => Err(Error::InvalidArgs(format!(
                "unknown command '{cmd}', try --help"
            ))),
            None => Ok(Command::Run(RunOptions::default())),
        }
    }
    pub fn help() -> String {
        let mut help = String::from(
            "lifelinetty - Serial-to-LCD daemon\n\nUSAGE:\n  lifelinetty run [--device <path>] [--baud <number>] [--cols <number>] [--rows <number>] [--payload-file <path>]\n  lifelinetty --help\n  lifelinetty --version\n\nOPTIONS:\n  --device <path>   Serial device path (default: /dev/ttyUSB0)\n  --baud <number>   Baud rate (default: 9600)\n  --flow-control <none|software|hardware>  Flow control override (default: none)\n  --parity <none|odd|even>       Parity override (default: none)\n  --stop-bits <1|2>              Stop bits override (default: 1)\n  --dtr-on-open <auto|on|off>    Control DTR state when opening the port (default: auto)\n  --serial-timeout-ms <number>   Read timeout in milliseconds (default: 500)\n  --cols <number>   LCD columns (default: 20)\n  --rows <number>   LCD rows (default: 4)\n  --payload-file <path>  Load a local JSON payload and render it once (testing helper)\n  --backoff-initial-ms <number>  Initial reconnect backoff (default: 500)\n  --backoff-max-ms <number>      Maximum reconnect backoff (default: 10000)\n  --pcf8574-addr <auto|0xNN>     PCF8574 I2C address or 'auto' to probe (default: auto)\n  --log-level <error|warn|info|debug|trace>  Log verbosity (default: info)\n  --log-file <path>              Append logs inside /run/serial_lcd_cache (also honors LIFELINETTY_LOG_PATH)\n  --polling                      Enable hardware polling (default: config)\n  --no-polling                   Disable hardware polling even if config enables it\n  --poll-interval-ms <number>    Polling interval in milliseconds (default: 5000)\n  --compressed                   Enable schema compression (applies to schema_v1 payloads)\n  --no-compressed                Disable compression even if config enables it\n  --codec <lz4|zstd>             Codec to use when compression is enabled (default: lz4)\n  --demo                         Run built-in demo pages on the LCD (no serial input)\n",
        );

        help.push_str(
            "  --serialsh                   Enable the optional serial shell that runs commands over the tunnel and streams remote stdout/stderr + exit codes\n",
        );

        help.push_str("  -h, --help        Show this help\n  -V, --version     Show version\n");
        help
    }

    pub fn print_help() {
        println!("{}", Self::help());
    }
}

fn parse_run_options(iter: &mut std::slice::Iter<String>) -> Result<RunOptions> {
    let mut opts = RunOptions::default();

    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--device" => {
                opts.device = Some(take_value(flag, iter)?);
            }
            "--baud" => {
                let raw = take_value(flag, iter)?;
                opts.baud = Some(raw.parse().map_err(|_| {
                    Error::InvalidArgs("baud must be a positive integer".to_string())
                })?);
            }
            "--flow-control" => {
                let raw = take_value(flag, iter)?;
                opts.flow_control = Some(raw.parse().map_err(|e: String| Error::InvalidArgs(e))?);
            }
            "--parity" => {
                let raw = take_value(flag, iter)?;
                opts.parity = Some(raw.parse().map_err(|e: String| Error::InvalidArgs(e))?);
            }
            "--stop-bits" => {
                let raw = take_value(flag, iter)?;
                opts.stop_bits = Some(raw.parse().map_err(|e: String| Error::InvalidArgs(e))?);
            }
            "--dtr-on-open" => {
                let raw = take_value(flag, iter)?;
                opts.dtr_on_open = Some(raw.parse().map_err(|e: String| Error::InvalidArgs(e))?);
            }
            "--serial-timeout-ms" => {
                let raw = take_value(flag, iter)?;
                opts.serial_timeout_ms = Some(raw.parse().map_err(|_| {
                    Error::InvalidArgs("serial-timeout-ms must be a positive integer".to_string())
                })?);
            }
            "--cols" => {
                let raw = take_value(flag, iter)?;
                opts.cols = Some(raw.parse().map_err(|_| {
                    Error::InvalidArgs("cols must be a positive integer".to_string())
                })?);
            }
            "--rows" => {
                let raw = take_value(flag, iter)?;
                opts.rows = Some(raw.parse().map_err(|_| {
                    Error::InvalidArgs("rows must be a positive integer".to_string())
                })?);
            }
            "--payload-file" => {
                opts.payload_file = Some(take_value(flag, iter)?);
            }
            "--backoff-initial-ms" => {
                let raw = take_value(flag, iter)?;
                opts.backoff_initial_ms = Some(raw.parse().map_err(|_| {
                    Error::InvalidArgs("backoff-initial-ms must be a positive integer".to_string())
                })?);
            }
            "--backoff-max-ms" => {
                let raw = take_value(flag, iter)?;
                opts.backoff_max_ms = Some(raw.parse().map_err(|_| {
                    Error::InvalidArgs("backoff-max-ms must be a positive integer".to_string())
                })?);
            }
            "--pcf8574-addr" => {
                let raw = take_value(flag, iter)?;
                opts.pcf8574_addr = Some(raw.parse().map_err(|_| {
                    Error::InvalidArgs(
                        "pcf8574-addr must be 'auto' or a hex/decimal address (e.g., 0x27)"
                            .to_string(),
                    )
                })?);
            }
            "--log-level" => {
                opts.log_level = Some(take_value(flag, iter)?);
            }
            "--log-file" => {
                opts.log_file = Some(take_value(flag, iter)?);
            }
            "--polling" => {
                opts.polling_enabled = Some(true);
            }
            "--no-polling" => {
                opts.polling_enabled = Some(false);
            }
            "--poll-interval-ms" => {
                let raw = take_value(flag, iter)?;
                opts.poll_interval_ms = Some(raw.parse().map_err(|_| {
                    Error::InvalidArgs("poll-interval-ms must be a positive integer".to_string())
                })?);
            }
            "--compressed" => {
                opts.compression_enabled = Some(true);
            }
            "--no-compressed" => {
                opts.compression_enabled = Some(false);
            }
            "--codec" => {
                let raw = take_value(flag, iter)?;
                opts.compression_codec =
                    Some(CompressionCodec::from_name(&raw).ok_or_else(|| {
                        Error::InvalidArgs("codec must be one of: none, lz4, zstd".to_string())
                    })?);
            }
            "--demo" => {
                opts.demo = true;
            }
            "--serialsh" => {
                // Milestone G: run the CLI serial shell through the command tunnel.
                opts.mode = RunMode::SerialShell;
            }
            other => {
                return Err(Error::InvalidArgs(format!(
                    "unknown flag '{other}', try --help"
                )));
            }
        }
    }

    validate_serialsh_options(&opts)?;
    Ok(opts)
}

fn take_value(flag: &str, iter: &mut std::slice::Iter<String>) -> Result<String> {
    iter.next()
        .cloned()
        .ok_or_else(|| Error::InvalidArgs(format!("expected a value after {flag}")))
}

fn validate_serialsh_options(opts: &RunOptions) -> Result<()> {
    if matches!(opts.mode, RunMode::SerialShell) && (opts.payload_file.is_some() || opts.demo) {
        return Err(Error::InvalidArgs(
            "--serialsh cannot be combined with --demo or --payload-file".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults_with_no_args() {
        let args: Vec<String> = vec![];
        let cmd = Command::parse(&args).unwrap();
        assert_eq!(cmd, Command::Run(RunOptions::default()));
    }

    #[test]
    fn parse_run_with_overrides() {
        let args = vec![
            "--device".into(),
            "/dev/ttyUSB0".into(),
            "--baud".into(),
            "9600".into(),
            "--cols".into(),
            "16".into(),
            "--rows".into(),
            "2".into(),
            "--flow-control".into(),
            "hardware".into(),
            "--parity".into(),
            "even".into(),
            "--stop-bits".into(),
            "2".into(),
            "--dtr-on-open".into(),
            "on".into(),
            "--serial-timeout-ms".into(),
            "1500".into(),
            "--payload-file".into(),
            "/tmp/payload.json".into(),
            "--backoff-initial-ms".into(),
            "750".into(),
            "--backoff-max-ms".into(),
            "9000".into(),
            "--pcf8574-addr".into(),
            "0x23".into(),
            "--log-level".into(),
            "debug".into(),
            "--log-file".into(),
            "/tmp/lifelinetty.log".into(),
            "--demo".into(),
        ];
        let expected = RunOptions {
            mode: RunMode::Daemon,
            device: Some("/dev/ttyUSB0".into()),
            baud: Some(9600),
            flow_control: Some(FlowControlMode::Hardware),
            parity: Some(ParityMode::Even),
            stop_bits: Some(StopBitsMode::Two),
            dtr_on_open: Some(DtrBehavior::Assert),
            serial_timeout_ms: Some(1500),
            cols: Some(16),
            rows: Some(2),
            payload_file: Some("/tmp/payload.json".into()),
            backoff_initial_ms: Some(750),
            backoff_max_ms: Some(9000),
            pcf8574_addr: Some(Pcf8574Addr::Addr(0x23)),
            log_level: Some("debug".into()),
            log_file: Some("/tmp/lifelinetty.log".into()),
            compression_enabled: None,
            compression_codec: None,
            polling_enabled: None,
            poll_interval_ms: None,
            demo: true,
        };
        let cmd = Command::parse(&args).unwrap();
        assert_eq!(cmd, Command::Run(expected));
    }

    #[test]
    fn parse_run_allows_implicit_subcommand() {
        let args = vec![
            "--device".into(),
            "/dev/ttyS1".into(),
            "--payload-file".into(),
            "/tmp/payload.json".into(),
        ];
        let expected = RunOptions {
            mode: RunMode::Daemon,
            device: Some("/dev/ttyS1".into()),
            baud: None,
            flow_control: None,
            parity: None,
            stop_bits: None,
            dtr_on_open: None,
            serial_timeout_ms: None,
            cols: None,
            rows: None,
            payload_file: Some("/tmp/payload.json".into()),
            backoff_initial_ms: None,
            backoff_max_ms: None,
            pcf8574_addr: None,
            log_level: None,
            log_file: None,
            compression_enabled: None,
            compression_codec: None,
            polling_enabled: None,
            poll_interval_ms: None,
            demo: false,
        };
        let cmd = Command::parse(&args).unwrap();
        assert_eq!(cmd, Command::Run(expected));
    }

    #[test]
    fn parse_compression_flags() {
        let args = vec!["--compressed".into(), "--codec".into(), "zstd".into()];
        let expected = RunOptions {
            compression_enabled: Some(true),
            compression_codec: Some(CompressionCodec::Zstd),
            ..Default::default()
        };
        let cmd = Command::parse(&args).unwrap();
        assert_eq!(cmd, Command::Run(expected));
    }

    #[test]
    fn parse_no_compression_flag() {
        let args = vec!["--no-compressed".into()];
        let expected = RunOptions {
            compression_enabled: Some(false),
            ..Default::default()
        };
        let cmd = Command::parse(&args).unwrap();
        assert_eq!(cmd, Command::Run(expected));
    }

    #[test]
    fn parse_polling_flags() {
        let args = vec![
            "--polling".into(),
            "--poll-interval-ms".into(),
            "3000".into(),
        ];
        let expected = RunOptions {
            polling_enabled: Some(true),
            poll_interval_ms: Some(3000),
            ..Default::default()
        };
        let cmd = Command::parse(&args).unwrap();
        assert_eq!(cmd, Command::Run(expected));
    }

    #[test]
    fn parse_polling_disable_flag() {
        let args = vec!["--no-polling".into()];
        let expected = RunOptions {
            polling_enabled: Some(false),
            ..Default::default()
        };
        let cmd = Command::parse(&args).unwrap();
        assert_eq!(cmd, Command::Run(expected));
    }

    #[test]
    fn parse_help() {
        let args = vec!["--help".into()];
        let cmd = Command::parse(&args).unwrap();
        assert_eq!(cmd, Command::ShowHelp);
    }

    #[test]
    fn parse_rejects_unknown_flag() {
        let args = vec!["--nope".into()];
        let err = Command::parse(&args).unwrap_err();
        assert!(format!("{err}").contains("unknown flag"));
    }

    #[test]
    fn parse_serialsh_flag_sets_mode() {
        let args = vec!["--serialsh".into(), "--device".into(), "fake".into()];
        let cmd = Command::parse(&args).unwrap();
        match cmd {
            Command::Run(opts) => assert!(matches!(opts.mode, RunMode::SerialShell)),
            other => panic!("expected Run variant, got {other:?}"),
        }
    }

    #[test]
    fn serialsh_disallows_demo_and_payload_file() {
        let args = vec!["--serialsh".into(), "--demo".into()];
        let err = Command::parse(&args).unwrap_err();
        assert!(format!("{err}").contains("serialsh"));

        let args = vec![
            "--serialsh".into(),
            "--payload-file".into(),
            "payload.json".into(),
        ];
        let err = Command::parse(&args).unwrap_err();
        assert!(format!("{err}").contains("serialsh"));
    }
}
