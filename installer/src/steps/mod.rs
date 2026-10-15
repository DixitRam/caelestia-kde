mod autostart;
mod backup;
mod configs;
mod kde_apps;
mod kde_theme;
mod optional_apps;
mod prereqs;
mod services;
mod submodules;
mod system_update;
mod wallpapers;
mod window_rules;

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::exit::Outcome;
use crate::menu::Answers;
use crate::output;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Prepare,
    Packages,
    Configure,
    Build,
    Finalize,
}

impl Phase {
    pub const ALL: [Phase; 5] = [
        Phase::Prepare,
        Phase::Packages,
        Phase::Configure,
        Phase::Build,
        Phase::Finalize,
    ];

    #[cfg(test)]
    pub fn id(self) -> &'static str {
        match self {
            Phase::Prepare => "prepare",
            Phase::Packages => "packages",
            Phase::Configure => "configure",
            Phase::Build => "build",
            Phase::Finalize => "finalize",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Phase::Prepare => "Prepare",
            Phase::Packages => "Packages",
            Phase::Configure => "Configure",
            Phase::Build => "Build",
            Phase::Finalize => "Finalize",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Pending,
    Running,
    Ok,
    Warn,
    Failed,
    Skipped,
    Ignored,
}

impl Status {
    #[cfg(test)]
    pub const ALL: [Status; 7] = [
        Status::Pending,
        Status::Running,
        Status::Ok,
        Status::Warn,
        Status::Failed,
        Status::Skipped,
        Status::Ignored,
    ];

    pub fn is_failure(self) -> bool {
        matches!(self, Status::Failed)
    }
}

pub enum Failure {
    Message(String),
    Reported,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure::Message(message)
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        Failure::Message(message.to_string())
    }
}

pub type Act = fn(&Context) -> Result<(), Failure>;

pub enum Body {
    Script(&'static str),
    Native(Act),
}

pub struct Step {
    pub name: &'static str,
    pub id: &'static str,
    pub phase: Phase,
    pub body: Body,
}

pub struct Context {
    pub bundle: PathBuf,
}

impl Context {
    pub fn variable(&self, name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|value| !value.is_empty())
    }

    pub fn flag(&self, name: &str, fallback: bool) -> bool {
        match std::env::var(name) {
            Ok(value) => value == "true",
            Err(_) => fallback,
        }
    }
}

pub const STEPS: [Step; 17] = [
    Step {
        name: "Update system",
        id: "00a-system-update",
        phase: Phase::Prepare,
        body: Body::Native(system_update::run),
    },
    Step {
        name: "Ensure prerequisites",
        id: "01-ensure-prereqs",
        phase: Phase::Prepare,
        body: Body::Native(prereqs::run),
    },
    Step {
        name: "Update submodules",
        id: "02a-submodules",
        phase: Phase::Prepare,
        body: Body::Native(submodules::run),
    },
    Step {
        name: "Back up current setup",
        id: "00-backup-themes",
        phase: Phase::Prepare,
        body: Body::Native(backup::run),
    },
    Step {
        name: "Install packages",
        id: "02-all-packages",
        phase: Phase::Packages,
        body: Body::Script("scripts/02-all-packages.sh"),
    },
    Step {
        name: "Set up Python tooling and matugen",
        id: "02-packages",
        phase: Phase::Packages,
        body: Body::Script("scripts/02-packages.sh"),
    },
    Step {
        name: "Deploy config files",
        id: "03-deploy-configs",
        phase: Phase::Configure,
        body: Body::Native(configs::run),
    },
    Step {
        name: "Download wallpapers",
        id: "03a-wallpapers",
        phase: Phase::Configure,
        body: Body::Native(wallpapers::run),
    },
    Step {
        name: "Apply KDE theme",
        id: "04-deploy-kde",
        phase: Phase::Configure,
        body: Body::Native(kde_theme::run),
    },
    Step {
        name: "Apply window rules",
        id: "04a-window-rules",
        phase: Phase::Configure,
        body: Body::Native(window_rules::run),
    },
    Step {
        name: "Install SDDM theme",
        id: "05-sddm-theme",
        phase: Phase::Configure,
        body: Body::Script("scripts/05-sddm-theme.sh"),
    },
    Step {
        name: "Enable system services",
        id: "06-services",
        phase: Phase::Configure,
        body: Body::Native(services::run),
    },
    Step {
        name: "Configure KDE applications",
        id: "07-kde-apps",
        phase: Phase::Configure,
        body: Body::Native(kde_apps::run),
    },
    Step {
        name: "Build Caelestia shell",
        id: "08-build-shell",
        phase: Phase::Build,
        body: Body::Script("scripts/08-build-shell.sh"),
    },
    Step {
        name: "Apply system tweaks",
        id: "09-system-tweaks",
        phase: Phase::Finalize,
        body: Body::Script("scripts/09-system-tweaks.sh"),
    },
    Step {
        name: "Create autostart entries",
        id: "10-autostart",
        phase: Phase::Finalize,
        body: Body::Native(autostart::run),
    },
    Step {
        name: "Install optional components",
        id: "11-optional-apps",
        phase: Phase::Finalize,
        body: Body::Native(optional_apps::run),
    },
];

pub fn find(id: &str) -> Option<&'static Step> {
    STEPS.iter().find(|step| step.id == id)
}

pub fn wallpapers_dir(context: &Context) -> PathBuf {
    if let Some(directory) = context.variable("CAELESTIA_WALLPAPERS_DIR") {
        return PathBuf::from(directory);
    }
    if let Some(directory) = context.variable("XDG_PICTURES_DIR") {
        return PathBuf::from(directory).join("Wallpapers");
    }
    if crate::process::available("xdg-user-dir")
        && let Some(pictures) = crate::process::capture("xdg-user-dir", &["PICTURES"])
    {
        return PathBuf::from(pictures).join("Wallpapers");
    }
    crate::paths::home().join("Pictures").join("Wallpapers")
}

pub fn ensure_five_desktops(context: &Context) -> Result<(), Failure> {
    // The marker the install entries set is "1"; anything else is an update.
    if context.variable("CAELESTIA_FRESH_INSTALL").as_deref() != Some("1") {
        output::skip("Not an install - leaving the virtual desktops alone.");
        return Ok(());
    }

    let count = crate::process::capture(
        "kreadconfig6",
        &["--file", "kwinrc", "--group", "Desktops", "--key", "Number"],
    )
    .unwrap_or_default();

    // KWin's own default of one desktop is not a configuration the user made.
    if !count.is_empty() && count != "1" {
        output::skip(&format!("Keeping the existing {count} virtual desktops."));
        return Ok(());
    }

    output::info("Configuring 5 virtual desktops...");

    let mut written = true;
    for (key, value) in [("Number", "5"), ("Rows", "1")] {
        written &= crate::process::succeeds(
            "kwriteconfig6",
            &[
                "--file", "kwinrc", "--group", "Desktops", "--key", key, value,
            ],
            None,
        );
    }
    for index in 1..=5 {
        written &= crate::process::succeeds(
            "kwriteconfig6",
            &[
                "--file",
                "kwinrc",
                "--group",
                "Desktops",
                "--key",
                &format!("Name_{index}"),
                &format!("Desktop {index}"),
            ],
            None,
        );
    }

    if written {
        output::ok("5 virtual desktops configured.");
    } else {
        output::warn("Could not write the virtual desktop configuration.");
    }
    Ok(())
}

pub fn run(step: &Step, context: &Context) -> i32 {
    match step.body {
        Body::Native(act) => match act(context) {
            Ok(()) => 0,
            Err(Failure::Message(problem)) => {
                output::err(&problem);
                1
            }
            Err(Failure::Reported) => 1,
        },
        Body::Script(script) => run_script(&context.bundle.join(script)),
    }
}

fn run_script(path: &Path) -> i32 {
    if !path.is_file() {
        output::err(&format!("{} is missing", path.display()));
        return Outcome::Environment.code().into();
    }
    match Command::new("bash").arg(path).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            output::err(&format!("could not run {}: {error}", path.display()));
            Outcome::Environment.code().into()
        }
    }
}

const OPTIONAL_APP_ANSWERS: [&str; 6] = [
    "INSTALL_VSCODE",
    "INSTALL_ZED",
    "INSTALL_SPICETIFY",
    "INSTALL_DISCORD",
    "INSTALL_TODOIST",
    "INSTALL_FIREFOX_THEME",
];

fn answer_is_true(answers: &Answers, name: &str) -> bool {
    answers.get(name).map(String::as_str) == Some("true")
}

pub fn is_skipped(step: &Step, answers: &Answers) -> bool {
    match step.name {
        "Update system" => answer_is_true(answers, "SKIP_SYSTEM_UPDATE"),
        "Install SDDM theme" => !answer_is_true(answers, "INSTALL_SDDM"),
        "Install optional components" => !OPTIONAL_APP_ANSWERS
            .iter()
            .any(|answer| answer_is_true(answers, answer)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn answers(pairs: &[(&'static str, &str)]) -> Answers {
        pairs
            .iter()
            .map(|(key, value)| (*key, value.to_string()))
            .collect()
    }

    #[test]
    fn every_script_step_names_a_script_under_scripts() {
        for step in STEPS.iter() {
            if let Body::Script(script) = step.body {
                assert!(script.starts_with("scripts/"), "{script}");
                assert!(script.ends_with(".sh"), "{script}");
                assert_eq!(
                    script
                        .trim_start_matches("scripts/")
                        .trim_end_matches(".sh"),
                    step.id,
                    "{} should be named after its script",
                    step.name
                );
            }
        }
    }

    #[test]
    fn step_ids_are_unique() {
        let mut ids: Vec<&str> = STEPS.iter().map(|step| step.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate step ids");
    }

    #[test]
    fn step_names_are_unique() {
        let mut names: Vec<&str> = STEPS.iter().map(|step| step.name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate step names");
    }

    #[test]
    fn every_phase_has_at_least_one_step() {
        for phase in Phase::ALL {
            assert!(
                STEPS.iter().any(|step| step.phase == phase),
                "no step in phase {}",
                phase.id()
            );
        }
    }

    #[test]
    fn the_backup_runs_after_the_submodule_update_on_purpose() {
        let position = |name: &str| STEPS.iter().position(|step| step.name == name).unwrap();
        assert!(position("Back up current setup") > position("Update submodules"));
    }

    #[test]
    fn the_system_update_is_skipped_only_when_asked() {
        let step = STEPS
            .iter()
            .find(|step| step.name == "Update system")
            .unwrap();
        assert!(!is_skipped(step, &BTreeMap::new()));
        assert!(is_skipped(
            step,
            &answers(&[("SKIP_SYSTEM_UPDATE", "true")])
        ));
        assert!(!is_skipped(
            step,
            &answers(&[("SKIP_SYSTEM_UPDATE", "false")])
        ));
    }

    #[test]
    fn the_sddm_theme_is_skipped_unless_asked_for() {
        let step = STEPS
            .iter()
            .find(|step| step.name == "Install SDDM theme")
            .unwrap();
        assert!(is_skipped(step, &BTreeMap::new()));
        assert!(!is_skipped(step, &answers(&[("INSTALL_SDDM", "true")])));
    }

    #[test]
    fn optional_components_run_only_when_one_app_is_asked_for() {
        let step = STEPS
            .iter()
            .find(|step| step.name == "Install optional components")
            .unwrap();
        assert!(is_skipped(step, &BTreeMap::new()));
        for answer in OPTIONAL_APP_ANSWERS {
            assert!(
                !is_skipped(step, &answers(&[(answer, "true")])),
                "{answer} should enable the optional components step"
            );
        }
    }

    #[test]
    fn no_other_step_is_ever_skipped() {
        let all_on: Answers = OPTIONAL_APP_ANSWERS
            .iter()
            .map(|answer| (*answer, "true".to_string()))
            .chain([
                ("SKIP_SYSTEM_UPDATE", "false".to_string()),
                ("INSTALL_SDDM", "true".to_string()),
            ])
            .collect();
        for step in STEPS.iter() {
            assert!(!is_skipped(step, &all_on), "{} was skipped", step.name);
        }
    }
}
