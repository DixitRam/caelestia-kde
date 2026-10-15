// The install-kind contract: which layout this install has and where each of its directories
// is. Every step and the shell wrappers read the same answers from here.

use std::path::PathBuf;

use crate::paths;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Source,
    Package,
}

pub fn kind() -> Kind {
    if let Ok(explicit) = std::env::var("CAELESTIA_INSTALL_KIND") {
        match explicit.trim() {
            "package" => return Kind::Package,
            "source" => return Kind::Source,
            _ => {}
        }
    }

    if std::env::current_exe()
        .map(|exe| exe.starts_with("/usr"))
        .unwrap_or(false)
    {
        Kind::Package
    } else {
        Kind::Source
    }
}

pub fn is_packaged() -> bool {
    kind() == Kind::Package
}

pub fn lib_dir() -> PathBuf {
    lib_dir_for(kind())
}

pub fn bin_dir() -> PathBuf {
    bin_dir_for(kind())
}

pub fn qml_import_path() -> String {
    qml_import_path_for(kind())
}

pub fn shell_config() -> PathBuf {
    shell_config_for(kind())
}

fn lib_dir_for(kind: Kind) -> PathBuf {
    match kind {
        Kind::Package => PathBuf::from("/usr/lib/caelestia"),
        Kind::Source => paths::home().join(".local").join("lib").join("caelestia"),
    }
}

fn bin_dir_for(kind: Kind) -> PathBuf {
    match kind {
        Kind::Package => PathBuf::from("/usr/bin"),
        Kind::Source => paths::home().join(".local").join("bin"),
    }
}

fn qml_import_path_for(kind: Kind) -> String {
    match kind {
        Kind::Package => "/usr/lib/qt6/qml:/etc/xdg/quickshell/caelestia".to_string(),
        Kind::Source => {
            let home = paths::home();
            format!(
                "{}:{}",
                home.join(".local")
                    .join("lib")
                    .join("qt6")
                    .join("qml")
                    .display(),
                home.join(".config")
                    .join("quickshell")
                    .join("caelestia")
                    .display()
            )
        }
    }
}

fn shell_config_for(kind: Kind) -> PathBuf {
    match kind {
        Kind::Package => PathBuf::from("/etc/xdg/quickshell/caelestia/shell.qml"),
        Kind::Source => paths::home()
            .join(".config")
            .join("quickshell")
            .join("caelestia")
            .join("shell.qml"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packaged_install_reads_and_writes_the_system_locations() {
        assert_eq!(
            lib_dir_for(Kind::Package),
            PathBuf::from("/usr/lib/caelestia")
        );
        assert_eq!(bin_dir_for(Kind::Package), PathBuf::from("/usr/bin"));
        assert_eq!(
            qml_import_path_for(Kind::Package),
            "/usr/lib/qt6/qml:/etc/xdg/quickshell/caelestia"
        );
        assert_eq!(
            shell_config_for(Kind::Package),
            PathBuf::from("/etc/xdg/quickshell/caelestia/shell.qml")
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_source_install_reads_and_writes_the_users_own_locations() {
        let home = paths::home();
        assert_eq!(lib_dir_for(Kind::Source), home.join(".local/lib/caelestia"));
        assert_eq!(bin_dir_for(Kind::Source), home.join(".local/bin"));
        assert_eq!(
            shell_config_for(Kind::Source),
            home.join(".config/quickshell/caelestia/shell.qml")
        );

        let imports = qml_import_path_for(Kind::Source);
        assert!(imports.contains("/.local/lib/qt6/qml"), "{imports}");
        assert!(
            imports.contains("/.config/quickshell/caelestia"),
            "{imports}"
        );
    }
}
