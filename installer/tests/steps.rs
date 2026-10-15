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

    #[cfg(unix)]
    fn cache_file(&self, name: &str) -> PathBuf {
        self.path("cache").join("caelestia-kde").join(name)
    }

    fn step(&self, id: &str, path: &Path, extra: &[(&str, &str)]) -> Output {
        self.in_bundle(id, path, None, extra)
    }

    #[cfg(unix)]
    fn step_in(&self, id: &str, path: &Path, bundle: &Path, extra: &[(&str, &str)]) -> Output {
        self.in_bundle(id, path, Some(bundle), extra)
    }

    fn in_bundle(
        &self,
        id: &str,
        path: &Path,
        bundle: Option<&Path>,
        extra: &[(&str, &str)],
    ) -> Output {
        let mut command = Command::new(BINARY);
        command.args(["step", id]);
        if let Some(bundle) = bundle {
            command.arg("--bundle").arg(bundle);
        }
        command
            .env("PATH", path)
            .env("HOME", &self.root)
            .env("XDG_CACHE_HOME", self.path("cache"))
            .env("CAELESTIA_WALLPAPERS_DIR", self.library())
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
    stub_program(fixture, "git", body)
}

#[cfg(unix)]
fn stub_program(fixture: &Fixture, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = fixture.path("bin");
    std::fs::create_dir_all(&directory).expect("a directory for the stub");
    let program = directory.join(name);
    std::fs::write(&program, format!("#!/bin/sh\n{body}\n")).expect("the stub");
    let mut permissions = std::fs::metadata(&program).expect("the stub").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&program, permissions).expect("an executable stub");

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
const KONSAVE_STUB: &str = r#"mode=""
dir=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        -e) mode=export ;;
        -s) mode=save ;;
        -d) dir="$2" ;;
    esac
    shift
done
if [ "${STUB_FAIL_SAVE:-0}" = "1" ] && [ "$mode" = "save" ]; then
    exit 1
fi
if [ "$mode" = "export" ]; then
    : > "$dir/caelestia-preinstall.knsv"
fi
exit 0"#;

#[cfg(unix)]
#[test]
fn the_backup_exports_a_profile_and_puts_the_users_configuration_back() {
    let fixture = Fixture::new("backup");
    let path = stub_program(&fixture, "konsave", KONSAVE_STUB);
    let bundle = fixture.path("bundle");
    std::fs::create_dir_all(&bundle).expect("the bundle");
    let user_conf = fixture.path(".config/konsave/conf.yaml");
    std::fs::create_dir_all(user_conf.parent().expect("a parent")).expect("the config directory");
    std::fs::write(&user_conf, "mine\n").expect("the user's configuration");

    let output = fixture.step_in("00-backup-themes", &path, &bundle, &[]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains("Saving konsave profile 'caelestia-preinstall'..."),
        "{}",
        output.stdout
    );
    assert!(
        output
            .stdout
            .contains("KDE settings backed up to caelestia-preinstall.knsv"),
        "{}",
        output.stdout
    );

    let backups: Vec<_> = std::fs::read_dir(bundle.join("backups"))
        .expect("the backups directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    assert_eq!(backups.len(), 1, "one backup directory");
    assert!(
        backups[0].join("caelestia-preinstall.knsv").is_file(),
        "the exported profile is in the backup"
    );

    let recorded = std::fs::read_to_string(fixture.cache_file("backup-dir.txt"))
        .expect("the backup directory is recorded");
    assert_eq!(recorded.trim(), backups[0].display().to_string());

    assert_eq!(
        std::fs::read_to_string(&user_conf).expect("the user's configuration"),
        "mine\n",
        "what the user had is what the user keeps"
    );
}

#[cfg(unix)]
#[test]
fn a_configuration_we_wrote_ourselves_is_taken_away_again() {
    let fixture = Fixture::new("backup-clean");
    let path = stub_program(&fixture, "konsave", KONSAVE_STUB);
    let bundle = fixture.path("bundle");
    std::fs::create_dir_all(&bundle).expect("the bundle");

    let output = fixture.step_in("00-backup-themes", &path, &bundle, &[]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        !fixture.path(".config/konsave/conf.yaml").exists(),
        "the configuration we wrote is removed again"
    );
}

#[cfg(unix)]
#[test]
fn a_failed_save_fails_the_step_and_still_restores_the_configuration() {
    let fixture = Fixture::new("backup-fails");
    let path = stub_program(&fixture, "konsave", KONSAVE_STUB);
    let bundle = fixture.path("bundle");
    std::fs::create_dir_all(&bundle).expect("the bundle");
    let user_conf = fixture.path(".config/konsave/conf.yaml");
    std::fs::create_dir_all(user_conf.parent().expect("a parent")).expect("the config directory");
    std::fs::write(&user_conf, "mine\n").expect("the user's configuration");

    let output = fixture.step_in(
        "00-backup-themes",
        &path,
        &bundle,
        &[("STUB_FAIL_SAVE", "1")],
    );
    assert_eq!(output.code, Some(1));
    assert!(
        output
            .stderr
            .contains("Failed to save the current KDE profile."),
        "{}",
        output.stderr
    );
    assert_eq!(
        std::fs::read_to_string(&user_conf).expect("the user's configuration"),
        "mine\n"
    );
}

#[cfg(unix)]
#[test]
fn the_system_update_can_be_skipped_without_touching_the_system() {
    let fixture = Fixture::new("skip-update");
    let output = fixture.step(
        "00a-system-update",
        &fixture.empty_path(),
        &[("SKIP_SYSTEM_UPDATE", "true")],
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains("Skipping full system update (SKIP_SYSTEM_UPDATE=true)."),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn an_unknown_distribution_skips_the_update_with_a_warning() {
    let fixture = Fixture::new("unknown-distro");
    let output = fixture.step(
        "00a-system-update",
        &fixture.empty_path(),
        &[("BASE_DISTRO", "unknown")],
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains("Distro not set properly, skipping system update."),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn a_stale_session_mark_from_another_session_is_cleared() {
    let fixture = Fixture::new("stale-session");
    let state = fixture.path("state/caelestia");
    std::fs::create_dir_all(&state).expect("the state directory");
    let stamp = state.join("stale-session");
    std::fs::write(&stamp, "1234 5678\n").expect("a stale mark");

    let output = fixture.step(
        "00a-system-update",
        &fixture.empty_path(),
        &[
            ("SKIP_SYSTEM_UPDATE", "true"),
            (
                "XDG_STATE_HOME",
                &fixture.path("state").display().to_string(),
            ),
        ],
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(!stamp.exists(), "the stale mark is cleared");
}

#[cfg(unix)]
#[test]
fn prerequisites_that_are_present_are_reported_as_present() {
    let fixture = Fixture::new("prereqs");
    let mut path = PathBuf::new();
    for program in ["yq", "jq", "g++", "apt-get"] {
        path = stub_program(&fixture, program, "exit 0");
    }

    let output = fixture.step("01-ensure-prereqs", &path, &[("BASE_DISTRO", "debian")]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains("Prerequisites are already installed."),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
const CURL_STUB: &str = r#"dest=""
url=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o) shift; dest="$1" ;;
        http*) url="$1" ;;
    esac
    shift
done
case "$url" in
    *.sha256) exit 22 ;;
esac
[ -n "$dest" ] || exit 1
printf 'appimage' > "$dest"
exit 0"#;

#[cfg(unix)]
#[test]
fn no_optional_component_asked_for_is_no_work() {
    let fixture = Fixture::new("optional-none");
    let output = fixture.step("11-optional-apps", &fixture.empty_path(), &[]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("Optional components done."),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn a_todoist_download_that_fails_is_reported_and_skipped() {
    let fixture = Fixture::new("optional-todoist-fail");
    let path = stub_program(&fixture, "curl", "exit 1");

    let output = fixture.step("11-optional-apps", &path, &[("INSTALL_TODOIST", "true")]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains("Todoist download failed (network?). Skipping."),
        "{}",
        output.stdout
    );
    assert!(!fixture.path(".local/bin/todoist.AppImage").exists());
}

#[cfg(unix)]
#[test]
fn a_todoist_appimage_without_a_published_checksum_is_installed_with_a_warning() {
    let fixture = Fixture::new("optional-todoist");
    let path = stub_program(&fixture, "curl", CURL_STUB);

    let output = fixture.step("11-optional-apps", &path, &[("INSTALL_TODOIST", "true")]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("Todoist publishes no checksum"),
        "{}",
        output.stdout
    );
    assert!(
        output.stdout.contains("Todoist installed to"),
        "{}",
        output.stdout
    );

    let installed = fixture.path(".local/bin/todoist.AppImage");
    assert!(installed.is_file());
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(&installed)
        .expect("the file")
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0o111, "the AppImage is made executable");
}

#[cfg(unix)]
#[test]
fn firefox_theming_without_a_profile_says_what_to_do() {
    let fixture = Fixture::new("optional-firefox");
    let output = fixture.step(
        "11-optional-apps",
        &fixture.empty_path(),
        &[
            ("INSTALL_FIREFOX_THEME", "true"),
            ("BASE_DISTRO", "unknown"),
        ],
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains("No Firefox profile directory yet. Launch Firefox once, then re-run."),
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
