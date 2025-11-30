use crate::{Error, Result};

/// Options for the `run` command; values are `None` when not provided on CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunOptions {
    pub device: Option<String>,
    pub baud: Option<u32>,
    pub cols: Option<u8>,
    pub rows: Option<u8>,
    pub payload_file: Option<String>,
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
            "seriallcd - Serial-to-LCD daemon\n",
            "\n",
            "USAGE:\n",
            "  seriallcd run [--device <path>] [--baud <number>] [--cols <number>] [--rows <number>] [--payload-file <path>]\n",
            "  seriallcd --help\n",
            "  seriallcd --version\n",
            "\n",
            "OPTIONS:\n",
            "  --device <path>   Serial device path (default: /dev/ttyAMA0)\n",
            "  --baud <number>   Baud rate (default: 115200)\n",
            "  --cols <number>   LCD columns (default: 20)\n",
            "  --rows <number>   LCD rows (default: 4)\n",
            "  --payload-file <path>  Load a local JSON payload and render it once (testing helper)\n",
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
                opts.baud = Some(
                    raw.parse()
                        .map_err(|_| Error::InvalidArgs("baud must be a positive integer".to_string()))?,
                );
            }
            "--cols" => {
                let raw = take_value(flag, iter)?;
                opts.cols = Some(
                    raw.parse()
                        .map_err(|_| Error::InvalidArgs("cols must be a positive integer".to_string()))?,
                );
            }
            "--rows" => {
                let raw = take_value(flag, iter)?;
                opts.rows = Some(
                    raw.parse()
                        .map_err(|_| Error::InvalidArgs("rows must be a positive integer".to_string()))?,
                );
            }
            "--payload-file" => {
                opts.payload_file = Some(take_value(flag, iter)?);
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
        ];
        let expected = RunOptions {
            device: Some("/dev/ttyUSB0".into()),
            baud: Some(9600),
            cols: Some(16),
            rows: Some(2),
            payload_file: Some("/tmp/payload.json".into()),
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
