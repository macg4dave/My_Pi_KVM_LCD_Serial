use lifelinetty::{
    app::App,
    cli::{Command, RunMode, RunOptions},
    Error, Result,
};

fn main() {
    if let Err(err) = try_main() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match Command::parse(&args) {
        Ok(Command::ShowHelp) => {
            Command::print_help();
            Ok(())
        }
        Ok(Command::ShowVersion) => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Ok(Command::Run(opts)) => match opts.mode {
            RunMode::Daemon => {
                let app = App::from_options(opts)?;
                app.run()
            }
            RunMode::SerialShell => run_serial_shell(opts),
        },
        Err(err) => {
            Command::print_help();
            Err(err)
        }
    }
}

#[cfg(feature = "serialsh")]
fn run_serial_shell(opts: RunOptions) -> Result<()> {
    // P7: placeholder wiring until Milestone A exposes the command tunnel.
    Err(Error::InvalidArgs(format!(
        "--serialsh is gated behind milestone A; received options: {:?}",
        opts
    )))
}

#[cfg(not(feature = "serialsh"))]
fn run_serial_shell(_opts: RunOptions) -> Result<()> {
    Err(Error::InvalidArgs(
        "--serialsh requires building with the 'serialsh' feature".into(),
    ))
}
