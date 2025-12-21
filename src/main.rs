use lifelinetty::app::serial_shell;
use lifelinetty::{
    app::App,
    cli::{Command, RunMode, RunOptions},
    Result,
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
        Ok(Command::Run(opts)) => {
            let opts = *opts;
            match opts.mode {
                RunMode::Daemon => {
                    let app = App::from_options(opts)?;
                    app.run()
                }
                RunMode::SerialShell => run_serial_shell(opts),
            }
        }
        Err(err) => {
            Command::print_help();
            Err(err)
        }
    }
}

fn run_serial_shell(opts: RunOptions) -> Result<()> {
    let exit_code = serial_shell::run_serial_shell(opts)?;
    std::process::exit(exit_code);
}
