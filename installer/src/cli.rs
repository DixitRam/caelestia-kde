use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PresetAction {
    Wizard,
    Update,
    Uninstall,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Args {
    pub bundle: Option<PathBuf>,
    pub action: PresetAction,
    pub debug: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invocation {
    Run(Args),
    Step {
        id: String,
        bundle: Option<PathBuf>,
        debug: bool,
    },
    Version,
    Help,
    Askpass(PathBuf),
}

pub fn parse<I>(args: I) -> Result<Invocation, String>
where
    I: IntoIterator<Item = OsString>,
{
    let collected: Vec<OsString> = args.into_iter().collect();
    if collected.first().map(OsString::as_os_str) == Some(OsStr::new("step")) {
        return parse_step(&collected[1..]);
    }

    let mut parsed = Args {
        bundle: None,
        action: PresetAction::Wizard,
        debug: false,
    };
    let mut seen_preset: Option<&'static str> = None;
    let mut args = collected.into_iter();

    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h") | Some("--help") => return Ok(Invocation::Help),
            Some("--version") => return Ok(Invocation::Version),
            Some("--debug") => parsed.debug = true,
            Some("--askpass") => {
                let pipe = args.next().ok_or("--askpass needs the pipe to read")?;
                return Ok(Invocation::Askpass(PathBuf::from(pipe)));
            }
            Some("--update") | Some("--uninstall") => {
                let flag = if arg == "--update" {
                    "--update"
                } else {
                    "--uninstall"
                };
                if let Some(previous) = seen_preset {
                    return Err(format!("{previous} and {flag} cannot be combined"));
                }
                seen_preset = Some(flag);
                parsed.action = if flag == "--update" {
                    PresetAction::Update
                } else {
                    PresetAction::Uninstall
                };
            }
            Some(text) if text.starts_with('-') && text != "-" => {
                return Err(format!("unknown option: {text}"));
            }
            _ => parsed.bundle = Some(PathBuf::from(arg)),
        }
    }

    Ok(Invocation::Run(parsed))
}

fn parse_step(args: &[OsString]) -> Result<Invocation, String> {
    let mut id: Option<String> = None;
    let mut bundle: Option<PathBuf> = None;
    let mut debug = false;
    let mut args = args.iter();

    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h") | Some("--help") => return Ok(Invocation::Help),
            Some("--version") => return Ok(Invocation::Version),
            Some("--debug") => debug = true,
            Some("--bundle") => {
                let directory = args.next().ok_or("--bundle needs the directory to use")?;
                bundle = Some(PathBuf::from(directory));
            }
            Some(text) if text.starts_with('-') => {
                return Err(format!("unknown option: {text}"));
            }
            _ => {
                if id.is_some() {
                    return Err(format!(
                        "step takes one step, not '{}'",
                        arg.to_string_lossy()
                    ));
                }
                id = Some(arg.to_string_lossy().to_string());
            }
        }
    }

    let id = id.ok_or("step needs the id of the step to run")?;
    Ok(Invocation::Step { id, bundle, debug })
}

#[cfg(test)]
mod tests {
    use super::{Args, Invocation, PresetAction, parse};
    use std::ffi::OsString;
    use std::path::PathBuf;

    fn parse_args(args: &[&str]) -> Result<Invocation, String> {
        parse(args.iter().map(OsString::from))
    }

    fn run(args: &[&str]) -> Args {
        match parse_args(args) {
            Ok(Invocation::Run(parsed)) => parsed,
            other => panic!("expected a run invocation, got {other:?}"),
        }
    }

    #[test]
    fn no_arguments_means_the_wizard_on_the_discovered_bundle() {
        let parsed = run(&[]);
        assert_eq!(parsed.action, PresetAction::Wizard);
        assert_eq!(parsed.bundle, None);
        assert!(!parsed.debug);
    }

    #[test]
    fn a_bare_argument_is_the_bundle_directory() {
        assert_eq!(
            run(&["/srv/caelestia"]).bundle,
            Some(PathBuf::from("/srv/caelestia"))
        );
    }

    #[test]
    fn the_last_bare_argument_wins() {
        assert_eq!(
            run(&["/first", "/second"]).bundle,
            Some(PathBuf::from("/second"))
        );
    }

    #[test]
    fn the_presets_select_the_handoff() {
        assert_eq!(run(&["--update"]).action, PresetAction::Update);
        assert_eq!(run(&["--uninstall"]).action, PresetAction::Uninstall);
    }

    #[test]
    fn combining_the_presets_is_a_usage_error() {
        let error = parse_args(&["--update", "--uninstall"]).unwrap_err();
        assert!(error.contains("--update"), "{error}");
        assert!(error.contains("--uninstall"), "{error}");
    }

    #[test]
    fn debug_is_off_until_asked_for() {
        assert!(run(&["--debug"]).debug);
    }

    #[test]
    fn version_and_help_short_circuit() {
        assert_eq!(parse_args(&["--version"]), Ok(Invocation::Version));
        assert_eq!(parse_args(&["--help"]), Ok(Invocation::Help));
        assert_eq!(parse_args(&["-h"]), Ok(Invocation::Help));
        assert_eq!(
            parse_args(&["--update", "--version"]),
            Ok(Invocation::Version)
        );
    }

    #[test]
    fn an_unknown_option_is_refused_rather_than_taken_for_a_bundle() {
        let error = parse_args(&["--bogus"]).unwrap_err();
        assert!(error.contains("--bogus"), "{error}");
    }

    #[test]
    fn a_lone_dash_is_still_a_bundle_argument() {
        assert_eq!(run(&["-"]).bundle, Some(PathBuf::from("-")));
    }

    #[test]
    fn askpass_carries_the_pipe_the_helper_must_read() {
        assert_eq!(
            parse_args(&["--askpass", "/run/user/1000/fifo"]),
            Ok(Invocation::Askpass(PathBuf::from("/run/user/1000/fifo")))
        );
        assert!(
            parse_args(&["--askpass"]).is_err(),
            "the pipe has to be named"
        );
    }

    #[test]
    fn a_step_invocation_carries_its_id() {
        assert_eq!(
            parse_args(&["step", "03a-wallpapers"]),
            Ok(Invocation::Step {
                id: "03a-wallpapers".to_string(),
                bundle: None,
                debug: false,
            })
        );
    }

    #[test]
    fn a_step_invocation_can_name_the_bundle_and_ask_for_the_trace() {
        assert_eq!(
            parse_args(&[
                "step",
                "03a-wallpapers",
                "--bundle",
                "/srv/caelestia",
                "--debug"
            ]),
            Ok(Invocation::Step {
                id: "03a-wallpapers".to_string(),
                bundle: Some(PathBuf::from("/srv/caelestia")),
                debug: true,
            })
        );
    }

    #[test]
    fn a_step_invocation_without_an_id_is_refused() {
        let error = parse_args(&["step"]).unwrap_err();
        assert!(error.contains("needs the id"), "{error}");
    }

    #[test]
    fn a_step_invocation_takes_one_step_only() {
        let error = parse_args(&["step", "one", "two"]).unwrap_err();
        assert!(error.contains("takes one step"), "{error}");
    }

    #[test]
    fn a_step_invocation_still_answers_help_and_version() {
        assert_eq!(parse_args(&["step", "--help"]), Ok(Invocation::Help));
        assert_eq!(
            parse_args(&["step", "one", "--version"]),
            Ok(Invocation::Version)
        );
    }
}
