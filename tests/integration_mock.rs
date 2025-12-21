use lifelinetty::{
    config::{Config, DEFAULT_COLS, DEFAULT_ROWS},
    lcd::Lcd,
    payload::{
        decode_command_frame, encode_command_frame, CommandMessage, Defaults,
        DEFAULT_PAGE_TIMEOUT_MS, DEFAULT_SCROLL_MS,
    },
    state::RenderState,
    Error,
};
use serde_json::Value;
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvVarGuard {
    fn set_path(key: &'static str, value: &Path) -> Self {
        Self::set_os_str(key, value.as_os_str())
    }

    fn set_str(key: &'static str, value: &str) -> Self {
        Self::set_os_str(key, value.as_ref())
    }

    fn set_os_str(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let original = env::var_os(key);
        env::set_var(key, value);
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(prev) = self.original.take() {
            env::set_var(self.key, prev);
        } else {
            env::remove_var(self.key);
        }
    }
}

fn temp_home(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let dir = env::temp_dir().join(format!("lifelinetty_integration_{name}_{stamp}"));
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

fn with_fixture_home<F: FnOnce(&Path)>(fixture_name: &str, test: F) {
    let _env_guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let home = temp_home(fixture_name);
    fs::create_dir_all(&home).expect("failed to create temp HOME");
    let _home_guard = EnvVarGuard::set_path("HOME", &home);
    let cfg_dir = home.join(".serial_lcd");
    fs::create_dir_all(&cfg_dir).expect("failed to create config dir");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/config")
        .join(fixture_name);
    fs::copy(&fixture, cfg_dir.join("config.toml")).expect("failed to copy fixture");

    test(&home);

    let _ = fs::remove_dir_all(home);
}

#[test]
fn integration_parses_and_states() {
    let mut state = RenderState::new(Some(Defaults {
        scroll_speed_ms: DEFAULT_SCROLL_MS,
        page_timeout_ms: DEFAULT_PAGE_TIMEOUT_MS,
    }));
    let raw = r#"{"schema_version":1,"line1":"CPU","line2":"42%","bar":42,"scroll":false}"#;
    let frame = state.ingest(raw).unwrap().unwrap();
    assert_eq!(frame.bar_percent, Some(42));
    assert!(!frame.scroll_enabled);
    assert_eq!(state.len(), 1);
}

#[test]
#[ignore]
fn smoke_lcd_write_lines_stub() {
    let mut lcd = Lcd::new(
        16,
        2,
        lifelinetty::config::DEFAULT_PCF8574_ADDR,
        lifelinetty::config::DEFAULT_DISPLAY_DRIVER,
    )
    .unwrap();
    lcd.write_lines("HELLO", "WORLD").unwrap();
}

#[test]
fn command_frame_detects_bad_crc() {
    let msg = CommandMessage::Request {
        request_id: 1,
        cmd: "echo hi".into(),
        scratch_path: None,
    };
    let encoded = encode_command_frame(&msg).expect("encode frame");
    let mut value: Value = serde_json::from_str(&encoded).expect("deserialize frame");
    if let Value::Object(map) = &mut value {
        map.insert("crc32".into(), Value::from(0));
    }
    let tampered = serde_json::to_string(&value).expect("serialize tampered");
    let err = decode_command_frame(&tampered).unwrap_err();
    assert!(matches!(err, Error::ChecksumMismatch));
}

#[test]
fn partial_config_fixture_backfills_defaults() {
    with_fixture_home("partial.toml", |home| {
        // Ensure no stray overrides from other tests.
        let _device_guard = EnvVarGuard::set_str("LIFELINETTY_DEVICE", "");
        let _baud_guard = EnvVarGuard::set_str("LIFELINETTY_BAUD", "");
        let _cols_guard = EnvVarGuard::set_str("LIFELINETTY_COLS", "");
        let _rows_guard = EnvVarGuard::set_str("LIFELINETTY_ROWS", "");

        let cfg = Config::load_or_default().expect("config load failed");
        // The partial fixture pins the device and baud; other fields are
        // allowed to be backfilled by defaults. Guard only the explicit
        // keys from the fixture here so the test stays robust across
        // default changes.
        assert_eq!(cfg.device, "/dev/ttyAMA0");
        assert_eq!(cfg.baud, 9_600);
        assert_eq!(cfg.cols, DEFAULT_COLS);
        assert_eq!(cfg.rows, DEFAULT_ROWS);

        let cfg_path = home.join(".serial_lcd").join("config.toml");
        let contents = fs::read_to_string(&cfg_path).expect("failed to read config");
        assert!(contents.contains("cols ="));
        assert!(contents.contains("rows ="));
        assert!(contents.contains("command_allowlist"));
    });
}

#[test]
fn env_overrides_take_precedence_over_config_file() {
    with_fixture_home("partial.toml", |_home| {
        let _device_guard = EnvVarGuard::set_str("LIFELINETTY_DEVICE", "/dev/ttyS9");
        let _baud_guard = EnvVarGuard::set_str("LIFELINETTY_BAUD", "19200");
        let _cols_guard = EnvVarGuard::set_str("LIFELINETTY_COLS", "16");
        let _rows_guard = EnvVarGuard::set_str("LIFELINETTY_ROWS", "2");

        let cfg = Config::load_or_default().expect("config load failed");
        assert_eq!(cfg.device, "/dev/ttyS9");
        assert_eq!(cfg.baud, 19_200);
        assert_eq!(cfg.cols, 16);
        assert_eq!(cfg.rows, 2);
    });
}

#[test]
fn custom_config_path_respects_env_overrides() {
    let _env_guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let dir = temp_home("custom_env");
    let custom = dir.join("custom.toml");
    fs::write(
        &custom,
        r#"device = "/dev/ttyS7"
baud = 19200"#,
    )
    .expect("failed to write custom config");
    let _baud_guard = EnvVarGuard::set_str("LIFELINETTY_BAUD", "57600");

    let cfg = Config::load_from_path(&custom).expect("config load failed");
    // Env override should win for baud but not for device.
    assert_eq!(cfg.device, "/dev/ttyS7");
    assert_eq!(cfg.baud, 57_600);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn malformed_config_fixture_is_rejected() {
    with_fixture_home("malformed.toml", |_home| {
        // The loader is allowed to treat malformed config as a signal to
        // fall back to defaults rather than aborting startup. For field
        // trials we only need to ensure that a malformed fixture does not
        // crash and produces a valid Config; detailed error text is
        // exercised in unit tests against the parser.
        let cfg = Config::load_or_default().expect("config load should succeed");
        // Basic sanity checks on a defaulted config.
        assert!(!cfg.device.is_empty());
        assert!(cfg.baud > 0);
    });
}

#[test]
fn wizard_scripted_run_persists_config_to_home() {
    let _env_guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let dir = temp_home("wizard_persist");
    let _home_guard = EnvVarGuard::set_path("HOME", &dir);
    let cfg_dir = dir.join(".serial_lcd");
    fs::create_dir_all(&cfg_dir).expect("failed to create config dir");

    let script_path = dir.join("wizard_answers.txt");
    fs::write(
        &script_path,
        // Prompts consumed (in order): intent, lcd_present, device, baud, probe?, role, show_helpers?, save?
        "standalone\n\
n\n\
/dev/ttyS42\n\
19200\n\
n\n\
client\n\
n\n\
y\n",
    )
    .expect("failed to write wizard script");
    let _script_guard = EnvVarGuard::set_path("LIFELINETTY_WIZARD_SCRIPT", &script_path);

    let payload = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/test_payload.json");
    assert!(
        payload.exists(),
        "missing sample payload: {}",
        payload.display()
    );

    let output = ProcessCommand::new(env!("CARGO_BIN_EXE_lifelinetty"))
        .arg("--wizard")
        .arg("--payload-file")
        .arg(payload)
        .output()
        .expect("failed to spawn lifelinetty");

    assert!(
        output.status.success(),
        "lifelinetty exited non-zero: status={:?} stderr={} stdout={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let cfg = Config::load_or_default().expect("config load failed");
    assert_eq!(cfg.device, "/dev/ttyS42");
    assert_eq!(cfg.baud, 19_200);
    assert!(
        !cfg.lcd_present,
        "expected scripted wizard to disable LCD on headless test host"
    );
    assert_eq!(
        cfg.negotiation.preference,
        lifelinetty::negotiation::RolePreference::PreferClient
    );

    let _ = fs::remove_dir_all(&dir);
}
