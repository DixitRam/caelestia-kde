use std::collections::BTreeMap;

pub type Answers = BTreeMap<&'static str, String>;

pub const BACK: &str = "action_back";
pub const REVIEW: &str = "action_review";

#[derive(Debug)]
pub enum Entry {
    Submenu {
        id: &'static str,
        title: &'static str,
        help: &'static str,
        items: &'static [Entry],
    },
    Toggle {
        id: &'static str,
        title: &'static str,
        help: &'static str,
        default: bool,
    },
    Choice {
        id: &'static str,
        title: &'static str,
        help: &'static str,
        default: &'static str,
        options: &'static [&'static str],
    },
    Action {
        id: &'static str,
        title: &'static str,
        help: &'static str,
    },
}

impl Entry {
    pub fn id(&self) -> &'static str {
        match self {
            Entry::Submenu { id, .. }
            | Entry::Toggle { id, .. }
            | Entry::Choice { id, .. }
            | Entry::Action { id, .. } => id,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Entry::Submenu { title, .. }
            | Entry::Toggle { title, .. }
            | Entry::Choice { title, .. }
            | Entry::Action { title, .. } => title,
        }
    }

    pub fn help(&self) -> &'static str {
        match self {
            Entry::Submenu { help, .. }
            | Entry::Toggle { help, .. }
            | Entry::Choice { help, .. }
            | Entry::Action { help, .. } => help,
        }
    }

    #[cfg(test)]
    pub fn is_answerable(&self) -> bool {
        matches!(self, Entry::Toggle { .. } | Entry::Choice { .. })
    }
}

const PACKAGES: [Entry; 6] = [
    Entry::Toggle {
        id: "INSTALL_FISH",
        title: "Fish shell",
        help: "Fish, with the Caelestia configuration.",
        default: true,
    },
    Entry::Toggle {
        id: "INSTALL_PAPIRUS",
        title: "Papirus icon theme",
        help: "The Papirus icon set.",
        default: true,
    },
    Entry::Toggle {
        id: "INSTALL_DARKLY",
        title: "Darkly theme package",
        help: "The Darkly Plasma and Qt theme packages.",
        default: true,
    },
    Entry::Toggle {
        id: "INSTALL_KVANTUM",
        title: "Kvantum theme engine",
        help: "Kvantum, the Qt style engine the Caelestia look is built on.",
        default: true,
    },
    Entry::Toggle {
        id: "INSTALL_THUNAR",
        title: "Thunar integration files",
        help: "Config files for the Thunar file manager.",
        default: false,
    },
    Entry::Action {
        id: BACK,
        title: "Back",
        help: "",
    },
];

const APPEARANCE: [Entry; 6] = [
    Entry::Toggle {
        id: "APPLY_DARKLY",
        title: "Apply Darkly theme",
        help: "Set the style, window decorations, and borders to Darkly.",
        default: true,
    },
    Entry::Toggle {
        id: "APPLY_FONTS",
        title: "Custom fonts",
        help: "Apply Rubik and JetBrains Mono Nerd Font across the system.",
        default: true,
    },
    Entry::Toggle {
        id: "APPLY_LOCKSCREEN",
        title: "Custom lockscreen",
        help: "Install the Caelestia lock screen greeter for KDE Plasma.",
        default: true,
    },
    Entry::Toggle {
        id: "INSTALL_SDDM",
        title: "SDDM greeter theme",
        help: "Material You themed login screen with wallpaper and color sync.",
        default: true,
    },
    Entry::Choice {
        id: "SDDM_THEME_VARIANT",
        title: "SDDM theme variant",
        help: "Full: clock, system info, quotes, avatar, blur effects. Mini: centered login card, clean and minimal.",
        default: "full",
        options: &["full", "mini"],
    },
    Entry::Action {
        id: BACK,
        title: "Back",
        help: "",
    },
];

const SHELL: [Entry; 2] = [
    Entry::Choice {
        id: "DEFAULT_SHELL",
        title: "Default shell",
        help: "The login shell Caelestia sets up.",
        default: "fish",
        options: &["bash", "zsh", "fish"],
    },
    Entry::Action {
        id: BACK,
        title: "Back",
        help: "",
    },
];

const APPS: [Entry; 7] = [
    Entry::Toggle {
        id: "INSTALL_VSCODE",
        title: "VSCode / VSCodium",
        help: "Settings, keybindings, and the Caelestia theme.",
        default: false,
    },
    Entry::Toggle {
        id: "INSTALL_ZED",
        title: "Zed",
        help: "Zed keymap and settings.",
        default: false,
    },
    Entry::Toggle {
        id: "INSTALL_SPICETIFY",
        title: "Spicetify",
        help: "Spotify theming through Spicetify.",
        default: false,
    },
    Entry::Toggle {
        id: "INSTALL_DISCORD",
        title: "Discord / Equibop",
        help: "The Caelestia look for Discord or Equibop.",
        default: false,
    },
    Entry::Toggle {
        id: "INSTALL_TODOIST",
        title: "Todoist",
        help: "The Todoist AppImage.",
        default: false,
    },
    Entry::Toggle {
        id: "INSTALL_FIREFOX_THEME",
        title: "Firefox theming",
        help: "userChrome.css and user.js theming for Firefox.",
        default: false,
    },
    Entry::Action {
        id: BACK,
        title: "Back",
        help: "",
    },
];

const OPTIONS: [Entry; 3] = [
    Entry::Toggle {
        id: "REMOVE_CACHE",
        title: "Remove build cache after install",
        help: "Clear the Caelestia build cache when installation finishes.",
        default: false,
    },
    Entry::Toggle {
        id: "SKIP_SYSTEM_UPDATE",
        title: "Skip the full system update",
        help: "Skip the full system update (pacman -Syu / dnf upgrade / apt upgrade).",
        default: false,
    },
    Entry::Action {
        id: BACK,
        title: "Back",
        help: "",
    },
];

pub const ROOT: [Entry; 6] = [
    Entry::Submenu {
        id: "menu_packages",
        title: "Packages",
        help: "Packages installed from your distribution's repositories and the AUR.",
        items: &PACKAGES,
    },
    Entry::Submenu {
        id: "menu_appearance",
        title: "Appearance",
        help: "Desktop look and feel, applied during configuration.",
        items: &APPEARANCE,
    },
    Entry::Submenu {
        id: "menu_shell",
        title: "Shell",
        help: "The login shell, set during finalization.",
        items: &SHELL,
    },
    Entry::Submenu {
        id: "menu_apps",
        title: "Applications",
        help: "Theming for third-party apps. Only applies to the ones you turn on.",
        items: &APPS,
    },
    Entry::Submenu {
        id: "menu_behavior",
        title: "Installer options",
        help: "How the installer runs.",
        items: &OPTIONS,
    },
    Entry::Action {
        id: REVIEW,
        title: "Review installation",
        help: "See every step, grouped by phase, before anything runs.",
    },
];

pub fn seed_answers(entries: &[Entry], answers: &mut Answers) {
    for entry in entries {
        match entry {
            Entry::Submenu { items, .. } => seed_answers(items, answers),
            Entry::Toggle { id, default, .. } => {
                answers.entry(id).or_insert_with(|| {
                    if *default {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                });
            }
            Entry::Choice {
                id,
                default,
                options,
                ..
            } => {
                let seed = if default.is_empty() {
                    options.first().copied().unwrap_or("")
                } else {
                    default
                };
                answers.entry(id).or_insert_with(|| seed.to_string());
            }
            Entry::Action { .. } => {}
        }
    }
}

pub fn value<'a>(entry: &Entry, answers: &'a Answers) -> Option<&'a str> {
    answers.get(entry.id()).map(String::as_str)
}

pub fn toggle(entry: &Entry, answers: &mut Answers) -> bool {
    let Entry::Toggle { id, .. } = entry else {
        return false;
    };
    let next = if value(entry, answers) == Some("true") {
        "false"
    } else {
        "true"
    };
    answers.insert(id, next.to_string());
    true
}

pub fn cycle(entry: &Entry, answers: &mut Answers, forward: bool) -> bool {
    let Entry::Choice { id, options, .. } = entry else {
        return false;
    };
    if options.is_empty() {
        return false;
    }
    let current = value(entry, answers).unwrap_or_default();
    let at = options
        .iter()
        .position(|option| *option == current)
        .unwrap_or(0);
    let next = if forward {
        (at + 1) % options.len()
    } else if at == 0 {
        options.len() - 1
    } else {
        at - 1
    };
    answers.insert(id, options[next].to_string());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_entries<'a>(entries: &'a [Entry], out: &mut Vec<&'a Entry>) {
        for entry in entries {
            out.push(entry);
            if let Entry::Submenu { items, .. } = entry {
                all_entries(items, out);
            }
        }
    }

    fn every_entry<'a>() -> Vec<&'a Entry> {
        let mut out = Vec::new();
        all_entries(&ROOT, &mut out);
        out
    }

    #[test]
    fn answers_scatter_without_duplicate_keys() {
        let mut seen = std::collections::BTreeSet::new();
        for entry in every_entry() {
            if entry.is_answerable() {
                assert!(
                    seen.insert(entry.id()),
                    "duplicate answer id {}",
                    entry.id()
                );
            }
        }
    }

    #[test]
    fn every_submenu_offers_a_way_back() {
        for entry in every_entry() {
            if let Entry::Submenu { items, .. } = entry {
                assert!(
                    items.iter().any(|item| item.id() == BACK),
                    "submenu {} has no way back",
                    entry.id()
                );
            }
        }
    }

    #[test]
    fn the_root_has_exactly_one_way_to_review() {
        assert_eq!(ROOT.iter().filter(|entry| entry.id() == REVIEW).count(), 1);
    }

    #[test]
    fn seeding_answers_fills_every_question_from_the_defaults() {
        let mut answers = Answers::new();
        seed_answers(&ROOT, &mut answers);
        for entry in every_entry() {
            if entry.is_answerable() {
                assert!(
                    answers.contains_key(entry.id()),
                    "{} was not seeded",
                    entry.id()
                );
            }
        }
        assert_eq!(
            answers.get("INSTALL_FISH").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            answers.get("INSTALL_ZED").map(String::as_str),
            Some("false")
        );
        assert_eq!(
            answers.get("DEFAULT_SHELL").map(String::as_str),
            Some("fish")
        );
    }

    #[test]
    fn seeding_never_overwrites_a_choice_the_user_made() {
        let mut answers = Answers::new();
        answers.insert("INSTALL_FISH", "false".to_string());
        seed_answers(&ROOT, &mut answers);
        assert_eq!(
            answers.get("INSTALL_FISH").map(String::as_str),
            Some("false")
        );
    }

    #[test]
    fn toggles_flip_and_only_toggles_flip() {
        let mut answers = Answers::new();
        seed_answers(&ROOT, &mut answers);
        let fish = every_entry()
            .into_iter()
            .find(|e| e.id() == "INSTALL_FISH")
            .unwrap();
        assert!(toggle(fish, &mut answers));
        assert_eq!(value(fish, &answers), Some("false"));

        let action = every_entry()
            .into_iter()
            .find(|e| e.id() == REVIEW)
            .unwrap();
        assert!(!toggle(action, &mut answers));
    }

    #[test]
    fn selects_wrap_in_both_directions() {
        let mut answers = Answers::new();
        seed_answers(&ROOT, &mut answers);
        let shell = every_entry()
            .into_iter()
            .find(|e| e.id() == "DEFAULT_SHELL")
            .unwrap();
        assert_eq!(value(shell, &answers), Some("fish"));

        assert!(cycle(shell, &mut answers, true));
        assert_eq!(
            value(shell, &answers),
            Some("bash"),
            "the last option wraps to the first"
        );

        assert!(cycle(shell, &mut answers, false));
        assert_eq!(
            value(shell, &answers),
            Some("fish"),
            "the first option wraps to the last"
        );

        assert!(cycle(shell, &mut answers, true));
        assert_eq!(value(shell, &answers), Some("bash"));
        assert!(cycle(shell, &mut answers, true));
        assert_eq!(
            value(shell, &answers),
            Some("zsh"),
            "steps through the middle"
        );

        let action = every_entry()
            .into_iter()
            .find(|e| e.id() == REVIEW)
            .unwrap();
        assert!(!cycle(action, &mut answers, true), "only selects cycle");
    }

    #[test]
    fn every_question_the_step_policy_reads_is_in_the_tree() {
        let mut answers = Answers::new();
        seed_answers(&ROOT, &mut answers);
        for key in [
            "SKIP_SYSTEM_UPDATE",
            "INSTALL_SDDM",
            "INSTALL_VSCODE",
            "INSTALL_ZED",
            "INSTALL_SPICETIFY",
            "INSTALL_DISCORD",
            "INSTALL_TODOIST",
            "INSTALL_FIREFOX_THEME",
        ] {
            assert!(
                answers.contains_key(key),
                "the step policy reads {key}, the menu has no entry"
            );
        }
    }
}
