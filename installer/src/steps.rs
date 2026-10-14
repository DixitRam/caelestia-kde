//! The canonical step table.
//!
//! The installer's list of steps, in the order the installer runs them. Two
//! other lists name the same scripts — the packaged install's half and the
//! update path's subset — and they keep their own copies because their failure
//! policies differ. Only membership is shared, and a test in the repository's
//! Python suite checks all three against `scripts/`.

use crate::menu::Answers;

/// A named group of steps, the primary organizer of the progress and review
/// screens.
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

    /// The phase's stable identifier, used by the table's tests.
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

/// The canonical step state, rendered by the progress screen, the live log and
/// the summary.
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

    /// Whether the run should stop and ask the user what to do.
    pub fn is_failure(self) -> bool {
        matches!(self, Status::Failed)
    }
}

pub struct Step {
    pub name: &'static str,
    pub script: &'static str,
    pub phase: Phase,
}

/// The order here is the order the installer runs them, and it is not the order
/// of the file names: backing up the current setup happens after the submodule
/// update on purpose, so the backup captures the state the update left behind.
pub const STEPS: [Step; 17] = [
    Step {
        name: "Update system",
        script: "scripts/00a-system-update.sh",
        phase: Phase::Prepare,
    },
    Step {
        name: "Ensure prerequisites",
        script: "scripts/01-ensure-prereqs.sh",
        phase: Phase::Prepare,
    },
    Step {
        name: "Update submodules",
        script: "scripts/02a-submodules.sh",
        phase: Phase::Prepare,
    },
    Step {
        name: "Back up current setup",
        script: "scripts/00-backup-themes.sh",
        phase: Phase::Prepare,
    },
    Step {
        name: "Install packages",
        script: "scripts/02-all-packages.sh",
        phase: Phase::Packages,
    },
    Step {
        name: "Set up Python tooling and matugen",
        script: "scripts/02-packages.sh",
        phase: Phase::Packages,
    },
    Step {
        name: "Deploy config files",
        script: "scripts/03-deploy-configs.sh",
        phase: Phase::Configure,
    },
    Step {
        name: "Download wallpapers",
        script: "scripts/03a-wallpapers.sh",
        phase: Phase::Configure,
    },
    Step {
        name: "Apply KDE theme",
        script: "scripts/04-deploy-kde.sh",
        phase: Phase::Configure,
    },
    Step {
        name: "Apply window rules",
        script: "scripts/04a-window-rules.sh",
        phase: Phase::Configure,
    },
    Step {
        name: "Install SDDM theme",
        script: "scripts/05-sddm-theme.sh",
        phase: Phase::Configure,
    },
    Step {
        name: "Enable system services",
        script: "scripts/06-services.sh",
        phase: Phase::Configure,
    },
    Step {
        name: "Configure KDE applications",
        script: "scripts/07-kde-apps.sh",
        phase: Phase::Configure,
    },
    Step {
        name: "Build Caelestia shell",
        script: "scripts/08-build-shell.sh",
        phase: Phase::Build,
    },
    Step {
        name: "Apply system tweaks",
        script: "scripts/09-system-tweaks.sh",
        phase: Phase::Finalize,
    },
    Step {
        name: "Create autostart entries",
        script: "scripts/10-autostart.sh",
        phase: Phase::Finalize,
    },
    Step {
        name: "Install optional components",
        script: "scripts/11-optional-apps.sh",
        phase: Phase::Finalize,
    },
];

/// Answers that turn a step off, and the answer that turns one on.
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

/// The three steps whose inclusion depends on the Configure answers.
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
    fn every_step_script_lives_under_scripts() {
        for step in STEPS.iter() {
            assert!(step.script.starts_with("scripts/"), "{}", step.script);
            assert!(step.script.ends_with(".sh"), "{}", step.script);
        }
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
