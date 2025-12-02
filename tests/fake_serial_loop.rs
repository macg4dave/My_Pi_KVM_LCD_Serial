// This test file requires platform-specific LCD driver stubs not available on this platform
// It will be enabled as part of P4 (LCD driver regression tests) in the roadmap

/*
use lifelinetty::{
    config::DEFAULT_PCF8574_ADDR,
    display::overlays::{render_frame_with_scroll, render_offline_message, render_parse_error},
    lcd::Lcd,
    payload::{Defaults, RenderFrame},
    serial::fake::FakeSerialPort,
    state::RenderState,
    Error,
};

fn defaults() -> Defaults {
    Defaults {
        scroll_speed_ms: lifelinetty::payload::DEFAULT_SCROLL_MS,
        page_timeout_ms: lifelinetty::payload::DEFAULT_PAGE_TIMEOUT_MS,
    }
}

fn render_frame(lcd: &mut Lcd, frame: &RenderFrame) {
    let _ = lcd.clear();
    let _ = lcd.set_backlight(frame.backlight_on);
    let _ = lcd.set_blink(frame.blink);
    let _ = render_frame_with_scroll(lcd, frame, (0, 0), false);
}

#[ignore]
#[test]
fn fake_serial_drives_frames_and_errors() {
    let mut serial = FakeSerialPort::new(vec![
        Ok(r#"{"line1":"HELLO","line2":"WORLD"}"#.into()),
        Ok(r#"{"line1":"HELLO","line2":"WORLD"}"#.into()), // duplicate ignored
        Ok(r#"{"line1":"NEXT","line2":"PAGE","blink":true,"backlight":false}"#.into()),
        Ok("not json".into()), // parse error
        Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "timeout",
        ))), // offline trigger
    ]);

    let mut lcd = Lcd::new(16, 2, DEFAULT_PCF8574_ADDR).unwrap();
    let mut state = RenderState::new(Some(defaults()));
    let mut buf = String::new();

    // first frame renders
    let read = serial.read_message_line(&mut buf).unwrap();
    assert!(read > 0);
    let frame = state.ingest(buf.trim()).unwrap().unwrap();
    render_frame(&mut lcd, &frame);
    assert_eq!(lcd.last_lines(), ("HELLO".into(), "WORLD".into()));
    assert!(lcd.last_backlight());
    assert!(!lcd.last_blink());

    // duplicate ignored, LCD unchanged
    let _ = serial.read_message_line(&mut buf).unwrap();
    assert!(state.ingest(buf.trim()).unwrap().is_none());
    assert_eq!(lcd.last_lines(), ("HELLO".into(), "WORLD".into()));

    // new frame toggles blink/backlight
    let _ = serial.read_message_line(&mut buf).unwrap();
    let frame = state.ingest(buf.trim()).unwrap().unwrap();
    render_frame(&mut lcd, &frame);
    assert_eq!(lcd.last_lines(), ("NEXT".into(), "PAGE".into()));
    assert!(!lcd.last_backlight());
    assert!(lcd.last_blink());

    // parse error renders error lines and forces backlight/blink
    let _ = serial.read_message_line(&mut buf).unwrap();
    let err = state.ingest(buf.trim()).unwrap_err();
    render_parse_error(&mut lcd, 16, &err).unwrap();
    let (l1, l2) = lcd.last_lines();
    assert_eq!(l1, "ERR PARSE");
    assert!(lcd.last_backlight());
    assert!(lcd.last_blink());
    assert!(l2.starts_with("parse error"));

    // offline message when IO error occurs
    let err = serial.read_message_line(&mut buf).unwrap_err();
    assert!(format!("{err}").contains("io error"));
    render_offline_message(&mut lcd, 16).unwrap();
    let (l1, l2) = lcd.last_lines();
    assert!(l1.starts_with("SERIAL OFFLINE"));
    assert!(l2.starts_with("will retry"));
    assert!(lcd.last_backlight());
    assert!(lcd.last_blink());
}

#[test]
fn fake_serial_tracks_writes_and_clear_count() {
    let mut serial = FakeSerialPort::new(vec![]);
    serial.send_command_line("INIT").unwrap();
    serial.send_command_line("PING").unwrap();
    assert_eq!(serial.writes(), &["INIT".to_string(), "PING".to_string()]);

    let mut lcd = Lcd::new(20, 4, DEFAULT_PCF8574_ADDR).unwrap();
    lcd.write_lines("A", "B").unwrap();
    assert_eq!(lcd.last_lines(), ("A".into(), "B".into()));
    lcd.clear().unwrap();
    assert_eq!(lcd.clear_count(), 1);
}
*/
