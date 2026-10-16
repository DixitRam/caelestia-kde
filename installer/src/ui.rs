// The chrome and the grid everything else is built on.
//
// Two rules decide this file. First, the surface is the container: a border is spent only on a
// modal, and sections are separated by a single rule, which gives back the four rows and the
// double frame the boxed screens used to cost. Second, everything sits on a grid: one indent, one
// width for a status glyph, one width for a state label, one right-hand column for counts.
//
// The layouts live here rather than in the screens so that the code that draws a pane and the code
// that scrolls it cannot disagree about how tall it is.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::app::App;
use crate::glyphs::{Glyphs, LABEL_CELL, status_label};
use crate::steps::Status;
use crate::theme::{Role, Theme};

/// Below this the TUI says so rather than drawing a broken frame.
pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 20;

/// Content stops growing here: past it a line is easier to read than a wider one.
/// The widest the content box ever gets. A line longer than this is hard to track back from, so
/// past it the extra width is left as margin - but below it the box grows with the terminal,
/// because a 100-column column in a 230-column window reads as a small window, not a wide one.
pub const MAX_CONTENT: u16 = 150;

/// The one left gutter every screen indents by.
pub const INDENT: u16 = 2;

/// The log pane spends its first row on a heading, which is not a line of log.
const LOG_HEADER_ROWS: u16 = 1;

/// One hint in the footer.
pub struct Key {
    pub keys: &'static str,
    pub what: &'static str,
}

pub const fn key(keys: &'static str, what: &'static str) -> Key {
    Key { keys, what }
}

/// What the header says on the left and on the right.
pub struct Header {
    pub left: String,
    pub right: String,
}

/// The rows the chrome owns, and the body left between them.
pub struct Chrome {
    pub body: Rect,
}

struct Rows {
    header: Rect,
    top_rule: Rect,
    body: Rect,
    bottom_rule: Rect,
    footer: Rect,
}

fn rows(area: Rect) -> Rows {
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas::<5>(area);
    Rows {
        header: rows[0],
        top_rule: rows[1],
        body: rows[2],
        bottom_rule: rows[3],
        footer: rows[4],
    }
}

pub struct RunLayout {
    pub checklist: Rect,
    pub rule: Rect,
    pub progress: Rect,
    pub log: Rect,
}

/// Draw the background, the header, the rules and the footer. Returns `None` when the terminal is
/// too small to hold a screen, having said so.
pub fn chrome(
    frame: &mut Frame,
    theme: &Theme,
    glyphs: Glyphs,
    header: &Header,
    keys: &[Key],
    counter: Option<&str>,
) -> Option<Chrome> {
    let area = frame.area();
    frame.render_widget(Block::default().style(theme.surface()), area);

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        too_small(frame, area, theme, glyphs);
        return None;
    }

    let rows = rows(area);
    heading(frame, rows.header, theme, header);
    rule(frame, rows.top_rule, theme, glyphs);
    footer(frame, rows.footer, theme, keys, counter);
    rule(frame, rows.bottom_rule, theme, glyphs);

    Some(Chrome {
        body: content(rows.body),
    })
}

/// The content box a screen draws into, derived from the whole area. The key handlers ask for the
/// same box, which is what keeps paging and drawing in agreement.
pub fn body(area: Rect) -> Rect {
    content(rows(area).body)
}

/// A horizontal rule: the only separator the TUI uses.
pub fn rule(frame: &mut Frame, area: Rect, theme: &Theme, glyphs: Glyphs) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let line = Line::from(Span::styled(
        glyphs.rule().to_string().repeat(area.width as usize),
        theme.rule(),
    ));
    frame.render_widget(Paragraph::new(line), area);
}

/// The body's content box: centred, capped, and indented by the one gutter.
pub fn content(area: Rect) -> Rect {
    let width = area.width.min(MAX_CONTENT);
    let left = area.x + area.width.saturating_sub(width) / 2;
    Rect {
        x: left + INDENT.min(width.saturating_sub(1)),
        y: area.y,
        width: width.saturating_sub(INDENT + INDENT).max(1),
        height: area.height,
    }
}

/// A centred modal. Borders are spent here and nowhere else.
pub fn panel(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    glyphs: Glyphs,
    title: &str,
    size: (u16, u16),
    border: Role,
) -> Rect {
    let width = size.0.min(area.width.saturating_sub(2)).max(8);
    let height = size.1.min(area.height).max(3);
    frame_panel(
        frame,
        centred(area, width, height),
        theme,
        glyphs,
        title,
        border,
    )
}

/// The same modal, placed by the caller: the full-screen log needs the same rectangle the key
/// handlers were given.
pub fn frame_panel(
    frame: &mut Frame,
    rect: Rect,
    theme: &Theme,
    glyphs: Glyphs,
    title: &str,
    border: Role,
) -> Rect {
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_set(glyphs.border())
        .title(format!(" {title} "))
        .title_style(theme.style(border))
        .border_style(theme.style(border));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    inner
}

/// The gutter a status glyph sits in, padding included, so the names beside it never shift
/// whatever the state is.
pub fn gutter(text: &str, style: Style, glyphs: Glyphs) -> Span<'static> {
    Span::styled(
        format!(" {:>width$} ", text, width = glyphs.status_cell()),
        style,
    )
}

/// The state, spelled out, padded to a column of its own.
pub fn label_cell(status: Status, theme: &Theme) -> Span<'static> {
    Span::styled(
        format!("{:>width$}", status_label(status), width = LABEL_CELL),
        theme.status(status),
    )
}

/// The glyph for a step that is working: the spinner where the terminal can show one, and the
/// static state glyph where it cannot. Either way the label beside it says "running".
pub fn running_glyph(app: &App) -> &'static str {
    if app.env.animates() {
        app.glyphs.spinner(app.spinner)
    } else {
        app.glyphs.status(Status::Running)
    }
}

/// `label   value`, with the label in a fixed column: the grid rule applied to prose.
pub fn labelled(label: &str, value: String, theme: &Theme, width: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {label:<width$}  "), theme.muted()),
        Span::styled(value, theme.body()),
    ])
}

/// A progress bar with its caption. The bar never grows past a readable length: a full-width
/// stripe would outweigh everything above it.
pub fn progress(
    filled: usize,
    total: usize,
    width: u16,
    caption: &str,
    theme: &Theme,
    glyphs: Glyphs,
) -> Line<'static> {
    const LONGEST_BAR: usize = 40;
    let suffix = format!(" {caption}");
    let room = (width as usize).saturating_sub(suffix.len());
    let bar = room.min(LONGEST_BAR);
    let done = if total == 0 {
        0
    } else {
        (filled * bar).div_ceil(total).min(bar)
    };
    let mut spans = vec![
        Span::styled(glyphs.bar_full().to_string().repeat(done), theme.accent()),
        Span::styled(
            glyphs.bar_empty().to_string().repeat(bar - done),
            theme.rule(),
        ),
    ];
    if room >= suffix.len() {
        spans.push(Span::styled(suffix, theme.muted()));
    }
    Line::from(spans)
}

/// A one-column scrollbar, drawn only where there is something to scroll: a pane that fits its
/// content gets the column back rather than a bare track.
pub fn scrollbar(
    frame: &mut Frame,
    area: Rect,
    offset: usize,
    total: usize,
    theme: &Theme,
    glyphs: Glyphs,
) {
    if area.height == 0 || area.width == 0 || total <= area.height as usize {
        return;
    }
    let viewport = area.height as usize;
    let (from, to) = thumb_range(offset, total, viewport);
    let lines: Vec<Line> = (0..viewport)
        .map(|row| {
            if row >= from && row < to {
                Line::from(Span::styled(glyphs.thumb().to_string(), theme.accent()))
            } else {
                Line::from(Span::styled(glyphs.track().to_string(), theme.rule()))
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(Text::from(lines)), area);
}

/// Where the thumb sits, as a range of rows. A pane that fits its content gets no thumb.
pub fn thumb_range(offset: usize, total: usize, viewport: usize) -> (usize, usize) {
    if viewport == 0 {
        return (0, 0);
    }
    if total <= viewport {
        return (0, 0);
    }
    let thumb = (viewport * viewport / total).max(1).min(viewport);
    let furthest = total - viewport;
    let position = (offset.min(furthest) * (viewport - thumb))
        .checked_div(furthest)
        .unwrap_or(0);
    (position, position + thumb)
}

/// How many rows of log fit, given the pane the layout handed out.
pub fn log_rows(log: Rect) -> u16 {
    log.height.saturating_sub(LOG_HEADER_ROWS)
}

/// The most rows the phase checklist may take, leaving the log the room to be read.
pub fn checklist_budget(body: Rect) -> u16 {
    body.height.saturating_sub(6).max(2)
}

pub fn run_layout(body: Rect, checklist_rows: u16) -> RunLayout {
    let rows = Layout::vertical([
        Constraint::Length(checklist_rows),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
    ])
    .areas::<4>(body);
    RunLayout {
        checklist: rows[0],
        rule: rows[1],
        progress: rows[2],
        log: rows[3],
    }
}

/// The summary row and the scrolling list of the review screen.
pub fn review_layout(body: Rect) -> (Rect, Rect) {
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
    ])
    .areas::<3>(body);
    (rows[0], rows[2])
}

/// Two columns of phases where there is room for them, one where there is not.
pub fn columns(list: Rect) -> (Rect, Option<Rect>) {
    if list.width < 90 {
        return (list, None);
    }
    let width = list.width.saturating_sub(4) / 2;
    let left = Rect { width, ..list };
    let right = Rect {
        x: list.x + width + 4,
        width,
        ..list
    };
    (left, Some(right))
}

/// Render prose into a box, wrapping rather than clipping it.
pub fn prose(frame: &mut Frame, area: Rect, lines: Vec<Line<'static>>, style: Style) {
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(style)
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// The same text, started a third of the way down when the area is taller than the text needs.
/// A short screen in a tall window otherwise sits against the top edge with the rest of the
/// window empty, which reads as a rendering fault rather than as room to breathe.
pub fn prose_middle(frame: &mut Frame, area: Rect, lines: Vec<Line<'static>>, style: Style) {
    let slack = (area.height as usize).saturating_sub(lines.len());
    let pad = (slack / 3).min(12) as u16;
    prose(
        frame,
        Rect {
            y: area.y + pad,
            height: area.height.saturating_sub(pad),
            ..area
        },
        lines,
        style,
    );
}

/// The full-screen log: its own modal, so it has its own geometry, and both the drawing and the
/// paging ask for it here.
pub fn log_overlay_rect(area: Rect) -> Rect {
    let width = area.width.saturating_sub(4).clamp(10, MAX_CONTENT + 20);
    let height = area.height.saturating_sub(4).max(6);
    centred(area, width, height)
}

pub fn log_overlay_inner(area: Rect) -> Rect {
    let rect = log_overlay_rect(area);
    Rect {
        x: rect.x + 1,
        y: rect.y + 1,
        width: rect.width.saturating_sub(2),
        height: rect.height.saturating_sub(2),
    }
}

pub fn log_overlay_rows(area: Rect) -> u16 {
    log_overlay_inner(area)
        .height
        .saturating_sub(LOG_HEADER_ROWS)
}

/// Text cut to a column, with a marker rather than a silent chop.
pub fn fit(text: &str, width: usize, ellipsis: &str) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let keep = width.saturating_sub(ellipsis.chars().count());
    let mut out: String = text.chars().take(keep).collect();
    out.push_str(ellipsis);
    out
}

pub fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("{count} {one}")
    } else {
        format!("{count} {many}")
    }
}

pub fn elapsed(seconds: u64) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn heading(frame: &mut Frame, area: Rect, theme: &Theme, header: &Header) {
    let left = Line::from(vec![
        Span::styled(" ", theme.body()),
        Span::styled(crate::theme::NAME.to_string(), theme.title()),
        Span::styled(format!(" · {}", header.left), theme.muted()),
    ]);
    let right_width = header.right.len() as u16 + 2;
    if area.width > left.width() as u16 + right_width {
        let right = Rect {
            x: area.x + area.width - right_width,
            width: right_width,
            ..area
        };
        frame.render_widget(Paragraph::new(left.clone()), area);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("{} ", header.right),
                theme.muted(),
            )))
            .alignment(Alignment::Right),
            right,
        );
    } else {
        frame.render_widget(Paragraph::new(left), area);
    }
}

fn footer(frame: &mut Frame, area: Rect, theme: &Theme, keys: &[Key], counter: Option<&str>) {
    let counter_width = counter.map_or(0, |text| text.len() as u16 + 1);
    let available = area.width.saturating_sub(counter_width);

    frame.render_widget(Paragraph::new(hints(theme, keys, available)), area);
    if let Some(counter) = counter {
        let right = Rect {
            x: area.x + area.width - counter_width,
            width: counter_width,
            ..area
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("{counter} "),
                theme.muted(),
            )))
            .alignment(Alignment::Right),
            right,
        );
    }
}

/// The keymap for a screen, whole groups only, dropping the least important hint first.
fn hints(theme: &Theme, keys: &[Key], width: u16) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for (index, key) in keys.iter().enumerate() {
        let group = format!("{} {}", key.keys, key.what);
        let separator = if index == 0 { 1 } else { 3 };
        let cost = group.len() + separator;
        if index > 0 && used + cost > width as usize {
            break;
        }
        if index > 0 {
            spans.push(Span::styled("   ".to_string(), theme.muted()));
        } else {
            spans.push(Span::styled(" ".to_string(), theme.muted()));
        }
        spans.push(Span::styled(
            key.keys.to_string(),
            if index == 0 {
                theme.accent()
            } else {
                theme.body()
            },
        ));
        spans.push(Span::styled(format!(" {}", key.what), theme.muted()));
        used += cost;
    }
    Line::from(spans)
}

fn too_small(frame: &mut Frame, area: Rect, theme: &Theme, glyphs: Glyphs) {
    let lines = vec![
        Line::from(Span::styled(
            "terminal too small".to_string(),
            theme.title(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                "this needs {}x{}, and this one is {}x{}",
                MIN_WIDTH, MIN_HEIGHT, area.width, area.height
            ),
            theme.muted(),
        )),
    ];
    let height = lines.len() as u16 + 4;
    let inner = panel(
        frame,
        area,
        theme,
        glyphs,
        "RESIZE",
        (56, height),
        Role::Rule,
    );
    prose(frame, inner, lines, theme.body());
}

fn centred(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(width: u16, height: u16) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    #[test]
    fn the_content_box_never_escapes_its_area() {
        for (width, height) in [(60, 20), (80, 24), (100, 30), (200, 60)] {
            let area = body(width, height);
            let inner = content(area);
            assert!(inner.x + inner.width <= area.width, "{width}x{height}");
            assert!(inner.width > 0, "{width}x{height}");
        }
    }

    #[test]
    fn wide_terminals_stop_growing() {
        let inner = content(body(240, 60));
        assert_eq!(inner.width, MAX_CONTENT - INDENT * 2);
        // And what there is, is centred.
        assert_eq!(inner.x, (240 - MAX_CONTENT) / 2 + INDENT);
    }

    #[test]
    fn the_log_keeps_its_rows_whatever_the_terminal_size() {
        for height in MIN_HEIGHT..=60 {
            let area = body(100, height);
            let budget = checklist_budget(area);
            let layout = run_layout(area, budget);
            assert!(
                log_rows(layout.log) >= 3,
                "{height} rows left the log {}",
                log_rows(layout.log)
            );
            assert!(
                layout.log.y + layout.log.height <= area.height,
                "{height} rows overflow"
            );
            assert_eq!(layout.checklist.y, 0);
            assert_eq!(layout.progress.y, budget + 1);
        }
    }

    #[test]
    fn the_checklist_is_capped_so_the_log_is_never_squeezed_out() {
        assert_eq!(checklist_budget(body(100, MIN_HEIGHT)), MIN_HEIGHT - 6);
        assert!(checklist_budget(body(100, 4)) >= 2);
    }

    #[test]
    fn two_columns_only_appear_where_they_fit() {
        assert!(columns(body(60, 20)).1.is_none());
        assert!(columns(body(80, 20)).1.is_none());
        let (left, right) = columns(body(100, 20));
        let right = right.expect("a second column");
        assert!(left.width > 10);
        assert!(right.x > left.x + left.width);
        assert!(right.x + right.width <= 100);
    }
    #[test]
    fn a_scrollbar_thumb_stays_inside_the_pane() {
        for total in [1usize, 5, 20, 200] {
            for viewport in [1usize, 3, 10, 20] {
                for offset in [0usize, 1, 7, 100, 1000] {
                    let (from, to) = thumb_range(offset, total, viewport);
                    assert!(to <= viewport, "total={total} viewport={viewport}");
                    assert!(from <= to);
                }
            }
        }
    }

    #[test]
    fn a_pane_that_fits_its_content_shows_no_thumb() {
        assert_eq!(thumb_range(0, 5, 10), (0, 0));
        assert_eq!(thumb_range(0, 10, 10), (0, 0));
    }

    #[test]
    fn the_thumb_reaches_both_ends() {
        let (from, _) = thumb_range(0, 100, 10);
        assert_eq!(from, 0);
        let (from, to) = thumb_range(90, 100, 10);
        assert_eq!(to, 10);
        assert!(from > 0);
    }

    fn theme() -> Theme {
        Theme::resolve(&crate::env::Env::default())
    }

    #[test]
    fn the_progress_bar_spends_its_width_exactly() {
        let theme = theme();
        let glyphs = Glyphs::resolve(&crate::env::Env::default());
        for width in [20u16, 40, 60, 100] {
            let line = progress(3, 17, width, "3/17 · 1:12", &theme, glyphs);
            assert!(
                line.width() <= width as usize,
                "width {width} drew {}",
                line.width()
            );
        }
        assert!(progress(0, 0, 40, "0/0", &theme, glyphs).width() <= 40);
        assert!(progress(17, 17, 40, "17/17", &theme, glyphs).width() > 0);
    }

    #[test]
    fn hints_drop_from_the_end_rather_than_overflowing() {
        let theme = theme();
        let keys = [
            key("Enter", "continue"),
            key("Esc", "back"),
            key("?", "keys"),
            key("q", "quit"),
        ];
        let wide = hints(&theme, &keys, 200).width();
        assert!(hints(&theme, &keys, 80).width() <= 80);
        assert!(hints(&theme, &keys, 40).width() <= 40);
        assert!(hints(&theme, &keys, 40).width() < wide);
        // The first hint is the one that always survives.
        let narrow = hints(&theme, &keys, 0);
        assert!(narrow.width() > 0);
        assert!(narrow.width() <= "Enter continue".len() + 2);
    }

    #[test]
    fn counts_read_like_english() {
        assert_eq!(plural(0, "step", "steps"), "0 steps");
        assert_eq!(plural(1, "step", "steps"), "1 step");
        assert_eq!(plural(2, "step", "steps"), "2 steps");
    }

    #[test]
    fn elapsed_time_grows_the_way_a_clock_does() {
        assert_eq!(elapsed(0), "0:00");
        assert_eq!(elapsed(9), "0:09");
        assert_eq!(elapsed(72), "1:12");
        assert_eq!(elapsed(3661), "1:01:01");
    }

    #[test]
    fn a_label_column_lines_up() {
        let theme = theme();
        let short = labelled("log", "x".to_string(), &theme, 7);
        let long = labelled("retry", "x".to_string(), &theme, 7);
        assert_eq!(short.width(), long.width());
    }
}
