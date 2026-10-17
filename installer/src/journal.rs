//! What a run has already done.
//!
//! An installer that takes ten minutes is an installer that gets interrupted, and a half-finished
//! machine is the state where a user most needs to know what happened. Each step records its own
//! outcome here as it finishes, so a run can be picked up where it stopped instead of repeating
//! the work - and so anybody debugging a broken machine can see exactly how far it got.
//!
//! The file is append-only and line-oriented: a line per step, the last line for a step winning.
//! A step that is killed mid-write leaves the previous line intact, which is the behaviour a
//! journal wants.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::menu::Answers;

/// Carries the plan's name to each step, so a step can record its own progress without being told
/// anything else. Set by `steps::environment`.
pub const PLAN_ID_VAR: &str = "CAELESTIA_PLAN_ID";

/// Names the plan: the answers, and the version that acts on them. Change either and the journal
/// starts over, because a step that was already done for one plan says nothing about another.
pub fn plan_id(answers: &Answers) -> String {
    let mut text = format!("{}\n", crate::VERSION.trim());
    for (key, value) in answers {
        text.push_str(key);
        text.push('=');
        text.push_str(value);
        text.push('\n');
    }

    // FNV-1a: change detection, not security. The input is the caller's own answers, there is
    // nothing adversarial to defend against, and a hash crate would be a dependency for 15 lines.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Where the journals live: the user's state directory, since this is a record of what happened
/// rather than data the shell reads.
pub fn directory() -> PathBuf {
    crate::paths::state_dir().join("caelestia")
}

pub fn path_for(plan: &str) -> PathBuf {
    directory().join(format!("install-{plan}.journal"))
}

fn read(path: &Path) -> BTreeMap<String, String> {
    let mut steps = BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return steps;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('|');
        if let (Some(id), Some(status)) = (fields.next(), fields.next()) {
            steps.insert(id.to_string(), status.to_string());
        }
    }
    steps
}

#[derive(Debug)]
pub struct Journal {
    path: PathBuf,
    steps: BTreeMap<String, String>,
}

impl Journal {
    /// The journal for a plan, read if it is there. An unreadable file reads as an empty one: a
    /// corrupt journal must never stop an install.
    pub fn for_plan(plan: &str) -> Self {
        Self::at(path_for(plan))
    }

    fn at(path: PathBuf) -> Self {
        let steps = read(&path);
        Self { path, steps }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Start this plan over. Called when a run is not resuming, so the journal describes the run
    /// that is happening rather than a mixture of several.
    pub fn clear(&self) {
        let _ = std::fs::remove_file(&self.path);
    }

    pub fn status(&self, step: &str) -> Option<&str> {
        self.steps.get(step).map(String::as_str)
    }

    /// A step that does not need running again: it finished, possibly with a warning, or the
    /// answers switched it off.
    pub fn done(&self, step: &str) -> bool {
        matches!(self.status(step), Some("ok" | "warn" | "skipped"))
    }

    pub fn record(&mut self, step: &str, status: &str, seconds: u64) {
        if let Some(parent) = self.path.parent()
            && std::fs::create_dir_all(parent).is_err()
        {
            return;
        }
        let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        else {
            return;
        };
        if writeln!(file, "{step}|{status}|{seconds}").is_ok() {
            self.steps.insert(step.to_string(), status.to_string());
        }
    }

    /// Steps that finished, worst first, for anything that wants to summarise a run.
    pub fn problems(&self) -> Vec<(&str, &str)> {
        self.steps
            .iter()
            .filter(|(_, status)| !matches!(status.as_str(), "ok" | "skipped"))
            .map(|(id, status)| (id.as_str(), status.as_str()))
            .collect()
    }
}

/// Record the step this process just ran, if it was started as part of a plan. Called by the
/// `step` subcommand, which is why the journal works the same whether the run came from the
/// wizard or from `--non-interactive`.
pub fn record_here(step: &str, status: &str, seconds: u64) {
    let Some(plan) = std::env::var(PLAN_ID_VAR)
        .ok()
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    Journal::for_plan(&plan).record(step, status, seconds);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answers(pairs: &[(&'static str, &str)]) -> Answers {
        pairs
            .iter()
            .map(|(key, value)| (*key, value.to_string()))
            .collect()
    }

    fn a_journal(text: &str) -> (crate::tmp::TempDir, Journal) {
        let work =
            crate::tmp::TempDir::new("caelestia-journal-test").expect("a temporary directory");
        let path = work.path().join("install-test.journal");
        std::fs::write(&path, text).expect("a journal");
        (work, Journal::at(path))
    }

    #[test]
    fn the_same_answers_name_the_same_plan_in_any_order() {
        let one = plan_id(&answers(&[("A", "1"), ("B", "2")]));
        let two = plan_id(&answers(&[("B", "2"), ("A", "1")]));
        assert_eq!(one, two, "the answers are a set, not a sequence");
    }

    #[test]
    fn a_changed_answer_is_a_different_plan() {
        assert_ne!(
            plan_id(&answers(&[("A", "1")])),
            plan_id(&answers(&[("A", "2")])),
            "a resume must not mix two plans"
        );
    }

    #[test]
    fn the_last_word_on_a_step_is_the_one_that_counts() {
        let (_work, journal) = a_journal("00a-system-update|failed|3\n00a-system-update|ok|4\n");
        assert_eq!(journal.status("00a-system-update"), Some("ok"));
        assert!(journal.done("00a-system-update"));
    }

    #[test]
    fn a_finished_step_is_done_and_a_failed_one_is_not() {
        let (_work, journal) = a_journal("a|ok|1\nb|warn|2\nc|skipped|0\nd|failed|3\n");
        assert!(journal.done("a"));
        assert!(journal.done("b"), "a warning still finished the step");
        assert!(journal.done("c"), "a switched-off step has nothing to redo");
        assert!(!journal.done("d"), "a failed step has everything to redo");
        assert!(!journal.done("missing"));

        // Done and clean are different questions: a warned step needs no second run, but anyone
        // resuming deserves to hear that it warned.
        assert_eq!(journal.problems(), vec![("b", "warn"), ("d", "failed")]);
    }

    #[test]
    fn recording_appends_and_clearing_starts_again() {
        let (_work, mut journal) = a_journal("a|ok|1\n");
        journal.record("b", "failed", 7);

        let text = std::fs::read_to_string(journal.path()).expect("the journal");
        assert!(text.contains("a|ok|1"), "{text}");
        assert!(text.contains("b|failed|7"), "{text}");
        assert_eq!(journal.status("b"), Some("failed"));

        journal.clear();
        assert!(!journal.path().exists(), "clearing removes the file");
    }

    #[test]
    fn a_line_nobody_can_read_is_skipped_rather_than_fatal() {
        let (_work, journal) = a_journal("nonsense\n\n# comment\nb|ok|2\n");
        assert_eq!(journal.status("b"), Some("ok"));
        assert_eq!(journal.problems(), Vec::new());
    }
}
