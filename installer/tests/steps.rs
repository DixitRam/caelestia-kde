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

#[cfg(unix)]
const CONFIG_TOOLS: &str = r##"
import os
import sys
from pathlib import Path


def arguments(argv):
    values = {"file": "kwinrulesrc", "group": "", "key": ""}
    positional = []
    index = 0
    while index < len(argv):
        argument = argv[index]
        if argument in ("--file", "--group", "--key") and index + 1 < len(argv):
            values[argument[2:]] = argv[index + 1]
            index += 2
        else:
            positional.append(argument)
            index += 1
    return values, positional


def read(path):
    sections = {}
    current = None
    if not path.is_file():
        return sections
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith(("#", ";")):
            continue
        if line.startswith("[") and line.endswith("]"):
            current = line[1:-1]
            sections.setdefault(current, {})
        elif current is not None and "=" in line:
            key, _, value = line.partition("=")
            sections[current][key.strip()] = value.strip()
    return sections


def write(path, sections):
    lines = []
    for name, entries in sections.items():
        if not entries:
            continue
        lines.append("[%s]" % name)
        for key, value in entries.items():
            lines.append("%s=%s" % (key, value))
        lines.append("")
    path.write_text("\n".join(lines))


def main():
    mode = sys.argv[1]
    values, positional = arguments(sys.argv[2:])
    path = Path(values["file"])
    if not path.is_absolute():
        path = Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config"))) / path
    sections = read(path)

    if mode == "get":
        value = sections.get(values["group"], {}).get(values["key"])
        if value is None:
            return 1
        print(value)
        return 0

    if mode == "delete":
        entries = sections.get(values["group"])
        if entries is not None:
            entries.pop(values["key"], None)
            if not entries:
                del sections[values["group"]]
        write(path, sections)
        return 0

    sections.setdefault(values["group"], {})[values["key"]] = positional[0]
    write(path, sections)
    return 0


sys.exit(main())
"##;

#[cfg(unix)]
struct WindowRules {
    path: PathBuf,
    config: PathBuf,
    calls: PathBuf,
}

#[cfg(unix)]
fn config_tools(fixture: &Fixture) -> WindowRules {
    use std::os::unix::fs::PermissionsExt;

    let config = fixture.path("config");
    let bin = fixture.path("bin");
    std::fs::create_dir_all(&config).expect("the config directory");
    std::fs::create_dir_all(&bin).expect("a directory for the stubs");

    let program = fixture.path("kwinrules.py");
    std::fs::write(&program, CONFIG_TOOLS).expect("the config-tool program");
    let calls = fixture.path("calls.log");
    std::fs::write(&calls, "").expect("the call log");

    let stub = |name: &str, body: String| {
        let path = bin.join(name);
        std::fs::write(&path, body).expect("a stub");
        let mut permissions = std::fs::metadata(&path).expect("the stub").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("an executable stub");
    };

    stub(
        "kreadconfig6",
        "#!/bin/sh\nexec python3 \"$KW_RULES\" get \"$@\"\n".to_string(),
    );
    stub(
        "kwriteconfig6",
        "#!/bin/sh\nprintf '%s %s\\n' 'kwriteconfig6' \"$*\" >> \"$KW_CALLS\"\n\
         if [ -n \"${KW_SILENT_REJECT:-}\" ]; then exit 0; fi\n\
         case \"$*\" in *--delete*) exec python3 \"$KW_RULES\" delete \"$@\" ;; esac\n\
         exec python3 \"$KW_RULES\" set \"$@\"\n"
            .to_string(),
    );
    stub(
        "qdbus6",
        "#!/bin/sh\nprintf '%s %s\\n' 'qdbus6' \"$*\" >> \"$KW_CALLS\"\nexit 0\n".to_string(),
    );

    let system = std::env::var("PATH").unwrap_or_default();
    let path = PathBuf::from(format!("{}:{system}", bin.display()));

    WindowRules {
        path,
        config,
        calls,
    }
}

#[cfg(unix)]
fn run_window_rules(fixture: &Fixture, tools: &WindowRules, extra: &[(&str, &str)]) -> Output {
    let program = fixture.path("kwinrules.py");
    let program = program.to_str().expect("a path");
    let config = tools.config.to_str().expect("a path");
    let calls = tools.calls.to_str().expect("a path");

    let mut environment: Vec<(&str, &str)> = vec![
        ("XDG_CONFIG_HOME", config),
        ("KW_RULES", program),
        ("KW_CALLS", calls),
    ];
    environment.extend_from_slice(extra);
    fixture.step("04a-window-rules", &tools.path, &environment)
}

#[cfg(unix)]
fn writer_calls(tools: &WindowRules) -> Vec<String> {
    std::fs::read_to_string(&tools.calls)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.starts_with("kwriteconfig6 "))
        .map(|line| line.trim_start_matches("kwriteconfig6 ").to_string())
        .collect()
}

#[cfg(unix)]
fn frozen_writes(opacity: &str, index: &str) -> Vec<String> {
    let count = index.split(',').count();
    [
        "--file kwinrulesrc --group caelestia-opacity --key Description Caelestia: window opacity",
        "--file kwinrulesrc --group caelestia-opacity --key types 33",
    ]
    .into_iter()
    .map(str::to_string)
    .chain([
        format!("--file kwinrulesrc --group caelestia-opacity --key opacityinactive {opacity}"),
        "--file kwinrulesrc --group caelestia-opacity --key opacityinactiverule 2".to_string(),
        "--file kwinrulesrc --group caelestia-dialogs --key Description Caelestia: center dialogs"
            .to_string(),
        "--file kwinrulesrc --group caelestia-dialogs --key types 32".to_string(),
        "--file kwinrulesrc --group caelestia-dialogs --key placement 5".to_string(),
        "--file kwinrulesrc --group caelestia-dialogs --key placementrule 2".to_string(),
        "--file kwinrulesrc --group caelestia-pip --key Description Caelestia: pin picture-in-picture"
            .to_string(),
        "--file kwinrulesrc --group caelestia-pip --key title Picture(-| )in(-| )[Pp]icture"
            .to_string(),
        "--file kwinrulesrc --group caelestia-pip --key titlematch 3".to_string(),
        "--file kwinrulesrc --group caelestia-pip --key above true".to_string(),
        "--file kwinrulesrc --group caelestia-pip --key aboverule 2".to_string(),
        format!("--file kwinrulesrc --group General --key rules {index}"),
        format!("--file kwinrulesrc --group General --key count {count}"),
    ])
    .collect()
}

#[cfg(unix)]
const FRESH_INDEX: &str = "caelestia-opacity,caelestia-dialogs,caelestia-pip";

#[cfg(unix)]
#[test]
fn a_fresh_run_writes_the_frozen_table_in_order() {
    if !process_available("python3") {
        return;
    }
    let fixture = Fixture::new("rules-fresh");
    let tools = config_tools(&fixture);

    let output = run_window_rules(&fixture, &tools, &[]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert_eq!(
        writer_calls(&tools),
        frozen_writes("95", FRESH_INDEX),
        "every key of the three groups, then the index"
    );
    assert!(
        output.stdout.contains("Window rules applied."),
        "{}",
        output.stdout
    );
    assert!(
        output.stdout.contains("Rule index names 3 group(s)."),
        "{}",
        output.stdout
    );

    let written = std::fs::read_to_string(tools.config.join("kwinrulesrc")).expect("the file");
    for pair in [
        "opacityinactive=95",
        "types=33",
        "types=32",
        "placement=5",
        "title=Picture(-| )in(-| )[Pp]icture",
        "above=true",
        "aboverule=2",
    ] {
        assert!(written.contains(pair), "{pair} is missing from {written}");
    }
    assert_eq!(written.matches("[caelestia-").count(), 3, "{written}");
}

#[cfg(unix)]
#[test]
fn a_second_run_finds_nothing_to_do() {
    if !process_available("python3") {
        return;
    }
    let fixture = Fixture::new("rules-again");
    let tools = config_tools(&fixture);

    let _ = run_window_rules(&fixture, &tools, &[]);
    std::fs::write(&tools.calls, "").expect("a fresh call log");
    let output = run_window_rules(&fixture, &tools, &[]);

    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(writer_calls(&tools).is_empty(), "nothing should be written");
    assert!(
        output.stdout.contains("Window rules already in place."),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn a_user_rule_keeps_its_place_in_the_index() {
    if !process_available("python3") {
        return;
    }
    let fixture = Fixture::new("rules-user");
    let tools = config_tools(&fixture);
    std::fs::write(
        tools.config.join("kwinrulesrc"),
        "\
[General]
rules=user-rule
count=1
Order=user-rule

[user-rule]
Description=Keep this window put
wmclass=keepme

[$Version]
update_info=kwinrules.upd:fix-rule-order
",
    )
    .expect("the user's rules file");

    let output = run_window_rules(&fixture, &tools, &[]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);

    let index = "user-rule,caelestia-opacity,caelestia-dialogs,caelestia-pip";
    assert!(
        writer_calls(&tools).contains(&format!(
            "--file kwinrulesrc --group General --key rules {index}"
        )),
        "{:?}",
        writer_calls(&tools)
    );
    let written = std::fs::read_to_string(tools.config.join("kwinrulesrc")).expect("the file");
    assert!(
        written.contains("wmclass=keepme"),
        "the user's rule survives"
    );
    assert!(
        written.contains("Order=user-rule"),
        "their order key survives"
    );
    assert!(
        !written.contains("$Version,"),
        "the metadata group never enters the index: {written}"
    );
    assert!(
        written.contains("rules=user-rule,caelestia-opacity,caelestia-dialogs,caelestia-pip"),
        "{written}"
    );
}

#[cfg(unix)]
#[test]
fn the_reload_comes_once_and_last() {
    if !process_available("python3") {
        return;
    }
    let fixture = Fixture::new("rules-reload");
    let tools = config_tools(&fixture);

    let _ = run_window_rules(&fixture, &tools, &[]);
    let calls = std::fs::read_to_string(&tools.calls).expect("the call log");
    let lines: Vec<&str> = calls.lines().collect();

    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("qdbus6 "))
            .count(),
        1,
        "one reload, after every write: {calls}"
    );
    assert_eq!(
        lines.last().map(|line| line.starts_with("qdbus6 ")),
        Some(true),
        "{calls}"
    );
}

#[cfg(unix)]
#[test]
fn the_inactive_opacity_comes_from_the_environment() {
    if !process_available("python3") {
        return;
    }
    let fixture = Fixture::new("rules-opacity");
    let tools = config_tools(&fixture);

    let _ = run_window_rules(&fixture, &tools, &[("WINDOW_OPACITY", "80")]);
    assert!(
        writer_calls(&tools).contains(
            &"--file kwinrulesrc --group caelestia-opacity --key opacityinactive 80".to_string()
        ),
        "{:?}",
        writer_calls(&tools)
    );
}

#[cfg(unix)]
#[test]
fn a_silent_rejection_is_reported_as_failed() {
    if !process_available("python3") {
        return;
    }
    let fixture = Fixture::new("rules-rejected");
    let tools = config_tools(&fixture);

    let output = run_window_rules(&fixture, &tools, &[("KW_SILENT_REJECT", "1")]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("could not be written"),
        "{}",
        output.stdout
    );
    assert!(
        output.stdout.contains("Window rules not fully applied"),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn a_missing_writer_warns_instead_of_failing() {
    let fixture = Fixture::new("rules-no-writer");
    let bare = fixture.path("bare");
    std::fs::create_dir_all(&bare).expect("a directory for the bare path");

    let output = fixture.step("04a-window-rules", &bare, &[]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains("kwriteconfig6 not found - skipping window rules."),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn the_step_can_be_switched_off() {
    if !process_available("python3") {
        return;
    }
    let fixture = Fixture::new("rules-off");
    let tools = config_tools(&fixture);

    let output = run_window_rules(&fixture, &tools, &[("APPLY_WINDOW_RULES", "false")]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("Skipping window rules"),
        "{}",
        output.stdout
    );
    assert!(writer_calls(&tools).is_empty(), "and writes nothing");
}

#[cfg(unix)]
fn process_available(program: &str) -> bool {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .any(|directory| {
            !directory.is_empty() && std::path::Path::new(directory).join(program).is_file()
        })
}

#[cfg(unix)]
fn stub_in(directory: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    std::fs::create_dir_all(directory).expect("a directory for the stub");
    let path = directory.join(name);
    std::fs::write(&path, body).expect("a stub");
    let mut permissions = std::fs::metadata(&path).expect("the stub").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("an executable stub");
    path
}

#[cfg(unix)]
struct KdeTools {
    path: PathBuf,
    calls: PathBuf,
    state: PathBuf,
}

#[cfg(unix)]
fn kde_tools(fixture: &Fixture) -> KdeTools {
    let bin = fixture.path("bin");
    let calls = fixture.path("calls.log");
    std::fs::write(&calls, "").expect("the call log");

    stub_in(
        &bin,
        "kwriteconfig6",
        "#!/bin/sh\nprintf '%s %s\\n' 'kwriteconfig6' \"$*\" >> \"$KW_CALLS\"\nexit 0\n",
    );
    stub_in(
        &bin,
        "systemctl",
        "#!/bin/sh\nprintf '%s %s\\n' 'systemctl' \"$*\" >> \"$KW_CALLS\"\nexit 0\n",
    );
    stub_in(
        &bin,
        "qdbus6",
        "#!/bin/sh\nprintf '%s %s\\n' 'qdbus6' \"$*\" >> \"$KW_CALLS\"\nexit 0\n",
    );
    stub_in(
        &bin,
        "lookandfeeltool",
        "#!/bin/sh\ncase \"$1\" in
    --list) [ -n \"$KW_LNF\" ] && printf '%s\\n' \"$KW_LNF\" ;;
    --apply) printf 'applied %s\\n' \"$2\" >> \"$KW_CALLS\" ;;
esac
exit 0",
    );

    let system = std::env::var("PATH").unwrap_or_default();
    KdeTools {
        path: PathBuf::from(format!("{}:{system}", bin.display())),
        calls,
        state: fixture.path("state"),
    }
}

#[cfg(unix)]
fn run_kde_theme(fixture: &Fixture, tools: &KdeTools, extra: &[(&str, &str)]) -> Output {
    let calls = tools.calls.to_str().expect("a path");
    let state = tools.state.to_str().expect("a path");
    let mut environment: Vec<(&str, &str)> = vec![
        ("KW_CALLS", calls),
        ("XDG_STATE_HOME", state),
        ("XDG_DATA_HOME", calls),
    ];
    environment.extend_from_slice(extra);

    let bundle = fixture.path("bundle");
    fixture.step_in("04-deploy-kde", &tools.path, &bundle, &environment)
}

#[cfg(unix)]
fn recorded(tools: &KdeTools, program: &str) -> Vec<String> {
    std::fs::read_to_string(&tools.calls)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.starts_with(program))
        .map(|line| line.trim_start_matches(program).trim().to_string())
        .collect()
}

#[cfg(unix)]
#[test]
fn the_darkly_settings_are_written_once_each() {
    let fixture = Fixture::new("kde-darkly");
    let tools = kde_tools(&fixture);

    let output = run_kde_theme(&fixture, &tools, &[("KW_LNF", "Darkly")]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);

    let writes = recorded(&tools, "kwriteconfig6");
    for expected in [
        "--file plasmarc --group Theme --key name darkly",
        "--file kdeglobals --group KDE --key widgetStyle darkly",
        "--file kdeglobals --group General --key ColorScheme Darkly",
        "--file kwinrc --group org.kde.kdecoration2 --key library org.kde.breeze",
        "--file kwinrc --group org.kde.kdecoration2 --key theme Breeze",
    ] {
        assert!(writes.contains(&expected.to_string()), "{writes:?}");
    }
    assert!(
        recorded(&tools, "systemctl").contains(&"--user enable --now cliphist.service".to_string()),
        "{:?}",
        recorded(&tools, "systemctl")
    );
    assert!(
        output.stdout.contains("KDE settings applied."),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn opting_out_of_darkly_leaves_the_theme_alone() {
    let fixture = Fixture::new("kde-no-darkly");
    let tools = kde_tools(&fixture);

    let output = run_kde_theme(&fixture, &tools, &[("APPLY_DARKLY", "false")]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("Skipping Darkly theme"),
        "{}",
        output.stdout
    );
    assert!(
        !recorded(&tools, "kwriteconfig6")
            .iter()
            .any(|call| call.contains("widgetStyle")),
        "no application style should be chosen"
    );
}

#[cfg(unix)]
#[test]
fn the_runner_feeds_the_history_and_serves_the_persister() {
    let fixture = Fixture::new("kde-cliphist");
    let tools = kde_tools(&fixture);
    let _ = run_kde_theme(&fixture, &tools, &[]);

    let runner = fixture.path(".local/bin/caelestia-cliphist");
    assert!(runner.is_file(), "the runner is installed");

    let watchers = fixture.path("watchers.log");
    let persister = fixture.path("persister.log");
    let bin = fixture.path("runner-bin");
    stub_in(
        &bin,
        "wl-paste",
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$KW_WATCHERS\"\nexit 0\n",
    );
    stub_in(&bin, "cliphist", "#!/bin/sh\nexit 0\n");
    stub_in(
        &bin,
        "wl-clip-persist",
        "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$KW_PERSISTER\"\nexit 0\n",
    );

    let watchers_path = watchers.to_str().expect("a path").to_string();
    let persister_path = persister.to_str().expect("a path").to_string();
    let output = Command::new(&runner)
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("KW_WATCHERS", &watchers_path)
        .env("KW_PERSISTER", &persister_path)
        .output()
        .expect("the runner runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let fed = std::fs::read_to_string(&watchers).unwrap_or_default();
    assert!(fed.contains("--type text --watch cliphist store"), "{fed}");
    assert!(fed.contains("--type image --watch cliphist store"), "{fed}");

    let serve = std::fs::read_to_string(&persister).unwrap_or_default();
    let lines: Vec<&str> = serve.lines().collect();
    assert_eq!(
        lines.first().map(|line| line.contains("--clipboard")),
        Some(true),
        "{lines:?}"
    );
    assert_eq!(
        lines.get(1).map(|line| line.contains("regular")),
        Some(true),
        "{lines:?}"
    );
    assert_eq!(
        lines
            .get(2)
            .map(|line| line.contains("--all-mime-type-regex")),
        Some(true),
        "{lines:?}"
    );
    assert_eq!(
        lines
            .get(3)
            .map(|line| line.contains("application/x-krita-")
                && line.contains("image/x-inkscape-svg")),
        Some(true),
        "the filter reaches the tool as one argument: {lines:?}"
    );
    assert_eq!(lines.len(), 4, "{lines:?}");
}

#[cfg(unix)]
#[test]
fn a_missing_helper_is_reported() {
    let fixture = Fixture::new("kde-cliphist-missing");
    let tools = kde_tools(&fixture);
    let _ = run_kde_theme(&fixture, &tools, &[]);

    let runner = fixture.path(".local/bin/caelestia-cliphist");
    let bin = fixture.path("runner-bin");
    stub_in(&bin, "wl-paste", "#!/bin/sh\nexit 0\n");
    stub_in(&bin, "cliphist", "#!/bin/sh\nexit 0\n");

    let output = Command::new(&runner)
        .env("PATH", bin.display().to_string())
        .output()
        .expect("the runner runs");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing: wl-clip-persist"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn the_wallpaper_in_use_is_kept_and_matched_to_the_logout_screen() {
    let fixture = Fixture::new("kde-wallpaper-keep");
    let tools = kde_tools(&fixture);
    let chosen = fixture.path("Pictures/My Walls/mine.png");
    std::fs::create_dir_all(chosen.parent().expect("a parent")).expect("the pictures directory");
    std::fs::write(&chosen, b"png").expect("the wallpaper");

    let state = fixture.path("state/caelestia/wallpaper");
    std::fs::create_dir_all(&state).expect("the state directory");
    std::fs::write(state.join("path.txt"), format!("{}\n", chosen.display())).expect("the record");

    let output = run_kde_theme(&fixture, &tools, &[]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains("Keeping the wallpaper in use: mine.png"),
        "{}",
        output.stdout
    );

    let log = std::fs::read_to_string(&tools.calls).expect("the call log");
    assert!(log.contains("d.writeConfig('Image'"), "{log}");
    assert!(
        log.contains("My\\u0020Walls"),
        "a space reaches the script escaped, not raw: {log}"
    );
    assert!(
        log.contains("file:///"),
        "and the slashes are left as they are: {log}"
    );
}

#[cfg(unix)]
#[test]
fn the_default_wallpaper_is_recorded_and_given_to_the_logout_screen() {
    let fixture = Fixture::new("kde-wallpaper-default");
    let tools = kde_tools(&fixture);

    let fallback = fixture.path("bundle/shell/assets/wallpaper.webp");
    std::fs::create_dir_all(fallback.parent().expect("a parent")).expect("the assets directory");
    std::fs::write(&fallback, b"webp").expect("the fallback wallpaper");

    let output = run_kde_theme(&fixture, &tools, &[]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains("Setting default wallpaper to wallpaper.webp"),
        "{}",
        output.stdout
    );

    let recorded_path = fixture.path("state/caelestia/wallpaper/path.txt");
    assert_eq!(
        std::fs::read_to_string(&recorded_path)
            .expect("the record")
            .trim(),
        fallback.display().to_string()
    );

    let writes = recorded(&tools, "kwriteconfig6");
    assert!(
        writes.contains(
            &"--file kscreenlockerrc --group Greeter --key WallpaperPlugin org.kde.image"
                .to_string()
        ),
        "{writes:?}"
    );
    assert!(
        writes
            .iter()
            .any(|call| call.contains("--group Greeter --group Wallpaper")),
        "{writes:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_missing_wallpaper_leaves_the_desktop_alone() {
    let fixture = Fixture::new("kde-wallpaper-none");
    let tools = kde_tools(&fixture);

    let output = run_kde_theme(&fixture, &tools, &[]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(recorded(&tools, "qdbus6").is_empty(), "nothing to set");
    assert!(!fixture.path("state/caelestia/wallpaper/path.txt").exists());
}

#[cfg(unix)]
struct Autostart {
    path: PathBuf,
    calls: PathBuf,
}

#[cfg(unix)]
fn autostart_tools(fixture: &Fixture) -> Autostart {
    let bin = fixture.path("bin");
    let calls = fixture.path("calls.log");
    std::fs::write(&calls, "").expect("the call log");

    stub_in(
        &bin,
        "systemctl",
        "#!/bin/sh\nprintf '%s %s\\n' 'systemctl' \"$*\" >> \"$KW_CALLS\"\nexit 0\n",
    );
    stub_in(&bin, "update-desktop-database", "#!/bin/sh\nexit 0\n");
    stub_in(&bin, "kbuildsycoca6", "#!/bin/sh\nexit 0\n");
    stub_in(
        &bin,
        "quickshell",
        "#!/bin/sh\n[ \"${KW_FAIL_QUICKSHELL:-}\" = 1 ] && exit 1\nexit 0\n",
    );

    let system = std::env::var("PATH").unwrap_or_default();
    Autostart {
        path: PathBuf::from(format!("{}:{system}", bin.display())),
        calls,
    }
}

#[cfg(unix)]
fn run_autostart(fixture: &Fixture, tools: &Autostart, extra: &[(&str, &str)]) -> Output {
    let calls = tools.calls.to_str().expect("a path");
    let mut environment: Vec<(&str, &str)> = vec![("KW_CALLS", calls)];
    environment.extend_from_slice(extra);
    let bundle = fixture.path("bundle");
    fixture.step_in("10-autostart", &tools.path, &bundle, &environment)
}

#[cfg(unix)]
fn autostart_calls(tools: &Autostart) -> Vec<String> {
    std::fs::read_to_string(&tools.calls)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.starts_with("systemctl"))
        .map(|line| line.trim_start_matches("systemctl").trim().to_string())
        .collect()
}

#[cfg(unix)]
#[test]
fn a_source_install_writes_the_launcher_the_unit_and_the_interface_declaration() {
    let fixture = Fixture::new("autostart-source");
    let tools = autostart_tools(&fixture);
    let shell_config = fixture.path(".config/quickshell/caelestia/shell.qml");
    std::fs::create_dir_all(shell_config.parent().expect("a parent")).expect("the config tree");
    std::fs::write(&shell_config, b"// shell").expect("the entrypoint");

    let source = fixture.path("bundle/assets/org.quickshell.desktop");
    std::fs::create_dir_all(source.parent().expect("a parent")).expect("the assets directory");
    std::fs::write(
        &source,
        "[Desktop Entry]\nExec=/usr/bin/quickshell\nX-KDE-Wayland-Interfaces=zkde_screencast_unstable_v1\n",
    )
    .expect("the asset");

    let output = run_autostart(&fixture, &tools, &[("CAELESTIA_INSTALL_KIND", "source")]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);

    let launcher = fixture.path(".local/bin/caelestia-autostart.sh");
    assert!(launcher.is_file(), "the launcher is installed");
    let script = std::fs::read_to_string(&launcher).expect("the launcher");
    assert!(
        script.contains("exec \"") && script.contains("\" -n -p \""),
        "it starts the entrypoint the install named: {script}"
    );
    assert!(
        script.contains(&shell_config.display().to_string()),
        "and the entrypoint is the one this install has: {script}"
    );

    let unit = fixture.path(".config/systemd/user/caelestia-shell.service");
    let unit_text = std::fs::read_to_string(&unit).expect("the unit");
    assert!(unit_text.contains("ExecStart=%h/.local/bin/caelestia-autostart.sh"));
    assert!(unit_text.contains("Before=xdg-desktop-autostart.target"));

    let calls = autostart_calls(&tools);
    assert!(
        calls.contains(&"--user reset-failed caelestia-shell.service".to_string()),
        "a failed unit is cleared before it is used: {calls:?}"
    );
    assert!(
        calls.contains(&"--user enable caelestia-shell.service".to_string()),
        "{calls:?}"
    );

    let declaration = fixture.path(".local/share/applications/quickshell.desktop");
    let declaration_text = std::fs::read_to_string(&declaration).expect("the declaration");
    assert!(declaration_text.contains("X-KDE-Wayland-Interfaces=zkde_screencast_unstable_v1"));

    let preview = fixture.path(".local/share/applications/org.quickshell.desktop");
    let preview_text = std::fs::read_to_string(&preview).expect("the preview declaration");
    assert!(
        preview_text.contains("Exec=/") && !preview_text.contains("Exec=/usr/bin/quickshell"),
        "the exec line points at the resolved binary: {preview_text}"
    );
    assert!(
        output.stdout.contains("Autostart entries configured."),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn a_packaged_install_takes_the_users_own_copies_out_of_the_way() {
    let fixture = Fixture::new("autostart-package");
    let tools = autostart_tools(&fixture);
    let unit = fixture.path(".config/systemd/user/caelestia-shell.service");
    let launcher = fixture.path(".local/bin/caelestia-autostart.sh");
    std::fs::create_dir_all(unit.parent().expect("a parent")).expect("the unit directory");
    std::fs::create_dir_all(launcher.parent().expect("a parent")).expect("the bin directory");
    std::fs::write(&unit, b"stale").expect("a stale unit");
    std::fs::write(&launcher, b"stale").expect("a stale launcher");

    let output = run_autostart(&fixture, &tools, &[("CAELESTIA_INSTALL_KIND", "package")]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(!unit.exists(), "the user's copy of the unit goes");
    assert!(!launcher.exists(), "and the user's copy of the launcher");
    assert!(
        output.stdout.contains("the package's is the one to use"),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn the_retired_entries_and_a_dead_enable_link_do_not_survive() {
    let fixture = Fixture::new("autostart-retire");
    let tools = autostart_tools(&fixture);
    let shell_config = fixture.path(".config/quickshell/caelestia/shell.qml");
    std::fs::create_dir_all(shell_config.parent().expect("a parent")).expect("the config tree");
    std::fs::write(&shell_config, b"// shell").expect("the entrypoint");

    let retired = fixture.path(".config/autostart/caelestiashell.desktop");
    std::fs::create_dir_all(retired.parent().expect("a parent")).expect("the autostart directory");
    std::fs::write(&retired, b"[Desktop Entry]").expect("the retired entry");

    let wants = fixture.path(".config/systemd/user/graphical-session.target.wants");
    std::fs::create_dir_all(&wants).expect("the wants directory");
    let dead = wants.join("caelestia-shell.service");
    std::os::unix::fs::symlink(fixture.path("gone"), &dead).expect("a dead link");

    let material_you = fixture.path(".config/systemd/user/kde-material-you-colors.service");
    std::fs::write(&material_you, b"[Unit]").expect("the material-you unit");
    let scheme = fixture.path(".local/share/color-schemes/MaterialYou.colors");
    std::fs::create_dir_all(scheme.parent().expect("a parent")).expect("the schemes directory");
    std::fs::write(&scheme, b"[Colors]").expect("a generated scheme");

    let output = run_autostart(&fixture, &tools, &[("CAELESTIA_INSTALL_KIND", "source")]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);

    assert!(!retired.exists(), "the retired autostart entry goes");
    assert!(!dead.exists(), "a dead enable link goes");
    assert!(!material_you.exists(), "the material-you unit goes");
    assert!(!scheme.exists(), "and the scheme it generated");

    let calls = autostart_calls(&tools);
    assert!(
        calls.contains(&"--user disable app-caelestiashell@autostart.service".to_string()),
        "{calls:?}"
    );
    assert!(
        calls.contains(&"--user disable --now kde-material-you-colors.service".to_string()),
        "{calls:?}"
    );
}

#[cfg(unix)]
#[test]
fn the_isolated_autostart_step_is_idempotent() {
    let fixture = Fixture::new("autostart-again");
    let tools = autostart_tools(&fixture);
    let shell_config = fixture.path(".config/quickshell/caelestia/shell.qml");
    std::fs::create_dir_all(shell_config.parent().expect("a parent")).expect("the config tree");
    std::fs::write(&shell_config, b"// shell").expect("the entrypoint");

    let launcher = fixture.path(".local/bin/caelestia-autostart.sh");
    let unit = fixture.path(".config/systemd/user/caelestia-shell.service");
    let declaration = fixture.path(".local/share/applications/quickshell.desktop");

    let first = run_autostart(&fixture, &tools, &[("CAELESTIA_INSTALL_KIND", "source")]);
    assert_eq!(first.code, Some(0), "{}", first.stderr);
    let before = [
        std::fs::read(&launcher).expect("the launcher"),
        std::fs::read(&unit).expect("the unit"),
        std::fs::read(&declaration).expect("the declaration"),
    ];

    let second = run_autostart(&fixture, &tools, &[("CAELESTIA_INSTALL_KIND", "source")]);
    assert_eq!(second.code, Some(0), "{}", second.stderr);
    let after = [
        std::fs::read(&launcher).expect("the launcher"),
        std::fs::read(&unit).expect("the unit"),
        std::fs::read(&declaration).expect("the declaration"),
    ];

    assert_eq!(
        before, after,
        "repeating the step keeps the files it generates byte for byte"
    );
}

#[cfg(unix)]
#[test]
fn without_quickshell_the_step_refuses_to_configure_anything() {
    let fixture = Fixture::new("autostart-no-quickshell");
    let bare = fixture.path("bare");
    std::fs::create_dir_all(&bare).expect("a directory for the bare path");

    let output = fixture.step(
        "10-autostart",
        &bare,
        &[("CAELESTIA_INSTALL_KIND", "source")],
    );
    assert_eq!(output.code, Some(1));
    assert!(
        output
            .stderr
            .contains("Quickshell is not installed or is not available in PATH."),
        "{}",
        output.stderr
    );
    assert!(
        !fixture
            .path(".config/systemd/user/caelestia-shell.service")
            .exists()
    );
}

#[cfg(unix)]
fn desktop_calls(fixture: &Fixture) -> Vec<String> {
    std::fs::read_to_string(fixture.path("calls.log"))
        .unwrap_or_default()
        .lines()
        .filter(|line| line.starts_with("kwriteconfig6 "))
        .filter(|line| line.contains("--group Desktops --key"))
        .map(|line| line.trim_start_matches("kwriteconfig6 ").to_string())
        .collect()
}

#[cfg(unix)]
#[test]
fn a_fresh_install_without_a_desktop_count_gets_five_named_desktops() {
    let fixture = Fixture::new("desktops-fresh");
    let tools = services_tools(&fixture);

    let output = run_services(
        &fixture,
        &tools,
        &[
            ("CAELESTIA_INSTALL_KIND", "package"),
            ("CAELESTIA_FRESH_INSTALL", "1"),
        ],
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);

    let written = desktop_calls(&fixture);
    assert!(
        written.contains(&"--file kwinrc --group Desktops --key Number 5".to_string()),
        "{written:?}"
    );
    assert!(
        written.contains(&"--file kwinrc --group Desktops --key Rows 1".to_string()),
        "the desktops are laid out in one row: {written:?}"
    );
    assert!(
        written.contains(&"--file kwinrc --group Desktops --key Name_5 Desktop 5".to_string()),
        "every desktop is named: {written:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_fresh_install_at_kwins_own_default_of_one_gets_five() {
    let fixture = Fixture::new("desktops-default");
    let tools = services_tools(&fixture);

    let output = run_services(
        &fixture,
        &tools,
        &[
            ("CAELESTIA_INSTALL_KIND", "package"),
            ("CAELESTIA_FRESH_INSTALL", "1"),
            ("KW_DESKTOPS", "1"),
        ],
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);

    let written = desktop_calls(&fixture);
    assert!(
        written.contains(&"--file kwinrc --group Desktops --key Number 5".to_string()),
        "a kwinrc carrying KWin's own default is not a user's configuration: {written:?}"
    );
    assert!(
        written.contains(&"--file kwinrc --group Desktops --key Name_1 Desktop 1".to_string()),
        "{written:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_fresh_install_keeps_a_desktop_count_the_user_chose() {
    let fixture = Fixture::new("desktops-chosen");
    let tools = services_tools(&fixture);

    let output = run_services(
        &fixture,
        &tools,
        &[
            ("CAELESTIA_INSTALL_KIND", "package"),
            ("CAELESTIA_FRESH_INSTALL", "1"),
            ("KW_DESKTOPS", "3"),
        ],
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert_eq!(
        desktop_calls(&fixture),
        Vec::<String>::new(),
        "the desktops a user configured are left alone"
    );
    assert!(
        output
            .stdout
            .contains("Keeping the existing 3 virtual desktops."),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn an_update_never_touches_the_desktop_count() {
    for count in ["1", ""] {
        let fixture = Fixture::new("desktops-update");
        let tools = services_tools(&fixture);

        let output = run_services(
            &fixture,
            &tools,
            &[
                ("CAELESTIA_INSTALL_KIND", "package"),
                ("CAELESTIA_FRESH_INSTALL", "0"),
                ("KW_DESKTOPS", count),
            ],
        );
        assert_eq!(output.code, Some(0), "{}", output.stderr);
        assert_eq!(
            desktop_calls(&fixture),
            Vec::<String>::new(),
            "an update must not be read as an install"
        );
        assert!(
            output.stdout.contains("Not an install"),
            "{}",
            output.stdout
        );
    }
}

#[cfg(unix)]
struct ConfigTools {
    path: PathBuf,
    calls: PathBuf,
}

#[cfg(unix)]
struct Services {
    path: PathBuf,
    calls: PathBuf,
}

#[cfg(unix)]
fn services_tools(fixture: &Fixture) -> Services {
    let bin = fixture.path("bin");
    let calls = fixture.path("calls.log");
    std::fs::write(&calls, "").expect("the call log");

    stub_in(
        &bin,
        "systemctl",
        "#!/bin/sh\n[ \"${KW_KEYD:-}\" = 1 ] && case \"$*\" in *keyd*) exit 0 ;; esac\nexit 1\n",
    );
    stub_in(
        &bin,
        "kwriteconfig6",
        "#!/bin/sh\nprintf 'kwriteconfig6 %s\\n' \"$*\" >> \"$KW_CALLS\"\nexit 0\n",
    );
    stub_in(
        &bin,
        "kreadconfig6",
        "#!/bin/sh\n[ -n \"${KW_DESKTOPS:-}\" ] && printf '%s\\n' \"$KW_DESKTOPS\"\nexit 0\n",
    );

    let system = std::env::var("PATH").unwrap_or_default();
    Services {
        path: PathBuf::from(format!("{}:{system}", bin.display())),
        calls,
    }
}

#[cfg(unix)]
fn run_services(fixture: &Fixture, tools: &Services, extra: &[(&str, &str)]) -> Output {
    let calls = tools.calls.to_str().expect("a path");
    let rules = fixture.path("rules");
    std::fs::create_dir_all(&rules).expect("a rules directory");
    // A current rule keeps the system-level branch, which escalates, out of these tests;
    // the decision itself is covered by the unit tests beside it.
    std::fs::write(
        rules.join("70-uinput.rules"),
        "KERNEL==\"uinput\", MODE=\"0600\", OPTIONS+=\"static_node=uinput\", TAG+=\"uaccess\"\n",
    )
    .expect("a rule that is already current");
    let rules = rules.to_string_lossy().to_string();

    let mut environment: Vec<(&str, &str)> = vec![
        ("KW_CALLS", calls),
        ("CAELESTIA_UDEV_RULES_DIR", rules.as_str()),
    ];
    environment.extend_from_slice(extra);
    let bundle = fixture.path("bundle");
    fixture.step_in("06-services", &tools.path, &bundle, &environment)
}

// The step's own text is what the tests below drive, so a stub that reports one status and
// logs the arguments it was handed is enough to see what the step did with it. `install`
// says whether the second run the step makes is taken to have worked.
#[cfg(unix)]
fn quick_share_bundle(fixture: &Fixture, status: &str, port: &str, install: bool) -> PathBuf {
    let bundle = fixture.path("bundle");
    let scripts = bundle.join("shell").join("scripts");
    std::fs::create_dir_all(&scripts).expect("the shell scripts directory");
    let refused = if install { "0" } else { "1" };
    stub_in(
        &scripts,
        "quickshare_setup.sh",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$KW_SETUP_CALLS\"\nprintf 'AVAHI=active\\nSETUP={status}\\n'\ncase \"$*\" in\n    *--status*) exit 0 ;;\nesac\nexit {refused}\n"
        ),
    );
    std::fs::write(scripts.join("quickshare-port"), format!("{port}\n")).expect("the port");

    bundle
}

#[cfg(unix)]
fn quick_share_calls(fixture: &Fixture) -> String {
    std::fs::read_to_string(fixture.path("quickshare.calls")).unwrap_or_default()
}

// Escalation has to be reachable from a test that is not root and has no terminal, so the
// sudo to escalate through is named rather than discovered - the same override the shell
// resolver offers.
#[cfg(unix)]
fn sudo_stub(fixture: &Fixture) -> PathBuf {
    stub_in(
        &fixture.path("bin"),
        "sudo",
        "#!/bin/sh\ncase \"$*\" in\n    '-n -v'*) exit 0 ;;\nesac\nshift\nexec \"$@\"\n",
    )
}

#[cfg(unix)]
fn quick_share_environment(
    fixture: &Fixture,
    kind: &str,
    sudo: Option<&str>,
) -> Vec<(&'static str, String)> {
    let calls = fixture
        .path("quickshare.calls")
        .to_string_lossy()
        .to_string();
    let mut environment = vec![
        ("CAELESTIA_INSTALL_KIND", kind.to_string()),
        ("CAELESTIA_FRESH_INSTALL", "0".to_string()),
        ("KW_SETUP_CALLS", calls),
    ];
    if let Some(sudo) = sudo {
        environment.push(("CAELESTIA_SUDO_BIN", sudo.to_string()));
    }
    environment
}

#[cfg(unix)]
fn run_quick_share(fixture: &Fixture, tools: &Services, environment: &[(&str, String)]) -> Output {
    let borrowed: Vec<(&str, &str)> = environment
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    run_services(fixture, tools, &borrowed)
}

#[cfg(unix)]
#[test]
fn the_ydotoold_wrapper_and_unit_are_deployed() {
    let fixture = Fixture::new("services-ydotoold");
    let tools = services_tools(&fixture);

    let output = run_services(
        &fixture,
        &tools,
        &[
            ("CAELESTIA_INSTALL_KIND", "package"),
            ("CAELESTIA_FRESH_INSTALL", "0"),
        ],
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);

    let wrapper = fixture.path(".local/bin/ydotoold-wrapper");
    let body = std::fs::read_to_string(&wrapper).expect("the wrapper");
    assert!(
        body.contains("exec /usr/bin/ydotoold"),
        "the wrapper starts ydotoold: {body}"
    );
    assert!(
        body.contains("--socket-perm=0660"),
        "with the socket permissions the session needs: {body}"
    );

    let unit = std::fs::read_to_string(fixture.path(".config/systemd/user/ydotoold.service"))
        .expect("the unit");
    assert!(
        unit.contains("ExecStart=%h/.local/bin/ydotoold-wrapper"),
        "{unit}"
    );
    assert!(unit.contains("WantedBy=graphical-session.target"), "{unit}");

    assert!(
        output.stdout.contains("ydotoold service configured."),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn a_packaged_install_leaves_the_system_level_alone() {
    let fixture = Fixture::new("services-packaged");
    let tools = services_tools(&fixture);

    let output = run_services(
        &fixture,
        &tools,
        &[
            ("CAELESTIA_INSTALL_KIND", "package"),
            ("CAELESTIA_FRESH_INSTALL", "0"),
        ],
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains("System-level configuration belongs to the package."),
        "{}",
        output.stdout
    );
    assert!(
        !std::fs::read_to_string(fixture.path("rules/70-uinput.rules"))
            .expect("the rule")
            .is_empty(),
        "and the rule the package owns is not rewritten"
    );
}

#[cfg(unix)]
#[test]
fn a_packaged_install_leaves_quick_share_to_the_shell() {
    let fixture = Fixture::new("services-quickshare-package");
    let tools = services_tools(&fixture);
    quick_share_bundle(&fixture, "needed", "1234", true);

    let output = run_quick_share(
        &fixture,
        &tools,
        &quick_share_environment(&fixture, "package", None),
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("packaged install"),
        "{}",
        output.stdout
    );
    assert_eq!(
        quick_share_calls(&fixture),
        "",
        "so nothing is run for Quick Share"
    );
}

#[cfg(unix)]
#[test]
fn a_quick_share_setup_that_is_missing_warns_without_raising_a_password_prompt() {
    let fixture = Fixture::new("services-quickshare-missing");
    let tools = services_tools(&fixture);

    let output = run_quick_share(
        &fixture,
        &tools,
        &quick_share_environment(&fixture, "source", None),
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("setup helper is missing"),
        "{}",
        output.stdout
    );
    assert_eq!(quick_share_calls(&fixture), "");
}

#[cfg(unix)]
#[test]
fn a_bundle_without_the_transfer_port_warns_without_raising_a_password_prompt() {
    let fixture = Fixture::new("services-quickshare-port");
    let tools = services_tools(&fixture);
    let bundle = quick_share_bundle(&fixture, "needed", "1234", true);
    std::fs::remove_file(bundle.join("shell/scripts/quickshare-port")).expect("the port is gone");

    let output = run_quick_share(
        &fixture,
        &tools,
        &quick_share_environment(&fixture, "source", None),
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("transfer port is missing"),
        "{}",
        output.stdout
    );
    assert_eq!(quick_share_calls(&fixture), "");
}

#[cfg(unix)]
#[test]
fn a_transfer_port_that_is_not_a_number_warns_without_raising_a_password_prompt() {
    let fixture = Fixture::new("services-quickshare-bad-port");
    let tools = services_tools(&fixture);
    quick_share_bundle(&fixture, "needed", "not-a-port", true);

    let output = run_quick_share(
        &fixture,
        &tools,
        &quick_share_environment(&fixture, "source", None),
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("is not a port number"),
        "{}",
        output.stdout
    );
    assert_eq!(quick_share_calls(&fixture), "");
}

#[cfg(unix)]
#[test]
fn a_quick_share_that_already_works_is_not_raised_for() {
    let fixture = Fixture::new("services-quickshare-ok");
    let tools = services_tools(&fixture);
    quick_share_bundle(&fixture, "ok", "1234", true);

    let output = run_quick_share(
        &fixture,
        &tools,
        &quick_share_environment(&fixture, "source", None),
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains("Quick Share already has the access it needs."),
        "{}",
        output.stdout
    );

    let calls = quick_share_calls(&fixture);
    assert_eq!(
        calls.lines().count(),
        1,
        "the status check decided, and nothing was run after it: {calls}"
    );
    assert!(
        calls.contains("--status --port 1234"),
        "and it was asked about the port the service binds: {calls}"
    );
}

#[cfg(unix)]
#[test]
fn a_quick_share_that_needs_setup_is_raised_for_with_the_port_the_service_binds() {
    let fixture = Fixture::new("services-quickshare-needed");
    let tools = services_tools(&fixture);
    quick_share_bundle(&fixture, "needed", "1234", true);
    let sudo = sudo_stub(&fixture).to_string_lossy().to_string();

    let output = run_quick_share(
        &fixture,
        &tools,
        &quick_share_environment(&fixture, "source", Some(&sudo)),
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);

    let calls = quick_share_calls(&fixture);
    let runs: Vec<&str> = calls.lines().collect();
    assert_eq!(runs.len(), 2, "the status check and the setup run: {calls}");
    assert_eq!(
        runs[1], "--port 1234",
        "the run that is elevated carries the port the service listens on"
    );
    assert!(
        output.stdout.contains("Quick Share can receive"),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn a_quick_share_setup_that_fails_warns_instead_of_failing_the_install() {
    let fixture = Fixture::new("services-quickshare-failed");
    let tools = services_tools(&fixture);
    quick_share_bundle(&fixture, "needed", "1234", false);
    let sudo = sudo_stub(&fixture).to_string_lossy().to_string();

    let output = run_quick_share(
        &fixture,
        &tools,
        &quick_share_environment(&fixture, "source", Some(&sudo)),
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("did not finish"),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
fn dots_tools(fixture: &Fixture) -> ConfigTools {
    let bin = fixture.path("bin");
    let calls = fixture.path("calls.log");
    std::fs::write(&calls, "").expect("the call log");

    // `cksum` stands in for sha256sum: it is POSIX, and its digest is over the bytes, so
    // it changes exactly when the file does.
    stub_in(&bin, "sha256sum", "#!/bin/sh\nexec cksum \"$@\"\n");
    stub_in(&bin, "id", "#!/bin/sh\nprintf 'tester\\n'\n");
    stub_in(
        &bin,
        "getent",
        "#!/bin/sh\nprintf 'tester:x:1000:1000::/home/tester:/bin/zsh\\n'\n",
    );
    stub_in(&bin, "date", "#!/bin/sh\nprintf '20240101_120000\\n'\n");
    stub_in(
        &bin,
        "update-desktop-database",
        "#!/bin/sh\nprintf 'update-desktop-database %s\\n' \"$*\" >> \"$KW_CALLS\"\nexit 0\n",
    );

    let system = std::env::var("PATH").unwrap_or_default();
    ConfigTools {
        path: PathBuf::from(format!("{}:{system}", bin.display())),
        calls,
    }
}

#[cfg(unix)]
fn dots_bundle(fixture: &Fixture) -> PathBuf {
    let bundle = fixture.path("bundle");
    let dots = bundle.join("src/dots");
    for (directory, file, contents) in [
        ("btop", "btop.conf", "theme = \"caelestia\"\n"),
        ("foot", "foot.ini", "[main]\n"),
        ("kitty", "kitty.conf", "font_size 11\n"),
    ] {
        std::fs::create_dir_all(dots.join(directory)).expect("a config tree");
        std::fs::write(dots.join(directory).join(file), contents).expect("a config file");
    }
    std::fs::write(dots.join("starship.toml"), "format = \"$all\"\n").expect("the prompt");

    let fish = bundle.join("src/dots-extra/fish");
    std::fs::create_dir_all(&fish).expect("an extra config tree");
    std::fs::write(fish.join("config.fish"), "set -gx EDITOR nvim\n").expect("a config file");
    let fastfetch = bundle.join("src/dots-extra/fastfetch");
    std::fs::create_dir_all(&fastfetch).expect("an extra config tree");
    std::fs::write(fastfetch.join("config.jsonc"), "{}\n").expect("a config file");

    let bin = bundle.join("src/bin");
    std::fs::create_dir_all(&bin).expect("a bridge directory");
    std::fs::write(bin.join("caelestia-color"), "#!/bin/sh\n").expect("a bridge file");
    std::fs::write(bin.join("helper.cpp"), "// c++\n").expect("a source file");
    std::fs::write(bin.join("CMakeLists.txt"), "add_executable(x)\n").expect("a build file");

    bundle
}

#[cfg(unix)]
fn run_configs(fixture: &Fixture, tools: &ConfigTools, extra: &[(&str, &str)]) -> Output {
    let calls = tools.calls.to_str().expect("a path");
    let mut environment: Vec<(&str, &str)> = vec![("KW_CALLS", calls)];
    environment.extend_from_slice(extra);
    let bundle = fixture.path("bundle");
    fixture.step_in("03-deploy-configs", &tools.path, &bundle, &environment)
}

#[cfg(unix)]
#[test]
fn a_source_install_deploys_the_configs_the_prompt_and_the_bridge_files() {
    let fixture = Fixture::new("configs-source");
    let tools = dots_tools(&fixture);
    dots_bundle(&fixture);
    std::fs::write(fixture.path(".zshrc"), b"export EDITOR=nvim\n").expect("the shell rc");

    let backup = fixture.path("backup").to_string_lossy().to_string();
    let output = run_configs(
        &fixture,
        &tools,
        &[
            ("CAELESTIA_INSTALL_KIND", "source"),
            ("BACKUP_DIR", backup.as_str()),
        ],
    );
    assert_eq!(output.code, Some(0), "{}", output.stderr);

    assert_eq!(
        std::fs::read_to_string(fixture.path(".config/btop/btop.conf")).expect("the config"),
        "theme = \"caelestia\"\n"
    );
    assert!(fixture.path(".config/kitty/kitty.conf").is_file());
    assert_eq!(
        std::fs::read_to_string(fixture.path(".config/starship.toml")).expect("the prompt"),
        "format = \"$all\"\n"
    );

    assert!(fixture.path(".local/bin/caelestia-color").is_file());
    assert!(
        !fixture.path(".local/bin/helper.cpp").exists(),
        "a compiled source file is not a bridge file"
    );
    assert!(
        !fixture.path(".local/bin/CMakeLists.txt").exists(),
        "and neither is a build file"
    );

    let shell = std::fs::read_to_string(fixture.path("backup/previous_shell.txt"));
    assert_eq!(shell.expect("the recorded shell"), "/bin/zsh\n");

    let states =
        std::fs::read_to_string(fixture.path("backup/shellrc/zshrc.state")).expect("the zsh state");
    assert_eq!(states, "present\n");
    assert_eq!(
        std::fs::read_to_string(fixture.path("backup/shellrc/fish_config.state"))
            .expect("the fish state"),
        "missing\n"
    );

    for name in ["btop", "kitty", "starship.toml"] {
        let stamp = fixture
            .path("cache/caelestia-kde/deployed")
            .join(format!("{name}.sha256"));
        assert!(stamp.is_file(), "{} is recorded", stamp.display());
    }

    assert!(
        output.stdout.contains("Config deployment complete."),
        "{}",
        output.stdout
    );
}

#[cfg(unix)]
#[test]
fn a_config_the_user_edited_is_preserved_while_the_others_are_replaced() {
    let fixture = Fixture::new("configs-modified");
    let tools = dots_tools(&fixture);
    let bundle = dots_bundle(&fixture);
    let backup = fixture.path("backup").to_string_lossy().to_string();
    let environment = [
        ("CAELESTIA_INSTALL_KIND", "source"),
        ("BACKUP_DIR", backup.as_str()),
    ];

    let first = run_configs(&fixture, &tools, &environment);
    assert_eq!(first.code, Some(0), "{}", first.stderr);

    std::fs::write(
        fixture.path(".config/btop/btop.conf"),
        "theme = \"material\"\n",
    )
    .expect("the user's own edit");
    std::fs::write(
        bundle.join("src/dots/foot/foot.ini"),
        "[main]\nfont=monospace\n",
    )
    .expect("the new shipped config");

    let second = run_configs(&fixture, &tools, &environment);
    assert_eq!(second.code, Some(0), "{}", second.stderr);

    assert_eq!(
        std::fs::read_to_string(fixture.path(".config/btop/btop.conf")).expect("the config"),
        "theme = \"material\"\n",
        "an edited config is the user's"
    );
    assert!(
        second
            .stdout
            .contains("Preserving locally modified config: btop"),
        "{}",
        second.stdout
    );
    assert_eq!(
        std::fs::read_to_string(fixture.path(".config/foot/foot.ini")).expect("the config"),
        "[main]\nfont=monospace\n",
        "an untouched config is updated"
    );
}

#[cfg(unix)]
#[test]
fn the_configs_the_user_had_are_backed_up_before_they_are_replaced() {
    let fixture = Fixture::new("configs-backup");
    let tools = dots_tools(&fixture);
    dots_bundle(&fixture);
    let backup = fixture.path("backup");
    let backup_dir = backup.to_string_lossy().to_string();
    let environment = [
        ("CAELESTIA_INSTALL_KIND", "source"),
        ("BACKUP_DIR", backup_dir.as_str()),
    ];

    std::fs::create_dir_all(fixture.path(".config/btop")).expect("the user's config tree");
    std::fs::write(fixture.path(".config/btop/btop.conf"), b"theirs\n").expect("a config file");
    std::fs::write(fixture.path(".config/konsolerc"), b"[Desktop Entry]\n").expect("the konsolerc");
    std::fs::create_dir_all(fixture.path(".local/share/konsole")).expect("the konsole tree");
    std::fs::write(
        fixture.path(".local/share/konsole/Caelestia.profile"),
        b"[General]\n",
    )
    .expect("a profile");

    let output = run_configs(&fixture, &tools, &environment);
    assert_eq!(output.code, Some(0), "{}", output.stderr);

    assert_eq!(
        std::fs::read_to_string(backup.join(".config/btop/btop.conf")).expect("the backup"),
        "theirs\n"
    );
    assert!(backup.join(".config/konsolerc").is_file());
    assert!(backup.join("local/konsole/Caelestia.profile").is_file());
    assert_eq!(
        std::fs::read_to_string(fixture.path(".config/btop/btop.conf")).expect("the config"),
        "theme = \"caelestia\"\n"
    );
}

#[cfg(unix)]
#[test]
fn thunar_and_fish_follow_the_answers_they_were_given() {
    let fixture = Fixture::new("configs-choices");
    let tools = dots_tools(&fixture);
    let bundle = dots_bundle(&fixture);
    let thunar = bundle.join("src/dots/thunar");
    std::fs::create_dir_all(&thunar).expect("the thunar tree");
    std::fs::write(thunar.join("uca.xml"), "<actions/>\n").expect("an action file");
    let backup = fixture.path("backup");
    let backup_dir = backup.to_string_lossy().to_string();

    let off = run_configs(
        &fixture,
        &tools,
        &[
            ("CAELESTIA_INSTALL_KIND", "source"),
            ("BACKUP_DIR", backup_dir.as_str()),
            ("INSTALL_THUNAR", "false"),
            ("INSTALL_FISH", "false"),
        ],
    );
    assert_eq!(off.code, Some(0), "{}", off.stderr);
    assert!(!fixture.path(".config/thunar").exists());
    assert!(!fixture.path(".config/fish").exists());
    assert!(
        fixture.path(".config/fastfetch/config.jsonc").is_file(),
        "the other extra config is not the one that was declined"
    );

    let on = run_configs(
        &fixture,
        &tools,
        &[
            ("CAELESTIA_INSTALL_KIND", "source"),
            ("BACKUP_DIR", backup_dir.as_str()),
            ("INSTALL_THUNAR", "true"),
            ("INSTALL_FISH", "true"),
        ],
    );
    assert_eq!(on.code, Some(0), "{}", on.stderr);
    assert_eq!(
        std::fs::read_to_string(fixture.path(".config/thunar/uca.xml")).expect("the action file"),
        "<actions/>\n"
    );
    assert!(fixture.path(".config/fish/config.fish").is_file());
    assert!(
        on.stdout
            .contains("Missing optional Thunar file: thunar/thunar-volman.xml"),
        "{}",
        on.stdout
    );
}

#[cfg(unix)]
#[test]
fn a_missing_dots_tree_names_the_step_that_fills_it() {
    let fixture = Fixture::new("configs-empty-dots");
    let tools = dots_tools(&fixture);
    let bundle = fixture.path("bundle");
    std::fs::create_dir_all(bundle.join("src/dots")).expect("an empty dots tree");
    let backup = fixture.path("backup");
    let backup_dir = backup.to_string_lossy().to_string();
    let environment = [
        ("CAELESTIA_INSTALL_KIND", "source"),
        ("BACKUP_DIR", backup_dir.as_str()),
    ];

    let output = run_configs(&fixture, &tools, &environment);
    assert_eq!(output.code, Some(1));
    assert!(
        output.stderr.contains("scripts/02a-submodules.sh"),
        "{}",
        output.stderr
    );
}

#[cfg(unix)]
#[test]
fn a_packaged_install_without_dotfiles_says_the_package_owes_them() {
    let fixture = Fixture::new("configs-packaged");
    let tools = dots_tools(&fixture);
    let bundle = fixture.path("bundle");
    std::fs::create_dir_all(bundle.join("src/dots")).expect("an empty dots tree");
    let backup = fixture.path("backup");
    let backup_dir = backup.to_string_lossy().to_string();
    let environment = [
        ("CAELESTIA_INSTALL_KIND", "package"),
        ("BACKUP_DIR", backup_dir.as_str()),
    ];

    let output = run_configs(&fixture, &tools, &environment);
    assert_eq!(output.code, Some(1));
    assert!(output.stderr.contains("reinstall it"), "{}", output.stderr);
}

#[cfg(unix)]
#[test]
fn the_backup_directory_a_previous_step_recorded_is_the_one_that_is_used() {
    let fixture = Fixture::new("configs-recorded");
    let tools = dots_tools(&fixture);
    dots_bundle(&fixture);

    let recorded = fixture.path("bundle/backups/20231231_235959");
    std::fs::create_dir_all(&recorded).expect("the recorded backup");
    std::fs::create_dir_all(fixture.path("cache/caelestia-kde")).expect("the cache directory");
    std::fs::write(
        fixture.path("cache/caelestia-kde/backup-dir.txt"),
        format!("{}\n", recorded.display()),
    )
    .expect("the record");

    let output = run_configs(&fixture, &tools, &[("CAELESTIA_INSTALL_KIND", "source")]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    assert!(
        recorded.join("previous_shell.txt").is_file(),
        "the recorded backup is where this run puts its files"
    );
}

#[cfg(unix)]
#[test]
fn a_recorded_backup_directory_outside_the_two_roots_is_ignored() {
    let fixture = Fixture::new("configs-stale-record");
    let tools = dots_tools(&fixture);
    dots_bundle(&fixture);

    let elsewhere = fixture.path("elsewhere/20230101_000000");
    std::fs::create_dir_all(&elsewhere).expect("a directory that is not a backup");
    std::fs::create_dir_all(fixture.path("cache/caelestia-kde")).expect("the cache directory");
    std::fs::write(
        fixture.path("cache/caelestia-kde/backup-dir.txt"),
        format!("{}\n", elsewhere.display()),
    )
    .expect("the record");

    let output = run_configs(&fixture, &tools, &[("CAELESTIA_INSTALL_KIND", "source")]);
    assert_eq!(output.code, Some(0), "{}", output.stderr);

    let fresh = fixture.path("bundle/backups/20240101_120000");
    assert!(
        fresh.join("previous_shell.txt").is_file(),
        "a fresh backup is made beside the bundle instead"
    );
    assert!(
        std::fs::read_dir(&elsewhere)
            .expect("the directory")
            .next()
            .is_none(),
        "and the directory the record named is left alone"
    );
}

#[cfg(unix)]
#[test]
fn the_prompt_the_user_edited_is_left_alone_and_backed_up() {
    let fixture = Fixture::new("configs-starship");
    let tools = dots_tools(&fixture);
    let bundle = dots_bundle(&fixture);
    let backup = fixture.path("backup");
    let backup_dir = backup.to_string_lossy().to_string();
    let environment = [
        ("CAELESTIA_INSTALL_KIND", "source"),
        ("BACKUP_DIR", backup_dir.as_str()),
    ];

    let first = run_configs(&fixture, &tools, &environment);
    assert_eq!(first.code, Some(0), "{}", first.stderr);

    std::fs::write(
        fixture.path(".config/starship.toml"),
        "format = \"$directory\"\n",
    )
    .expect("the user's own prompt");
    std::fs::write(
        bundle.join("src/dots/starship.toml"),
        "format = \"$all$time\"\n",
    )
    .expect("the new shipped prompt");

    let second = run_configs(&fixture, &tools, &environment);
    assert_eq!(second.code, Some(0), "{}", second.stderr);
    assert_eq!(
        std::fs::read_to_string(fixture.path(".config/starship.toml")).expect("the prompt"),
        "format = \"$directory\"\n"
    );
    assert_eq!(
        std::fs::read_to_string(backup.join(".config/starship.toml")).expect("the backup"),
        "format = \"$directory\"\n"
    );
    assert!(
        second
            .stdout
            .contains("Preserving locally modified config: starship.toml"),
        "{}",
        second.stdout
    );
}
