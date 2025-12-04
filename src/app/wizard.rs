use crate::{
    cli::RunOptions,
    config::{loader, Config, DEFAULT_DEVICE, MAX_COLS, MAX_ROWS, MIN_BAUD, MIN_COLS, MIN_ROWS},
    lcd::Lcd,
    negotiation::RolePreference,
    serial::{SerialOptions, SerialPort},
    Result, CACHE_DIR,
};
use humantime::format_rfc3339;
use std::{
    fs::{self, OpenOptions},
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Trigger the guided wizard on first run or when explicitly requested.
pub fn maybe_run(opts: &RunOptions) -> Result<()> {
    let config_path = loader::default_config_path()?;
    let config_exists = config_path.exists();
    let forced_env = std::env::var_os("LIFELINETTY_FORCE_WIZARD").is_some();
    let should_run = opts.wizard || forced_env || !config_exists;
    if !should_run {
        return Ok(());
    }

    let existing_cfg = if config_exists {
        Some(Config::load_from_path(&config_path)?)
    } else {
        None
    };

    let prompt_input = determine_prompt_input();
    if let PromptInput::AutoDefaults { reason } = &prompt_input {
        eprintln!(
            "lifelinetty wizard: {reason}. Defaults recorded; run `lifelinetty --wizard` in an interactive shell to customize."
        );
    }

    let defaults = existing_cfg.unwrap_or_default();
    let mut wizard = FirstRunWizard::new(config_path, defaults)?;
    wizard.run(prompt_input)
}

fn determine_prompt_input() -> PromptInput {
    if let Ok(script_path) = std::env::var("LIFELINETTY_WIZARD_SCRIPT") {
        let path = Path::new(&script_path);
        match fs::read_to_string(path) {
            Ok(contents) => {
                let lines = contents
                    .lines()
                    .map(|line| line.trim().to_string())
                    .collect::<Vec<_>>();
                return PromptInput::Scripted { lines, cursor: 0 };
            }
            Err(err) => {
                eprintln!(
                    "lifelinetty wizard: failed to read script at {}: {err}",
                    path.display()
                );
            }
        }
    }

    if cfg!(test) {
        return PromptInput::AutoDefaults {
            reason: "test mode auto-default".to_string(),
        };
    }

    if io::stdin().is_terminal() {
        PromptInput::Interactive
    } else {
        PromptInput::AutoDefaults {
            reason: "stdin is not interactive".to_string(),
        }
    }
}

struct FirstRunWizard {
    config_path: PathBuf,
    defaults: Config,
    summary: WizardSummary,
}

impl FirstRunWizard {
    fn new(config_path: PathBuf, defaults: Config) -> Result<Self> {
        Ok(Self {
            config_path,
            defaults,
            summary: WizardSummary::new(),
        })
    }

    fn run(&mut self, input: PromptInput) -> Result<()> {
        let mut prompter = WizardPrompter::new(input);
        let lcd_present = prompt_lcd_presence(&mut prompter)?;
        let mut display = WizardDisplay::new(&self.defaults, lcd_present);
        display.banner("First-run wizard", "Check console");

        let candidate_devices = self.device_candidates();
        let answers =
            self.collect_answers(&candidate_devices, &mut prompter, &mut display, lcd_present)?;
        self.save_config(&answers)?;

        let probes = run_probes(&answers.device, answers.baud);
        let mode_label = prompter.mode_label();
        let mode_note = prompter.mode_note().map(|s| s.to_string());
        self.summary.record(WizardSummaryEntry::new(
            mode_label, mode_note, &answers, &probes,
        ));

        display.banner("Wizard complete", "Config saved");
        Ok(())
    }

    fn save_config(&self, answers: &WizardAnswers) -> Result<()> {
        let mut cfg = self.defaults.clone();
        cfg.device = answers.device.clone();
        cfg.baud = answers.baud;
        cfg.cols = answers.cols;
        cfg.rows = answers.rows;
        cfg.lcd_present = answers.lcd_present;
        cfg.negotiation.preference = answers.preference;
        cfg.save_to_path(&self.config_path)
    }

    fn collect_answers(
        &self,
        candidates: &[String],
        prompter: &mut WizardPrompter,
        display: &mut WizardDisplay,
        lcd_present: bool,
    ) -> Result<WizardAnswers> {
        println!("\n=== LifelineTTY first-run wizard ===");
        println!("Answer the following prompts to finish setup. Press enter to accept the suggested value.");

        let default_device = candidates
            .first()
            .cloned()
            .unwrap_or_else(|| DEFAULT_DEVICE.to_string());
        println!("\nDetected serial devices:");
        if candidates.is_empty() {
            println!("  (none discovered under /dev; enter a full path manually)");
        } else {
            for (idx, dev) in candidates.iter().enumerate() {
                println!("  [{}] {}", idx + 1, dev);
            }
        }
        display.banner("Select device", &default_device);
        let device = prompt_device(prompter, candidates, &default_device)?;

        let default_baud = self.defaults.baud.max(MIN_BAUD);
        display.banner("Target baud", &format!("{} bps", default_baud));
        let baud = prompt_baud(prompter, default_baud)?;

        let (cols, rows) = if lcd_present {
            display.banner("LCD columns", &format!("{} cols", self.defaults.cols));
            let cols = prompt_dimension(
                prompter,
                "LCD columns",
                self.defaults.cols,
                MIN_COLS,
                MAX_COLS,
            )?;
            display.banner("LCD rows", &format!("{} rows", self.defaults.rows));
            let rows =
                prompt_dimension(prompter, "LCD rows", self.defaults.rows, MIN_ROWS, MAX_ROWS)?;
            (cols, rows)
        } else {
            (self.defaults.cols, 2)
        };

        display.banner("Role preference", "server/client/auto");
        let preference = prompt_role(prompter, self.defaults.negotiation.preference)?;

        Ok(WizardAnswers {
            device,
            baud,
            cols,
            rows,
            preference,
            lcd_present,
        })
    }

    fn device_candidates(&self) -> Vec<String> {
        let mut devices = Vec::new();
        append_unique(&mut devices, self.defaults.device.clone());
        for dev in enumerate_serial_devices() {
            append_unique(&mut devices, dev);
        }
        append_unique(&mut devices, DEFAULT_DEVICE.to_string());
        devices.retain(|d| !d.is_empty());
        devices
    }
}

fn append_unique(list: &mut Vec<String>, candidate: String) {
    if candidate.is_empty() {
        return;
    }
    if !list.iter().any(|existing| existing == &candidate) {
        list.push(candidate);
    }
}

#[derive(Clone)]
struct WizardAnswers {
    device: String,
    baud: u32,
    cols: u8,
    rows: u8,
    preference: RolePreference,
    lcd_present: bool,
}

fn run_probes(device: &str, target_baud: u32) -> Vec<ProbeResult> {
    let mut rates = vec![MIN_BAUD];
    if target_baud != MIN_BAUD {
        rates.push(target_baud);
    }
    let mut results = Vec::new();
    for rate in rates {
        let mut opts = SerialOptions::default();
        opts.baud = rate;
        let result = match SerialPort::connect(device, opts) {
            Ok(_) => ProbeResult {
                baud: rate,
                success: true,
                message: "port opened successfully".to_string(),
            },
            Err(err) => ProbeResult {
                baud: rate,
                success: false,
                message: err.to_string(),
            },
        };
        results.push(result);
    }
    results
}

#[derive(Clone)]
struct ProbeResult {
    baud: u32,
    success: bool,
    message: String,
}

struct WizardSummary {
    path: PathBuf,
}

impl WizardSummary {
    fn new() -> Self {
        let path = Path::new(CACHE_DIR).join("wizard").join("summary.log");
        Self { path }
    }

    fn record(&self, entry: WizardSummaryEntry) {
        if let Err(err) = self.try_record(&entry) {
            eprintln!(
                "lifelinetty wizard: failed to write summary at {}: {err}",
                self.path.display()
            );
        }
    }

    fn try_record(&self, entry: &WizardSummaryEntry) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "timestamp: {}", format_rfc3339(entry.timestamp))?;
        writeln!(file, "mode: {}", entry.mode_label)?;
        if let Some(note) = entry.mode_note.as_deref() {
            writeln!(file, "mode_note: {note}")?;
        }
        writeln!(file, "device: {}", entry.answers.device)?;
        writeln!(file, "baud: {}", entry.answers.baud)?;
        writeln!(file, "cols: {}", entry.answers.cols)?;
        writeln!(file, "rows: {}", entry.answers.rows)?;
        writeln!(file, "lcd_present: {}", entry.answers.lcd_present)?;
        writeln!(file, "preference: {}", entry.answers.preference.as_str())?;
        for probe in &entry.probes {
            let status = if probe.success { "ok" } else { "error" };
            writeln!(
                file,
                "probe baud {}: {} ({})",
                probe.baud, status, probe.message
            )?;
        }
        writeln!(file)?;
        Ok(())
    }
}

struct WizardSummaryEntry {
    timestamp: SystemTime,
    mode_label: &'static str,
    mode_note: Option<String>,
    answers: WizardAnswers,
    probes: Vec<ProbeResult>,
}

impl WizardSummaryEntry {
    fn new(
        mode_label: &'static str,
        mode_note: Option<String>,
        answers: &WizardAnswers,
        probes: &[ProbeResult],
    ) -> Self {
        Self {
            timestamp: SystemTime::now(),
            mode_label,
            mode_note,
            answers: answers.clone(),
            probes: probes.to_vec(),
        }
    }
}

struct WizardDisplay {
    lcd: Option<Lcd>,
}

impl WizardDisplay {
    fn new(defaults: &Config, attempt_lcd: bool) -> Self {
        let lcd = if attempt_lcd {
            Lcd::new(
                defaults.cols,
                defaults.rows,
                defaults.pcf8574_addr.clone(),
                defaults.display_driver,
            )
            .map_err(|err| {
                eprintln!("lifelinetty wizard: LCD unavailable ({err})");
                err
            })
            .ok()
        } else {
            None
        };
        Self { lcd }
    }

    fn banner(&mut self, line1: &str, line2: &str) {
        if let Some(lcd) = self.lcd.as_mut() {
            let cols = lcd.cols() as usize;
            let fit_line = |line: &str| -> String { line.chars().take(cols).collect() };
            let _ = lcd.write_line(0, &fit_line(line1));
            if lcd.rows() > 1 {
                let _ = lcd.write_line(1, &fit_line(line2));
            }
        }
    }
}

enum PromptInput {
    Interactive,
    Scripted { lines: Vec<String>, cursor: usize },
    AutoDefaults { reason: String },
}

impl PromptInput {
    fn label(&self) -> &'static str {
        match self {
            PromptInput::Interactive => "interactive",
            PromptInput::Scripted { .. } => "scripted",
            PromptInput::AutoDefaults { .. } => "auto_defaults",
        }
    }

    fn note(&self) -> Option<&str> {
        match self {
            PromptInput::AutoDefaults { reason } => Some(reason.as_str()),
            _ => None,
        }
    }
}

struct WizardPrompter {
    input: PromptInput,
}

impl WizardPrompter {
    fn new(input: PromptInput) -> Self {
        Self { input }
    }

    fn mode_label(&self) -> &'static str {
        self.input.label()
    }

    fn mode_note(&self) -> Option<&str> {
        self.input.note()
    }

    fn prompt(&mut self, question: &str, default: &str) -> Result<String> {
        match &mut self.input {
            PromptInput::Interactive => {
                print!("{question}");
                if !default.is_empty() {
                    print!(" [{default}]");
                }
                print!(" > ");
                io::stdout().flush()?;
                let mut buf = String::new();
                io::stdin().read_line(&mut buf)?;
                let trimmed = buf.trim();
                if trimmed.is_empty() {
                    Ok(default.to_string())
                } else {
                    Ok(trimmed.to_string())
                }
            }
            PromptInput::Scripted { lines, cursor } => {
                if *cursor >= lines.len() {
                    return Ok(default.to_string());
                }
                let value = lines[*cursor].clone();
                *cursor += 1;
                if value.trim().is_empty() {
                    Ok(default.to_string())
                } else {
                    Ok(value.trim().to_string())
                }
            }
            PromptInput::AutoDefaults { .. } => Ok(default.to_string()),
        }
    }
}

fn prompt_device(
    prompter: &mut WizardPrompter,
    candidates: &[String],
    default: &str,
) -> Result<String> {
    loop {
        let response = prompter.prompt("Serial device path or index", default)?;
        let trimmed = response.trim();
        if trimmed.is_empty() {
            return Ok(default.to_string());
        }
        if let Ok(idx) = trimmed.parse::<usize>() {
            if idx >= 1 && idx <= candidates.len() {
                return Ok(candidates[idx - 1].clone());
            }
        }
        if trimmed.starts_with("/dev/") {
            return Ok(trimmed.to_string());
        }
        eprintln!(
            "Input '{trimmed}' was not a /dev path or a device index; enter a full path (e.g., /dev/ttyUSB0) or one of the listed numbers."
        );
    }
}

fn prompt_lcd_presence(prompter: &mut WizardPrompter) -> Result<bool> {
    loop {
        let response = prompter.prompt("Is an LCD connected (y/n)", "y")?;
        match response.trim().to_ascii_lowercase().as_str() {
            "" | "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            other => eprintln!("'{other}' is not a yes or no answer. Try y/n."),
        }
    }
}

fn prompt_baud(prompter: &mut WizardPrompter, default: u32) -> Result<u32> {
    loop {
        let response = prompter.prompt(
            &format!("Target baud rate (>= {MIN_BAUD})"),
            &default.to_string(),
        )?;
        match response.trim().parse::<u32>() {
            Ok(value) if value >= MIN_BAUD => return Ok(value),
            _ => eprintln!("Enter a baud rate of at least {MIN_BAUD} (e.g., 9600, 19200, 115200)."),
        }
    }
}

fn prompt_dimension(
    prompter: &mut WizardPrompter,
    label: &str,
    default: u8,
    min_value: u8,
    max_value: u8,
) -> Result<u8> {
    loop {
        let response = prompter.prompt(
            &format!("{label} (between {min_value} and {max_value})"),
            &default.to_string(),
        )?;
        match response.trim().parse::<u8>() {
            Ok(value) if (min_value..=max_value).contains(&value) => return Ok(value),
            _ => eprintln!("Enter a value between {min_value} and {max_value}."),
        }
    }
}

fn prompt_role(prompter: &mut WizardPrompter, default: RolePreference) -> Result<RolePreference> {
    let default_label = match default {
        RolePreference::PreferServer => "server",
        RolePreference::PreferClient => "client",
        RolePreference::NoPreference => "auto",
    };
    loop {
        let response = prompter.prompt("Preferred role (server/client/auto)", default_label)?;
        match response.trim().to_ascii_lowercase().as_str() {
            "server" => return Ok(RolePreference::PreferServer),
            "client" => return Ok(RolePreference::PreferClient),
            "auto" | "none" => return Ok(RolePreference::NoPreference),
            other => eprintln!("Unknown role '{other}', choose server, client, or auto."),
        }
    }
}

fn enumerate_serial_devices() -> Vec<String> {
    let mut devices = Vec::new();
    if let Ok(entries) = fs::read_dir("/dev") {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("ttyUSB")
                    || name.starts_with("ttyACM")
                    || name.starts_with("ttyAMA")
                    || name.starts_with("ttyS")
                {
                    devices.push(format!("/dev/{name}"));
                }
            }
        }
    }
    devices.sort();
    devices
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn scripted_input(lines: &[&str]) -> PromptInput {
        PromptInput::Scripted {
            lines: lines.iter().map(|s| s.to_string()).collect(),
            cursor: 0,
        }
    }

    #[test]
    fn scripted_wizard_persists_answers() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let defaults = Config::default();
        let mut wizard = FirstRunWizard::new(config_path.clone(), defaults).unwrap();
        let answers = ["y", "/dev/ttyS42", "19200", "16", "2", "client"];
        wizard
            .run(scripted_input(&answers))
            .expect("wizard run failed");
        let cfg = Config::load_from_path(&config_path).expect("config missing");
        assert_eq!(cfg.device, "/dev/ttyS42");
        assert_eq!(cfg.baud, 19_200);
        assert_eq!(cfg.cols, 16);
        assert_eq!(cfg.rows, 2);
        assert_eq!(cfg.negotiation.preference, RolePreference::PreferClient);
    }
}
