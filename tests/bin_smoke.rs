use lifelinetty::{
    app::App,
    cli::{Command, RunOptions},
    payload::{Defaults as PayloadDefaults, RenderFrame},
};
use std::{
    env, fs,
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

fn with_temp_home<F: FnOnce()>(f: F) {
    let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let original_home = env::var_os("HOME");
    let home = temp_home();
    fs::create_dir_all(&home).expect("failed to create temp HOME");
    env::set_var("HOME", &home);
    f();
    if let Some(val) = original_home {
        env::set_var("HOME", val);
    } else {
        env::remove_var("HOME");
    }
    let _ = fs::remove_dir_all(home);
}

// B3/B4: CLI + storage guardrails (library-level to avoid hardware dependency)

#[test]
fn rejects_log_file_outside_cache() {
    with_temp_home(|| {
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
    with_temp_home(|| {
        let original = env::var_os("LIFELINETTY_LOG_PATH");
        env::set_var("LIFELINETTY_LOG_PATH", "/tmp/env.log");
        let err = App::from_options(RunOptions::default())
            .err()
            .expect("expected env log path to be rejected");
        if let Some(val) = original {
            env::set_var("LIFELINETTY_LOG_PATH", val);
        } else {
            env::remove_var("LIFELINETTY_LOG_PATH");
        }
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
    for flag in ["--device", "--cols", "--rows", "--demo"] {
        assert!(
            help.contains(flag),
            "help output missing flag {flag}: {help}"
        );
    }
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
