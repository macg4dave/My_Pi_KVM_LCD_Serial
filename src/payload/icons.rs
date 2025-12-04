/// Display modes for the LCD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayMode {
    Normal,
    Dashboard,
    Banner,
}

/// Supported icon glyphs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Icon {
    Battery,
    Arrow,
    Heart,
    Wifi,
}

pub(crate) fn parse_display_mode(raw: Option<String>) -> DisplayMode {
    match raw.as_deref() {
        Some("dashboard") => DisplayMode::Dashboard,
        Some("banner") => DisplayMode::Banner,
        _ => DisplayMode::Normal,
    }
}

pub(crate) fn parse_icons(raw: Option<Vec<String>>) -> Vec<Icon> {
    raw.unwrap_or_default()
        .into_iter()
        .filter_map(|name| match name.to_lowercase().as_str() {
            "battery" => Some(Icon::Battery),
            "arrow" => Some(Icon::Arrow),
            "heart" => Some(Icon::Heart),
            "wifi" => Some(Icon::Wifi),
            _ => None,
        })
        .collect()
}
