use seriallcd::{
    lcd::Lcd,
    payload::{Defaults, DEFAULT_PAGE_TIMEOUT_MS, DEFAULT_SCROLL_MS},
    state::RenderState,
};

#[test]
fn integration_parses_and_states() {
    let mut state = RenderState::new(Some(Defaults {
        scroll_speed_ms: DEFAULT_SCROLL_MS,
        page_timeout_ms: DEFAULT_PAGE_TIMEOUT_MS,
    }));
    let raw = r#"{"line1":"CPU","line2":"42%","bar":42,"scroll":false}"#;
    let frame = state.ingest(raw).unwrap().unwrap();
    assert_eq!(frame.bar_percent, Some(42));
    assert!(!frame.scroll_enabled);
    assert_eq!(state.len(), 1);
}

#[test]
fn smoke_lcd_write_lines_stub() {
    let mut lcd = Lcd::new(16, 2, seriallcd::config::DEFAULT_PCF8574_ADDR).unwrap();
    lcd.write_lines("HELLO", "WORLD").unwrap();
}
