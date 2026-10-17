//! What a run would do, printed instead of done.
//!
//! The Review screen already works this out before the user commits; this is the same plan for
//! anyone outside the TUI — a pipeline that wants to see the steps, a person deciding whether to
//! run the thing, or a support engineer asking what this machine would actually attempt. Nothing
//! in here touches the machine: no package manager, no sudo, no writes.

use std::path::Path;

use crate::menu::{self, Answers};
use crate::steps::{self, Phase};

pub struct Row {
    pub phase: Phase,
    pub id: &'static str,
    pub name: &'static str,
    pub runs: bool,
}

/// The answers a run would use, built the way the wizard builds them: the table's defaults, then
/// what this machine actually has, then whatever the caller overrode. Anything the caller did not
/// answer stays at the default it would have been shown.
pub fn answers(set: &[(&'static str, String)]) -> Answers {
    let mut answers = Answers::new();
    menu::seed_answers(&menu::QUESTIONS, &mut answers);
    for (id, answer) in crate::apps::Apps::here().answers() {
        answers.insert(id, answer.to_string());
    }
    for (id, value) in set {
        answers.insert(id, value.clone());
    }
    answers
}

pub fn rows(answers: &Answers) -> Vec<Row> {
    steps::STEPS
        .iter()
        .map(|step| Row {
            phase: step.phase,
            id: step.id,
            name: step.name,
            runs: !steps::is_skipped(step, answers),
        })
        .collect()
}

fn count(rows: &[Row]) -> (usize, usize) {
    (rows.iter().filter(|row| row.runs).count(), rows.len())
}

/// One line per step, grouped by phase, with the same tally the Review screen prints. Plain text
/// on purpose: this is read by people piping it into a file as often as by people reading it.
pub fn text(bundle: &Path, version: &str, rows: &[Row]) -> String {
    let mut out = String::new();
    out.push_str(&format!("caelestia-installer {version}\n"));
    out.push_str(&format!("bundle: {}\n\n", bundle.display()));

    for phase in Phase::ALL {
        let in_phase: Vec<&Row> = rows.iter().filter(|row| row.phase == phase).collect();
        if in_phase.is_empty() {
            continue;
        }
        out.push_str(&format!("{}\n", phase.title()));
        for row in in_phase {
            let mark = if row.runs { "run " } else { "skip" };
            out.push_str(&format!("  {mark}  {:<20}  {}\n", row.id, row.name));
        }
        out.push('\n');
    }

    let (will_run, total) = count(rows);
    out.push_str(&format!("{will_run} of {total} steps would run"));
    if will_run == total {
        out.push_str(" · nothing switched off");
    }
    out.push('\n');
    out
}

/// The same plan for a machine to read. Hand-rolled because this crate has no serializer and one
/// fixed shape does not justify a dependency; every string goes through `escape`.
pub fn json(bundle: &Path, version: &str, rows: &[Row]) -> String {
    let steps: Vec<String> = rows
        .iter()
        .map(|row| {
            format!(
                "{{\"id\":\"{}\",\"name\":\"{}\",\"phase\":\"{}\",\"runs\":{}}}",
                escape(row.id),
                escape(row.name),
                row.phase.id(),
                row.runs
            )
        })
        .collect();
    let (will_run, total) = count(rows);
    format!(
        "{{\"version\":\"{}\",\"bundle\":\"{}\",\"steps\":[{}],\"steps_that_run\":{will_run},\"steps_total\":{total}}}",
        escape(version),
        escape(&bundle.display().to_string()),
        steps.join(",")
    )
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32))
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Answers {
        answers(&[])
    }

    #[test]
    fn every_step_appears_once_and_in_phase_order() {
        let rows = rows(&defaults());
        assert_eq!(rows.len(), steps::STEPS.len());
        let order: Vec<usize> = rows
            .iter()
            .map(|row| {
                Phase::ALL
                    .iter()
                    .position(|phase| *phase == row.phase)
                    .expect("phase is one of the five")
            })
            .collect();
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(
            order, sorted,
            "phases are not grouped in the order they run"
        );
    }

    #[test]
    fn an_answer_switches_its_step_off() {
        let off = answers(&[("SKIP_SYSTEM_UPDATE", "true".to_string())]);
        let rows = rows(&off);
        let update = rows
            .iter()
            .find(|row| row.id == "00a-system-update")
            .expect("the update step is in the plan");
        assert!(!update.runs, "asking to skip the update must skip it");
    }

    #[test]
    fn the_tally_says_what_would_run_and_says_so_when_nothing_is_off() {
        let all_on = text(Path::new("/b"), "1.0", &rows(&defaults()));
        assert!(all_on.contains("nothing switched off"), "{all_on}");

        let trimmed = text(
            Path::new("/b"),
            "1.0",
            &rows(&answers(&[("INSTALL_SDDM", "false".to_string())])),
        );
        assert!(!trimmed.contains("nothing switched off"), "{trimmed}");
        assert!(trimmed.contains("skip  05-sddm-theme"), "{trimmed}");
    }

    #[test]
    fn the_json_is_one_object_with_a_row_per_step() {
        let plan = json(Path::new("/b"), "1.0", &rows(&defaults()));
        assert!(plan.starts_with('{') && plan.ends_with('}'), "{plan}");
        assert_eq!(plan.matches("\"id\":").count(), steps::STEPS.len());
        assert!(plan.contains("\"runs\":true"), "{plan}");
        assert!(!plan.contains("\"runs\":false"), "{plan}");
    }

    #[test]
    fn a_path_with_a_quote_in_it_cannot_break_the_json() {
        let plan = json(Path::new("/tmp/od\"d\\dir"), "1.0", &rows(&defaults()));
        assert!(plan.contains("/tmp/od\\\"d\\\\dir"), "{plan}");
        assert_eq!(plan.matches('{').count(), plan.matches('}').count());
    }
}
