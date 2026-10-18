// The screens. Each draws into the box the chrome hands out.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{Action, App, AuthMethod, Confirm, FailureCard, Overlay, Stage};
use crate::glyphs::Glyphs;
use crate::logline::{Kind, LogLine};
use crate::menu::{self, Entry};
use crate::steps::{STEPS, Status};
use crate::theme::{self, Role, Theme};
use crate::ui;

/// The width of the value column on the options screen.
const VALUE_COLUMN: usize = 12;

/// How wide the name column ever gets.
const NAME_CELL: usize = 52;

/// How far a row that belongs to the row above it is moved in.
const CHILD_INDENT: u16 = 2;

/// The label column of the failure card and the summary.
const LABEL_COLUMN: usize = 8;

pub fn draw(frame: &mut Frame, app: &mut App) {
    app.area = frame.area();
    let header = app.header();
    let counter = app.counter();
    let keys = app.keys();
    let Some(chrome) = ui::chrome(
        frame,
        &app.theme,
        app.glyphs,
        &header,
        &keys,
        counter.as_deref(),
    ) else {
        return;
    };
    let body = chrome.body;

    match app.stage {
        Stage::Home => home(frame, app, body),
        Stage::Detect => detect(frame, app, body),
        Stage::Privilege => privilege(frame, app, body),
        Stage::Auth => auth(frame, app, body),
        Stage::Options => options(frame, app, body),
        Stage::Run => run(frame, app),
        Stage::Done => done(frame, app, body),
    }

    match app.overlay {
        Some(Overlay::Keys) => keys_overlay(frame, app),
        Some(Overlay::Log) => log_overlay(frame, app),
        Some(Overlay::Failure) => failure_overlay(frame, app),
        Some(Overlay::Confirm) => confirm_overlay(frame, app),
        None => {}
    }
}

// Home ------------------------------------------------------------------------------

fn home(frame: &mut Frame, app: &App, body: Rect) {
    let theme = &app.theme;
    let mark = banner(body);
    let named = mark.is_none();
    let credits = credits(app);
    let rest = notice(app);

    let identity_lines = identity(app, named);
    let gap = column_gap(body.width);
    let panel_width = ui::panel_width(actions_width(app), body.width);
    let mark_rows = if named {
        0
    } else {
        theme::SPLASH.len() as u16 + 1
    };
    // The left column is as wide as the mark where the whole mark fits beside the panel, and only
    // as wide as the words under it where it does not. A terminal in between - which is most of
    // them - still gets the panel beside the prose rather than a column of nothing on the right.
    let mark_width = theme::SPLASH[0].chars().count() as u16;
    let text_width = identity_lines
        .iter()
        .chain(credits.iter())
        .map(|line| line.width() as u16)
        .max()
        .unwrap_or(0);
    let left_width = if body.width >= mark_width + gap + panel_width {
        HOME_LEFT.max(mark_width)
    } else {
        text_width
    };
    let two_columns = body.width >= left_width + gap + panel_width;
    let left_rows = identity_lines.len() + 2 + credits.len();

    // One composition: the mark, and under it either two columns or one. Sized together and
    // centred together, because a small block pinned to the corner of a large frame reads as a
    // screen that stopped drawing rather than as a layout.
    let (width, height) = if two_columns {
        (
            left_width + gap + panel_width,
            mark_rows + left_rows.max(ACTIONS_PANEL as usize + rest.len()) as u16,
        )
    } else {
        (
            ui::PROSE_WIDTH.min(body.width),
            mark_rows
                + identity_lines.len() as u16
                + Action::ALL.len() as u16
                + rest.len() as u16
                + credits.len() as u16
                + 3,
        )
    };
    let block = ui::middle(body, width, height);

    let below = match mark {
        Some(area) => {
            ui::prose(
                frame,
                Rect {
                    height: area.height,
                    ..block
                },
                mark_lines(app),
                theme.accent(),
            );
            Rect {
                y: block.y + mark_rows,
                height: block.height.saturating_sub(mark_rows),
                ..block
            }
        }
        None => block,
    };

    if two_columns {
        // What the program is on the left, what it can do on the right.
        let left = Rect {
            width: left_width.min(below.width),
            ..below
        };
        let right = Rect {
            x: below.x + left.width + gap,
            width: below.width.saturating_sub(left.width + gap),
            ..below
        };
        let mut lines = identity_lines.clone();
        lines.push(blank());
        lines.extend(credits.clone());
        ui::prose(frame, left, lines, theme.body());
        actions_panel(frame, app, right);
        return;
    }

    // One column: the same content, in the order it is read.
    let mut lines = identity_lines;
    lines.extend(actions(app, below.width));
    lines.extend(rest);
    lines.push(blank());
    lines.extend(credits);
    ui::prose(frame, below, lines, theme.body());
}

/// The left column of the home screen, and the least air beside it.
const HOME_LEFT: u16 = 54;
const HOME_GAP: u16 = 4;

/// How much air goes between two columns. It grows with the frame, so a composition on a wide
/// screen is spread out rather than sitting in the corner as a small block.
fn column_gap(width: u16) -> u16 {
    HOME_GAP.max((width / 20).min(12))
}

/// The action list in its panel, with anything the machine has to say under it.
fn actions_panel(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let width = ui::panel_width(actions_width(app), area.width);
    let inner = ui::section(
        frame,
        ui::panel_rect(area, width, Action::ALL.len() as u16),
        theme,
        app.glyphs,
        None,
        Role::Rule,
    );
    ui::prose(frame, inner, actions(app, inner.width), theme.body());
    let rest = notice(app);
    if !rest.is_empty() {
        ui::prose(
            frame,
            ui::dropped(area, ACTIONS_PANEL + 1),
            rest,
            theme.body(),
        );
    }
}

/// The action panel: three rows and its borders.
const ACTIONS_PANEL: u16 = 5;

/// Below this the screen would be the mark and nothing else, so it is dropped rather than clipped.
const MARK_MIN_HEIGHT: u16 = 22;

/// Where the mark goes, or `None` where it does not fit whole.
fn banner(body: Rect) -> Option<Rect> {
    let width = theme::SPLASH[0].chars().count() as u16;
    if body.width < width || body.height < MARK_MIN_HEIGHT {
        return None;
    }
    Some(Rect {
        height: theme::SPLASH.len() as u16,
        ..body
    })
}

/// The mark, one row per line.
fn mark_lines(app: &App) -> Vec<Line<'static>> {
    theme::SPLASH
        .iter()
        .map(|row| Line::from(Span::styled(row.to_string(), app.theme.accent())))
        .collect()
}

/// The name, where the mark has not already said it, and what this machine turned out to be.
fn identity(app: &App, named: bool) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let mut lines: Vec<Line<'static>> = Vec::new();
    if named {
        lines.push(Line::from(Span::styled(
            theme::NAME.to_string(),
            theme.title(),
        )));
    }
    lines.push(Line::from(Span::styled(
        theme::TAGLINE.to_string(),
        theme.muted(),
    )));
    lines.push(blank());
    lines.push(state_line(app));
    lines
}

/// What was found, in the line the actions beside it depend on.
fn state_line(app: &App) -> Line<'static> {
    let theme = &app.theme;
    let installed = app.installed();
    Line::from(Span::styled(
        if installed {
            "Caelestia is installed"
        } else {
            "nothing installed yet"
        },
        if installed {
            theme.body()
        } else {
            theme.muted()
        },
    ))
}

/// The three actions. The label is the whole row: what each one does is the verb, and anything
/// that needs saying about it belongs on the confirmation that asks for it. A row that is not on
/// offer carries the reason it is grey.
fn actions(app: &App, width: u16) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let column = name_column();
    Action::ALL
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let enabled = app.action_enabled(*action);
            let name = format!("{:<column$}", action.title(), column = column);
            if !enabled {
                return Line::from(vec![
                    Span::styled("  ", theme.muted()),
                    Span::styled(name, theme.muted()),
                    Span::styled(format!("  {}", Action::unavailable(*action)), theme.muted()),
                ]);
            }
            if index == app.home {
                return ui::cursor_row(
                    theme,
                    glyphs,
                    &format!("{} {name}", glyphs.cursor()),
                    width,
                );
            }
            Line::from(vec![
                Span::styled("  ", theme.body()),
                Span::styled(name, theme.body()),
            ])
        })
        .collect()
}

/// The column the action names are padded to, so the reasons line up under each other.
fn name_column() -> usize {
    Action::ALL
        .iter()
        .map(|action| action.title().len())
        .max()
        .unwrap_or(0)
}

/// How wide the action list has to be.
fn actions_width(app: &App) -> u16 {
    let column = name_column();
    Action::ALL
        .iter()
        .map(|action| {
            if app.action_enabled(*action) {
                2 + column
            } else {
                2 + column + 2 + Action::unavailable(*action).chars().count()
            }
        })
        .max()
        .unwrap_or(0) as u16
}

fn credits(app: &App) -> Vec<Line<'static>> {
    let theme = &app.theme;
    vec![
        Line::from(Span::styled(theme::AUTHOR.to_string(), theme.muted())),
        Line::from(Span::styled(theme::CO_AUTHOR.to_string(), theme.muted())),
    ]
}

// Privilege -------------------------------------------------------------------------

fn privilege(frame: &mut Frame, app: &App, body: Rect) {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    // One measure for the whole card.
    const MEASURE: u16 = 56;
    let mut lines = if app.auth == Some(AuthMethod::Fingerprint) {
        fingerprint_card(app)
    } else {
        password_card(app)
    };

    lines.push(blank());
    // The wait belongs where the answer will appear, so it reads as work rather than as a freeze.
    if app.checking.is_some() {
        let waiting = if app.auth == Some(AuthMethod::Fingerprint) {
            "Waiting for your finger..."
        } else {
            "Checking your password..."
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", ui::running_glyph(app)), theme.accent()),
            Span::styled(waiting.to_string(), theme.muted()),
        ]));
    } else if let Some(problem) = &app.problem {
        for line in wrap(problem, MEASURE as usize) {
            lines.push(Line::from(Span::styled(line, theme.error_text())));
        }
    }
    if app.attempts > 0 {
        lines.push(Line::from(Span::styled(
            format!("attempt {}", app.attempts + 1),
            theme.warn_text(),
        )));
    }

    ui::card(
        frame,
        body,
        theme,
        glyphs,
        Some("root privileges"),
        MEASURE,
        lines,
    );
}

/// The reader's card: what to do with it, and nothing else. What the reader is for is the name of
/// the route, and the reader answers when the finger arrives rather than when a key does.
fn fingerprint_card(app: &App) -> Vec<Line<'static>> {
    vec![Line::from(Span::styled(
        AuthMethod::Fingerprint.title().to_string(),
        app.theme.body(),
    ))]
}

fn password_card(app: &App) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    const MEASURE: u16 = 56;
    // Short enough to survive the narrowest terminal the installer draws in.
    let mut lines = vec![Line::from(Span::styled(
        "Your password is sent to sudo and is not saved.".to_string(),
        theme.muted(),
    ))];

    let mut field = vec![
        Span::styled(format!("{:<10}", "password"), theme.muted()),
        Span::styled(
            glyphs
                .mask()
                .repeat(app.password.chars().count().min(MEASURE as usize)),
            theme.body(),
        ),
    ];
    if app.password.is_empty() {
        field.push(Span::styled("(type, then ".to_string(), theme.muted()));
        field.push(Span::styled(glyphs.enter().to_string(), theme.accent()));
        field.push(Span::styled(")".to_string(), theme.muted()));
    }
    lines.push(Line::from(field));
    lines
}

// This machine -----------------------------------------------------------------------

/// What was found before anything is decided.
fn detect(frame: &mut Frame, app: &App, body: Rect) {
    let theme = &app.theme;

    // Two columns where there is room: the findings on the left, what they mean on the right.
    if ui::rail(body, 44).is_some() {
        let rows = findings(app);
        let label_width = label_column(&rows);
        let left_width = ui::panel_width(finding_width(&rows, label_width), body.width / 2);

        let mut narrative = verdict_body(app, ui::PROSE_WIDTH);
        let right_width = ui::panel_width(
            narrative.iter().map(|line| line.width()).max().unwrap_or(0) as u16,
            body.width / 2,
        );
        // Added once the panel's width is known, so the bar fills it exactly the way Home's
        // selected action does rather than stopping at the end of its own text.
        narrative.push(blank());
        narrative.push(ui::cursor_row(
            theme,
            app.glyphs,
            &format!("{} continue", app.glyphs.cursor()),
            right_width.saturating_sub(2),
        ));

        // The pair is centred as one block. Pinned to the left edge, two small panels in a wide
        // frame read as a drawing that failed part way through.
        const GAP: u16 = 4;
        let total = left_width + GAP + right_width;
        let x = body.x + body.width.saturating_sub(total) / 2;
        let left_height = rows.len() as u16 + 2;
        let right_height = narrative.len() as u16 + 2;
        let height = left_height.max(right_height);
        let y = body.y + body.height.saturating_sub(height) / 2;

        let inner = ui::section(
            frame,
            Rect {
                x,
                y,
                width: left_width,
                height: left_height,
            },
            theme,
            app.glyphs,
            Some("this machine"),
            Role::Rule,
        );
        ui::prose(
            frame,
            inner,
            finding_lines(&rows, label_width, theme),
            theme.body(),
        );

        // The verdict is prose, so it reads in a measure. Its border carries the answer, so the
        // panel needs no title: the headline inside says it.
        let border = verdict(app);
        let inner = ui::section(
            frame,
            Rect {
                x: x + left_width + GAP,
                y,
                width: right_width,
                height: right_height,
            },
            theme,
            app.glyphs,
            None,
            border,
        );
        ui::prose(frame, inner, narrative, theme.body());
        return;
    }

    let rows = findings(app);
    let label_width = label_column(&rows);
    let headline = headline(app);
    let tail = reasons(app, body.width);
    let explanation = installed_explanation(app, body.width);

    let mut lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled("This machine".to_string(), theme.heading())),
        blank(),
    ];

    // Reading order, budgeted by importance: the fact that changes the decision and the reason it
    // cannot go on survive a short terminal; the rows and the explanation give way.
    let mut room = (body.height as usize)
        .saturating_sub(lines.len() + 1 + tail.len())
        .saturating_sub(explanation.len());
    let row_count = rows.len();
    let show_rows = row_count < room;
    if show_rows {
        room = room.saturating_sub(row_count + 1);
    }
    let show_explanation = !explanation.is_empty() && explanation.len() <= room;

    if show_rows {
        lines.extend(finding_lines(&rows, label_width, theme));
    }
    lines.push(headline);
    if show_explanation {
        for line in explanation {
            lines.push(Line::from(Span::styled(format!(" {line}"), theme.muted())));
        }
    }
    lines.extend(tail);

    lines.extend(notice(app));
    lines.push(blank());
    lines.push(ui::cursor_row(
        &app.theme,
        app.glyphs,
        &format!("{} continue", app.glyphs.cursor()),
        body.width.min(ui::PROSE_WIDTH),
    ));
    ui::prose_middle(frame, body, lines, theme.body());
}

/// The findings: what was looked for, what was found, and whether the install depends on it.
fn findings(app: &App) -> Vec<(&'static str, String, bool)> {
    let found = &app.detection;
    vec![
        (
            "distribution",
            found.distro_label().to_string(),
            found.distro != crate::distro::Distro::Unknown,
        ),
        (
            "install layout",
            format!("{} install", found.kind_label()),
            true,
        ),
        (
            "package manager",
            found
                .package_manager
                .map(str::to_string)
                .unwrap_or_else(|| "not found".to_string()),
            found.package_manager.is_some(),
        ),
        (
            "quickshell",
            if found.quickshell {
                "found".to_string()
            } else {
                "not installed yet".to_string()
            },
            true,
        ),
        (
            "git",
            if found.git {
                "found".to_string()
            } else {
                "missing".to_string()
            },
            found.git,
        ),
    ]
}

/// The label column is as wide as the labels in it.
fn label_column(rows: &[(&'static str, String, bool)]) -> usize {
    rows.iter()
        .map(|(label, _, _)| label.chars().count())
        .max()
        .unwrap_or(0)
}

fn finding_lines(
    rows: &[(&'static str, String, bool)],
    label_width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    rows.iter()
        .map(|(label, value, ok)| {
            let value_style = if *ok { theme.body() } else { theme.warn_text() };
            Line::from(vec![
                Span::styled(
                    format!(" {label:<label_width$}  "),
                    if *ok {
                        theme.muted()
                    } else {
                        theme.warn_text()
                    },
                ),
                Span::styled(value.clone(), value_style),
            ])
        })
        .collect()
}

/// How wide the findings panel has to be to hold its longest row.
fn finding_width(rows: &[(&'static str, String, bool)], label_width: usize) -> u16 {
    rows.iter()
        .map(|(_, value, _)| label_width + value.chars().count() + 3)
        .max()
        .unwrap_or(0) as u16
}

/// The fact this screen turns on, in one line.
fn headline(app: &App) -> Line<'static> {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    if app.detection.installed {
        Line::from(vec![
            Span::styled(
                format!("{} ", glyphs.status(Status::Failed)),
                theme.error_text(),
            ),
            Span::styled("Caelestia is already installed.", theme.error_text()),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                format!("{} ", glyphs.status(Status::Ok)),
                theme.status(Status::Ok),
            ),
            Span::styled("Nothing is installed in the way.", theme.body()),
        ])
    }
}

/// Why the screen will not go on: a missing precondition, or the reason a guard sent the user back.
fn reasons(app: &App, width: u16) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let mut lines: Vec<Line<'static>> = Vec::new();
    for blocker in app.detection.blockers() {
        lines.push(blank());
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", glyphs.status(Status::Failed)),
                theme.error_text(),
            ),
            Span::styled(blocker.to_string(), theme.error_text()),
        ]));
    }
    if let Some(problem) = &app.problem {
        lines.push(blank());
        for line in wrap(problem, (width as usize).saturating_sub(4)) {
            lines.push(Line::from(Span::styled(
                format!(" {line}"),
                theme.error_text(),
            )));
        }
    }
    lines
}

/// What an existing install means for what happens next.
fn installed_explanation(app: &App, width: u16) -> Vec<String> {
    if !app.detection.installed {
        return Vec::new();
    }
    wrap(
        "Continuing installs over it: the shell configuration and the deployed files are \
         replaced. Your own configs are backed up first.",
        (width as usize).saturating_sub(4),
    )
}

/// Everything the verdict panel says: the fact, what it means, and why the screen will not go on.
fn verdict_body(app: &App, width: u16) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let mut lines = vec![headline(app)];
    let explanation = installed_explanation(app, width);
    if !explanation.is_empty() {
        lines.push(blank());
        lines.extend(
            explanation
                .into_iter()
                .map(|line| Line::from(Span::styled(format!(" {line}"), theme.muted()))),
        );
    }
    lines.extend(reasons(app, width));
    lines
}

/// What the findings mean, as the role the verdict's border carries. Ordinary "ready" stays
/// neutral, since color is spent on what is wrong.
fn verdict(app: &App) -> Role {
    let found = &app.detection;
    if !found.blockers().is_empty() {
        Role::Error
    } else if found.installed {
        Role::Warn
    } else {
        Role::Rule
    }
}

// Auth -------------------------------------------------------------------------------

fn auth(frame: &mut Frame, app: &App, body: Rect) {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    // One measure for the whole card, so the prose and the rows under it break in the same place.
    const MEASURE: u16 = 68;
    let mut lines: Vec<Line<'static>> = Vec::new();

    for (index, method) in app.auth_methods().iter().enumerate() {
        let chosen = index == app.cursor;
        // A selected row looks the same on every screen: the pointer on a filled bar, which is
        // what Home's actions have always used. Anything else and the user has to learn the
        // highlight again per screen.
        if chosen {
            lines.push(ui::cursor_row(
                theme,
                glyphs,
                &format!("{} {}", glyphs.cursor(), method.title()),
                MEASURE,
            ));
        } else {
            lines.push(Line::from(vec![
                Span::styled("   ".to_string(), theme.body()),
                Span::styled(method.title().to_string(), theme.body()),
            ]));
        }
    }

    if let Some(problem) = &app.problem {
        lines.push(blank());
        for line in wrap(problem, MEASURE as usize) {
            lines.push(Line::from(Span::styled(line, theme.error_text())));
        }
    }
    lines.extend(notice(app));

    ui::card(
        frame,
        body,
        theme,
        glyphs,
        Some("root privileges"),
        MEASURE,
        lines,
    );
}

// Options ---------------------------------------------------------------------------

fn options(frame: &mut Frame, app: &App, body: Rect) {
    let theme = &app.theme;

    // Two columns: the questions down the left as a map of the setup, the question on the right.
    let rail_width = ui::panel_width(question_rail_width(), body.width);
    let gap = column_gap(body.width);
    let pane_width = PANE_WIDTH.min(body.width.saturating_sub(rail_width + gap));
    if pane_width >= PANE_MIN {
        // The pane is as wide as its widest row needs and no wider, and the pair is centred as one
        // block: a question pinned to the left of a wide frame leaves a screen of nothing beside it.
        let listed = option_rows(app, pane_width).len() as u16 + 2;
        let width = rail_width + gap + pane_width;
        // The heading, the list, and the rows the help and anything else the screen has to say
        // need under it: a pane clipped to the list is a pane whose help is never drawn.
        let height = (listed + HELP_ROWS + 2).max(question_rail(app).len() as u16 + 2);
        let block = ui::middle(body, width, height);

        let rail = question_rail(app);
        let left = Rect {
            width: rail_width,
            ..block
        };
        let inner = ui::section(
            frame,
            ui::panel_rect(left, rail_width, rail.len() as u16),
            theme,
            app.glyphs,
            None,
            Role::Rule,
        );
        ui::prose(frame, inner, rail, theme.body());

        let right = Rect {
            x: block.x + rail_width + gap,
            width: pane_width,
            ..block
        };
        question_pane(frame, app, right);
        return;
    }

    question_pane(
        frame,
        app,
        ui::middle(body, body.width.min(PANE_WIDE), body.height),
    );
}

/// The room the question pane asks for, and the least it will be drawn at before the map is given
/// up instead. The heading and the help are rows the pane keeps whatever is in the list.
const PANE_WIDTH: u16 = 74;
const PANE_WIDE: u16 = 96;
const PANE_MIN: u16 = 44;
const HELP_ROWS: u16 = 2;

/// The map of the setup: every question, and which one is on screen. How far along the wizard is,
/// is on the bar at the bottom of the frame, where it is for every screen.
fn question_rail(app: &App) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let mut lines: Vec<Line<'static>> = Vec::new();
    for index in 0..menu::question_count() {
        let title = menu::question(index);
        let here = index == app.question;
        let name_style = if here { theme.selected() } else { theme.body() };
        let changed = if app.question_changed(index) {
            format!("  {}", glyphs.changed())
        } else {
            String::new()
        };
        lines.push(Line::from(vec![
            // The rail says where you are, not what Enter will act on: the pointer belongs to the
            // selected row alone, so this marks the position with weight and the accent instead.
            Span::styled(
                if here { "  · " } else { "    " },
                if here { theme.accent() } else { theme.muted() },
            ),
            Span::styled(title.to_string(), name_style),
            Span::styled(changed, theme.accent()),
        ]));
    }
    lines
}

/// How wide the map has to be for its longest name plus its change marker.
fn question_rail_width() -> u16 {
    let mut width = 0;
    for index in 0..menu::question_count() {
        let title = menu::question(index);
        width = width.max(2 + title.len() + 2 + 2);
    }
    width as u16
}

/// One question and its answers.
fn question_pane(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    // The answers, then the row that goes on, which is the last thing in the list so the arrows
    // reach it the way they reach everything else. The rows get exactly the lines they need: a row
    // that is not drawn is a row nobody can answer, so the help text gives way first.
    let mut listed = option_rows(app, area.width);
    listed.push(blank());
    listed.push(continue_row(app));
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(listed.len() as u16),
        Constraint::Min(0),
    ])
    .areas::<3>(area);

    // What is being asked, on the one line, with the fact that it has been edited beside it: the
    // rail names the question too, but only on a terminal wide enough to hold it.
    let mut heading_spans = vec![Span::styled(
        menu::question(app.question).to_string(),
        theme.heading(),
    )];
    if app.question_changed(app.question) {
        heading_spans.push(Span::styled(
            format!("  {} changed", app.glyphs.changed()),
            theme.accent(),
        ));
    }
    ui::prose(
        frame,
        rows[0],
        vec![Line::from(heading_spans)],
        theme.body(),
    );

    ui::prose(frame, rows[1], listed, theme.body());

    let mut help_lines: Vec<Line<'static>> = Vec::new();
    if let Some(text) = app.selected().and_then(Entry::help) {
        for line in wrap(text, (rows[2].width as usize).saturating_sub(2)) {
            help_lines.push(Line::from(Span::styled(line, theme.muted())));
        }
    }
    help_lines.extend(notice(app));
    ui::prose(frame, rows[2], help_lines, theme.body());
}

/// The last row of a question: the way on, drawn like every other row so the keys can reach it.
fn continue_row(app: &App) -> Line<'static> {
    let theme = &app.theme;
    let chosen = app.on_continue();
    let style = if chosen {
        theme.selected()
    } else {
        theme.body()
    };
    Line::from(vec![
        Span::styled(
            format!(" {} ", if chosen { app.glyphs.cursor() } else { " " }),
            style,
        ),
        Span::styled("  continue".to_string(), style),
    ])
}

/// One row per option: cursor, changed marker, name, then the value in its own column.
pub(crate) fn option_rows(app: &App, width: u16) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    // The name column takes what is left, up to a point; past that it is margin.
    let name_width = (width as usize)
        .saturating_sub(VALUE_COLUMN + 5)
        .clamp(8, NAME_CELL);
    let entries = app.current_entries();
    if entries.is_empty() {
        return vec![Line::from(Span::styled(
            "Nothing on this machine to theme.".to_string(),
            theme.muted(),
        ))];
    }
    entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let chosen = index == app.cursor;
            let changed = if app.is_changed(entry) {
                format!("{} ", glyphs.changed())
            } else {
                "  ".to_string()
            };
            // A row that only exists under another one is drawn under it: the indent is what says
            // that "Login screen style" belongs to "Login screen" rather than beside it.
            let indent = if menu::parent_of(entry).is_some() {
                CHILD_INDENT as usize
            } else {
                0
            };
            let room = name_width.saturating_sub(indent);
            let name = format!(
                "{:indent$}{:<room$}",
                "",
                ui::fit(entry.title(), room, glyphs.ellipsis()),
                indent = indent,
                room = room
            );
            let (value, value_style) = value_cell(entry, app);
            let value = format!("{:>width$}", value, width = VALUE_COLUMN);
            // The pointer and the weight, which is how DMS marks the row the keys act on. No
            // background: the only filled surface in the program is the footer.
            let pointer = format!(" {} ", if chosen { glyphs.cursor() } else { " " });
            let label = if chosen {
                theme.selected()
            } else {
                theme.body()
            };

            Line::from(vec![
                Span::styled(
                    pointer,
                    if chosen {
                        theme.selected()
                    } else {
                        theme.body()
                    },
                ),
                Span::styled(changed, theme.accent()),
                Span::styled(name, label),
                Span::styled(value, value_style),
            ])
        })
        .collect()
}

/// What the row will do, with the colour of its consequence. An app row has three states rather
/// than two, because "on" means "install it" and the answer depends on whether it is already
/// there; DMS draws the same three with the same markers.
fn value_cell(entry: &'static Entry, app: &App) -> (String, Style) {
    let glyphs = app.glyphs;
    let theme = &app.theme;
    match entry {
        // A bulk row is a command, not a state: it acts on the rows below it, and giving it a
        // "skip" of its own reads as if it were one of them.
        Entry::Preset { .. } => (String::new(), theme.muted()),
        Entry::Toggle { id, .. } => {
            let on = menu::value(entry, &app.answers) == Some("true");
            if !on {
                return (format!("{} skip", glyphs.cross()), theme.muted());
            }
            if app.apps.has(id) {
                (
                    format!("{} installed", glyphs.tick()),
                    theme.status(Status::Ok),
                )
            } else if app.apps.is_app_row(id) {
                (
                    format!("{} will install", glyphs.circle()),
                    theme.status(Status::Warn),
                )
            } else {
                (format!("{} on", glyphs.tick()), theme.status(Status::Ok))
            }
        }
        Entry::Choice { .. } => (
            format!(
                "{} {} {}",
                glyphs.open(),
                menu::value(entry, &app.answers).unwrap_or_default(),
                glyphs.close()
            ),
            theme.muted(),
        ),
        Entry::Submenu { .. } => (glyphs.submenu().to_string(), theme.muted()),
    }
}

// Run -------------------------------------------------------------------------------

fn run(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let layout = app.run_layout();

    // The step it is on, how far along, and anything that did not go cleanly: three lines, the way
    // DMS draws an install. Everything else is the log, which is the only thing with something to
    // read; the phases and their step counts were the installer's business, not the reader's.
    ui::prose(frame, layout.step, vec![step_line(app)], theme.body());
    ui::prose(
        frame,
        layout.bar,
        vec![bar_line(app, layout.bar.width)],
        theme.body(),
    );
    ui::prose(frame, layout.note, vec![note_line(app)], theme.body());
    log_pane(frame, app, layout.log, app.log_rows());
}

/// What is happening now: the spinner and the step's own name.
fn step_line(app: &App) -> Line<'static> {
    let theme = &app.theme;
    let running = app.running_step();
    let index = running.or_else(|| app.next_step());
    let Some(index) = index else {
        return Line::from(Span::styled(
            format!("{} Finishing up", app.glyphs.status(Status::Ok)),
            theme.status(Status::Ok),
        ));
    };
    let style = if running.is_some() {
        theme.selected()
    } else {
        theme.body()
    };
    let glyph = if running.is_some() {
        ui::running_glyph(app)
    } else {
        app.glyphs.status(Status::Pending)
    };
    Line::from(vec![
        ui::gutter(glyph, style, app.glyphs),
        Span::styled(STEPS[index].name.to_string(), style),
    ])
}

/// The one bar on the screen, with how far along and how long it has taken.
fn bar_line(app: &App, width: u16) -> Line<'static> {
    let done = app.finished();
    let total = STEPS.len();
    let percent = done
        .saturating_mul(100)
        .checked_div(total)
        .unwrap_or_default();
    let elapsed = app.total_elapsed().as_secs();
    // A run that has only just started has no time to report, and "0:00" beside the bar reads as
    // a stopwatch that is not moving.
    let caption = if elapsed == 0 {
        format!("{percent:>3}%")
    } else {
        format!("{percent:>3}%   {}", ui::elapsed(elapsed))
    };
    ui::progress(done, total, width, &caption, &app.theme, app.glyphs)
}

/// What did not go cleanly, on the line under the bar, so that a warning is seen without opening
/// the log. Nothing here means nothing went wrong, and the line stays empty.
fn note_line(app: &App) -> Line<'static> {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let counts = app.tally();
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (count, status, one, many) in [
        (counts.failed, Status::Failed, "failure", "failures"),
        (counts.warn, Status::Warn, "warning", "warnings"),
        (counts.ignored, Status::Ignored, "ignored", "ignored"),
    ] {
        if count == 0 {
            continue;
        }
        if !spans.is_empty() {
            spans.push(Span::styled("   ".to_string(), theme.muted()));
        }
        spans.push(ui::gutter(
            glyphs.status(status),
            theme.status(status),
            glyphs,
        ));
        spans.push(Span::styled(
            ui::plural(count, one, many),
            theme.status(status),
        ));
    }
    Line::from(spans)
}

/// A log pane: a heading row, the lines, then a scrollbar. The run screen and the full-screen log
/// draw the same thing at different sizes.
fn log_pane(frame: &mut Frame, app: &App, area: Rect, rows: usize) {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    if area.height < 2 || area.width < 4 {
        return;
    }

    let heading = Rect { height: 1, ..area };
    let body = Rect {
        y: area.y + 1,
        height: area.height - 1,
        ..area
    };
    // The log sits on a raised block of its own.
    frame.render_widget(Block::default().style(theme.panel()), area);
    let (start, rows) = app.log_window_for(rows);
    let total = app.visible_log().len();
    let overflowing = total > rows;

    let mut badge = vec![Span::styled(" install log".to_string(), theme.muted())];
    badge.push(Span::styled("  ·  ".to_string(), theme.muted()));
    badge.push(Span::styled(
        if overflowing {
            format!("{}-{} of {total}", start + 1, (start + rows).min(total))
        } else {
            ui::plural(total, "line", "lines")
        },
        theme.muted(),
    ));
    if app.log_loud_only {
        badge.push(Span::styled("  ·  ".to_string(), theme.muted()));
        badge.push(Span::styled(
            "warnings and errors only".to_string(),
            theme.warn_text(),
        ));
    }
    ui::prose(frame, heading, vec![Line::from(badge)], theme.body());

    let follow_width = 7.min(area.width);
    let follow = Rect {
        x: area.x + area.width - follow_width,
        width: follow_width,
        ..heading
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            if app.log_follow { "FOLLOW" } else { "PAUSED" }.to_string(),
            if app.log_follow {
                theme.selected()
            } else {
                theme.warn_text()
            },
        )))
        .alignment(Alignment::Right),
        follow,
    );

    // The scrollbar column is only spent when there is something to scroll.
    let (text_area, bar_area) = if overflowing {
        (
            Rect {
                width: body.width - 1,
                ..body
            },
            Some(Rect {
                x: body.x + body.width - 1,
                width: 1,
                ..body
            }),
        )
    } else {
        (body, None)
    };
    let mut lines: Vec<Line<'static>> = Vec::new();
    for line in app.visible_log().into_iter().skip(start).take(rows) {
        lines.push(log_line(line, theme, glyphs));
    }
    ui::prose(frame, text_area, lines, theme.body());
    if let Some(bar_area) = bar_area {
        ui::scrollbar(frame, bar_area, start, total, theme, glyphs);
    }
}

fn log_line(line: &LogLine, theme: &Theme, glyphs: Glyphs) -> Line<'static> {
    let style = line.style(theme);
    let gutter = match line.glyph(glyphs) {
        Some(glyph) => ui::gutter(
            glyph,
            if line.kind == Kind::Heading {
                theme.rule()
            } else {
                style
            },
            glyphs,
        ),
        None => Span::styled(" ".repeat(glyphs.status_cell() + 2), theme.body()),
    };
    Line::from(vec![
        gutter,
        Span::styled(
            format!("{}{}", " ".repeat(usize::from(line.depth) * 2), line.text),
            style,
        ),
    ])
}

// Done ------------------------------------------------------------------------------

fn done(frame: &mut Frame, app: &App, body: Rect) {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let tally = app.tally();
    let rows = Layout::vertical([Constraint::Min(6), Constraint::Length(2)]).areas::<2>(body);

    let (glyph, title, style) = if tally.failed > 0 {
        (
            glyphs.status(Status::Failed),
            format!(
                "finished with {}",
                ui::plural(tally.failed, "failed step", "failed steps")
            ),
            theme.error_text(),
        )
    } else if tally.ignored > 0 {
        (
            glyphs.status(Status::Ignored),
            format!(
                "finished with {}",
                ui::plural(tally.ignored, "ignored step", "ignored steps")
            ),
            theme.ignored_text(),
        )
    } else if tally.warn > 0 {
        (
            glyphs.status(Status::Warn),
            format!(
                "complete, with {}",
                ui::plural(tally.warn, "warning", "warnings")
            ),
            theme.warn_text(),
        )
    } else {
        (
            glyphs.status(Status::Ok),
            "complete".to_string(),
            theme.ok_text(),
        )
    };

    let mut lines: Vec<Line<'static>> = vec![
        Line::from(vec![
            Span::styled(format!(" {glyph} "), style),
            Span::styled("The install ".to_string(), theme.body()),
            Span::styled(title, theme.title()),
        ]),
        blank(),
        Line::from(Span::styled(
            format!("{} of {} finished", tally.ran(), tally.total),
            theme.body(),
        )),
    ];
    if tally.warn > 0 {
        lines.push(Line::from(Span::styled(
            format!(
                "{} reported warnings",
                ui::plural(tally.warn, "step", "steps")
            ),
            theme.warn_text(),
        )));
    }
    if tally.unresolved() > 0 {
        lines.push(Line::from(Span::styled(
            ui::plural(
                tally.unresolved(),
                "step needs your attention",
                "steps need your attention",
            ),
            theme.error_text(),
        )));
    }
    if tally.skipped > 0 {
        lines.push(Line::from(Span::styled(
            ui::plural(
                tally.skipped,
                "step was switched off",
                "steps were switched off",
            ),
            theme.muted(),
        )));
    }

    let unresolved: Vec<usize> = STEPS
        .iter()
        .enumerate()
        .filter(|(index, _)| matches!(app.statuses[*index], Status::Failed | Status::Ignored))
        .map(|(index, _)| index)
        .collect();
    if !unresolved.is_empty() {
        lines.push(blank());
        for index in unresolved {
            lines.push(ui::labeled(
                if app.statuses[index] == Status::Ignored {
                    "ignored"
                } else {
                    "failed"
                },
                format!(
                    "{}  ·  caelestia-install step {}",
                    STEPS[index].name, STEPS[index].id
                ),
                theme,
                LABEL_COLUMN,
            ));
        }
        lines.push(ui::labeled(
            "retry",
            "r here, or that command in a terminal".to_string(),
            theme,
            LABEL_COLUMN,
        ));
    }
    lines.push(ui::labeled(
        "log",
        app.log_path.display().to_string(),
        theme,
        LABEL_COLUMN,
    ));
    lines.extend(notice(app));

    let closing: Vec<Line<'static>> = if tally.unresolved() > 0 {
        vec![Line::from(Span::styled(
            "Nothing was rolled back: the steps that finished are still installed.".to_string(),
            theme.muted(),
        ))]
    } else {
        vec![Line::from(Span::styled(
            "Log out and back in so the new session settings apply.".to_string(),
            theme.muted(),
        ))]
    };
    ui::prose(frame, rows[0], lines, theme.body());
    ui::prose(frame, rows[1], closing, theme.body());
}

// Overlays --------------------------------------------------------------------------

fn keys_overlay(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let entries: [(&str, &str); 12] = [
        ("\u{2191}\u{2193} / j k", "move"),
        ("\u{2190}\u{2192} / h l", "change a value"),
        (glyphs.enter(), "continue, or open"),
        (glyphs.escape(), "back one step"),
        ("q", "quit"),
        ("ctrl+c", "stop and quit"),
        ("t", "dark or light"),
        ("f", "follow the log"),
        ("w", "warnings and errors only"),
        ("l", "the whole log, full screen"),
        ("r / R", "reset one option, or all of them"),
        ("?", "this list"),
    ];
    let inner = ui::panel(
        frame,
        frame.area(),
        theme,
        glyphs,
        Some("KEYS"),
        (62, entries.len() as u16 + 5),
        Role::Rule,
    );
    let mut lines: Vec<Line<'static>> = Vec::new();
    for (keys, what) in entries {
        lines.push(Line::from(vec![
            Span::styled(format!("  {keys:<12}"), theme.accent()),
            Span::styled(what.to_string(), theme.body()),
        ]));
    }
    ui::prose(frame, inner, lines, theme.body());
}

fn log_overlay(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let area = frame.area();
    let inner = ui::frame_panel(
        frame,
        ui::log_overlay_rect(area),
        theme,
        glyphs,
        Some("INSTALL LOG"),
        Role::Rule,
    );
    let rows = inner.height.saturating_sub(1).max(1) as usize;
    log_pane(frame, app, inner, rows);
}

fn failure_overlay(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let Some(card) = &app.failure else {
        return;
    };
    let height = card.detail.len() as u16 + FailureCard::CHOICES.len() as u16 + 10;
    let inner = ui::panel(
        frame,
        frame.area(),
        theme,
        glyphs,
        None,
        (78, height),
        Role::Error,
    );

    let mut lines: Vec<Line<'static>> = vec![
        Line::from(vec![
            Span::styled(
                format!(" {} ", glyphs.status(Status::Failed)),
                theme.error_text(),
            ),
            Span::styled(card.step.to_string(), theme.heading()),
            Span::styled(" did not finish".to_string(), theme.muted()),
        ]),
        blank(),
    ];
    if card.detail.is_empty() {
        lines.push(Line::from(Span::styled(
            "the step printed nothing".to_string(),
            theme.muted(),
        )));
    }
    for line in &card.detail {
        lines.push(log_line(line, theme, glyphs));
    }
    lines.push(blank());
    lines.push(ui::labeled(
        "log",
        app.log_path.display().to_string(),
        theme,
        LABEL_COLUMN,
    ));
    lines.push(ui::labeled(
        "reproduce",
        card.reproduction(),
        theme,
        LABEL_COLUMN,
    ));
    lines.push(blank());
    for (index, choice) in FailureCard::CHOICES.iter().enumerate() {
        let chosen = index == card.choice;
        let pointer = format!(" {} ", if chosen { glyphs.cursor() } else { " " });
        let label = format!("{} {:<8}", choice.key, choice.label);
        if chosen {
            let mut spans = vec![Span::raw(pointer), Span::raw(label)];
            if !choice.help.is_empty() {
                spans.push(Span::raw(choice.help.to_string()));
            }
            lines.push(ui::cursor_spans(theme, spans, inner.width));
        } else {
            let mut spans = vec![
                Span::styled(pointer, theme.muted()),
                Span::styled(label, theme.body()),
            ];
            if !choice.help.is_empty() {
                spans.push(Span::styled(choice.help.to_string(), theme.muted()));
            }
            lines.push(Line::from(spans));
        }
    }
    lines.push(blank());
    lines.push(Line::from(Span::styled(
        format!(
            "{} closes this without deciding, and leaves the run paused",
            glyphs.escape()
        ),
        theme.muted(),
    )));
    ui::prose(frame, inner, lines, theme.body());
}

fn confirm_overlay(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let asked = app.confirm.unwrap_or(Confirm::Install);
    let (subject, question, border) = match app.confirm {
        Some(Confirm::Handoff(action)) => (
            action.title(),
            "The installer closes and hands the terminal over.".to_string(),
            Role::Accent,
        ),
        Some(Confirm::Install) => (
            "Install Caelestia now?",
            "Packages are installed, the configuration is deployed and the shell is built."
                .to_string(),
            Role::Accent,
        ),
        Some(Confirm::Quit) | None => (
            "Stop the install here?",
            "A step that is running is stopped first. Work already done stays done.".to_string(),
            Role::Warn,
        ),
    };

    // The card is as tall as what is in it: a panel with rows of nothing under the buttons
    // reads as a screen that failed to draw the rest.
    let width = 70u16.min(frame.area().width.saturating_sub(2)).max(8);
    let measure = width.saturating_sub(4) as usize;
    let mut lines = vec![
        Line::from(Span::styled(subject.to_string(), theme.heading())),
        blank(),
    ];
    for line in wrap(&question, measure) {
        lines.push(Line::from(Span::styled(line, theme.muted())));
    }
    // What the yes actually starts, in the terms of the person saying it.
    if app.confirm == Some(Confirm::Install) {
        let changed = app.changed_answers();
        lines.push(blank());
        lines.push(Line::from(Span::styled(
            "Your settings are backed up first.".to_string(),
            theme.muted(),
        )));
        if changed > 0 {
            lines.push(Line::from(Span::styled(
                format!(
                    "{} changed from the defaults.",
                    ui::plural(changed, "option", "options")
                ),
                theme.accent(),
            )));
        }
    }
    lines.push(blank());
    // The choices are rows the arrows can reach, drawn the way every other choice is drawn: the
    // highlighted one is filled from edge to edge, so the key that acts is never in doubt.
    for (index, label) in asked.buttons().iter().enumerate() {
        let chosen = index == app.confirm_focus;
        let pointer = format!(" {} ", if chosen { glyphs.cursor() } else { " " });
        let letter = if index == 0 { "y" } else { "n" };
        if chosen {
            let spans = vec![Span::raw(pointer), Span::raw(format!("{label} ({letter})"))];
            lines.push(ui::cursor_spans(theme, spans, width - 2));
        } else {
            lines.push(Line::from(vec![
                Span::styled(pointer, theme.muted()),
                Span::styled(format!("{label} "), theme.body()),
                Span::styled(format!("({letter})"), theme.muted()),
            ]));
        }
    }
    let height = lines.len() as u16 + 2;
    let inner = ui::panel(
        frame,
        frame.area(),
        theme,
        glyphs,
        None,
        (width, height),
        border,
    );
    ui::prose(frame, inner, lines, theme.body());
}

// Small shared pieces ----------------------------------------------------------------

fn blank() -> Line<'static> {
    Line::from("")
}

/// The transient answer to the last key, shown until the next one.
fn notice(app: &App) -> Vec<Line<'static>> {
    match &app.notice {
        Some(text) => vec![Line::from(Span::styled(text.clone(), app.theme.accent()))],
        None => Vec::new(),
    }
}

/// Wrap prose to a column without a `Paragraph`, for the places that lay lines out by hand.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(20);
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AuthMethod, Flow, Overlay, Stage};
    use crate::env::Env;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn app() -> App {
        App::new(
            PathBuf::from("/tmp/caelestia"),
            PathBuf::from("/tmp/install.log"),
            Env::default(),
        )
    }

    /// The same, on a terminal that can draw a fill and a border.
    fn color_app() -> App {
        App::new(
            PathBuf::from("/tmp/caelestia"),
            PathBuf::from("/tmp/install.log"),
            Env {
                term: Some("xterm-256color".to_string()),
                colorterm: Some("truecolor".to_string()),
                ..Env::default()
            },
        )
    }

    /// The frame as cells rather than as text.
    fn cells(app: &mut App, width: u16, height: u16) -> ratatui::buffer::Buffer {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).expect("a test terminal");
        terminal
            .draw(|frame| draw(frame, app))
            .expect("a drawn frame");
        terminal.backend().buffer().clone()
    }

    fn row_of(buffer: &ratatui::buffer::Buffer, needle: &str) -> u16 {
        (0..buffer.area.height)
            .find(|y| {
                let mut text = String::new();
                for x in 0..buffer.area.width {
                    text.push_str(buffer[(x, *y)].symbol());
                }
                text.contains(needle)
            })
            .unwrap_or_else(|| panic!("no row holds {needle:?}"))
    }

    #[test]
    fn the_footer_is_filled_right_across_the_terminal() {
        let mut app = color_app();
        let accent = app.theme.color(Role::Accent);
        let buffer = cells(&mut app, 100, 30);
        let bar = buffer.area.height - 2;
        for x in 1..buffer.area.width - 1 {
            assert_eq!(buffer[(x, bar)].bg, accent, "the bar stops at column {x}");
        }
        // Inside the frame, and not over it.
        assert_ne!(
            buffer[(0, bar)].bg,
            accent,
            "the bar covers the frame's edge"
        );
        assert_ne!(
            buffer[(1, bar - 1)].bg,
            accent,
            "the body above it is filled"
        );
    }

    #[test]
    fn the_row_the_cursor_is_on_is_the_only_filled_row_of_the_list() {
        let mut app = color_app();
        let accent = app.theme.color(Role::Accent);
        let buffer = cells(&mut app, 100, 30);
        let chosen = row_of(&buffer, "Install Caelestia");
        let other = row_of(&buffer, "Uninstall Caelestia");
        // Where the panel sits depends on how wide the frame is, so the row is asked whether it
        // carries the fill anywhere rather than at a column that only one size puts it on.
        let filled = |row: u16| (0..buffer.area.width).any(|x| buffer[(x, row)].bg == accent);
        assert!(filled(chosen), "the cursor's row is not filled");
        assert!(!filled(other), "a row the cursor is not on is filled");
    }

    /// How many blank cells there are between the frame's left edge and the first thing drawn,
    /// counting cells rather than bytes.
    fn left_margin(line: &str) -> usize {
        let inside: String = line.chars().skip(1).collect();
        inside.chars().count() - inside.trim_start().chars().count()
    }

    #[test]
    fn a_wide_frame_puts_the_home_composition_in_the_middle() {
        // The mark and the panel are one block of a fixed size. Pinned to the corner of a large
        // frame it reads as a screen that stopped drawing part way; it belongs in the middle.
        let width = 236u16;
        let height = 60u16;
        let mut app = app();
        let text = render(&mut app, width, height);
        let lines: Vec<&str> = text.lines().collect();

        let mark_row = lines
            .iter()
            .position(|line| line.contains('\u{2588}'))
            .unwrap_or_else(|| panic!("the mark is on screen:\n{text}"));
        let panel_bottom = lines
            .iter()
            .position(|line| line.contains('\u{2514}'))
            .expect("the action panel is on screen");

        let left = left_margin(lines[mark_row]);
        let corner = lines[panel_bottom]
            .chars()
            .collect::<Vec<char>>()
            .iter()
            .rposition(|cell| *cell == '\u{2518}')
            .expect("a panel corner");
        let right = width as usize - corner - 2;
        assert!(
            left > 20,
            "the composition is against the left edge: {left} columns of air"
        );
        assert!(
            (left as isize - right as isize).abs() <= 4,
            "the block is off centre: {left} columns on the left, {right} on the right"
        );

        let below = height as usize - 1 - panel_bottom;
        assert!(
            (mark_row as isize - below as isize).abs() <= 4,
            "the block is off centre: {mark_row} rows above, {below} below"
        );
    }

    #[test]
    fn a_wide_frame_puts_the_questions_in_the_middle() {
        let width = 236u16;
        let mut app = app();
        app.stage = Stage::Options;
        let text = render(&mut app, width, 60);
        let lines: Vec<&str> = text.lines().collect();
        let rail_top = lines
            .iter()
            .position(|line| line.contains('\u{250c}') && line.contains("Appearance"))
            .expect("the map is on screen");
        let left = left_margin(lines[rail_top]);
        // The map and the question beside it are one block: the map starts where the block
        // starts, so its own margin is the block's margin.
        assert!(left > 20, "the map is against the left edge: {left}");
        assert!(
            left < (width / 2) as usize,
            "the block is past the middle, not in it: {left}"
        );

        // And the question is beside the map rather than under it: the row the map's own top
        // corner is on is the row the question's heading is on.
        assert!(
            lines[rail_top].contains('\u{250c}'),
            "the map's corner is on the heading row:\n{text}"
        );
    }

    #[test]
    fn a_middling_terminal_still_puts_the_actions_beside_the_words() {
        // Most terminals are wider than the mark plus the panel and narrower than the rest: the
        // panel belongs beside the prose there too, not under it with a column of nothing on the
        // right. What is left of the left column is the words the mark would have stood over.
        let mut app = app();
        let text = render(&mut app, 120, 40);
        let lines: Vec<&str> = text.lines().collect();

        let panel_row = lines
            .iter()
            .position(|line| line.contains("\u{25b6} Install Caelestia"))
            .expect("the actions are on screen");
        // The panel's own top-left corner, which is the only one on that row: the frame's corner
        // is a different glyph.
        let panel_col = lines[panel_row - 1]
            .chars()
            .collect::<Vec<char>>()
            .iter()
            .position(|cell| *cell == '\u{250c}')
            .expect("the actions have a panel around them");
        assert!(
            panel_col > 20,
            "the panel is not beside anything: {panel_col}"
        );
        assert!(
            panel_col < 60,
            "the panel belongs beside the words, not out past them: {panel_col}"
        );
    }

    #[test]
    fn a_wide_terminal_spends_its_width_on_a_second_column() {
        let mut wide = app();
        wide.stage = Stage::Options;
        wide.action = crate::app::Action::Install;
        let text = render(&mut wide, 235, 58);
        // The rail names every question; the pane names only the one being asked.
        assert!(text.contains("Advanced"), "no map:\n{text}");

        let mut narrow = app();
        narrow.stage = Stage::Options;
        narrow.action = crate::app::Action::Install;
        let text = render(&mut narrow, 62, 24);
        assert!(!text.contains("Advanced"), "a map in one column:\n{text}");
        assert!(
            text.contains("Appearance"),
            "and the question left:\n{text}"
        );
    }

    #[test]
    fn a_wide_detection_screen_says_what_it_found_and_what_it_means() {
        let mut app = app();
        app.stage = Stage::Detect;
        app.detection = crate::app::Detection {
            distro: crate::distro::Distro::Arch,
            kind: crate::install_kind::Kind::Source,
            installed: true,
            package_manager: Some("pacman"),
            quickshell: true,
            git: true,
        };
        let text = render(&mut app, 235, 58);
        assert!(text.contains("this machine"), "{text}");
        assert!(text.contains("already installed"), "no verdict: {text}");
        assert!(text.contains("Arch-based Linux"), "{text}");
    }

    #[test]
    fn the_detection_screen_lists_what_it_found_about_this_machine() {
        let mut app = app();
        app.stage = Stage::Detect;
        app.detection = crate::app::Detection {
            distro: crate::distro::Distro::Arch,
            kind: crate::install_kind::Kind::Source,
            installed: false,
            package_manager: Some("pacman"),
            quickshell: true,
            git: true,
        };
        let text = render(&mut app, 100, 30);
        assert!(text.contains("This machine"), "{text}");
        for row in [
            "distribution",
            "Arch-based Linux",
            "package manager",
            "pacman",
            "git",
            "found",
        ] {
            assert!(text.contains(row), "{row} is missing from:\n{text}");
        }
        assert!(
            text.contains("Nothing is installed in the way"),
            "an install with a clear field says so: {text}"
        );
        // The screen has nothing to select, so the way on has to be visible on the screen itself
        // and not only in the footer bar.
        assert!(
            text.contains("continue"),
            "the way off this screen is on it: {text}"
        );
    }

    #[test]
    fn the_detection_screen_is_centred_rather_than_pinned_to_the_left_edge() {
        let mut app = app();
        app.stage = Stage::Detect;
        app.detection = crate::app::Detection {
            distro: crate::distro::Distro::Arch,
            kind: crate::install_kind::Kind::Source,
            installed: false,
            package_manager: Some("pacman"),
            quickshell: true,
            git: true,
        };

        // The findings panel's border, on a wide terminal: it must not start in the first column,
        // or two small panels in a wide frame read as a drawing that stopped part way through.
        let wide = render(&mut app, 164, 46);
        let first = wide
            .lines()
            .find(|line| line.contains("this machine"))
            .unwrap_or_else(|| panic!("no findings panel in:\n{wide}"));
        let indent = first.chars().take_while(|c| *c == ' ' || *c == '│').count();
        assert!(indent > 20, "the panel starts at column {indent}:\n{wide}");
    }

    #[test]
    fn the_detection_screen_warns_before_an_install_is_overwritten() {
        let mut app = app();
        app.stage = Stage::Detect;
        app.detection = crate::app::Detection {
            distro: crate::distro::Distro::Arch,
            kind: crate::install_kind::Kind::Source,
            installed: true,
            package_manager: Some("pacman"),
            quickshell: true,
            git: true,
        };
        app.detection.installed = true;
        let text = render(&mut app, 100, 30);
        assert!(
            text.contains("already installed"),
            "the one fact that changes the decision: {text}"
        );
        assert!(
            text.contains("replaced"),
            "and what continuing does: {text}"
        );
        assert!(
            text.contains("update instead"),
            "and the cheaper route, on the footer: {text}"
        );
    }

    #[test]
    fn the_detection_screen_names_what_would_stop_the_install() {
        // One cause, said once.
        let mut unknown = app();
        unknown.stage = Stage::Detect;
        unknown.detection = crate::app::Detection {
            distro: crate::distro::Distro::Unknown,
            kind: crate::install_kind::Kind::Source,
            installed: false,
            package_manager: None,
            quickshell: false,
            git: false,
        };
        let text = render(&mut unknown, 100, 30);
        assert!(
            text.contains("not one the installer knows"),
            "a distribution it cannot help with is said plainly: {text}"
        );
        assert_eq!(
            unknown.detection.blockers().len(),
            1,
            "and said once: {:?}",
            unknown.detection.blockers()
        );

        // On a distribution it does know, the things it is missing are named.
        let mut known = app();
        known.stage = Stage::Detect;
        known.detection = crate::app::Detection {
            distro: crate::distro::Distro::Arch,
            kind: crate::install_kind::Kind::Source,
            installed: false,
            package_manager: None,
            quickshell: false,
            git: false,
        };
        let text = render(&mut known, 100, 30);
        assert!(text.contains("No supported package manager"), "{text}");
        assert!(text.contains("git is missing"), "{text}");

        // The keymap overlay is a panel of a fixed size: every line of it has to fit inside.
        let mut overlay = app();
        overlay.overlay = Some(Overlay::Keys);
        let text = render(&mut overlay, 100, 34);
        for line in ["dark or light", "this list"] {
            assert!(text.contains(line), "{line} is missing from:\n{text}");
        }
        assert_eq!(
            text.matches("reset").count(),
            1,
            "a key is described once, in the list: {text}"
        );
    }

    #[test]
    fn the_reason_a_screen_is_blocking_survives_a_short_terminal() {
        let problem = "Sort out the above first.";
        // The worst case: an existing install, one blocker, and a guard that sent the user back,
        // at the smallest terminal the installer draws in.
        let worst = crate::app::Detection {
            distro: crate::distro::Distro::Unknown,
            kind: crate::install_kind::Kind::Source,
            installed: true,
            package_manager: None,
            quickshell: false,
            git: false,
        };

        // With room, the screen explains what continuing would do.
        let mut roomy = app();
        roomy.stage = Stage::Detect;
        roomy.detection = crate::app::Detection {
            distro: crate::distro::Distro::Arch,
            ..worst.clone()
        };
        assert!(render(&mut roomy, 100, 30).contains("Your own configs are backed up first"));

        let mut tight = app();
        tight.stage = Stage::Detect;
        tight.detection = worst;
        tight.problem = Some(problem.to_string());
        let text = render(&mut tight, 60, 20);
        assert!(
            text.contains("not one the installer knows"),
            "the reason survives: {text}"
        );
        assert!(
            text.contains("already installed"),
            "and what it means for the decision: {text}"
        );
        assert!(
            text.contains("Sort out the above first"),
            "and the answer to the key: {text}"
        );
        assert!(
            !text.contains("Your own configs are backed up first"),
            "at the cost of the explanation: {text}"
        );
        // And the reading order holds: the fact comes before what continuing would do about it.
        let headline = text.find("already installed").expect("the headline");
        let explanation = text.find("Continuing installs over it");
        assert!(
            explanation.is_none_or(|at| at > headline),
            "the explanation follows the thing it explains: {text}"
        );
    }

    #[test]
    fn the_privilege_routes_are_offered_by_name() {
        let mut app = app();
        app.stage = Stage::Auth;
        let text = render(&mut app, 100, 30);
        for phrase in ["root privileges", "Type your password here"] {
            assert!(text.contains(phrase), "{phrase} is missing from:\n{text}");
        }
        assert!(
            !text.contains("sudo -v"),
            "the command appears with the route that needs it, not before: {text}"
        );
        assert!(
            !text.contains("For when it cannot ask"),
            "and nothing is said twice: {text}"
        );
    }

    #[test]
    fn a_reader_is_offered_by_name_and_asked_by_touching_it() {
        let mut app = app().with_fingerprint(true);
        app.stage = Stage::Auth;
        let text = render(&mut app, 100, 30);
        assert!(
            text.contains("Touch the fingerprint reader"),
            "the route is named on the list: {text}"
        );

        // Taking it puts the reader on the card, and nothing about passwords on it.
        app.auth = Some(AuthMethod::Fingerprint);
        app.stage = Stage::Privilege;
        let (sender, receiver) = std::sync::mpsc::channel();
        sender.send(true).expect("the answer fits");
        app.checking = Some(receiver);
        let waiting = render(&mut app, 100, 30);
        assert!(
            waiting.contains("Touch the fingerprint reader"),
            "{waiting}"
        );
        assert!(
            !waiting.contains("password"),
            "a reader is not typed into: {waiting}"
        );
        assert!(
            waiting.contains("Waiting for your finger"),
            "and the wait says what it is waiting for: {waiting}"
        );

        // A read that did not happen says so, in the reader's own words.
        app.checking = None;
        app.problem = Some("No finger was recognized.".to_string());
        let refused = render(&mut app, 100, 30);
        assert!(refused.contains("No finger was recognized."), "{refused}");
        assert!(
            !refused.contains("password"),
            "and still asks for no password: {refused}"
        );
    }

    #[test]
    fn the_detection_screen_says_why_a_guard_sent_the_user_back() {
        let mut app = app();
        app.stage = Stage::Detect;
        app.detection = crate::app::Detection {
            distro: crate::distro::Distro::Arch,
            kind: crate::install_kind::Kind::Source,
            installed: true,
            package_manager: Some("pacman"),
            quickshell: true,
            git: true,
        };
        app.installed_when_started = false;
        app.problem = Some("Caelestia was installed while the wizard was open.".to_string());

        let text = render(&mut app, 100, 30);
        assert!(
            text.contains("while the wizard was open"),
            "an explained block, not a screen that changed its mind: {text}"
        );
        assert!(text.contains("already installed"), "{text}");
    }

    #[test]
    fn a_question_screen_names_the_question_and_says_where_it_is() {
        let mut app = app();
        app.stage = Stage::Options;
        app.question = 0;
        let text = render(&mut app, 100, 30);
        assert!(text.contains(menu::question(0)), "{text}");
        assert_eq!(
            menu::question(0),
            "Appearance",
            "and the heading is the whole of it"
        );
        assert!(
            text.contains(&format!("question 1/{}", menu::question_count())),
            "and how far along it is: {text}"
        );

        app.question = menu::question_count() - 1;
        let last = render(&mut app, 100, 30);
        assert!(
            last.contains(&format!(
                "question {}/{}",
                menu::question_count(),
                menu::question_count()
            )),
            "{last}"
        );
    }

    #[test]
    fn a_question_that_was_answered_says_so_above_its_rows() {
        let mut app = app();
        app.stage = Stage::Options;
        app.question = 0;
        assert!(!render(&mut app, 100, 30).contains("changed"));

        char_key(&mut app, ' ');
        assert!(
            render(&mut app, 100, 30).contains("changed"),
            "stepping back to a screen that was edited should say so"
        );
    }

    #[test]
    fn the_confirmation_states_what_saying_yes_starts() {
        let mut app = app();
        app.confirm = Some(Confirm::Install);
        app.overlay = Some(Overlay::Confirm);
        let text = render(&mut app, 100, 30);
        assert!(text.contains("Install Caelestia now?"), "{text}");
        assert!(
            text.contains("Install (y)"),
            "the answer is a button: {text}"
        );
        assert!(text.contains("Cancel (n)"), "and so is the other: {text}");
        // What a person is told is what happens to their machine, not how many steps it takes.
        assert!(text.contains("backed up"), "{text}");
        assert!(
            !text.contains("steps"),
            "the step count is the installer's business: {text}"
        );
    }

    fn char_key(app: &mut App, character: char) -> Flow {
        app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))
    }

    /// The frame as text, so a test can read what a user would see.
    fn render(app: &mut App, width: u16, height: u16) -> String {
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

    /// The rows that mention a phrase.
    fn rows_with(text: &str, needle: &str) -> Vec<String> {
        text.lines()
            .filter(|row| row.contains(needle))
            .map(str::to_string)
            .collect()
    }

    fn running(statuses: &[Status], log: &[&str]) -> App {
        let mut app = app();
        app.stage = Stage::Run;
        for (index, status) in statuses.iter().enumerate() {
            app.statuses[index] = *status;
        }
        app.log = log.iter().map(|line| LogLine::parse(line)).collect();
        app
    }

    #[test]
    fn a_wide_run_screen_gives_the_log_the_whole_width() {
        let mut app = running(&[Status::Ok, Status::Running], &["* fetching packages"]);
        let long = format!("{}END", "x".repeat(120));
        app.log = (0..30)
            .map(|index| LogLine::parse(&format!("  [INFO]  {index} {long}")))
            .collect();
        let text = render(&mut app, 200, 40);
        let log = rows_with(&text, "install log")
            .first()
            .expect("no log heading")
            .clone();
        // Nothing sits beside the log: it starts at the left gutter and runs to the frame.
        assert!(
            log.find("install log").expect("a log heading") < 20,
            "the log was pushed into a column:\n{text}"
        );
        assert!(
            text.contains("END"),
            "the log is clipped to a column:\n{text}"
        );
    }

    #[test]
    fn the_home_screen_offers_the_three_actions_and_says_what_they_do() {
        let text = render(&mut app(), 100, 30);
        assert!(text.contains("Install Caelestia"), "{text}");
        assert!(text.contains("Update this install"), "{text}");
        assert!(text.contains("Uninstall Caelestia"), "{text}");
        assert!(text.contains(theme::TAGLINE), "{text}");
        assert!(
            text.contains("██████╗"),
            "the mark is drawn where it fits: {text}"
        );
        assert!(text.contains(theme::AUTHOR), "{text}");
    }

    #[test]
    fn the_mark_is_dropped_rather_than_clipped_when_there_is_no_room_for_it() {
        let text = render(&mut app(), 62, 22);
        assert!(!text.contains("██████╗"), "{text}");
        assert!(text.contains(theme::NAME), "the name stands in: {text}");
        assert!(
            text.contains("Install Caelestia"),
            "and the launcher stays: {text}"
        );
    }

    #[test]
    fn a_terminal_too_small_says_so_instead_of_drawing_a_broken_frame() {
        let text = render(&mut app(), 40, 10);
        assert!(text.contains("terminal too small"), "{text}");
        assert!(text.contains("60x20"), "{text}");
        assert!(!text.contains("Install Caelestia"), "{text}");
    }

    #[test]
    fn the_chrome_shows_where_the_user_is_and_how_the_install_went() {
        let mut app = app();
        app.stage = Stage::Options;
        app.action = crate::app::Action::Install;
        let text = render(&mut app, 100, 30);
        assert!(text.contains("caelestia · install"), "{text}");
        assert!(text.contains("↑↓ move"), "the real keymap: {text}");
    }

    #[test]
    fn the_privilege_screen_never_shows_the_password() {
        let mut app = app();
        app.stage = Stage::Privilege;
        app.password = "hunter2".to_string();
        let problem = app.problem.clone();
        assert_eq!(problem, None);
        let text = render(&mut app, 80, 24);
        assert!(!text.contains("hunter2"), "{text}");
        assert!(
            text.contains("•••••••"),
            "and shows that something was typed: {text}"
        );
        assert!(text.contains("not saved"), "{text}");
    }

    #[test]
    fn a_wrong_password_is_shown_as_a_problem_with_what_to_do_about_it() {
        let mut app = app();
        app.stage = Stage::Privilege;
        app.attempts = 3;
        app.problem = Some("Not accepted. A wrong entry now locks the account.".to_string());
        let text = render(&mut app, 100, 30);
        assert!(text.contains("Not accepted"), "{text}");
        assert!(text.contains("locks the account"), "{text}");
        assert!(text.contains("attempt 4"), "{text}");
    }

    #[test]
    fn the_wait_for_sudo_is_shown_where_the_answer_will_appear() {
        // A password in flight used to leave the frame looking frozen, so people typed it again.
        let mut app = app();
        app.stage = Stage::Privilege;
        app.password = "hunter2".to_string();
        let (sender, receiver) = std::sync::mpsc::channel();
        app.checking = Some(receiver);
        let text = render(&mut app, 100, 30);
        assert!(text.contains("Checking your password"), "{text}");
        assert!(!text.contains("hunter2"), "{text}");

        // The answer arrives, and the line gives way to it.
        sender.send(false).expect("the answer fits");
        app.tick();
        let text = render(&mut app, 100, 30);
        assert!(text.contains("accepted"), "{text}");
        assert!(!text.contains("Checking your password"), "{text}");
    }

    #[test]
    fn every_question_ends_with_a_row_that_goes_on() {
        // The way out of a question is the last row of its list, so the arrows reach it: a key
        // hint at the foot of the screen was something to read, not something to move onto.
        let mut app = app();
        app.stage = Stage::Options;
        for question in 0..menu::question_count() {
            app.question = question;
            app.cursor = 0;
            let text = render(&mut app, 100, 30);
            assert!(
                text.contains("continue"),
                "question {question} has no row to go on with:\n{text}"
            );
        }
    }

    #[test]
    fn the_longest_question_shows_its_last_row_and_the_way_on_at_the_smallest_size() {
        // A row the pane clips is a row nobody can answer, so the rows are given the lines they
        // need before the help text is given any.
        let mut app = app();
        app.stage = Stage::Options;
        app.question = (0..menu::question_count())
            .find(|index| {
                menu::items_of(*index)
                    .iter()
                    .any(|entry| entry.id() == menu::PRESET_THEME_APPS)
            })
            .expect("the app question is in the tree");
        let text = render(&mut app, 60, 20);
        assert!(text.contains("Firefox"), "the last row was clipped: {text}");
        assert!(text.contains("continue"), "and the way on: {text}");
    }

    #[test]
    fn the_app_question_offers_every_app_even_on_a_machine_that_has_none() {
        // The rows are an install decision, so a machine with none of the apps has to see all of
        // them - that is the machine that needs them.
        let mut app = app().with_apps(crate::apps::Apps::default());
        app.stage = Stage::Options;
        app.question = (0..menu::question_count())
            .find(|index| {
                menu::items_of(*index)
                    .iter()
                    .any(|entry| entry.id() == menu::PRESET_THEME_APPS)
            })
            .expect("the app question is in the tree");

        let text = render(&mut app, 100, 30);
        for id in crate::apps::Apps::IDS {
            let entry = menu::every_entry()
                .into_iter()
                .find(|entry| entry.id() == id)
                .expect("every app id is a row");
            assert!(
                text.contains(entry.title()),
                "{} is missing: {text}",
                entry.title()
            );
        }
        assert!(text.contains("All of them"), "{text}");
        assert!(text.contains("None of them"), "{text}");
    }

    #[test]
    fn a_row_that_belongs_to_the_row_above_it_is_drawn_under_it() {
        // "Login screen style" is only a decision once the login screen is being replaced, so it
        // is drawn under that row rather than beside it.
        let mut app = app();
        app.stage = Stage::Options;
        app.question = 0;
        let entries = app.current_entries();
        let parent = entries
            .iter()
            .position(|entry| entry.id() == "INSTALL_SDDM")
            .expect("the login screen row");
        let child = entries
            .iter()
            .position(|entry| entry.id() == "SDDM_THEME_VARIANT")
            .expect("the login screen style row");
        let rows = option_rows(&app, 96);
        // Columns, not byte offsets: the cursor glyph is three bytes wide and one cell wide.
        let column = |index: usize| -> usize {
            let text: String = rows[index]
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            let bytes = text.find("Login").expect("the word Login starts the row");
            text[..bytes].chars().count()
        };
        assert_eq!(
            column(child),
            column(parent) + CHILD_INDENT as usize,
            "the style is not drawn under the login screen"
        );
    }

    #[test]
    fn the_options_screen_puts_values_in_a_column() {
        let mut app = app();
        app.stage = Stage::Options;
        let rows = option_rows(&app, 96);
        assert_eq!(rows.len(), app.current_entries().len());

        let widths: Vec<usize> = rows.iter().map(Line::width).collect();
        assert!(
            widths.windows(2).all(|pair| pair[0] == pair[1]),
            "the rows are not a grid: {widths:?}"
        );

        // And what is drawn agrees with what was built.
        let text = render(&mut app, 100, 30);
        for entry in app.current_entries() {
            assert!(text.contains(entry.title()), "{}", entry.title());
        }
    }

    #[test]
    fn a_value_that_is_too_long_for_its_column_is_marked_rather_than_silently_cut() {
        let mut app = app();
        app.stage = Stage::Options;
        let narrow = option_rows(&app, 24);
        let drawn: Vec<String> = narrow.iter().map(|row| row.to_string()).collect();
        assert!(
            drawn.iter().any(|row| row.contains('…')),
            "a clipped name says so: {drawn:?}"
        );
        // And a clipped row is still the same width as its neighbors.
        let widths: Vec<usize> = narrow.iter().map(Line::width).collect();
        assert!(
            widths.windows(2).all(|pair| pair[0] == pair[1]),
            "{widths:?}"
        );
    }

    #[test]
    fn the_options_screen_marks_what_was_changed_and_shows_the_help() {
        let mut app = app();
        app.stage = Stage::Options;
        app.question = 0;
        app.cursor = 0;
        let before = render(&mut app, 100, 30);
        assert!(!before.contains("•"), "{before}");
        // A row whose label says it all shows nothing under the list: the caption that used to sit
        // there was the label again.
        assert!(
            !before.contains("Replace the login screen"),
            "a row's label is not repeated as its help: {before}"
        );

        // The row that does carry a fact shows it, and only while it is under the cursor.
        app.cursor = 1;
        let shot = render(&mut app, 100, 30);
        assert!(
            shot.contains("the login card alone"),
            "the row's own help is on screen:\n{shot}"
        );
        app.cursor = 0;
        assert!(
            !render(&mut app, 100, 30).contains("the login card alone"),
            "and it goes with the cursor"
        );

        // A change is marked on the row, on the question, and in the rail's map.
        app.key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        let after = render(&mut app, 100, 30);
        assert!(after.contains("•"), "the change is marked: {after}");
        assert!(
            after.contains("changed"),
            "and the question says so: {after}"
        );
        assert!(after.contains(menu::question(0)), "{after}");
    }
    #[test]
    fn the_run_screen_names_the_step_it_is_on_and_how_far_along_it_is() {
        // DMS's install screen, which is what this one now is: the step, one bar, and the log.
        // Phases and their step counts were the installer's business, not the reader's.
        let mut app = running(
            &[Status::Ok, Status::Warn, Status::Running],
            &["  [INFO]  syncing databases", "  [WARN]  slow mirror"],
        );
        let text = render(&mut app, 100, 30);
        let step = STEPS[2].name;
        assert!(text.contains(step), "the step it is on: {text}");
        assert!(text.contains('%'), "how far along: {text}");
        assert!(
            text.contains("1 warning"),
            "what did not go cleanly: {text}"
        );
        assert!(
            !text.contains("PENDING") && !text.contains("RUNNING"),
            "the states are no longer spelled out: {text}"
        );
    }

    #[test]
    fn the_run_screen_gives_the_log_the_room_it_never_had() {
        let mut app = running(&[Status::Running], &["  [INFO]  one", "  [INFO]  two"]);
        app.log = (0..40)
            .map(|index| LogLine::parse(&format!("  [INFO]  line {index}")))
            .collect();
        let text = render(&mut app, 80, 24);
        // On a 24-row terminal the old screen showed two lines of log; this one shows a pane.
        let shown = (0..40)
            .filter(|index| text.contains(&format!("line {index}")))
            .count();
        assert!(
            shown >= 6,
            "only {shown} lines of log made it onto the screen: {text}"
        );
    }

    #[test]
    fn the_log_pane_says_where_it_is_and_whether_it_follows() {
        let mut app = running(&[Status::Running], &[]);
        app.log = (0..60)
            .map(|index| LogLine::parse(&format!("  [INFO]  line {index}")))
            .collect();
        let following = render(&mut app, 100, 30);
        assert!(following.contains("FOLLOW"), "{following}");
        assert!(
            following.contains(" of 60"),
            "the position is shown: {following}"
        );

        app.log_follow = false;
        app.log_scroll = 4;
        let paused = render(&mut app, 100, 30);
        assert!(paused.contains("PAUSED"), "{paused}");
    }

    #[test]
    fn the_log_is_classified_so_a_failure_does_not_look_like_chatter() {
        let mut app = running(
            &[Status::Running],
            &[
                "  [INFO]  reading packages",
                "  [WARN]  slow mirror",
                "  [ERR]   package not found",
            ],
        );
        let text = render(&mut app, 100, 30);

        let warning = rows_with(&text, "slow mirror");
        assert!(
            warning.iter().any(|row| row.contains('!')),
            "the warning has its own glyph: {warning:?}"
        );
        let failure = rows_with(&text, "package not found");
        assert!(
            failure.iter().any(|row| row.contains('✗')),
            "and the failure its own: {failure:?}"
        );
        let chatter = rows_with(&text, "reading packages");
        assert!(
            chatter
                .iter()
                .all(|row| !row.contains('!') && !row.contains('✗')),
            "and chatter stays plain: {chatter:?}"
        );
    }

    #[test]
    fn the_log_can_be_narrowed_to_the_lines_that_matter() {
        let mut app = running(
            &[Status::Running],
            &["  [INFO]  chatter", "  [ERR]   a package failed"],
        );
        app.log_loud_only = true;
        let text = render(&mut app, 100, 30);
        assert!(text.contains("a package failed"), "{text}");
        assert!(text.contains("warnings and errors only"), "{text}");
        assert!(!text.contains("chatter"), "{text}");
    }

    #[test]
    fn the_failure_card_says_what_to_do_and_what_each_answer_costs() {
        let mut app = app();
        app.stage = Stage::Run;
        app.statuses[0] = Status::Failed;
        app.failure = Some(crate::app::FailureCard::new(
            &STEPS[0],
            &[LogLine::parse("  [ERR]   the mirror is unreachable")],
        ));
        app.overlay = Some(Overlay::Failure);
        let text = render(&mut app, 100, 32);
        assert!(text.contains("did not finish"), "{text}");
        assert!(text.contains("Update system"), "{text}");
        assert!(text.contains("the mirror is unreachable"), "{text}");
        assert!(
            text.contains("caelestia-install step 00a-system-update"),
            "{text}"
        );
        for choice in crate::app::FailureCard::CHOICES {
            assert!(text.contains(choice.label), "{}", choice.label);
        }
        assert!(text.contains("leaves the run paused"), "{text}");
    }

    #[test]
    fn the_summary_reads_as_a_success_when_it_was_one() {
        let mut app = app();
        app.stage = Stage::Done;
        app.statuses = vec![Status::Ok; STEPS.len()];
        let text = render(&mut app, 100, 30);
        assert!(
            text.contains("Install complete") || text.contains("complete"),
            "{text}"
        );
        assert!(text.contains("17 of 17 finished"), "{text}");
        assert!(text.contains("Log out and back in"), "{text}");
    }

    #[test]
    fn the_summary_does_not_read_as_a_success_when_it_was_not() {
        let mut app = app();
        app.stage = Stage::Done;
        app.statuses = vec![Status::Ok; STEPS.len()];
        app.statuses[2] = Status::Failed;
        app.statuses[5] = Status::Ignored;
        let text = render(&mut app, 110, 32);
        assert!(text.contains("1 failed step"), "{text}");
        assert!(text.contains("2 steps need your attention"), "{text}");
        assert!(
            text.contains("caelestia-install step"),
            "the way to retry: {text}"
        );
        assert!(text.contains("Nothing was rolled back"), "{text}");
    }

    #[test]
    fn the_summary_counts_read_like_english() {
        let mut app = app();
        app.stage = Stage::Done;
        app.statuses = vec![Status::Ok; STEPS.len()];
        app.statuses[1] = Status::Warn;
        let text = render(&mut app, 110, 32);
        assert!(text.contains("1 step"), "{text}");
        assert!(text.contains("1 warning"), "{text}");
        assert!(!text.contains("1 steps"), "no plurals of one: {text}");
        assert!(!text.contains("1 warnings"), "{text}");
    }

    #[test]
    fn the_confirm_card_says_what_will_happen_to_a_running_step() {
        let mut app = app();
        app.stage = Stage::Run;
        app.statuses[0] = Status::Running;
        app.ask(crate::app::Confirm::Quit);
        let text = render(&mut app, 100, 30);
        assert!(text.contains("Stop the install here?"), "{text}");
        assert!(
            text.contains("A step that is running") && text.contains("stopped first"),
            "true whether or not a step happens to be running: {text}"
        );
        // The question has buttons, and the one that stops the run is not under the highlight:
        // the key that opened this must not be able to end the run.
        assert!(text.contains("Stop (y)"), "the answer is a button: {text}");
        assert!(
            text.contains("Keep going (n)"),
            "and so is the one that keeps it, with the letter that picks it: {text}"
        );
        assert_eq!(app.confirm_focus, 1);
    }

    #[test]
    fn the_confirm_card_names_the_action_it_hands_over_to() {
        let mut app = app();
        app.confirm = Some(crate::app::Confirm::Handoff(crate::app::Action::Uninstall));
        app.overlay = Some(Overlay::Confirm);
        let text = render(&mut app, 100, 30);
        assert!(text.contains("Uninstall Caelestia"), "{text}");
        assert!(text.contains("hands the terminal over"), "{text}");
    }

    #[test]
    fn the_keymap_overlay_lists_the_keys_the_screens_answer_to() {
        let mut app = app();
        app.overlay = Some(Overlay::Keys);
        let text = render(&mut app, 100, 30);
        assert!(text.contains("KEYS"), "{text}");
        for key in ["ctrl+c", "?", "w", "l", "r / R"] {
            assert!(text.contains(key), "{key} is missing from {text}");
        }
    }

    #[test]
    fn the_full_screen_log_leaves_the_run_screen_behind() {
        let mut app = running(&[Status::Running], &[]);
        app.log = (0..100)
            .map(|index| LogLine::parse(&format!("  [INFO]  line {index}")))
            .collect();
        app.overlay = Some(Overlay::Log);
        let text = render(&mut app, 100, 34);
        assert!(text.contains("INSTALL LOG"), "{text}");
        let shown = (0..100)
            .filter(|index| text.contains(&format!("line {index}")))
            .count();
        assert!(shown > 20, "the overlay is the whole screen: {shown}");
    }

    #[test]
    fn every_screen_draws_something_at_the_smallest_size_it_accepts() {
        let stages = [
            Stage::Home,
            Stage::Privilege,
            Stage::Options,
            Stage::Auth,
            Stage::Run,
            Stage::Done,
        ];
        for stage in stages {
            let mut app = app();
            app.stage = stage;
            app.statuses[0] = Status::Running;
            let text = render(&mut app, ui::MIN_WIDTH, ui::MIN_HEIGHT);
            assert!(!text.trim().is_empty(), "{stage:?} drew nothing");
            assert!(
                text.contains("caelestia"),
                "{stage:?} lost its header: {text}"
            );
        }
    }

    #[test]
    fn the_frame_never_spills_out_of_the_terminal() {
        for (width, height) in [(60u16, 20u16), (80, 24), (100, 30), (200, 50)] {
            let mut app = app();
            app.stage = Stage::Run;
            app.statuses = vec![Status::Ok; STEPS.len()];
            app.failure = Some(crate::app::FailureCard::new(&STEPS[0], &[]));
            app.overlay = Some(Overlay::Failure);
            let text = render(&mut app, width, height);
            for row in text.lines() {
                assert!(
                    row.chars().count() <= width as usize,
                    "a row is wider than the terminal: {row:?}"
                );
            }
            assert_eq!(text.lines().count(), height as usize);
        }
    }

    #[test]
    fn the_keymap_the_screen_shows_is_the_keymap_it_answers_to() {
        let mut app = app();
        app.stage = Stage::Home;
        app.home = 1;
        let text = render(&mut app, 100, 30);
        assert!(text.contains("choose"), "{text}");
        assert!(text.contains("quit"), "{text}");

        // And the handler agrees with the hint it printed.
        let before = app.theme.mode;
        assert_eq!(
            app.key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('t'),
                crossterm::event::KeyModifiers::NONE,
            )),
            Flow::Continue
        );
        assert_ne!(app.theme.mode, before, "'t theme' did not do what it says");
    }
}
