use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const BINARY: &str = env!("CARGO_BIN_EXE_caelestia-install");
const VERSION_MARKER: &str = include_str!("../data/tui.version");

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Bundle {
    root: PathBuf,
}

impl Bundle {
    fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "caelestia-install-test-{}-{label}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a temporary directory");
        Self { root: path }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    // A step no longer runs a script, but a bundle still owes its scripts/ directory: that is
    // where setup.sh, the update helper and the step libraries are installed from.
    fn with_scripts_dir(self) -> Self {
        fs::create_dir_all(self.root.join("scripts")).expect("a scripts directory");
        self
    }

    #[cfg(unix)]
    fn with_script(self, name: &str, body: &str) -> Self {
        fs::write(self.root.join(name), body).expect("a bundle script");
        self
    }
}

impl Drop for Bundle {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn run(args: &[&str]) -> Output {
    Command::new(BINARY)
        .args(args)
        .output()
        .expect("the installer runs")
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("an exit status, not a signal")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn version_is_the_marker_the_launcher_compares() {
    let output = run(&["--version"]);
    assert_eq!(code(&output), 0);
    assert_eq!(stdout(&output).trim(), VERSION_MARKER.trim());
}

#[test]
fn help_lists_the_exit_codes_and_succeeds() {
    let output = run(&["--help"]);
    assert_eq!(code(&output), 0);
    assert!(
        stdout(&output).contains("Exit codes"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn an_unknown_option_is_a_usage_error() {
    let output = run(&["--bogus"]);
    assert_eq!(code(&output), 2);
    assert!(stderr(&output).contains("--bogus"), "{}", stderr(&output));
}

#[test]
fn a_bundle_without_step_scripts_is_reported_as_the_environment() {
    let bundle = Bundle::new("bare");
    let output = run(&[&bundle.path().to_string_lossy()]);
    assert_eq!(code(&output), 4);
    assert!(
        stderr(&output).contains("step scripts not found"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_wizard_refuses_to_start_without_a_terminal() {
    let bundle = Bundle::new("with-scripts").with_scripts_dir();
    let output = run(&[&bundle.path().to_string_lossy()]);
    assert_eq!(code(&output), 4);
    assert!(
        stderr(&output).contains("needs a terminal"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn uninstall_without_its_script_is_reported_as_the_environment() {
    let bundle = Bundle::new("no-uninstaller").with_scripts_dir();
    let output = run(&[&bundle.path().to_string_lossy(), "--uninstall"]);
    assert_eq!(code(&output), 4);
    assert!(
        stderr(&output).contains("uninstall.sh"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_phase_trace_only_appears_when_asked_for() {
    let bundle = Bundle::new("trace").with_scripts_dir();
    let path = bundle.path().to_string_lossy().into_owned();

    let quiet = run(&[&path]);
    assert!(
        !stderr(&quiet).contains("[installer]"),
        "{}",
        stderr(&quiet)
    );

    let traced = run(&[&path, "--debug"]);
    assert!(
        stderr(&traced).contains("[installer] bundle dir:"),
        "{}",
        stderr(&traced)
    );
}

#[cfg(unix)]
#[test]
fn an_update_runs_in_the_installer_and_says_what_it_needs() {
    // The update is native now, so what the binary does with --update is its own flow. A
    // bundle whose PATH carries no build tools is the cheapest place to see that: the flow
    // starts, refuses to go on, and names the command it is missing.
    let bundle = Bundle::new("update").with_scripts_dir();
    let path = bundle.path().to_string_lossy().into_owned();
    let empty = bundle.path().join("no-tools");
    fs::create_dir_all(&empty).expect("an empty directory");

    let output = Command::new(BINARY)
        .args([&path, "--update"])
        .env("PATH", &empty)
        .env("XDG_RUNTIME_DIR", bundle.path())
        .output()
        .expect("the installer runs");

    assert_eq!(code(&output), 1, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("Checking dependencies..."),
        "{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("'git' is missing")
            || stderr(&output).contains("'cmake' is missing"),
        "{}",
        stderr(&output)
    );
}

#[cfg(unix)]
#[test]
fn a_failing_handoff_is_a_failure() {
    let bundle = Bundle::new("failing").with_scripts_dir();
    let bundle = bundle.with_script("uninstall.sh", "#!/bin/bash\nexit 7\n");

    let output = run(&[&bundle.path().to_string_lossy(), "--uninstall"]);
    assert_eq!(code(&output), 1);
    assert!(
        stdout(&output).contains("Finished with errors."),
        "{}",
        stdout(&output)
    );
    assert!(stderr(&output).contains("exited 7"), "{}", stderr(&output));
}
