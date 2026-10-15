use ratatui::style::{Color, Modifier, Style};

use crate::steps::Status;

pub struct Palette {
    pub primary: Color,
    pub accent: Color,
    pub container: Color,
    pub surface: Color,
    pub on_surface: Color,
    pub secondary: Color,
    pub tertiary: Color,
    pub muted: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

pub const PALETTE: Palette = Palette {
    primary: Color::Rgb(0x9b, 0xd0, 0xcc),
    accent: Color::Rgb(0x6a, 0xe5, 0xe1),
    container: Color::Rgb(0x25, 0x5b, 0x58),
    surface: Color::Rgb(0x0a, 0x0f, 0x0f),
    on_surface: Color::Rgb(0xdc, 0xe8, 0xe6),
    secondary: Color::Rgb(0xb0, 0xcc, 0xc9),
    tertiary: Color::Rgb(0xd5, 0xef, 0xff),
    muted: Color::Rgb(0x6d, 0x78, 0x76),
    success: Color::Rgb(0xb5, 0xcc, 0xba),
    warning: Color::Rgb(0xe8, 0xc8, 0x7a),
    error: Color::Rgb(0xfa, 0x74, 0x6f),
};

pub const SPLASH: [&str; 6] = [
    "   _____            _           _   _       ",
    "  / ____|          | |         | | (_)      ",
    " | |     __ _  ___ | | ___  ___| |_ _  __ _ ",
    " | |    / _` |/ _ \\| |/ _ \\/ __| __| |/ _` |",
    " | |___| (_| | (_) | |  __/\\__ \\ |_| | (_| |",
    "  \\_____\\__,_|\\___/|_|\\___||___/\\__|_|\\__,_|",
];

pub const SPLASH_COLOR: Color = PALETTE.accent;

pub const AUTHOR: &str = "By @ladybug-me";
pub const CO_AUTHOR: &str = "Co-maintainer: 0xSolanaceae";

pub const CHECK_ON: &str = "[x]";
pub const CHECK_OFF: &str = "[ ]";
pub const SELECT_LEFT: &str = "<";
pub const SELECT_RIGHT: &str = ">";

pub fn status_glyph(status: Status) -> &'static str {
    match status {
        Status::Pending => "[ ]",
        Status::Running => "[>]",
        Status::Ok => "[OK]",
        Status::Warn => "[WARN]",
        Status::Failed => "[ERR]",
        Status::Skipped => "[SKIP]",
        Status::Ignored => "[IGNORED]",
    }
}

pub fn status_color(status: Status) -> Color {
    match status {
        Status::Running => PALETTE.primary,
        Status::Ok => PALETTE.success,
        Status::Warn | Status::Ignored => PALETTE.warning,
        Status::Failed => PALETTE.error,
        Status::Pending | Status::Skipped => PALETTE.muted,
    }
}

pub fn status_style(status: Status) -> Style {
    Style::default().fg(status_color(status))
}

pub fn title_style() -> Style {
    Style::default()
        .fg(PALETTE.primary)
        .add_modifier(Modifier::BOLD)
}

pub fn background_style() -> Style {
    Style::default().bg(PALETTE.surface)
}

pub fn border_style() -> Style {
    Style::default().fg(PALETTE.container)
}

pub fn phase_style() -> Style {
    Style::default()
        .fg(PALETTE.tertiary)
        .add_modifier(Modifier::BOLD)
}

pub fn heading_style() -> Style {
    Style::default().fg(PALETTE.on_surface)
}

pub fn hint_style() -> Style {
    Style::default().fg(PALETTE.muted)
}

pub fn body_style() -> Style {
    Style::default().fg(PALETTE.secondary)
}

pub fn selected_style() -> Style {
    Style::default()
        .fg(PALETTE.primary)
        .add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_has_a_glyph_and_a_colour() {
        for status in Status::ALL {
            assert!(!status_glyph(status).is_empty());
            let _ = status_color(status);
        }
    }

    #[test]
    fn failures_are_not_drawn_like_successes() {
        assert_ne!(status_color(Status::Failed), status_color(Status::Ok));
    }
}
