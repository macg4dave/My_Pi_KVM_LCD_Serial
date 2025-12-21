use crate::{
    cli::RunOptions,
    config::{loader, Config, DEFAULT_DEVICE, MAX_COLS, MAX_ROWS, MIN_BAUD, MIN_COLS, MIN_ROWS},
    lcd::Lcd,
    negotiation::RolePreference,
    payload::{decode_tunnel_frame, encode_tunnel_msg, TunnelMsgOwned},
    serial::{SerialOptions, SerialPort},
    Result, CACHE_DIR,
};
use humantime::format_rfc3339;
use serde_json;
use std::{
    fs::{self, OpenOptions},
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    thread,
    time::Duration,
    time::SystemTime,
};

/// Trigger the guided wizard on first run or when explicitly requested.
pub fn maybe_run(opts: &RunOptions) -> Result<()> {
    let forced_env = std::env::var_os("LIFELINETTY_FORCE_WIZARD").is_some();
    if opts.config_file.is_some() && !opts.wizard && !forced_env {
        return Ok(());
    }

    let config_path = loader::default_config_path()?;
    let config_exists = config_path.exists();
    let (existing_cfg, requires_repair, repair_reason) = inspect_existing_config(&config_path);

    let should_run = opts.wizard || forced_env || !config_exists || requires_repair;
    if !should_run {
        return Ok(());
    }

    if let Some(reason) = repair_reason.as_deref() {
        eprintln!("lifelinetty wizard: {reason}");
    }

    let prompt_input = determine_prompt_input();
    if let PromptInput::AutoDefaults { reason } = &prompt_input {
        eprintln!(
            "lifelinetty wizard: {reason}. Defaults recorded; run `lifelinetty --wizard` in an interactive shell to customize."
        );
    }

    let has_existing_config = config_exists && !requires_repair && existing_cfg.is_some();
    let defaults = existing_cfg.unwrap_or_default();
    let mut wizard = FirstRunWizard::new(config_path, defaults, has_existing_config)?;
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
                    .filter(|line| !line.is_empty() && !line.starts_with('#'))
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

fn inspect_existing_config(path: &Path) -> (Option<Config>, bool, Option<String>) {
    if !path.exists() {
        return (None, false, None);
    }

    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) => {
            return (
                None,
                true,
                Some(format!(
                    "failed to read existing config at {}: {err}; running wizard to repair",
                    path.display()
                )),
            );
        }
    };

    if raw.trim().is_empty() {
        return (
            None,
            true,
            Some(format!(
                "existing config at {} is empty; running wizard to initialize",
                path.display()
            )),
        );
    }

    let parsed = match loader::parse(&raw) {
        Ok(cfg) => cfg,
        Err(err) => {
            return (
                None,
                true,
                Some(format!(
                    "existing config at {} is invalid ({err}); running wizard to repair",
                    path.display()
                )),
            );
        }
    };

    if let Err(err) = crate::config::validate(&parsed) {
        return (
            None,
            true,
            Some(format!(
                "existing config at {} is invalid ({err}); running wizard to repair",
                path.display()
            )),
        );
    }

    (Some(parsed), false, None)
}

struct FirstRunWizard {
    config_path: PathBuf,
    defaults: Config,
    has_existing_config: bool,
    summary: WizardSummary,
    transcript: WizardTranscript,
}

impl FirstRunWizard {
    fn new(config_path: PathBuf, defaults: Config, has_existing_config: bool) -> Result<Self> {
        Ok(Self {
            config_path,
            defaults,
            summary: WizardSummary::new(),
            has_existing_config,
            transcript: WizardTranscript::new(),
        })
    }

    fn run(&mut self, input: PromptInput) -> Result<()> {
        let mut prompter = WizardPrompter::new(input);

        let default_intent =
            UsageIntent::from_role_preference(self.defaults.negotiation.preference);
        let intent = prompt_usage_intent(&mut prompter, default_intent)?;
        let lcd_present = prompt_lcd_presence(&mut prompter, self.defaults.lcd_present)?;
        let mut display = WizardDisplay::new(&self.defaults, lcd_present);
        display.banner("First-run wizard", "Check console");

        let mut candidate_devices = self.device_candidates();
        let answers = self.collect_answers(
            &mut candidate_devices,
            &mut prompter,
            &mut display,
            intent,
            lcd_present,
        )?;

        println!("\n=== Review settings ===");
        println!("Config path: {}", self.config_path.display());
        println!("Device: {}", answers.device);
        println!("Baud: {}", answers.baud);
        println!("LCD present: {}", answers.lcd_present);
        println!(
            "LCD geometry: {} cols x {} rows",
            answers.cols, answers.rows
        );
        println!("Usage intent: {}", answers.intent.as_str());
        println!("Role preference: {}", answers.preference.as_str());
        println!("Probe serial: {}", answers.run_probe);
        println!("Link rehearsal: {}", answers.run_link_rehearsal);
        println!("Show helper snippets: {}", answers.show_helpers);

        let save_confirmed =
            prompt_yes_no(&mut prompter, "Write these settings to disk (y/n)", true)?;
        if !save_confirmed {
            return Err(crate::Error::InvalidArgs(
                "wizard aborted; config not saved".to_string(),
            ));
        }

        self.save_config(&answers)?;

        let probes = if answers.run_probe {
            run_probes(&answers.device, answers.baud)
        } else {
            Vec::new()
        };

        if answers.show_helpers {
            print_wizard_helper_snippets();
        }

        let mode_label = prompter.mode_label();
        let mode_note = prompter.mode_note().map(|s| s.to_string());
        self.summary.record(WizardSummaryEntry::new(
            mode_label, mode_note, &answers, &probes,
        ));

        self.transcript.record(WizardTranscriptEntry::new(
            mode_label,
            prompter.mode_note().map(|s| s.to_string()),
            prompter.take_transcript(),
            &answers,
            &candidate_devices,
            &probes,
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
        candidates: &mut Vec<String>,
        prompter: &mut WizardPrompter,
        display: &mut WizardDisplay,
        intent: UsageIntent,
        lcd_present: bool,
    ) -> Result<WizardAnswers> {
        println!("\n=== LifelineTTY first-run wizard ===");
        println!("Answer the following prompts to finish setup. Press enter to accept the suggested value.");

        println!("\nUsage intent: {}", intent.as_str());

        let device = loop {
            let default_device = candidates
                .first()
                .cloned()
                .unwrap_or_else(|| DEFAULT_DEVICE.to_string());
            println!("\nDetected serial devices (ranked):");
            if candidates.is_empty() {
                println!("  (none discovered under /dev; enter a full path manually)");
            } else {
                for (idx, dev) in candidates.iter().enumerate() {
                    println!("  [{}] {}", idx + 1, dev);
                }
            }
            println!("  (enter a number, a /dev path, or 'r' to rescan)");
            display.banner("Select device", &default_device);

            match prompt_device(prompter, candidates, &default_device)? {
                DeviceSelection::Selected(device) => break device,
                DeviceSelection::Rescan => {
                    *candidates = self.device_candidates();
                }
            }
        };

        let run_link_rehearsal =
            prompter.is_interactive() && !matches!(intent, UsageIntent::Standalone);

        let (baud, run_probe) = if run_link_rehearsal {
            let mut candidates = vec![MIN_BAUD, 19_200, 38_400, 57_600, 115_200];
            if self.defaults.baud > MIN_BAUD && !candidates.contains(&self.defaults.baud) {
                candidates.push(self.defaults.baud);
            }
            candidates.sort_unstable();
            candidates.dedup();

            display.banner("Rehearsal", "testing bauds");
            println!("\n=== Link-speed rehearsal ===");
            println!("Run this on both ends at the same time with the same device selected.");
            println!("Candidates: {:?}", candidates);
            let base_options = SerialOptions {
                baud: MIN_BAUD,
                timeout_ms: self.defaults.serial_timeout_ms,
                flow_control: self.defaults.flow_control,
                parity: self.defaults.parity,
                stop_bits: self.defaults.stop_bits,
                dtr: self.defaults.dtr_on_open,
            };
            let (chosen, attempts) = run_link_speed_rehearsal(
                &device,
                base_options,
                &self.defaults.negotiation,
                self.defaults.protocol.compression_enabled,
                &candidates,
            );
            println!("Results:");
            for attempt in &attempts {
                let status = if attempt.success { "ok" } else { "error" };
                println!("  - {status} baud {}: {}", attempt.baud, attempt.message);
            }
            println!("Selected baud: {chosen}");
            (chosen, false)
        } else {
            let default_baud = self.defaults.baud.max(MIN_BAUD);
            display.banner("Target baud", &format!("{default_baud} bps"));
            let baud = prompt_baud(prompter, default_baud)?;

            display.banner("Probe serial", "optional");
            let run_probe = prompt_yes_no(
                prompter,
                "Probe the selected device at 9600 and the target baud (y/n)",
                prompter.is_interactive(),
            )?;
            (baud, run_probe)
        };

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
        let preference = prompt_role(prompter, intent.to_role_preference())?;

        let show_helpers = prompt_yes_no(
            prompter,
            "Show helper snippets (ssh/scp/tmux) (y/n)",
            prompter.is_interactive(),
        )?;

        Ok(WizardAnswers {
            device,
            baud,
            cols,
            rows,
            preference,
            lcd_present,
            intent,
            run_probe,
            run_link_rehearsal,
            show_helpers,
        })
    }

    fn device_candidates(&self) -> Vec<String> {
        // Always present a *ranked* list to the user.
        //
        // If an existing config is present, keep that selection as the first
        // (most likely correct) default, but still show the remaining
        // candidates in ranked order.

        let mut ranked = Vec::new();
        for dev in enumerate_serial_devices_ranked() {
            append_unique(&mut ranked, dev);
        }
        append_unique(&mut ranked, self.defaults.device.clone());
        append_unique(&mut ranked, DEFAULT_DEVICE.to_string());
        ranked.retain(|d| !d.is_empty());
        rank_serial_devices(&mut ranked);

        if !self.has_existing_config {
            return ranked;
        }

        let mut devices = Vec::new();
        append_unique(&mut devices, self.defaults.device.clone());
        for dev in ranked {
            if dev != self.defaults.device {
                append_unique(&mut devices, dev);
            }
        }
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
    intent: UsageIntent,
    run_probe: bool,
    run_link_rehearsal: bool,
    show_helpers: bool,
}

fn run_probes(device: &str, target_baud: u32) -> Vec<ProbeResult> {
    run_probes_with_backoff(device, target_baud, 50, 500, 3)
}

#[derive(Clone)]
struct LinkRehearsalAttempt {
    baud: u32,
    success: bool,
    message: String,
}

struct LinkRehearsalLog {
    path: PathBuf,
}

impl LinkRehearsalLog {
    fn new() -> Self {
        let path = Path::new(CACHE_DIR)
            .join("wizard")
            .join("link_rehearsal.log");
        Self { path }
    }

    fn record(&self, message: impl AsRef<str>) {
        if let Err(err) = self.try_record(message.as_ref()) {
            eprintln!(
                "lifelinetty wizard: failed to write link rehearsal log at {}: {err}",
                self.path.display()
            );
        }
    }

    fn try_record(&self, message: &str) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{message}")?;
        Ok(())
    }
}

fn run_link_speed_rehearsal(
    device: &str,
    base_options: SerialOptions,
    negotiation: &crate::config::NegotiationConfig,
    compression_enabled: bool,
    candidates: &[u32],
) -> (u32, Vec<LinkRehearsalAttempt>) {
    run_link_speed_rehearsal_with(
        device,
        base_options,
        negotiation,
        compression_enabled,
        candidates,
        SerialPort::connect,
    )
}

fn run_link_speed_rehearsal_with<IO, Connect>(
    device: &str,
    mut base_options: SerialOptions,
    negotiation: &crate::config::NegotiationConfig,
    compression_enabled: bool,
    candidates: &[u32],
    mut connect: Connect,
) -> (u32, Vec<LinkRehearsalAttempt>)
where
    IO: crate::serial::LineIo,
    Connect: FnMut(&str, SerialOptions) -> Result<IO>,
{
    let log = LinkRehearsalLog::new();
    log.record(format!(
        "=== link-speed rehearsal start device={device} candidates={:?} ===",
        candidates
    ));

    let mut attempts = Vec::new();
    let mut best_baud: Option<u32> = None;
    let bounded = candidates
        .iter()
        .copied()
        .filter(|b| *b >= MIN_BAUD)
        .take(8);

    for baud in bounded {
        base_options.baud = baud;
        let attempt_label = format!("baud={baud}");
        let mut success = false;
        let mut last_message = String::new();

        for retry in 0..3u8 {
            if retry != 0 && !cfg!(test) {
                thread::sleep(Duration::from_millis(150 * retry as u64));
            }

            let mut port = match connect(device, base_options) {
                Ok(port) => port,
                Err(err) => {
                    last_message = format!("connect failed: {err}");
                    continue;
                }
            };

            if let Err(err) = port.send_command_line("INIT") {
                last_message = format!("INIT send failed: {err}");
                continue;
            }

            match rehearsal_handshake(&mut port, negotiation, compression_enabled) {
                Ok(()) => {}
                Err(err) => {
                    last_message = format!("handshake failed: {err}");
                    continue;
                }
            }

            match rehearsal_crc_roundtrip(&mut port) {
                Ok(()) => {
                    success = true;
                    last_message = "ok".to_string();
                    break;
                }
                Err(err) => {
                    last_message = format!("crc probe failed: {err}");
                }
            }
        }

        log.record(format!("{attempt_label} success={success} {last_message}"));
        attempts.push(LinkRehearsalAttempt {
            baud,
            success,
            message: last_message.clone(),
        });

        if success {
            best_baud = Some(baud);
            if !cfg!(test) {
                thread::sleep(Duration::from_millis(250));
            }
        } else {
            break;
        }
    }

    let chosen = best_baud.unwrap_or(MIN_BAUD);
    log.record(format!("chosen_baud={chosen}"));
    log.record("=== link-speed rehearsal end ===");
    (chosen, attempts)
}

fn rehearsal_handshake<IO: crate::serial::LineIo>(
    io: &mut IO,
    negotiation: &crate::config::NegotiationConfig,
    compression_enabled: bool,
) -> Result<()> {
    let negotiator = crate::app::negotiation::Negotiator::new(negotiation, compression_enabled);
    let hello_frame = negotiator.hello_frame();
    let hello_payload = serde_json::to_string(&hello_frame)
        .map_err(|e| crate::Error::Parse(format!("json: {e}")))?;
    io.send_command_line(&hello_payload)?;

    let deadline = std::time::Instant::now() + Duration::from_millis(negotiation.timeout_ms);
    let mut buffer = String::new();
    while std::time::Instant::now() < deadline {
        let read = io.read_message_line(&mut buffer)?;
        if read == 0 {
            continue;
        }
        let trimmed = buffer.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<crate::negotiation::ControlFrame>(trimmed) {
            Ok(crate::negotiation::ControlFrame::Hello {
                node_id,
                caps,
                pref,
                ..
            }) => {
                let (remote, _) =
                    crate::app::negotiation::RemoteHello::from_parts(node_id, &pref, caps.bits);
                let decision = negotiator.decide_roles(&remote);
                let ack = crate::negotiation::ControlFrame::HelloAck {
                    chosen_role: decision.remote_role.as_str().to_string(),
                    peer_caps: crate::negotiation::ControlCaps {
                        bits: negotiator.local_caps().bits(),
                    },
                };
                let ack_payload = serde_json::to_string(&ack)
                    .map_err(|e| crate::Error::Parse(format!("json: {e}")))?;
                io.send_command_line(&ack_payload)?;
                continue;
            }
            Ok(crate::negotiation::ControlFrame::HelloAck { .. }) => return Ok(()),
            Ok(crate::negotiation::ControlFrame::LegacyFallback) => {
                return Err(crate::Error::Parse("peer requested legacy fallback".into()))
            }
            Err(_) => {
                return Err(crate::Error::Parse(
                    "unexpected non-control frame during rehearsal handshake".into(),
                ))
            }
        }
    }

    Err(crate::Error::Parse("handshake timed out".into()))
}

fn rehearsal_crc_roundtrip<IO: crate::serial::LineIo>(io: &mut IO) -> Result<()> {
    let frame = encode_tunnel_msg(&TunnelMsgOwned::Heartbeat)?;
    io.send_command_line(&frame)?;

    let mut buf = String::new();
    let deadline = std::time::Instant::now() + Duration::from_millis(600);
    while std::time::Instant::now() < deadline {
        let read = io.read_message_line(&mut buf)?;
        if read == 0 {
            continue;
        }
        let trimmed = buf.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg = decode_tunnel_frame(trimmed)?;
        if matches!(msg, TunnelMsgOwned::Heartbeat) {
            return Ok(());
        }
    }

    Err(crate::Error::Parse(
        "timed out waiting for heartbeat echo".into(),
    ))
}

fn run_probes_with_backoff(
    device: &str,
    target_baud: u32,
    backoff_initial_ms: u64,
    backoff_max_ms: u64,
    attempts: u8,
) -> Vec<ProbeResult> {
    let mut rates = vec![MIN_BAUD];
    if target_baud != MIN_BAUD {
        rates.push(target_baud);
    }
    rates
        .into_iter()
        .map(|rate| probe_with_backoff(device, rate, backoff_initial_ms, backoff_max_ms, attempts))
        .collect()
}

fn probe_with_backoff(
    device: &str,
    baud: u32,
    backoff_initial_ms: u64,
    backoff_max_ms: u64,
    attempts: u8,
) -> ProbeResult {
    let mut attempts_taken = 0u8;
    let mut last_err: Option<String> = None;
    let mut delay_ms = 0u64;

    let max_attempts = attempts.max(1);
    for _ in 0..max_attempts {
        attempts_taken = attempts_taken.saturating_add(1);
        if delay_ms != 0 && !cfg!(test) {
            thread::sleep(Duration::from_millis(delay_ms));
        }

        let opts = SerialOptions {
            baud,
            ..Default::default()
        };

        match SerialPort::connect(device, opts) {
            Ok(_) => {
                return ProbeResult {
                    baud,
                    attempts: attempts_taken,
                    success: true,
                    message: "port opened successfully".to_string(),
                }
            }
            Err(err) => last_err = Some(err.to_string()),
        }

        delay_ms = if delay_ms == 0 {
            backoff_initial_ms
        } else {
            (delay_ms.saturating_mul(2)).min(backoff_max_ms)
        };
    }

    ProbeResult {
        baud,
        attempts: attempts_taken,
        success: false,
        message: last_err.unwrap_or_else(|| "unknown error".to_string()),
    }
}

#[derive(Clone)]
struct ProbeResult {
    baud: u32,
    attempts: u8,
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
        writeln!(file, "intent: {}", entry.answers.intent.as_str())?;
        writeln!(file, "preference: {}", entry.answers.preference.as_str())?;
        writeln!(file, "run_probe: {}", entry.answers.run_probe)?;
        writeln!(
            file,
            "run_link_rehearsal: {}",
            entry.answers.run_link_rehearsal
        )?;
        writeln!(file, "show_helpers: {}", entry.answers.show_helpers)?;
        for probe in &entry.probes {
            let status = if probe.success { "ok" } else { "error" };
            writeln!(
                file,
                "probe baud {}: {} (attempts={} {})",
                probe.baud, status, probe.attempts, probe.message
            )?;
        }
        writeln!(file)?;
        Ok(())
    }
}

struct WizardTranscript {
    path: PathBuf,
}

impl WizardTranscript {
    fn new() -> Self {
        let path = Path::new(CACHE_DIR).join("wizard.log");
        Self { path }
    }

    fn record(&self, entry: WizardTranscriptEntry) {
        if let Err(err) = self.try_record(&entry) {
            eprintln!(
                "lifelinetty wizard: failed to write transcript at {}: {err}",
                self.path.display()
            );
        }
    }

    fn try_record(&self, entry: &WizardTranscriptEntry) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        writeln!(file, "=== lifelinetty wizard run ===")?;
        writeln!(file, "timestamp: {}", format_rfc3339(entry.timestamp))?;
        writeln!(file, "mode: {}", entry.mode_label)?;
        if let Some(note) = entry.mode_note.as_deref() {
            writeln!(file, "mode_note: {note}")?;
        }
        writeln!(file, "intent: {}", entry.answers.intent.as_str())?;
        writeln!(file, "device: {}", entry.answers.device)?;
        writeln!(file, "baud: {}", entry.answers.baud)?;
        writeln!(file, "cols: {}", entry.answers.cols)?;
        writeln!(file, "rows: {}", entry.answers.rows)?;
        writeln!(file, "lcd_present: {}", entry.answers.lcd_present)?;
        writeln!(file, "preference: {}", entry.answers.preference.as_str())?;
        writeln!(file, "run_probe: {}", entry.answers.run_probe)?;
        writeln!(
            file,
            "run_link_rehearsal: {}",
            entry.answers.run_link_rehearsal
        )?;
        writeln!(file, "show_helpers: {}", entry.answers.show_helpers)?;

        writeln!(file, "candidates:")?;
        if entry.candidates.is_empty() {
            writeln!(file, "  (none)")?;
        } else {
            for dev in &entry.candidates {
                writeln!(file, "  - {dev}")?;
            }
        }

        writeln!(file, "prompts:")?;
        for line in &entry.prompt_transcript {
            writeln!(file, "  {line}")?;
        }

        if entry.probes.is_empty() {
            writeln!(file, "probes: (skipped)")?;
        } else {
            writeln!(file, "probes:")?;
            for probe in &entry.probes {
                let status = if probe.success { "ok" } else { "error" };
                writeln!(
                    file,
                    "  - baud {}: {} (attempts={} {})",
                    probe.baud, status, probe.attempts, probe.message
                )?;
            }
        }

        writeln!(file)?;
        Ok(())
    }
}

struct WizardTranscriptEntry {
    timestamp: SystemTime,
    mode_label: &'static str,
    mode_note: Option<String>,
    prompt_transcript: Vec<String>,
    answers: WizardAnswers,
    candidates: Vec<String>,
    probes: Vec<ProbeResult>,
}

impl WizardTranscriptEntry {
    fn new(
        mode_label: &'static str,
        mode_note: Option<String>,
        prompt_transcript: Vec<String>,
        answers: &WizardAnswers,
        candidates: &[String],
        probes: &[ProbeResult],
    ) -> Self {
        Self {
            timestamp: SystemTime::now(),
            mode_label,
            mode_note,
            prompt_transcript,
            answers: answers.clone(),
            candidates: candidates.to_vec(),
            probes: probes.to_vec(),
        }
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
    transcript: Vec<String>,
}

impl WizardPrompter {
    fn new(input: PromptInput) -> Self {
        Self {
            input,
            transcript: Vec::new(),
        }
    }

    fn mode_label(&self) -> &'static str {
        self.input.label()
    }

    fn mode_note(&self) -> Option<&str> {
        self.input.note()
    }

    fn is_interactive(&self) -> bool {
        matches!(self.input, PromptInput::Interactive)
    }

    fn take_transcript(&mut self) -> Vec<String> {
        std::mem::take(&mut self.transcript)
    }

    fn prompt(&mut self, question: &str, default: &str) -> Result<String> {
        let answer = match &mut self.input {
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
                    default.to_string()
                } else {
                    trimmed.to_string()
                }
            }
            PromptInput::Scripted { lines, cursor } => {
                if *cursor >= lines.len() {
                    default.to_string()
                } else {
                    let value = lines[*cursor].clone();
                    *cursor += 1;
                    if value.trim().is_empty() {
                        default.to_string()
                    } else {
                        value.trim().to_string()
                    }
                }
            }
            PromptInput::AutoDefaults { .. } => default.to_string(),
        };

        self.transcript
            .push(format!("Q: {question} [default={default}] A: {answer}"));

        Ok(answer)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UsageIntent {
    Server,
    Client,
    Standalone,
}

impl UsageIntent {
    fn as_str(self) -> &'static str {
        match self {
            UsageIntent::Server => "server",
            UsageIntent::Client => "client",
            UsageIntent::Standalone => "standalone",
        }
    }

    fn to_role_preference(self) -> RolePreference {
        match self {
            UsageIntent::Server => RolePreference::PreferServer,
            UsageIntent::Client => RolePreference::PreferClient,
            UsageIntent::Standalone => RolePreference::NoPreference,
        }
    }

    fn from_role_preference(pref: RolePreference) -> Self {
        match pref {
            RolePreference::PreferServer => UsageIntent::Server,
            RolePreference::PreferClient => UsageIntent::Client,
            RolePreference::NoPreference => UsageIntent::Standalone,
        }
    }
}

fn prompt_yes_no(prompter: &mut WizardPrompter, question: &str, default: bool) -> Result<bool> {
    let default_label = if default { "y" } else { "n" };
    loop {
        let response = prompter.prompt(question, default_label)?;
        let trimmed = response.trim();
        if trimmed.is_empty() {
            return Ok(default);
        }

        match trimmed.to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            other => eprintln!("'{other}' is not a yes or no answer. Try y/n."),
        }
    }
}

fn prompt_usage_intent(prompter: &mut WizardPrompter, default: UsageIntent) -> Result<UsageIntent> {
    loop {
        let response = prompter.prompt(
            "Usage intent (1=server, 2=client, 3=standalone)",
            default.as_str(),
        )?;
        let trimmed = response.trim();
        if trimmed.is_empty() {
            return Ok(default);
        }

        match trimmed.to_ascii_lowercase().as_str() {
            "1" | "server" => return Ok(UsageIntent::Server),
            "2" | "client" => return Ok(UsageIntent::Client),
            "3" | "standalone" | "solo" | "auto" => return Ok(UsageIntent::Standalone),
            "?" | "help" => eprintln!("Choose 1=server, 2=client, 3=standalone."),
            other => eprintln!("Unknown intent '{other}', choose server, client, or standalone."),
        }
    }
}

fn print_wizard_helper_snippets() {
    println!("\n=== Wizard helper snippets (copy/paste) ===");
    println!("Nothing runs automatically. These are optional snippets you can paste yourself.");
    println!();
    println!("Copy the lifelinetty binary to a Pi (adjust paths/users as needed):");
    println!("  scp ./target/release/lifelinetty pi@raspberrypi.local:/usr/local/bin/lifelinetty");
    println!();
    println!(
        "Copy your config to the Pi (keeps persistence limited to ~/.serial_lcd/config.toml):"
    );
    println!("  scp ~/.serial_lcd/config.toml pi@raspberrypi.local:~/.serial_lcd/config.toml");
    println!();
    println!("Pull wizard/cache logs back to your laptop:");
    println!("  scp -r pi@raspberrypi.local:/run/serial_lcd_cache ./pi-logs/");
    println!();
    println!(
        "If lifelinetty is already running under systemd on the target, avoid TTY contention:"
    );
    println!("  ssh -t pi@raspberrypi.local 'sudo systemctl stop lifelinetty.service'");
    println!(
        "  ssh -t pi@raspberrypi.local 'sudo systemctl status lifelinetty.service --no-pager'"
    );
    println!();
    println!("Run serial shell in a persistent SSH+tmux session (adjust device/baud):");
    println!("  ssh -t pi@raspberrypi.local \\");
    println!(
        "    'tmux new -A -s lifelinetty_serialsh \"lifelinetty --serialsh --device /dev/ttyUSB0 --baud 9600\"'"
    );
    println!();
    println!("When finished, restart the service and follow logs:");
    println!("  ssh -t pi@raspberrypi.local 'sudo systemctl restart lifelinetty.service'");
    println!("  ssh -t pi@raspberrypi.local 'sudo journalctl -u lifelinetty.service -f'");
    println!();
    println!("Tail wizard + serial backoff logs on the Pi (tmux keeps it alive):");
    println!("  ssh -t pi@raspberrypi.local \\");
    println!("    'tmux new -A -s lifelinetty \"cd /run/serial_lcd_cache && tail -F wizard.log wizard/summary.log serial_backoff.log\"'");
    println!();
    println!("If you updated config and want to restart the systemd unit:");
    println!("  ssh -t pi@raspberrypi.local 'sudo systemctl restart lifelinetty && sudo journalctl -u lifelinetty -f'");
}

fn enumerate_serial_devices_ranked() -> Vec<String> {
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

    rank_serial_devices(&mut devices);
    devices
}

fn rank_serial_devices(devices: &mut [String]) {
    devices.sort_by(|a, b| {
        let (wa, ka) = device_rank_key(a);
        let (wb, kb) = device_rank_key(b);
        wa.cmp(&wb).then_with(|| ka.cmp(kb))
    });
}

fn device_rank_key(path: &str) -> (u8, &str) {
    let name = path.rsplit('/').next().unwrap_or(path);
    let weight = if name.starts_with("ttyUSB") {
        0
    } else if name.starts_with("ttyACM") {
        1
    } else if name.starts_with("ttyAMA") {
        2
    } else if name.starts_with("ttyS") {
        3
    } else {
        4
    };
    (weight, name)
}

enum DeviceSelection {
    Selected(String),
    Rescan,
}

fn prompt_device(
    prompter: &mut WizardPrompter,
    candidates: &[String],
    default: &str,
) -> Result<DeviceSelection> {
    loop {
        let response = prompter.prompt("Serial device path or index", default)?;
        let trimmed = response.trim();
        if trimmed.is_empty() {
            return Ok(DeviceSelection::Selected(default.to_string()));
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "r" | "rescan" => return Ok(DeviceSelection::Rescan),
            _ => {}
        }
        if let Ok(idx) = trimmed.parse::<usize>() {
            if idx >= 1 && idx <= candidates.len() {
                return Ok(DeviceSelection::Selected(candidates[idx - 1].clone()));
            }
        }
        if trimmed.starts_with("/dev/") {
            return Ok(DeviceSelection::Selected(trimmed.to_string()));
        }
        eprintln!(
            "Input '{trimmed}' was not a /dev path or a device index; enter a full path (e.g., /dev/ttyUSB0) or one of the listed numbers."
        );
    }
}

fn prompt_lcd_presence(prompter: &mut WizardPrompter, default: bool) -> Result<bool> {
    prompt_yes_no(prompter, "Is an LCD connected (y/n)", default)
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
        RolePreference::NoPreference => "standalone",
    };
    loop {
        let response =
            prompter.prompt("Preferred role (1=server, 2=client, 3=auto)", default_label)?;
        match response.trim().to_ascii_lowercase().as_str() {
            "1" | "server" => return Ok(RolePreference::PreferServer),
            "2" | "client" => return Ok(RolePreference::PreferClient),
            "3" | "standalone" | "auto" | "none" => return Ok(RolePreference::NoPreference),
            "?" | "help" => eprintln!("Choose 1=server, 2=client, 3=auto."),
            other => {
                eprintln!("Unknown role '{other}', choose server, client, standalone, or auto.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial::fake::FakeSerialPort;
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
        let mut wizard = FirstRunWizard::new(config_path.clone(), defaults, false).unwrap();
        let answers = [
            "standalone",
            "y",
            "/dev/ttyS42",
            "19200",
            "n",
            "16",
            "2",
            "client",
            "n",
            "y",
        ];
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

    #[test]
    fn ranks_devices_by_likelihood() {
        let mut devices = vec![
            "/dev/ttyS0".to_string(),
            "/dev/ttyUSB1".to_string(),
            "/dev/ttyAMA0".to_string(),
            "/dev/ttyUSB0".to_string(),
            "/dev/ttyACM0".to_string(),
        ];
        rank_serial_devices(&mut devices);
        assert_eq!(
            devices,
            vec![
                "/dev/ttyUSB0",
                "/dev/ttyUSB1",
                "/dev/ttyACM0",
                "/dev/ttyAMA0",
                "/dev/ttyS0"
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn link_rehearsal_selects_highest_successful_baud() {
        let negotiation = crate::config::NegotiationConfig::default();
        let base_options = SerialOptions::default();
        let candidates = [MIN_BAUD, 19_200, 38_400];
        let heartbeat = encode_tunnel_msg(&TunnelMsgOwned::Heartbeat).unwrap();

        let mut ports: std::collections::VecDeque<FakeSerialPort> = std::collections::VecDeque::from([
            // 9600 attempt: peer replies with hello_ack and then heartbeat.
            FakeSerialPort::new(vec![
                Ok("{\"type\":\"hello_ack\",\"chosen_role\":\"server\",\"peer_caps\":{\"bits\":1}}".into()),
                Ok(heartbeat.clone()),
            ]),
            // 19200 attempt: same success.
            FakeSerialPort::new(vec![
                Ok("{\"type\":\"hello_ack\",\"chosen_role\":\"server\",\"peer_caps\":{\"bits\":1}}".into()),
                Ok(heartbeat.clone()),
            ]),
            // 38400 attempt: handshake fails (non-control frame).
            FakeSerialPort::new(vec![Ok("not-json".into())]),
        ]);

        let (chosen, attempts) = run_link_speed_rehearsal_with::<FakeSerialPort, _>(
            "/dev/fake0",
            base_options,
            &negotiation,
            false,
            &candidates,
            |_device, _options| {
                ports
                    .pop_front()
                    .ok_or_else(|| crate::Error::Parse("no port".into()))
            },
        );

        assert_eq!(chosen, 19_200);
        assert_eq!(attempts.len(), 3);
        assert!(attempts[0].success);
        assert!(attempts[1].success);
        assert!(!attempts[2].success);
    }

    #[test]
    fn link_rehearsal_log_stays_under_cache_dir() {
        let log = LinkRehearsalLog::new();
        assert!(log.path.starts_with(CACHE_DIR));
        assert!(log.path.ends_with(Path::new("wizard/link_rehearsal.log")));
    }
}
