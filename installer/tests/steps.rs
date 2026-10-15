use std::path::{Path, PathBuf};
use std::process::Command;

const BINARY: &str = env!("CARGO_BIN_EXE_caelestia-install");

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("caelestia-steps.{name}.{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a fixture directory");
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn library(&self) -> PathBuf {
        self.path("Wallpapers")
    }

    fn step(&self, id: &str, path: &Path, extra: &[(&str, &str)]) -> Output {
        let mut command = Command::new(BINARY);
        command
            .args(["step", id])
            .env("PATH", path)
            .env("CAELESTIA_WALLPAPERS_DIR", self.library())
            .env("HOME", &self.root)
            .env_remove("XDG_PICTURES_DIR")
            .env_remove("TMPDIR");
        for (key, value) in extra {
            command.env(key, value);
        }
        let output = command.output().expect("the installer runs");
        Output {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
    }

    fn empty_path(&self) -> PathBuf {
        let path = self.path("empty-bin");
        std::fs::create_dir_all(&path).expect("a directory for the path");
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct Output {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

#[test]
fn a_step_that_does_not_exist_is_a_usage_error() {
    let fixture = Fixture::new("unknown");
    let output = fixture.step("not-a-step", &fixture.empty_path(), &[]);
    assert_eq!(output.code, Some(2));
    assert!(
        output.stderr.contains("no step is called"),
        "{}",
        output.stderr
    );
}

#[test]
fn asking_a_step_for_help_prints_the_usage() {
    let output = Command::new(BINARY)
        .args(["step", "--help"])
        .output()
        .expect("the installer runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("caelestia-install step"), "{stdout}");
}

#[test]
fn the_wallpaper_pack_is_left_alone_when_it_is_already_there() {
    let fixture = Fixture::new("present");
    let pack = fixture.library().join("dharmx-digital");
    std::fs::create_dir_all(&pack).expect("the pack");
    std::fs::write(
        pack.join("a_couple_of_people_standing_on_a_mountain.png"),
        b"x",
    )
    .expect("the default wallpaper");

    let output = fixture.step("03a-wallpapers", &fixture.empty_path(), &[]);
    assert_eq!(output.code, Some(0));
    assert!(
        output.stdout.contains("Wallpaper pack already present"),
        "{}",
        output.stdout
    );
}

#[test]
fn no_git_on_the_path_skips_the_download_without_failing() {
    let fixture = Fixture::new("no-git");
    let output = fixture.step("03a-wallpapers", &fixture.empty_path(), &[]);
    assert_eq!(output.code, Some(0));
    assert!(
        output
            .stdout
            .contains("git not found - skipping wallpaper pack download."),
        "{}",
        output.stdout
    );
    assert!(!fixture.library().exists(), "nothing should be written");
}

#[cfg(unix)]
fn stub_git(fixture: &Fixture, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = fixture.path("bin");
    std::fs::create_dir_all(&directory).expect("a directory for the stub");
    let git = directory.join("git");
    std::fs::write(&git, format!("#!/bin/sh\n{body}\n")).expect("the stub");
    let mut permissions = std::fs::metadata(&git).expect("the stub").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&git, permissions).expect("an executable stub");

    let system = std::env::var("PATH").unwrap_or_default();
    PathBuf::from(format!("{}:{system}", directory.display()))
}

#[cfg(unix)]
#[test]
fn the_pack_is_fetched_through_a_sparse_checkout() {
    let fixture = Fixture::new("fetch");
    let path = stub_git(
        &fixture,
        r#"for arg in "$@"; do dest="$arg"; done
case "$1" in
    clone)
        mkdir -p "$dest/digital"
        : > "$dest/digital/one.png"
        : > "$dest/digital/two.JPG"
        : > "$dest/digital/notes.txt"
        ;;
esac
exit 0"#,
    );

    let output = fixture.step("03a-wallpapers", &path, &[]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("Downloaded 2 wallpapers to"),
        "{}",
        output.stdout
    );

    let pack = fixture.library().join("dharmx-digital");
    assert!(pack.join("one.png").is_file());
    assert!(pack.join("two.JPG").is_file());
    assert!(!pack.join("notes.txt").exists(), "only images are taken");
    assert!(
        !fixture.library().join(".dharmx-digital.tmp").exists(),
        "the staging directory is gone"
    );
    assert!(
        output
            .stdout
            .contains("Expected default wallpaper missing from the pack"),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn a_failed_clone_leaves_the_library_as_it_was() {
    let fixture = Fixture::new("clone-fails");
    let path = stub_git(&fixture, "exit 1");

    let pack = fixture.library().join("dharmx-digital");
    std::fs::create_dir_all(fixture.library()).expect("the library");

    let output = fixture.step("03a-wallpapers", &path, &[]);
    assert_eq!(output.code, Some(0));
    assert!(
        output
            .stdout
            .contains("Failed to clone the wallpaper repository"),
        "{}",
        output.stdout
    );
    assert!(!pack.exists());
}
