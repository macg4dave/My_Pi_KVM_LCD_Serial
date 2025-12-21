use lifelinetty::{
    app::{App, AppConfig},
    cli::{Command, RunOptions},
    config::{Config, DisplayDriver},
    negotiation::RolePreference,
    payload::{Defaults as PayloadDefaults, RenderFrame},
};
use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn temp_home() -> PathBuf {
    let mut dir = env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_micros();
    dir.push(format!("lifelinetty_test_home_{stamp}"));
    dir
}

fn with_temp_home<F: FnOnce(&Path)>(f: F) {
    let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let home = temp_home();
    fs::create_dir_all(&home).expect("failed to create temp HOME");
    let _home_guard = EnvVarGuard::set_path("HOME", &home);
    f(&home);
    let _ = fs::remove_dir_all(home);
}

fn write_config(home: &Path, contents: &str) {
    let cfg_dir = home.join(".serial_lcd");
    fs::create_dir_all(&cfg_dir).expect("failed to create config dir");
    fs::write(cfg_dir.join("config.toml"), contents).expect("failed to write config");
}

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvVarGuard {
    fn set_path(key: &'static str, value: &Path) -> Self {
        Self::set_os_str(key, value.as_os_str())
    }

    fn set_str(key: &'static str, value: &str) -> Self {
        Self::set_os_str(key, OsStr::new(value))
    }

    fn set_os_str(key: &'static str, value: &OsStr) -> Self {
        let original = env::var_os(key);
        env::set_var(key, value);
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.original.take() {
            env::set_var(self.key, previous);
        } else {
            env::remove_var(self.key);
        }
    }
}

fn install_wizard_script(home: &Path, name: &str, contents: &str) -> EnvVarGuard {
    let script_path = home.join(name);
    fs::write(&script_path, contents).expect("failed to write wizard script");
    EnvVarGuard::set_path("LIFELINETTY_WIZARD_SCRIPT", &script_path)
}

fn install_default_wizard_script(home: &Path) -> EnvVarGuard {
    install_wizard_script(
        home,
        "wizard_defaults.txt",
        "standalone\ny\n/dev/ttyUSB0\n9600\nn\n16\n2\nauto\nn\n",
    )
}

// B3/B4/B2: CLI + storage guardrails + device precedence (library-level to avoid hardware dependency)

#[test]
fn rejects_log_file_outside_cache() {
    with_temp_home(|home| {
        let _script_guard = install_default_wizard_script(home);
        let mut opts = RunOptions::default();
        opts.log_file = Some("/tmp/out.log".into());
        let err = App::from_options(opts)
            .err()
            .expect("expected invalid log path to be rejected");
        assert!(
            format!("{err}").contains("/run/serial_lcd_cache"),
            "error did not mention cache dir: {err}"
        );
    });
}

#[test]
fn rejects_env_log_path_outside_cache() {
    with_temp_home(|home| {
        let _script_guard = install_default_wizard_script(home);
        let _log_guard = EnvVarGuard::set_str("LIFELINETTY_LOG_PATH", "/tmp/env.log");
        let err = App::from_options(RunOptions::default())
            .err()
            .expect("expected env log path to be rejected");
        assert!(
            format!("{err}").contains("/run/serial_lcd_cache"),
            "error did not mention cache dir: {err}"
        );
    });
}

#[test]
fn prints_version() {
    let args = vec!["--version".to_string()];
    let cmd = Command::parse(&args).unwrap();
    assert!(matches!(cmd, Command::ShowVersion));
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
}

#[test]
fn help_lists_core_flags() {
    let help = Command::help();
    for flag in ["--device", "--cols", "--rows", "--demo", "--config-file"] {
        assert!(
            help.contains(flag),
            "help output missing flag {flag}: {help}"
        );
    }

    for flag in ["--serialsh", "--wizard"] {
        assert!(help.contains(flag), "help output missing {flag}: {help}");
    }
}

#[test]
fn config_file_override_respects_env_and_skips_default() {
    with_temp_home(|home| {
        let custom = home.join("custom-config.toml");
        fs::write(&custom, "device = \"/dev/ttyS2\"\nbaud = 19200\n")
            .expect("failed to write custom config");
        let _baud_guard = EnvVarGuard::set_str("LIFELINETTY_BAUD", "38400");
        let mut opts = RunOptions::default();
        opts.config_file = Some(custom.to_string_lossy().to_string());

        let app = App::from_options(opts).expect("app init failed");
        assert_eq!(app.config().device, "/dev/ttyS2");
        assert_eq!(app.config().baud, 38_400);
        let default_path = home.join(".serial_lcd").join("config.toml");
        assert!(
            !default_path.exists(),
            "default config should not be created when --config-file is used"
        );
    });
}

#[test]
fn payload_sample_parses() {
    let payload = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/test_payload.json");
    assert!(
        payload.exists(),
        "expected sample payload at {}",
        payload.display()
    );
    let raw = fs::read_to_string(&payload).expect("failed to read sample payload");
    let defaults = PayloadDefaults {
        scroll_speed_ms: lifelinetty::config::DEFAULT_SCROLL_MS,
        page_timeout_ms: lifelinetty::config::DEFAULT_PAGE_TIMEOUT_MS,
    };
    RenderFrame::from_payload_json_with_defaults(&raw, defaults)
        .expect("sample payload failed to parse");
}

#[test]
fn payload_examples_ndjson_lines_parse() {
    let payload = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/payload_examples.json");
    assert!(
        payload.exists(),
        "expected sample payload at {}",
        payload.display()
    );
    let raw = fs::read_to_string(&payload).expect("failed to read sample payload");
    let defaults = PayloadDefaults {
        scroll_speed_ms: lifelinetty::config::DEFAULT_SCROLL_MS,
        page_timeout_ms: lifelinetty::config::DEFAULT_PAGE_TIMEOUT_MS,
    };

    for (idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        RenderFrame::from_payload_json_with_defaults(trimmed, defaults)
            .unwrap_or_else(|e| panic!("ndjson line {} failed to parse: {e} ({trimmed})", idx + 1));
    }
}

#[test]
fn lcd_dashboard_payload_parses_and_sets_fields() {
    let raw = r#"{
            "schema_version":1,
            "line1":"CPU",
            "line2":"Load 42%",
            "bar_value":42,
            "bar_max":100,
            "bar_label":"CPU",
            "mode":"dashboard",
            "page_timeout_ms":2000
        }"#;
    let defaults = PayloadDefaults {
        scroll_speed_ms: lifelinetty::config::DEFAULT_SCROLL_MS,
        page_timeout_ms: lifelinetty::config::DEFAULT_PAGE_TIMEOUT_MS,
    };
    let frame = RenderFrame::from_payload_json_with_defaults(raw, defaults)
        .expect("dashboard payload failed to parse");

    assert_eq!(frame.line1, "CPU");
    assert_eq!(frame.line2, "Load 42%");
    assert_eq!(frame.bar_percent, Some(42));
    assert_eq!(frame.bar_label.as_deref(), Some("CPU"));
    assert_eq!(frame.page_timeout_ms, 2000);
    assert!(!frame.config_reload);
}

#[test]
fn config_supports_alt_ttys() {
    with_temp_home(|home| {
        write_config(
            home,
            r#"
device = "/dev/ttyAMA0"
baud = 9600
        "#,
        );
        let cfg = Config::load_or_default().expect("config load failed");
        let merged = AppConfig::from_sources(cfg, RunOptions::default());
        assert_eq!(merged.device, "/dev/ttyAMA0");
        assert_eq!(merged.baud, 9_600);
    });
}

#[test]
fn cli_overrides_config_device_and_baud() {
    with_temp_home(|home| {
        write_config(
            home,
            r#"
device = "/dev/ttyAMA0"
baud = 9600
        "#,
        );
        let cfg = Config::load_or_default().expect("config load failed");
        let mut opts = RunOptions::default();
        opts.device = Some("/dev/ttyS1".into());
        opts.baud = Some(19_200);
        let merged = AppConfig::from_sources(cfg, opts);
        assert_eq!(merged.device, "/dev/ttyS1");
        assert_eq!(merged.baud, 19_200);
    });
}

#[test]
fn cli_overrides_cols_and_rows() {
    with_temp_home(|home| {
        write_config(
            home,
            r#"
device = "/dev/ttyUSB0"
cols = 20
rows = 4
        "#,
        );
        let cfg = Config::load_or_default().expect("config load failed");
        let mut opts = RunOptions::default();
        opts.cols = Some(16);
        opts.rows = Some(2);

        let merged = AppConfig::from_sources(cfg, opts);
        assert_eq!(merged.cols, 16);
        assert_eq!(merged.rows, 2);
    });
}

#[test]
fn config_allows_display_driver_selection() {
    with_temp_home(|home| {
        write_config(
            home,
            r#"
device = "/dev/ttyUSB0"
display_driver = "hd44780-driver"
        "#,
        );
        let cfg = Config::load_or_default().expect("config load failed");
        assert_eq!(cfg.display_driver, DisplayDriver::Hd44780Driver);
    });
}

#[test]
fn wizard_auto_runs_with_script_when_missing_config() {
    with_temp_home(|home| {
        let _script_guard = install_wizard_script(
            home,
            "wizard_answers.txt",
            "server\ny\n/dev/ttyS9\n19200\nn\n16\n2\nserver\nn\n",
        );

        let app = App::from_options(RunOptions::default()).expect("wizard-driven app init failed");
        drop(app);

        let cfg = Config::load_or_default().expect("config load failed");
        assert_eq!(cfg.device, "/dev/ttyS9");
        assert_eq!(cfg.baud, 19_200);
        assert_eq!(cfg.cols, 16);
        assert_eq!(cfg.rows, 2);
        assert_eq!(cfg.negotiation.preference, RolePreference::PreferServer);
    });
}

#[test]
fn wizard_skips_when_config_exists_without_force() {
    with_temp_home(|home| {
        write_config(
            home,
            r#"
device = "/dev/ttyAMA0"
baud = 38400
rows = 4
cols = 20
            "#,
        );

        let _script_guard = install_wizard_script(
            home,
            "wizard_skip.txt",
            "standalone\ny\n/dev/ttyUSB9\n57600\nn\n20\n4\nclient\nn\n",
        );

        let app = App::from_options(RunOptions::default()).expect("app init failed");
        drop(app);

        let cfg = Config::load_or_default().expect("config load failed");
        assert_eq!(cfg.device, "/dev/ttyAMA0");
        assert_eq!(cfg.baud, 38_400);
        assert_eq!(cfg.negotiation.preference, RolePreference::NoPreference);
    });
}

#[test]
fn wizard_runs_when_config_exists_but_empty() {
    with_temp_home(|home| {
        write_config(home, "   \n");
        let _script_guard = install_wizard_script(
            home,
            "wizard_empty.txt",
            "standalone\ny\n/dev/ttyS8\n19200\nn\n16\n2\nauto\nn\n",
        );

        let app = App::from_options(RunOptions::default()).expect("wizard repair init failed");
        drop(app);

        let cfg = Config::load_or_default().expect("config load failed");
        assert_eq!(cfg.device, "/dev/ttyS8");
        assert_eq!(cfg.baud, 19_200);
        assert_eq!(cfg.negotiation.preference, RolePreference::NoPreference);
    });
}

#[test]
fn wizard_runs_when_config_is_unparseable() {
    with_temp_home(|home| {
        write_config(home, "device \"/dev/ttyUSB0\"\n");
        let _script_guard = install_wizard_script(
            home,
            "wizard_bad_cfg.txt",
            "client\ny\n/dev/ttyACM9\n57600\nn\n20\n4\nclient\nn\n",
        );

        let app = App::from_options(RunOptions::default()).expect("wizard repair init failed");
        drop(app);

        let cfg = Config::load_or_default().expect("config load failed");
        assert_eq!(cfg.device, "/dev/ttyACM9");
        assert_eq!(cfg.baud, 57_600);
        assert_eq!(cfg.negotiation.preference, RolePreference::PreferClient);
    });
}

#[test]
fn wizard_force_env_overrides_existing_config() {
    with_temp_home(|home| {
        write_config(
            home,
            r#"
device = "/dev/ttyAMA0"
baud = 38400
rows = 4
cols = 20
            "#,
        );

        let _script_guard = install_wizard_script(
            home,
            "wizard_force.txt",
            "client\ny\n/dev/ttyACM1\n57600\nn\n20\n4\nclient\nn\n",
        );
        let _force_guard = EnvVarGuard::set_str("LIFELINETTY_FORCE_WIZARD", "1");

        let app = App::from_options(RunOptions::default()).expect("app init failed");
        drop(app);

        let cfg = Config::load_or_default().expect("config load failed");
        assert_eq!(cfg.device, "/dev/ttyACM1");
        assert_eq!(cfg.baud, 57_600);
        assert_eq!(cfg.negotiation.preference, RolePreference::PreferClient);
    });
}

mod serialsh_smoke {
    use lifelinetty::app::serial_shell::{drive_serial_shell_loop, SerialShellTransport};
    use lifelinetty::payload::{encode_tunnel_msg, TunnelMsgOwned};
    use lifelinetty::serial::fake::FakeSerialPort;
    use lifelinetty::Result;
    use std::io::Cursor;

    struct FakeTransport(FakeSerialPort);

    impl FakeTransport {
        fn new(script: Vec<Result<String>>) -> Self {
            Self(FakeSerialPort::new(script))
        }

        fn writes(&self) -> &[String] {
            self.0.writes()
        }
    }

    impl SerialShellTransport for FakeTransport {
        fn send_command_line(&mut self, line: &str) -> Result<()> {
            self.0.send_command_line(line)
        }

        fn read_message_line(&mut self, buf: &mut String) -> Result<usize> {
            self.0.read_message_line(buf)
        }
    }

    fn encoded(msg: TunnelMsgOwned) -> String {
        encode_tunnel_msg(&msg).expect("failed to encode tunnel frame")
    }

    #[test]
    fn serial_shell_round_trip_delivers_output() {
        let mut serial = FakeTransport::new(vec![
            Ok(encoded(TunnelMsgOwned::Stdout {
                chunk: b"hello".to_vec(),
            })),
            Ok(encoded(TunnelMsgOwned::Stderr {
                chunk: b"warn".to_vec(),
            })),
            Ok(encoded(TunnelMsgOwned::Exit { code: 42 })),
        ]);
        let mut input = Cursor::new("echo hi\nexit\n");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = drive_serial_shell_loop(&mut serial, &mut input, &mut stdout, &mut stderr)
            .expect("serial shell failed");

        assert_eq!(exit_code, 42);
        let out_text = String::from_utf8_lossy(&stdout);
        assert!(out_text.contains("serialsh> "));
        assert!(out_text.contains("hello"));
        let err_text = String::from_utf8_lossy(&stderr);
        assert!(err_text.contains("warn"));
        assert_eq!(
            serial.writes(),
            &[
                "INIT".to_string(),
                encoded(TunnelMsgOwned::CmdRequest {
                    cmd: "echo hi".into()
                })
            ]
        );
    }

    #[test]
    fn serial_shell_busy_response_returns_one() {
        let mut serial = FakeTransport::new(vec![Ok(encoded(TunnelMsgOwned::Busy))]);
        let mut input = Cursor::new("list\nexit\n");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = drive_serial_shell_loop(&mut serial, &mut input, &mut stdout, &mut stderr)
            .expect("serial shell failed");

        assert_eq!(exit_code, 1);
        assert!(String::from_utf8_lossy(&stderr).contains("remote busy"));
        assert_eq!(
            serial.writes(),
            &[
                "INIT".to_string(),
                encoded(TunnelMsgOwned::CmdRequest { cmd: "list".into() })
            ]
        );
    }
}
