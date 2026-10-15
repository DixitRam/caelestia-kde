use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::app::{App, ErrorDialog, Screen};
use crate::menu::{self, Entry};
use crate::steps::{self, Phase, STEPS, Status};
use crate::theme;

pub fn draw(frame: &mut Frame, app: &App) {
    frame.render_widget(
        Block::default().style(theme::background_style()),
        frame.area(),
    );
    match app.screen {
        Screen::Welcome => welcome(frame, app),
        Screen::Password => password(frame, app),
        Screen::Theming => theming(frame, app),
        Screen::Configure => configure(frame, app),
        Screen::Review => review(frame, app),
        Screen::Install => install(frame, app),
        Screen::Complete => complete(frame, app),
    }
    if let Some(error) = &app.error {
        dialog(frame, error);
    }
}

fn padded(title: &str) -> Block<'_> {
    Block::bordered()
        .title(format!(" {title} "))
        .border_style(theme::border_style())
}

fn hint(area: Rect) -> Rect {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area)[1]
}

fn centred(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    }
}

fn hint_line(text: &str) -> Paragraph<'_> {
    Paragraph::new(Line::from(Span::styled(
        text.to_string(),
        theme::hint_style(),
    )))
}

fn distro_label() -> String {
    match std::env::var("BASE_DISTRO").as_deref() {
        Ok("arch") => "Arch-based Linux".to_string(),
        Ok("fedora") => "Fedora".to_string(),
        Ok("debian") => "Debian-based Linux".to_string(),
        Ok("") | Err(_) => "unknown".to_string(),
        Ok(other) => other.to_string(),
    }
}

fn welcome(frame: &mut Frame, _app: &App) {
    let area = frame.area();
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    for art in theme::SPLASH {
        lines.push(Line::from(Span::styled(
            art.to_string(),
            Style::default().fg(theme::SPLASH_COLOR),
        )));
    }
    lines.push(Line::from(""));
    lines.push(
        Line::from(Span::styled(theme::AUTHOR, theme::hint_style())).alignment(Alignment::Center),
    );
    lines.push(
        Line::from(Span::styled(theme::CO_AUTHOR, theme::hint_style()))
            .alignment(Alignment::Center),
    );
    lines.push(Line::from(""));
    lines.push(
        Line::from(Span::styled("Caelestia installer", theme::title_style()))
            .alignment(Alignment::Center),
    );
    lines.push(
        Line::from(Span::styled(
            format!("Detected distribution: {}", distro_label()),
            theme::body_style(),
        ))
        .alignment(Alignment::Center),
    );

    let height = lines.len() as u16 + 2;
    let width = 60.min(area.width);
    let body = centred(area, width, height);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).alignment(Alignment::Center),
        body,
    );
    frame.render_widget(hint_line("Enter - continue    Esc - quit"), hint(area));
}

fn password(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let box_area = centred(area, 62.min(area.width), 9);
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Root privileges are required to install packages.",
            theme::heading_style(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Password: ", theme::title_style()),
            Span::styled(
                "*".repeat(app.password.chars().count()),
                theme::body_style(),
            ),
        ]),
        Line::from(""),
    ];
    if let Some(problem) = &app.problem {
        lines.push(Line::from(Span::styled(
            problem.clone(),
            theme::status_style(Status::Failed),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "Your password is never written to disk.",
            theme::hint_style(),
        )));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(padded("PRIVILEGE ESCALATION")),
        box_area,
    );
    frame.render_widget(hint_line("Enter - continue    Esc - cancel"), hint(area));
}

fn theming(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Theme the apps you use with the Caelestia look?",
            theme::heading_style(),
        )),
        Line::from(""),
    ];
    for (index, (label, help)) in [
        (
            "Yes, theme my apps",
            "VSCode/VSCodium, Zed, Spicetify, Discord/Equibop, Todoist, Firefox.",
        ),
        (
            "No, leave them alone",
            "You can still change each one on the next screen.",
        ),
    ]
    .iter()
    .enumerate()
    {
        let style = if app.theming == index {
            theme::selected_style()
        } else {
            theme::hint_style()
        };
        lines.push(Line::from(Span::styled(
            format!("  {} {label}", if app.theming == index { ">" } else { " " }),
            style,
        )));
        if app.theming == index {
            lines.push(Line::from(Span::styled(
                format!("      {help}"),
                theme::body_style(),
            )));
        }
    }
    let box_area = centred(area, 70.min(area.width), lines.len() as u16 + 2);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(padded("OPTIONAL APP THEMING")),
        box_area,
    );
    frame.render_widget(
        hint_line("Up/Down choose    Enter - continue    Esc - back"),
        hint(area),
    );
}

fn configure(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let entries = app.current_entries();
    let title = if app.menu_path.is_empty() {
        "CONFIGURATION"
    } else {
        app.selected_title_or("CONFIGURATION")
    };

    let mut lines: Vec<Line> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let chosen = index == app.cursor;
        let name = entry.title();
        let value = match entry {
            Entry::Toggle { .. } => {
                let on = menu::value(entry, &app.answers) == Some("true");
                format!(
                    " {}",
                    if on {
                        theme::CHECK_ON
                    } else {
                        theme::CHECK_OFF
                    }
                )
            }
            Entry::Choice { .. } => {
                let value = menu::value(entry, &app.answers).unwrap_or_default();
                format!(" {} {value} {}", theme::SELECT_LEFT, theme::SELECT_RIGHT)
            }
            Entry::Submenu { .. } => " ...".to_string(),
            Entry::Action { .. } => String::new(),
        };
        let style = if chosen {
            theme::selected_style()
        } else {
            theme::body_style()
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", if chosen { ">" } else { " " }), style),
            Span::styled(name.to_string(), style),
            Span::styled(value, style),
        ]));
    }

    let help = app.selected().map(Entry::help).unwrap_or_default();
    if !help.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  {help}"),
            theme::hint_style(),
        )));
    }

    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(padded(title))
            .wrap(Wrap { trim: false }),
        body[0],
    );
    frame.render_widget(
        hint_line("Up/Down move    Enter choose    Left/Right change    Esc back"),
        body[1],
    );
}

fn review(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let mut lines: Vec<Line> = Vec::new();
    for phase in Phase::ALL {
        lines.push(Line::from(Span::styled(
            phase.title().to_uppercase(),
            theme::phase_style(),
        )));
        for step in STEPS.iter().filter(|step| step.phase == phase) {
            let skipped = steps::is_skipped(step, &app.answers);
            let status = if skipped {
                Status::Skipped
            } else {
                Status::Pending
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", theme::status_glyph(status)),
                    theme::status_style(status),
                ),
                Span::styled(
                    step.name.to_string(),
                    if skipped {
                        theme::hint_style()
                    } else {
                        theme::body_style()
                    },
                ),
            ]));
        }
        lines.push(Line::from(""));
    }
    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(padded("REVIEW")),
        body[0],
    );
    frame.render_widget(
        hint_line("Enter - install    Esc - back to configuration"),
        body[1],
    );
}

fn install(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(STEPS.len() as u16 + 2),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    let mut steps: Vec<Line> = Vec::new();
    for (index, step) in STEPS.iter().enumerate() {
        let status = app.statuses[index];
        steps.push(Line::from(vec![
            Span::styled(
                format!(" {} ", theme::status_glyph(status)),
                theme::status_style(status),
            ),
            Span::styled(
                step.name.to_string(),
                if status == Status::Running {
                    theme::selected_style()
                } else {
                    theme::body_style()
                },
            ),
        ]));
    }
    frame.render_widget(
        Paragraph::new(Text::from(steps)).block(padded("INSTALLING")),
        body[0],
    );

    let visible = body[1].height.saturating_sub(2) as usize;
    let start = if app.log_follow {
        app.log.len().saturating_sub(visible)
    } else {
        app.log_scroll
            .min(app.log.len().saturating_sub(visible.min(app.log.len())))
    };
    let tail: Vec<Line> = app
        .log
        .iter()
        .skip(start)
        .take(visible.max(1))
        .map(|line| Line::from(Span::styled(line.clone(), theme::hint_style())))
        .collect();
    frame.render_widget(
        Paragraph::new(Text::from(tail)).block(padded("INSTALL LOG")),
        body[1],
    );
    frame.render_widget(
        hint_line("PgUp/PgDn scroll    f follow    Ctrl+C quit"),
        body[2],
    );
}

fn complete(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let (ok, warn, failed, skipped) = app.tally();
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled("Installation finished.", theme::title_style())),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {ok} steps completed"),
            theme::status_style(Status::Ok),
        )),
    ];
    if warn > 0 {
        lines.push(Line::from(Span::styled(
            format!("  {warn} steps reported warnings"),
            theme::status_style(Status::Warn),
        )));
    }
    if failed > 0 {
        lines.push(Line::from(Span::styled(
            format!("  {failed} steps need your attention"),
            theme::status_style(Status::Failed),
        )));
    }
    if skipped > 0 {
        lines.push(Line::from(Span::styled(
            format!("  {skipped} steps were skipped"),
            theme::hint_style(),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Log out and back in so the new session settings apply.",
        theme::body_style(),
    )));
    let box_area = centred(area, 64.min(area.width), lines.len() as u16 + 2);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(padded("COMPLETE")),
        box_area,
    );
    frame.render_widget(hint_line("Enter - finish"), hint(area));
}

fn dialog(frame: &mut Frame, error: &ErrorDialog) {
    let area = frame.area();
    let height =
        (error.detail.len() as u16 + ErrorDialog::CHOICES.len() as u16 + 5).min(area.height);
    let box_area = centred(area, 72.min(area.width), height);
    frame.render_widget(Clear, box_area);

    let mut lines = vec![Line::from(Span::styled(
        format!(" {} failed", error.step),
        theme::status_style(Status::Failed),
    ))];
    lines.push(Line::from(""));
    for detail in &error.detail {
        lines.push(Line::from(Span::styled(
            format!("  {detail}"),
            theme::hint_style(),
        )));
    }
    lines.push(Line::from(""));
    let choices: Vec<Span> = ErrorDialog::CHOICES
        .iter()
        .enumerate()
        .flat_map(|(index, choice)| {
            let style = if index == error.choice {
                theme::selected_style()
            } else {
                theme::hint_style()
            };
            [Span::styled(
                format!(
                    "  {} {choice}  ",
                    if index == error.choice { ">" } else { " " }
                ),
                style,
            )]
        })
        .collect();
    lines.push(Line::from(choices));

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(
                Block::bordered()
                    .title(" STEP FAILED ")
                    .border_style(theme::status_style(Status::Failed)),
            )
            .wrap(Wrap { trim: true }),
        box_area,
    );
}

impl App {
    fn selected_title_or(&self, fallback: &'static str) -> &'static str {
        let mut entries: &'static [Entry] = &menu::ROOT;
        let mut title = fallback;
        for index in &self.menu_path {
            match entries.get(*index) {
                Some(Entry::Submenu {
                    items, title: name, ..
                }) => {
                    title = name;
                    entries = items;
                }
                _ => break,
            }
        }
        title
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;

    fn render(app: &App, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).expect("a test terminal");
        terminal
            .draw(|frame| draw(frame, app))
            .expect("a drawn frame");
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }

    fn app() -> App {
        App::new(
            std::path::PathBuf::from("/tmp/caelestia"),
            std::path::PathBuf::from("/tmp/install.log"),
        )
    }

    #[test]
    fn the_welcome_screen_shows_the_splash_and_the_way_on() {
        let text = render(&app(), 80, 24);
        assert!(text.contains("Caelestia installer"), "{text}");
        assert!(text.contains("_____"), "the splash art is rendered: {text}");
        assert!(text.contains("Enter - continue"), "{text}");
    }

    #[test]
    fn the_welcome_screen_survives_a_narrow_terminal() {
        let text = render(&app(), 24, 8);
        assert!(!text.is_empty());
    }

    #[test]
    fn the_password_screen_never_shows_the_password() {
        let mut app = app();
        app.screen = Screen::Password;
        app.password = "hunter2".to_string();
        let text = render(&app, 80, 24);
        assert!(!text.contains("hunter2"), "{text}");
        assert!(text.contains("*******"), "{text}");
        assert!(text.contains("never written to disk"), "{text}");
    }

    #[test]
    fn the_configure_screen_shows_the_values_the_tree_holds() {
        let mut app = app();
        app.screen = Screen::Configure;
        app.cursor = 0;
        let text = render(&app, 80, 30);
        assert!(text.contains("Packages"), "{text}");
        assert!(text.contains("Review installation"), "{text}");

        app.choose();
        let text = render(&app, 80, 30);
        assert!(
            text.contains("Fish shell"),
            "the submenu's own entries render: {text}"
        );
        assert!(text.contains("[x]"), "a seeded default is ticked: {text}");
    }

    #[test]
    fn the_review_screen_lists_phases_and_marks_skipped_steps() {
        let mut app = app();
        app.screen = Screen::Review;
        let text = render(&app, 80, 40);
        assert!(text.contains("PREPARE"), "{text}");
        assert!(text.contains("Update system"), "{text}");
        assert!(text.contains("[SKIP]"), "{text}");
    }

    #[test]
    fn the_install_screen_shows_every_step_and_the_log() {
        let mut app = app();
        app.screen = Screen::Install;
        app.statuses[0] = Status::Ok;
        app.statuses[1] = Status::Running;
        app.log = vec![
            "reading packages".to_string(),
            "[WARN] slow mirror".to_string(),
        ];
        let text = render(&app, 100, 40);
        assert!(text.contains("[OK]"), "{text}");
        assert!(text.contains("[>]"), "{text}");
        assert!(text.contains("slow mirror"), "{text}");
    }

    #[test]
    fn the_summary_counts_what_happened() {
        let mut app = app();
        app.screen = Screen::Complete;
        app.statuses[0] = Status::Ok;
        app.statuses[1] = Status::Warn;
        app.statuses[2] = Status::Ignored;
        app.statuses[3] = Status::Skipped;
        let text = render(&app, 80, 30);
        assert!(text.contains("1 steps completed"), "{text}");
        assert!(text.contains("1 steps reported warnings"), "{text}");
        assert!(text.contains("1 steps need your attention"), "{text}");
        assert!(text.contains("Log out and back in"), "{text}");
    }

    #[test]
    fn the_error_dialog_offers_retry_ignore_and_exit() {
        let mut app = app();
        app.screen = Screen::Install;
        app.error = Some(ErrorDialog::new("Install packages", &["boom".to_string()]));
        let text = render(&app, 100, 40);
        assert!(text.contains("STEP FAILED"), "{text}");
        assert!(text.contains("Retry"), "{text}");
        assert!(text.contains("Ignore"), "{text}");
        assert!(text.contains("Exit"), "{text}");
        assert!(text.contains("boom"), "{text}");
    }
}
