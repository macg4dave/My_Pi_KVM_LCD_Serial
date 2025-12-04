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
    env::temp_dir().join(format!("lifelinetty_integration_{name}_{stamp}"))
}

fn with_fixture_home<F: FnOnce(&Path)>(fixture_name: &str, test: F) {
    let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
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
        let cfg = Config::load_or_default().expect("config load failed");
        assert_eq!(cfg.device, "/dev/ttyAMA0");
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
fn malformed_config_fixture_is_rejected() {
    with_fixture_home("malformed.toml", |_home| {
        let err = Config::load_or_default().unwrap_err();
        assert!(format!("{err}").contains("invalid config line"));
    });
}
