use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn available(program: &str) -> bool {
    locate(program).is_some()
}

pub fn locate(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(program);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        for extension in windows_extensions() {
            let candidate = directory.join(format!("{program}{extension}"));
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn succeeds(program: &str, args: &[&str], directory: Option<&Path>) -> bool {
    command(program, args, directory)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn status(program: &str, args: &[&str], directory: Option<&Path>) -> Option<i32> {
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null());
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    command.status().ok().and_then(|status| status.code())
}

pub fn status_quietly(program: &str, args: &[&str], directory: Option<&Path>) -> Option<i32> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    command.status().ok().and_then(|status| status.code())
}

pub fn capture(program: &str, args: &[&str]) -> Option<String> {
    capture_in(program, args, None)
}

pub fn capture_in(program: &str, args: &[&str], directory: Option<&Path>) -> Option<String> {
    let output = command(program, args, directory)
        .stdout(Stdio::piped())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn command(program: &str, args: &[&str], directory: Option<&Path>) -> Command {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    command
}

#[cfg(unix)]
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.is_file()
        && path
            .metadata()
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(not(unix))]
pub fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(windows)]
fn windows_extensions() -> Vec<String> {
    std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
        .split(';')
        .filter(|extension| !extension.is_empty())
        .map(|extension| extension.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_program_that_is_running_is_located_on_the_path() {
        assert!(locate("cargo").is_some(), "cargo runs this test");
    }

    #[test]
    fn a_program_that_does_not_exist_is_not_located() {
        assert!(!available("caelestia-definitely-not-a-program"));
    }

    #[test]
    fn a_successful_command_is_reported_as_one() {
        assert!(succeeds("cargo", &["--version"], None));
    }

    #[test]
    fn a_failing_command_is_reported_as_one() {
        assert!(!succeeds("cargo", &["--definitely-not-a-flag"], None));
    }

    #[test]
    fn a_missing_program_is_not_a_success() {
        assert!(!succeeds("caelestia-definitely-not-a-program", &[], None));
    }

    #[test]
    fn output_is_captured_for_a_call_that_succeeds() {
        let version = capture("cargo", &["--version"]).expect("cargo reports its version");
        assert!(version.starts_with("cargo "), "{version}");
    }

    #[test]
    fn output_is_captured_from_the_directory_that_is_asked_for() {
        let here = capture_in(
            "cargo",
            &["locate-project", "--message-format", "plain"],
            Some(Path::new(".")),
        )
        .expect("the manifest is found from the crate root");
        assert!(here.ends_with("Cargo.toml"), "{here}");
        assert!(Path::new(&here).is_absolute(), "{here}");

        let outside =
            crate::tmp::TempDir::new("caelestia-process-test").expect("a temporary directory");
        assert!(
            capture_in(
                "cargo",
                &["locate-project", "--message-format", "plain"],
                Some(outside.path())
            )
            .is_none(),
            "a directory outside the crate has no manifest"
        );
    }
}
