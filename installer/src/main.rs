//! The Caelestia installer.
//!
//! This entry point owns the argument surface and the exit-code contract. The
//! wizard, the step runner and the privilege shim arrive behind it; the codes in
//! `exit` are what `scripts/setup.sh` reads, so they are the one thing here that
//! cannot change on its own.

mod bundle;
mod cli;
mod exit;
mod handoff;

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};

use bundle::Bundle;
use cli::{Args, Invocation, PresetAction};
use exit::Outcome;

/// The version marker the launcher checks an installed binary against.
const VERSION: &str = include_str!("../data/tui.version");

const USAGE: &str = "\
caelestia-install [OPTIONS] [BUNDLE_DIR]

  Install, update or remove Caelestia. With no arguments this opens the wizard.

Arguments:
  BUNDLE_DIR      the tree to operate on (default: this binary's directory)

Options:
  --update        skip the wizard and run the update path
  --uninstall     skip the wizard and run the uninstall path
  --debug         write the phase trace to stderr
  --version       print the installer version and exit
  -h, --help      print this help and exit

Exit codes:
  0 finished                1 a step failed
  2 bad arguments           3 cancelled by the user
  4 wrong place, or missing scripts     5 installer bug
  130 interrupted
";

static DEBUG: AtomicBool = AtomicBool::new(false);

fn trace(message: &str) {
    if DEBUG.load(Ordering::Relaxed) {
        eprintln!("[installer] {message}");
    }
}

fn main() -> ExitCode {
    let invocation = match cli::parse(std::env::args_os().skip(1)) {
        Ok(invocation) => invocation,
        Err(problem) => {
            eprintln!("caelestia-install: {problem}");
            eprint!("{USAGE}");
            return ExitCode::from(Outcome::Usage.code());
        }
    };

    let outcome = match invocation {
        Invocation::Help => {
            print!("{USAGE}");
            Outcome::Ok
        }
        Invocation::Version => {
            println!("{}", VERSION.trim());
            Outcome::Ok
        }
        Invocation::Run(args) => run(args),
    };

    ExitCode::from(outcome.code())
}

fn run(args: Args) -> Outcome {
    DEBUG.store(args.debug, Ordering::Relaxed);

    let bundle = Bundle::discover(args.bundle);
    trace(&format!("bundle dir: {}", bundle.root().display()));

    match args.action {
        PresetAction::Update | PresetAction::Uninstall => {
            let script_name = match args.action {
                PresetAction::Uninstall => "uninstall.sh",
                _ => "update.sh",
            };
            let script = bundle.root().join(script_name);
            trace(&format!("handing off to {}", script.display()));
            handoff::run_script(&script)
        }
        PresetAction::Wizard => {
            // The wizard lands with the screens. Until it exists, saying so is
            // the only honest answer, and the code says so out loud.
            if let Err(problem) = bundle.check_scripts() {
                eprintln!("caelestia-install: {problem}");
                return Outcome::Environment;
            }
            eprintln!("caelestia-install: the interactive wizard is not implemented yet");
            eprintln!("caelestia-install: use --update or --uninstall, or run setup.sh");
            Outcome::Internal
        }
    }
}
