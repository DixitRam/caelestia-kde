mod app;
mod bundle;
mod cli;
mod distro;
mod exit;
mod handoff;
mod menu;
mod output;
mod paths;
mod privileges;
mod process;
mod runner;
mod screens;
mod signals;
mod steps;
mod theme;
mod tmp;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};

use bundle::Bundle;
use cli::{Args, Invocation, PresetAction};
use exit::Outcome;

const VERSION: &str = include_str!("../data/tui.version");

const USAGE: &str = "\
caelestia-install [OPTIONS] [BUNDLE_DIR]
caelestia-install step [OPTIONS] STEP_ID

  Install, update or remove Caelestia. With no arguments this opens the wizard.

Arguments:
  BUNDLE_DIR      the tree to operate on (default: this binary's directory)
  STEP_ID         a step to run on its own, by its id

Options:
  --update        skip the wizard and run the update path
  --uninstall     skip the wizard and run the uninstall path
  --bundle DIR    the tree a step operates on
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
        Invocation::Askpass(pipe) => askpass(&pipe),
        Invocation::Step { id, bundle, debug } => run_step(&id, bundle, debug),
        Invocation::Run(args) => run(args),
    };

    ExitCode::from(outcome.code())
}

fn run_step(id: &str, bundle: Option<PathBuf>, debug: bool) -> Outcome {
    DEBUG.store(debug, Ordering::Relaxed);

    let requested = bundle.or_else(|| std::env::var_os("BUNDLE_DIR").map(PathBuf::from));
    let bundle = Bundle::discover(requested);
    trace(&format!("bundle dir: {}", bundle.root().display()));

    let Some(step) = steps::find(id) else {
        eprintln!("caelestia-install: no step is called '{id}'");
        return Outcome::Usage;
    };
    trace(&format!("step {} ({})", step.id, step.name));

    let context = steps::Context {
        bundle: bundle.root().to_path_buf(),
    };
    match steps::run(step, &context) {
        0 => Outcome::Ok,
        _ => Outcome::Failed,
    }
}

fn askpass(pipe: &Path) -> Outcome {
    match privileges::read_secret(pipe) {
        Ok(secret) => {
            println!("{secret}");
            Outcome::Ok
        }
        Err(problem) => {
            eprintln!("caelestia-install: {problem}");
            Outcome::Environment
        }
    }
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
            if let Err(problem) = bundle.check_scripts() {
                eprintln!("caelestia-install: {problem}");
                return Outcome::Environment;
            }
            trace("phase 1: welcome_screen");
            app::run(bundle.root().to_path_buf(), app::log_path())
        }
    }
}
