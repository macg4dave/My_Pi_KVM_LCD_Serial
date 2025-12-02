use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_home() -> PathBuf {
    let mut dir = env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_micros();
    dir.push(format!("lifelinetty_test_home_{stamp}"));
    dir
}

fn run_with_home(args: &[&str]) -> std::process::Output {
    let home = temp_home();
    fs::create_dir_all(&home).expect("failed to create temp HOME");
    let bin = env::var("CARGO_BIN_EXE_lifelinetty").expect("CARGO_BIN_EXE_lifelinetty not set");
    let output = Command::new(bin)
        .args(args)
        .env("HOME", &home)
        .output()
        .expect("failed to run lifelinetty");
    let _ = fs::remove_dir_all(&home);
    output
}

// B4: CLI docs/tests parity - these tests require proper Cargo integration
// They are part of the blocker B4 work

#[test]
#[ignore]
fn prints_version() {
    let output = run_with_home(&["--version"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "lifelinetty --version failed: status {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.trim().starts_with(env!("CARGO_PKG_VERSION")),
        "unexpected version output: {stdout}"
    );
}

#[test]
#[ignore]
fn payload_smoke_exits_cleanly() {
    let payload = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/test_payload.json");
    assert!(
        payload.exists(),
        "expected sample payload at {}",
        payload.display()
    );

    let output = run_with_home(&[
        "--payload-file",
        payload.to_str().expect("payload path not valid utf-8"),
        "--rows",
        "2",
        "--cols",
        "16",
    ]);

    assert!(
        output.status.success(),
        "lifelinetty payload smoke failed: status {:?}, stdout: {}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
