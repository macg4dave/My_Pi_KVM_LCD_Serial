use crate::{config::Pcf8574Addr, Error, Result};

/// Options for the `run` command; values are `None` when not provided on CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunOptions {
    pub device: Option<String>,
    pub baud: Option<u32>,
    pub cols: Option<u8>,
    pub rows: Option<u8>,
    pub payload_file: Option<String>,
    pub backoff_initial_ms: Option<u64>,
    pub backoff_max_ms: Option<u64>,
    pub pcf8574_addr: Option<Pcf8574Addr>,
    pub log_level: Option<String>,
    pub log_file: Option<String>,
    pub demo: bool,
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

    pub fn help() -> &'static str {
        concat!(
            "lifelinetty - Serial-to-LCD daemon\n",
            "\n",
            "USAGE:\n",
            "  lifelinetty run [--device <path>] [--baud <number>] [--cols <number>] [--rows <number>] [--payload-file <path>]\n",
            "  lifelinetty --help\n",
            "  lifelinetty --version\n",
            "\n",
            "OPTIONS:\n",
            "  --device <path>   Serial device path (default: /dev/ttyUSB0)\n",
            "  --baud <number>   Baud rate (default: 9600)\n",
            "  --cols <number>   LCD columns (default: 20)\n",
            "  --rows <number>   LCD rows (default: 4)\n",
            "  --payload-file <path>  Load a local JSON payload and render it once (testing helper)\n",
            "  --backoff-initial-ms <number>  Initial reconnect backoff (default: 500)\n",
            "  --backoff-max-ms <number>      Maximum reconnect backoff (default: 10000)\n",
            "  --pcf8574-addr <auto|0xNN>     PCF8574 I2C address or 'auto' to probe (default: auto)\n",
            "  --log-level <error|warn|info|debug|trace>  Log verbosity (default: info)\n",
            "  --log-file <path>              Append logs inside /run/serial_lcd_cache (also honors LIFELINETTY_LOG_PATH)\n",
            "  --demo                         Run built-in demo pages on the LCD (no serial input)\n",
            "  -h, --help        Show this help\n",
            "  -V, --version     Show version\n",
        )
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
            "--demo" => {
                opts.demo = true;
            }
            other => {
                return Err(Error::InvalidArgs(format!(
                    "unknown flag '{other}', try --help"
                )));
            }
        }
    }

    Ok(opts)
}

fn take_value(flag: &str, iter: &mut std::slice::Iter<String>) -> Result<String> {
    iter.next()
        .cloned()
        .ok_or_else(|| Error::InvalidArgs(format!("expected a value after {flag}")))
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
            device: Some("/dev/ttyUSB0".into()),
            baud: Some(9600),
            cols: Some(16),
            rows: Some(2),
            payload_file: Some("/tmp/payload.json".into()),
            backoff_initial_ms: Some(750),
            backoff_max_ms: Some(9000),
            pcf8574_addr: Some(Pcf8574Addr::Addr(0x23)),
            log_level: Some("debug".into()),
            log_file: Some("/tmp/lifelinetty.log".into()),
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
            device: Some("/dev/ttyS1".into()),
            baud: None,
            cols: None,
            rows: None,
            payload_file: Some("/tmp/payload.json".into()),
            backoff_initial_ms: None,
            backoff_max_ms: None,
            pcf8574_addr: None,
            log_level: None,
            log_file: None,
            demo: false,
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
}
