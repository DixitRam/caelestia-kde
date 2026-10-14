//! Running the step scripts and reading what they said.
//!
//! One step is one script, spawned the way the shell runs it, with its output
//! captured into the install log so the wizard can tail it while it works.

use std::fs::OpenOptions;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};

use crate::steps::Status;

/// The marker the step scripts' own logging helper writes when something is
/// wrong but not fatal, and the one it writes when something failed.
const WARN_MARKER: &str = "[WARN]";

/// A step script that is running.
pub struct Running {
    child: Child,
    log: PathBuf,
    offset: u64,
    warned: bool,
}

impl Running {
    /// Spawn the step. `stdbuf` is used when it exists so the script's output
    /// reaches the log line by line instead of in one block at exit — without
    /// it the live log view would show nothing until the step finished.
    pub fn spawn(
        bundle: &Path,
        script: &str,
        log: &Path,
        env: &[(&'static str, String)],
    ) -> Result<Self, String> {
        let script_path = bundle.join(script);
        if !script_path.is_file() {
            return Err(format!("missing step script: {}", script_path.display()));
        }
        let out = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .map_err(|error| format!("cannot open the install log: {error}"))?;
        let err = out.try_clone().map_err(|error| error.to_string())?;

        // Whatever the log already holds belongs to an earlier run, or to a step
        // that has already been read: only what follows is this step's.
        let offset = std::fs::metadata(log).map(|meta| meta.len()).unwrap_or(0);

        let mut command = Command::new("stdbuf");
        if command
            .args(["-oL", "-eL", "bash"])
            .arg(&script_path)
            .current_dir(bundle)
            .envs(env.iter().map(|(key, value)| (*key, value.clone())))
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .is_err()
        {
            // No stdbuf on this system: run the script anyway.
            let out = OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)
                .map_err(|error| format!("cannot open the install log: {error}"))?;
            let err = out.try_clone().map_err(|error| error.to_string())?;
            command = Command::new("bash");
            command
                .arg(&script_path)
                .current_dir(bundle)
                .envs(env.iter().map(|(key, value)| (*key, value.clone())))
                .stdin(Stdio::null())
                .stdout(Stdio::from(out))
                .stderr(Stdio::from(err));
        }
        let child = command
            .spawn()
            .map_err(|error| format!("cannot run {}: {error}", script_path.display()))?;

        Ok(Self {
            child,
            log: log.to_path_buf(),
            offset,
            warned: false,
        })
    }

    /// The step's exit status, or `None` while it is still running.
    pub fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
        self.child
            .try_wait()
            .map_err(|error| format!("cannot wait for the step: {error}"))
    }

    /// Append whatever the step has written since the last call.
    pub fn drain(&mut self, into: &mut Vec<String>) -> usize {
        let mut fresh = Vec::new();
        if tail_lines(&self.log, &mut self.offset, &mut fresh).is_err() {
            return 0;
        }
        let added = fresh.len();
        self.warned |= fresh.iter().any(|line| line.contains(WARN_MARKER));
        into.extend(fresh);
        added
    }

    /// How the step's ending should be recorded.
    pub fn outcome(&self, status: ExitStatus) -> Status {
        if !status.success() {
            Status::Failed
        } else if self.warned {
            Status::Warn
        } else {
            Status::Ok
        }
    }
}

/// Read the log from `from` onwards, appending whole lines.
///
/// Only whole lines are returned: a partial line at the end of the file is left
/// for the next call, so the live view never shows half a line.
pub fn tail_lines(log: &Path, from: &mut u64, into: &mut Vec<String>) -> io::Result<()> {
    let mut file = OpenOptions::new().read(true).open(log)?;
    let length = file.metadata()?.len();
    if length <= *from {
        return Ok(());
    }
    file.seek(SeekFrom::Start(*from))?;
    let mut raw = String::new();
    file.read_to_string(&mut raw)?;

    let complete = match raw.rfind('\n') {
        Some(end) => end + 1,
        None => return Ok(()),
    };
    // split_terminator, not split: the trailing newline would otherwise add an
    // empty line to the view after every step.
    for line in raw[..complete].split_terminator('\n') {
        into.push(strip_ansi(line));
    }
    *from += complete as u64;
    Ok(())
}

/// Remove ANSI escapes so the log view shows text, not control sequences.
///
/// The scripts colour their output; without this the tail would be unreadable.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        match chars.peek() {
            // CSI: parameters then a final byte.
            Some('[') => {
                chars.next();
                for next in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&next) {
                        break;
                    }
                }
            }
            // OSC: runs to BEL or to the string terminator.
            Some(']') => {
                chars.next();
                while let Some(next) = chars.next() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            // A two-character escape.
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out.trim_end_matches('\r').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_escapes_are_removed_and_text_is_kept() {
        assert_eq!(
            strip_ansi("\u{1b}[38;2;1;2;3mhello\u{1b}[0m world"),
            "hello world"
        );
        assert_eq!(strip_ansi("\u{1b}]0;title\u{7}text"), "text");
        assert_eq!(strip_ansi("plain"), "plain");
    }

    #[test]
    fn carriage_returns_do_not_reach_the_view() {
        assert_eq!(strip_ansi("progress\r"), "progress");
    }

    #[test]
    fn an_empty_escape_is_not_a_panic() {
        assert_eq!(strip_ansi("\u{1b}"), "");
        assert_eq!(strip_ansi("\u{1b}["), "");
    }

    #[test]
    fn missing_logs_and_fresh_logs_are_not_errors() {
        let mut from = 0;
        let mut lines = Vec::new();
        assert!(tail_lines(Path::new("/nonexistent/install.log"), &mut from, &mut lines).is_err());
        assert!(lines.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_partial_line_waits_for_its_newline() {
        let dir = std::env::temp_dir().join(format!("caelestia-tail-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let log = dir.join("install.log");
        std::fs::write(&log, "one\ntwo").expect("the log is writable");

        let mut from = 0;
        let mut lines = Vec::new();
        tail_lines(&log, &mut from, &mut lines).expect("the log is readable");
        assert_eq!(
            lines,
            vec!["one".to_string()],
            "the unfinished line is held back"
        );

        std::fs::write(&log, "one\ntwo\nthree\n").expect("the log is writable");
        let mut more = Vec::new();
        tail_lines(&log, &mut from, &mut more).expect("the log is readable");
        assert_eq!(more, vec!["two".to_string(), "three".to_string()]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_step_is_spawned_written_to_the_log_and_judged_by_its_status() {
        let dir = std::env::temp_dir().join(format!("caelestia-run-{}", std::process::id()));
        let scripts = dir.join("scripts");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&scripts).expect("a scripts directory");
        std::fs::write(
            scripts.join("00a-system-update.sh"),
            "#!/bin/bash\necho hello\necho '[WARN] careful'\nexit 0\n",
        )
        .expect("a step script");
        let log = dir.join("install.log");

        let mut running =
            Running::spawn(&dir, "scripts/00a-system-update.sh", &log, &[]).expect("spawn");
        let status = loop {
            if let Some(status) = running.poll().expect("poll") {
                break status;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        let mut lines = Vec::new();
        running.drain(&mut lines);
        assert_eq!(
            running.outcome(status),
            Status::Warn,
            "a warning in the log is a warning"
        );
        assert!(lines.iter().any(|line| line.contains("hello")), "{lines:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_step_that_fails_is_a_failure() {
        let dir = std::env::temp_dir().join(format!("caelestia-run-fail-{}", std::process::id()));
        let scripts = dir.join("scripts");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&scripts).expect("a scripts directory");
        std::fs::write(
            scripts.join("00a-system-update.sh"),
            "#!/bin/bash\nexit 3\n",
        )
        .expect("a step script");
        let log = dir.join("install.log");

        let mut running =
            Running::spawn(&dir, "scripts/00a-system-update.sh", &log, &[]).expect("spawn");
        let status = loop {
            if let Some(status) = running.poll().expect("poll") {
                break status;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert_eq!(running.outcome(status), Status::Failed);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_step_can_be_given_environment_the_scripts_read() {
        let dir = std::env::temp_dir().join(format!("caelestia-run-env-{}", std::process::id()));
        let scripts = dir.join("scripts");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&scripts).expect("a scripts directory");
        std::fs::write(
            scripts.join("00a-system-update.sh"),
            "#!/bin/bash\necho \"$CAELESTIA_TEST\"\n",
        )
        .expect("a step script");
        let log = dir.join("install.log");
        let env = [("CAELESTIA_TEST", "from-the-environment".to_string())];

        let mut running =
            Running::spawn(&dir, "scripts/00a-system-update.sh", &log, &env).expect("spawn");
        let status = loop {
            if let Some(status) = running.poll().expect("poll") {
                break status;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        let mut lines = Vec::new();
        running.drain(&mut lines);
        assert_eq!(running.outcome(status), Status::Ok);
        assert!(
            lines
                .iter()
                .any(|line| line.contains("from-the-environment")),
            "the answers reach the step scripts: {lines:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_script_is_reported_rather_than_spawned() {
        let error = match Running::spawn(
            Path::new("/nonexistent"),
            "scripts/00a-system-update.sh",
            Path::new("/nonexistent/install.log"),
            &[],
        ) {
            Ok(_) => panic!("a missing script must not spawn"),
            Err(error) => error,
        };
        assert!(error.contains("missing step script"), "{error}");
    }
}
