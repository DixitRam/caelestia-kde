// The screens. Every one of them draws into the content box the chrome hands out, on the one
// grid: a fixed gutter, a fixed column for a status glyph, a fixed column for a value, and a
// right-hand column for a count.
//
// No screen surrounds itself with a border. Structure is a rule, or the space between things.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

use crate::app::{Action, App, Confirm, FailureCard, Overlay, Stage};
use crate::glyphs::Glyphs;
use crate::logline::{Kind, LogLine};
use crate::menu::{self, Entry};
use crate::steps::{Phase, STEPS, Status};
use crate::theme::{self, Role, Theme};
use crate::ui;

/// The width of the value column on the options screen: the widest value, its arrows, and room to
/// spare so the column never jitters between rows.
const VALUE_COLUMN: usize = 12;

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
        Stage::Privilege => privilege(frame, app, body),
        Stage::Options => options(frame, app, body),
        Stage::Review => review(frame, app, body),
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
    let glyphs = app.glyphs;

    let mut lines: Vec<Line<'static>> = Vec::new();

    // The mark is drawn only where it fits whole: clipped art reads as a rendering bug.
    if body.width >= 48 && body.height >= 24 {
        for art in theme::SPLASH {
            lines.push(Line::from(Span::styled(art.to_string(), theme.accent())));
        }
        lines.push(blank());
    }
    lines.push(Line::from(Span::styled(
        theme::NAME.to_string(),
        theme.title(),
    )));
    lines.push(Line::from(Span::styled(
        theme::TAGLINE.to_string(),
        theme.muted(),
    )));
    lines.push(blank());
    lines.push(Line::from(Span::styled(
        format!(
            "{} · {} install · {}",
            app.distro,
            app.kind,
            if app.installed {
                "Caelestia is installed"
            } else {
                "nothing installed yet"
            }
        ),
        theme.muted(),
    )));
    lines.push(blank());

    // Each action carries its own line, the way the installer this is modelled on lists its
    // options: the menu is the whole description, so there is no paragraph under it to read.
    let column = Action::ALL
        .iter()
        .map(|action| action.title().len())
        .max()
        .unwrap_or(0);
    for (index, action) in Action::ALL.iter().enumerate() {
        let chosen = index == app.home;
        let enabled = app.action_enabled(*action);
        let name = if !enabled {
            theme.muted()
        } else if chosen {
            theme.selected()
        } else {
            theme.body()
        };
        let detail = if enabled {
            action.help()
        } else {
            "Caelestia is not installed"
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", if chosen { glyphs.cursor() } else { " " }),
                if chosen { theme.selected() } else { name },
            ),
            Span::styled(format!("{:<width$}", action.title(), width = column), name),
            Span::styled(format!("  {detail}"), theme.muted()),
        ]));
    }

    lines.extend(notice(app));

    // The credits belong with the rest of the text: pinned to the bottom of the window they
    // read as a separate bar rather than as part of what the installer is saying.
    lines.push(blank());
    lines.push(Line::from(Span::styled(
        theme::AUTHOR.to_string(),
        theme.muted(),
    )));
    lines.push(Line::from(Span::styled(
        theme::CO_AUTHOR.to_string(),
        theme.muted(),
    )));

    ui::prose_middle(frame, body, lines, theme.body());
}

// Privilege -------------------------------------------------------------------------

fn privilege(frame: &mut Frame, app: &App, body: Rect) {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let mut lines = vec![
        Line::from(Span::styled("root privileges".to_string(), theme.heading())),
        blank(),
    ];
    // Two short lines, short enough to survive the narrowest terminal the installer will draw
    // in, so the explanation is never cut in half.
    lines.push(Line::from(Span::styled(
        "Installing needs root.".to_string(),
        theme.muted(),
    )));
    lines.push(Line::from(Span::styled(
        "Your password is sent to sudo and is not saved.".to_string(),
        theme.muted(),
    )));
    lines.push(blank());

    let mut field = vec![
        Span::styled(format!("{:<10}", "password"), theme.muted()),
        Span::styled(
            glyphs
                .mask()
                .repeat(app.password.chars().count().min(body.width as usize)),
            theme.body(),
        ),
    ];
    if app.password.is_empty() {
        field.push(Span::styled("(type, then ".to_string(), theme.muted()));
        field.push(Span::styled(glyphs.enter().to_string(), theme.accent()));
        field.push(Span::styled(")".to_string(), theme.muted()));
    }
    lines.push(Line::from(field));
    lines.push(blank());

    // What sudo is doing with the password belongs where the answer will appear, so the wait
    // reads as work rather than as a freeze.
    if app.checking.is_some() {
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", ui::running_glyph(app)), theme.accent()),
            Span::styled("Checking your password...".to_string(), theme.muted()),
        ]));
    } else if let Some(problem) = &app.problem {
        lines.push(Line::from(Span::styled(
            problem.clone(),
            theme.error_text(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "The session is kept awake while it runs.".to_string(),
            theme.muted(),
        )));
    }
    if app.attempts > 0 {
        lines.push(Line::from(Span::styled(
            format!("attempt {}", app.attempts + 1),
            theme.warn_text(),
        )));
    }

    ui::prose_middle(frame, body, lines, theme.body());
}

// Options ---------------------------------------------------------------------------

fn options(frame: &mut Frame, app: &App, body: Rect) {
    let theme = &app.theme;
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(4),
        Constraint::Length(1),
        Constraint::Min(2),
    ])
    .areas::<5>(body);

    ui::prose(
        frame,
        rows[0],
        vec![Line::from(Span::styled(app.breadcrumb(), theme.muted()))],
        theme.body(),
    );
    ui::prose(
        frame,
        rows[2],
        option_rows(app, rows[2].width),
        theme.body(),
    );

    let mut help: Vec<Line<'static>> = Vec::new();
    let text = app.selected().map(Entry::help).unwrap_or_default();
    if !text.is_empty() {
        for line in wrap(text, (body.width as usize).saturating_sub(2)) {
            help.push(Line::from(Span::styled(line, theme.muted())));
        }
    }
    help.extend(notice(app));
    ui::prose(frame, rows[4], help, theme.body());
}

/// One row per option, on the grid: cursor, changed marker, name, then the value in a column of
/// its own. Every row is the same width, which is the whole point of the columns.
pub(crate) fn option_rows(app: &App, width: u16) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let name_width = (width as usize).saturating_sub(VALUE_COLUMN + 5).max(8);
    app.current_entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let chosen = index == app.cursor;
            Line::from(vec![
                Span::styled(
                    format!(" {} ", if chosen { glyphs.cursor() } else { " " }),
                    if chosen {
                        theme.selected()
                    } else {
                        theme.body()
                    },
                ),
                Span::styled(
                    if menu::is_changed(entry, &app.answers) {
                        format!("{} ", glyphs.changed())
                    } else {
                        "  ".to_string()
                    },
                    theme.accent(),
                ),
                Span::styled(
                    format!(
                        "{:<width$}",
                        ui::fit(entry.title(), name_width, glyphs.ellipsis()),
                        width = name_width
                    ),
                    if chosen {
                        theme.selected()
                    } else {
                        theme.body()
                    },
                ),
                Span::styled(
                    format!("{:>width$}", value_of(entry, app), width = VALUE_COLUMN),
                    if chosen {
                        theme.selected()
                    } else {
                        theme.muted()
                    },
                ),
            ])
        })
        .collect()
}

/// The right-hand value column: one shape per kind of answer.
fn value_of(entry: &'static Entry, app: &App) -> String {
    let glyphs = app.glyphs;
    match entry {
        Entry::Toggle { .. } => on_off(menu::value(entry, &app.answers) == Some("true"), glyphs),
        Entry::Preset { .. } => on_off(menu::preset_on(entry, &app.answers), glyphs),
        Entry::Choice { .. } => format!(
            "{} {} {}",
            glyphs.open(),
            menu::value(entry, &app.answers).unwrap_or_default(),
            glyphs.close()
        ),
        Entry::Submenu { .. } => glyphs.submenu().to_string(),
        Entry::Action { .. } => String::new(),
    }
}

fn on_off(on: bool, glyphs: Glyphs) -> String {
    if on {
        format!("{} on", glyphs.status(Status::Ok))
    } else {
        format!("{} off", glyphs.mark())
    }
}

// Review ----------------------------------------------------------------------------

fn review(frame: &mut Frame, app: &App, body: Rect) {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let (summary, list) = ui::review_layout(body);

    let will_run = STEPS
        .iter()
        .filter(|step| !crate::steps::is_skipped(step, &app.answers))
        .count();
    let changed = count_changed(app);
    ui::prose(
        frame,
        summary,
        vec![Line::from(vec![
            Span::styled(
                format!(
                    "{} of {} will run",
                    will_run,
                    ui::plural(STEPS.len(), "step", "steps")
                ),
                theme.body(),
            ),
            Span::styled(" · ", theme.muted()),
            Span::styled(
                if changed == 0 {
                    "every option is at its default".to_string()
                } else {
                    ui::plural(changed, "option changed", "options changed")
                },
                if changed == 0 {
                    theme.muted()
                } else {
                    theme.accent()
                },
            ),
        ])],
        theme.body(),
    );

    let (left, right) = review_columns(app);
    let total = left.len().max(right.as_ref().map_or(0, Vec::len));
    let rows = list.height.max(1) as usize;
    let offset = app.review_scroll.min(total.saturating_sub(rows)) as u16;

    let (left_area, right_area) = ui::columns(list);
    render_list(frame, left_area, left, offset);
    if let (Some(right_lines), Some(area)) = (right, right_area) {
        render_list(frame, area, right_lines, offset);
    }
    if total > rows {
        let bar = Rect {
            x: list.x + list.width.saturating_sub(1),
            width: 1,
            ..list
        };
        ui::scrollbar(frame, bar, app.review_scroll, total, theme, glyphs);
    }
}

fn render_list(frame: &mut Frame, area: Rect, lines: Vec<Line<'static>>, offset: u16) {
    frame.render_widget(Paragraph::new(Text::from(lines)).scroll((offset, 0)), area);
}

/// Every phase with its steps, as one block per phase so the two columns can be balanced.
fn review_sections(app: &App) -> Vec<Vec<Line<'static>>> {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let mut sections = Vec::new();
    for phase in Phase::ALL {
        let mut section: Vec<Line<'static>> = vec![Line::from(vec![
            Span::styled(phase.title().to_uppercase(), theme.title()),
            Span::styled(
                format!(
                    " · {} of {}",
                    STEPS.iter().filter(|step| step.phase == phase).count(),
                    ui::plural(STEPS.len(), "step", "steps")
                ),
                theme.muted(),
            ),
        ])];
        for step in STEPS.iter().filter(|step| step.phase == phase) {
            let skipped = crate::steps::is_skipped(step, &app.answers);
            let status = if skipped {
                Status::Skipped
            } else {
                Status::Pending
            };
            section.push(Line::from(vec![
                Span::styled(" ".to_string(), theme.body()),
                ui::gutter(glyphs.status(status), theme.status(status), glyphs),
                Span::styled(
                    ui::fit(step.name, 52, glyphs.ellipsis()),
                    if skipped { theme.muted() } else { theme.body() },
                ),
            ]));
        }
        section.push(blank());
        sections.push(section);
    }
    sections
}

/// The review list split into two columns where there is room, balanced by line count.
fn review_columns(app: &App) -> (Vec<Line<'static>>, Option<Vec<Line<'static>>>) {
    let sections = review_sections(app);
    let (_, list) = ui::review_layout(ui::body(app.area));
    if ui::columns(list).1.is_none() {
        return (sections.concat(), None);
    }

    let heights: Vec<usize> = sections.iter().map(Vec::len).collect();
    let total: usize = heights.iter().sum();
    let mut running = 0;
    let mut split = 0;
    for (index, height) in heights.iter().enumerate() {
        if running + height > total / 2 {
            break;
        }
        running += height;
        split = index + 1;
    }
    let mut left = Vec::new();
    let mut right = Vec::new();
    for (index, section) in sections.into_iter().enumerate() {
        if index < split {
            left.extend(section);
        } else {
            right.extend(section);
        }
    }
    (left, Some(right))
}

/// How many lines the review list holds: what the page keys clamp against.
pub fn review_total(app: &App) -> usize {
    let (left, right) = review_columns(app);
    left.len().max(right.as_ref().map_or(0, Vec::len))
}

fn count_changed(app: &App) -> usize {
    fn walk(entries: &'static [Entry], app: &App) -> usize {
        entries
            .iter()
            .map(|entry| match entry {
                Entry::Submenu { items, .. } => walk(items, app),
                other => usize::from(menu::is_changed(other, &app.answers)),
            })
            .sum()
    }
    walk(&menu::ROOT, app)
}

// Run -------------------------------------------------------------------------------

fn run(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let layout = app.run_layout();

    ui::prose(frame, layout.checklist, checklist(app), theme.body());
    ui::rule(frame, layout.rule, theme, app.glyphs);
    ui::prose(
        frame,
        layout.progress,
        vec![ui::progress(
            app.finished(),
            STEPS.len(),
            layout.progress.width,
            &format!(
                "{}/{} · {}",
                app.finished(),
                STEPS.len(),
                ui::elapsed(app.total_elapsed().as_secs())
            ),
            theme,
            app.glyphs,
        )],
        theme.body(),
    );
    log_pane(frame, app, layout.log, app.log_rows());
}

/// The checklist with the finished phases collapsed to a line each: seventeen rows of list became
/// five, and the log got the room back.
pub(crate) fn checklist(app: &App) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let mut lines = Vec::new();

    for phase in Phase::ALL {
        let status = app.phase_status(phase);
        let (done, total) = app.phase_progress(phase);
        let running = STEPS
            .iter()
            .enumerate()
            .any(|(index, step)| step.phase == phase && app.statuses[index] == Status::Running);
        let glyph = if running {
            ui::running_glyph(app)
        } else {
            glyphs.status(status)
        };
        let style = if running {
            theme.selected()
        } else {
            theme.status(status)
        };

        let mut spans = vec![
            ui::gutter(glyph, style, glyphs),
            Span::styled(
                phase.title().to_string(),
                if running {
                    theme.selected()
                } else {
                    theme.body()
                },
            ),
        ];
        if running {
            spans.push(Span::styled(
                format!(" · {done} of {total} steps"),
                theme.muted(),
            ));
        } else {
            spans.push(Span::styled(
                format!(" · {}", ui::plural(total, "step", "steps")),
                theme.muted(),
            ));
            let elapsed = app.phase_elapsed(phase);
            if elapsed.as_secs() > 0 {
                spans.push(Span::styled(
                    format!(" · {}", ui::elapsed(elapsed.as_secs())),
                    theme.muted(),
                ));
            }
            spans.extend(phase_notes(app, phase));
        }
        lines.push(Line::from(spans));

        if let Some(index) = phase_detail(app, phase) {
            lines.push(detail_line(app, index));
        }
    }
    lines
}

/// What did not go cleanly in a phase that has finished.
fn phase_notes(app: &App, phase: Phase) -> Vec<Span<'static>> {
    let theme = &app.theme;
    let count = |status: Status| {
        STEPS
            .iter()
            .enumerate()
            .filter(|(index, step)| step.phase == phase && app.statuses[*index] == status)
            .count()
    };
    let mut spans = Vec::new();
    for (count, one, many, style) in [
        (
            count(Status::Failed),
            "failure",
            "failures",
            theme.error_text(),
        ),
        (
            count(Status::Ignored),
            "ignored",
            "ignored",
            theme.ignored_text(),
        ),
        (
            count(Status::Warn),
            "warning",
            "warnings",
            theme.warn_text(),
        ),
        (count(Status::Skipped), "skipped", "skipped", theme.muted()),
    ] {
        if count > 0 {
            spans.push(Span::styled(
                format!(" · {}", ui::plural(count, one, many)),
                style,
            ));
        }
    }
    spans
}

/// The step of a phase that deserves a line of its own: the one running, or the one that stopped
/// the run.
fn phase_detail(app: &App, phase: Phase) -> Option<usize> {
    let running = STEPS
        .iter()
        .enumerate()
        .find(|(index, step)| step.phase == phase && app.statuses[*index] == Status::Running);
    if let Some((index, _)) = running {
        return Some(index);
    }
    if !app.paused {
        return None;
    }
    STEPS
        .iter()
        .enumerate()
        .find(|(index, step)| step.phase == phase && app.statuses[*index].is_failure())
        .map(|(index, _)| index)
}

fn detail_line(app: &App, index: usize) -> Line<'static> {
    let theme = &app.theme;
    let glyphs = app.glyphs;
    let status = app.statuses[index];
    let running = status == Status::Running;
    let glyph = if running {
        ui::running_glyph(app)
    } else {
        glyphs.status(status)
    };
    let mut spans = vec![
        Span::styled("  ".to_string(), theme.body()),
        ui::gutter(
            glyph,
            if running {
                theme.selected()
            } else {
                theme.status(status)
            },
            glyphs,
        ),
        Span::styled(
            STEPS[index].name.to_string(),
            if running {
                theme.selected()
            } else {
                theme.body()
            },
        ),
        Span::raw("  "),
        ui::label_cell(status, theme),
    ];
    if let Some(elapsed) = app.step_elapsed(index) {
        spans.push(Span::styled(
            format!("  {}", ui::elapsed(elapsed.as_secs())),
            theme.muted(),
        ));
    }
    Line::from(spans)
}

/// How many rows the checklist needs, capped so the log is never squeezed out.
pub fn checklist_rows(app: &App) -> u16 {
    let lines = checklist(app).len() as u16;
    lines.min(ui::checklist_budget(ui::body(app.area))).max(1)
}

/// A log pane: a heading row, then the lines, then a scrollbar. The run screen and the
/// full-screen log draw the same thing at different sizes.
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
            lines.push(ui::labelled(
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
        lines.push(ui::labelled(
            "retry",
            "r here, or that command in a terminal".to_string(),
            theme,
            LABEL_COLUMN,
        ));
    }
    lines.push(ui::labelled(
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
        "KEYS",
        (62, entries.len() as u16 + 6),
        Role::Rule,
    );
    let mut lines: Vec<Line<'static>> = Vec::new();
    for (keys, what) in entries {
        lines.push(Line::from(vec![
            Span::styled(format!("  {keys:<12}"), theme.accent()),
            Span::styled(what.to_string(), theme.body()),
        ]));
    }
    lines.push(blank());
    lines.push(Line::from(Span::styled(
        format!("  {} closes this", glyphs.escape()),
        theme.muted(),
    )));
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
        "INSTALL LOG",
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
        "STEP FAILED",
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
    lines.push(ui::labelled(
        "log",
        app.log_path.display().to_string(),
        theme,
        LABEL_COLUMN,
    ));
    lines.push(ui::labelled(
        "reproduce",
        card.reproduction(),
        theme,
        LABEL_COLUMN,
    ));
    lines.push(blank());
    for (index, choice) in FailureCard::CHOICES.iter().enumerate() {
        let chosen = index == card.choice;
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", if chosen { glyphs.cursor() } else { " " }),
                if chosen {
                    theme.selected()
                } else {
                    theme.muted()
                },
            ),
            Span::styled(
                format!("{} {:<8}", choice.key, choice.label),
                if chosen {
                    theme.selected()
                } else {
                    theme.body()
                },
            ),
            Span::styled(choice.help.to_string(), theme.muted()),
        ]));
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
    let (title, subject, question, note, border) = match app.confirm {
        Some(Confirm::Handoff(action)) => (
            action.header().to_uppercase(),
            action.title(),
            action.help(),
            "The installer closes and hands the terminal over.".to_string(),
            Role::Accent,
        ),
        Some(Confirm::Quit) | None => (
            "STOP".to_string(),
            "Stop the running step?",
            "The step is stopped before the installer leaves, so nothing is left running in the \
             background. Work it had already done stays done.",
            "y stops it, then quits".to_string(),
            Role::Warn,
        ),
    };

    let inner = ui::panel(frame, frame.area(), theme, glyphs, &title, (70, 12), border);
    let mut lines = vec![
        Line::from(Span::styled(subject.to_string(), theme.heading())),
        blank(),
    ];
    for line in wrap(question, (inner.width as usize).saturating_sub(2)) {
        lines.push(Line::from(Span::styled(line, theme.muted())));
    }
    lines.push(blank());
    lines.push(Line::from(vec![
        Span::styled(" y ".to_string(), theme.selected()),
        Span::styled("yes".to_string(), theme.body()),
        Span::styled("     n ".to_string(), theme.muted()),
        Span::styled("no".to_string(), theme.body()),
    ]));
    lines.push(Line::from(Span::styled(format!(" {note}"), theme.muted())));
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
    use crate::app::{Flow, Overlay, Stage};
    use crate::env::Env;
    use std::path::PathBuf;

    fn app() -> App {
        App::new(
            PathBuf::from("/tmp/caelestia"),
            PathBuf::from("/tmp/install.log"),
            Env::default(),
        )
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

    /// The rows that mention a phrase: a log line appears twice, once in the pane and once as the
    /// activity line, so a test asks about all of them.
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
    fn the_home_screen_offers_the_three_actions_and_says_what_they_do() {
        let text = render(&mut app(), 100, 30);
        assert!(text.contains("Install Caelestia"), "{text}");
        assert!(text.contains("Update this install"), "{text}");
        assert!(text.contains("Uninstall Caelestia"), "{text}");
        assert!(text.contains(theme::TAGLINE), "{text}");
        assert!(
            text.contains("_____"),
            "the mark is drawn where it fits: {text}"
        );
        assert!(text.contains(theme::AUTHOR), "{text}");
    }

    #[test]
    fn the_mark_is_dropped_rather_than_clipped_when_there_is_no_room_for_it() {
        let text = render(&mut app(), 62, 22);
        assert!(!text.contains("_____"), "{text}");
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
        // A password in flight used to leave the frame looking frozen, which is what had people
        // typing it again while the first attempt was still going.
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
        // And a clipped row is still the same width as its neighbours.
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
        app.cursor = 0;
        let before = render(&mut app, 100, 30);
        assert!(!before.contains("*"), "{before}");

        app.choose();
        let after = render(&mut app, 100, 30);
        assert!(after.contains("•"), "the change is marked: {after}");
        assert!(
            after.contains("stays editable"),
            "and the help is on screen: {after}"
        );
    }

    #[test]
    fn the_review_screen_lists_every_phase_and_marks_what_is_switched_off() {
        let mut app = app();
        app.stage = Stage::Review;
        app.answers.insert("SKIP_SYSTEM_UPDATE", "true".to_string());
        let text = render(&mut app, 80, 40);
        assert!(text.contains("PREPARE"), "{text}");
        assert!(text.contains("FINALIZE"), "{text}");
        assert!(text.contains("Update system"), "{text}");
        assert!(
            text.contains("will run"),
            "the summary answers the question: {text}"
        );
        assert!(
            text.contains("– "),
            "and a switched-off step is marked: {text}"
        );
    }

    #[test]
    fn the_review_screen_scrolls_rather_than_cutting_the_list_off() {
        let mut app = app();
        app.stage = Stage::Review;
        let narrow = render(&mut app, 80, 24);
        assert!(narrow.contains("PREPARE"), "{narrow}");
        assert!(
            !narrow.contains("FINALIZE"),
            "the end of the list is below the fold: {narrow}"
        );

        app.review_scroll = app.review_limit();
        let scrolled = render(&mut app, 80, 24);
        assert!(scrolled.contains("FINALIZE"), "{scrolled}");
    }

    #[test]
    fn the_review_screen_uses_two_columns_where_there_is_room() {
        let mut app = app();
        app.stage = Stage::Review;
        let wide = render(&mut app, 140, 30);
        // Both the first phase and the last are on screen at once, which one column could not do.
        assert!(wide.contains("PREPARE"), "{wide}");
        assert!(wide.contains("FINALIZE"), "{wide}");
    }

    #[test]
    fn the_run_screen_shows_the_worst_of_each_phase_on_one_line() {
        let mut app = running(
            &[
                Status::Ok,
                Status::Warn,
                Status::Ok,
                Status::Ok,
                Status::Running,
            ],
            &["  [INFO]  syncing databases", "  [WARN]  slow mirror"],
        );
        let text = render(&mut app, 100, 30);
        assert!(text.contains("Prepare"), "{text}");
        assert!(
            text.contains("1 warning"),
            "the phase says what happened: {text}"
        );
        assert!(text.contains("Packages"), "{text}");
        assert!(
            text.contains("1 of 2 steps"),
            "and how far along it is: {text}"
        );
        assert!(text.contains("RUNNING"), "the state is spelled out: {text}");
        assert!(
            !text.contains("Update submodules"),
            "past steps are collapsed: {text}"
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
        assert!(text.contains("STEP FAILED"), "{text}");
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
        app.confirm = Some(crate::app::Confirm::Quit);
        app.overlay = Some(Overlay::Confirm);
        let text = render(&mut app, 100, 30);
        assert!(text.contains("Stop the running step?"), "{text}");
        assert!(
            text.contains("stopped before the installer leaves"),
            "{text}"
        );
        assert!(text.contains("y yes"), "{text}");
    }

    #[test]
    fn the_confirm_card_names_the_action_it_hands_over_to() {
        let mut app = app();
        app.confirm = Some(crate::app::Confirm::Handoff(crate::app::Action::Uninstall));
        app.overlay = Some(Overlay::Confirm);
        let text = render(&mut app, 100, 30);
        assert!(text.contains("UNINSTALL"), "{text}");
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
            Stage::Review,
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
