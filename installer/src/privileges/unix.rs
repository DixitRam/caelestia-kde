use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

const HELPER: &str = "askpass.sh";
const PIPE: &str = "askpass.fifo";
const WRAPPER: &str = "sudo";

pub struct Credential {
    dir: PathBuf,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Credential {
    pub fn install(secret: &str, exe: &Path) -> Result<Self, String> {
        let dir = private_dir()?;
        let pipe = dir.join(PIPE);
        let pipe_c = std::ffi::CString::new(pipe.as_os_str().as_encoded_bytes())
            .map_err(|_| "the pipe path is not usable".to_string())?;
        if unsafe { libc::mkfifo(pipe_c.as_ptr(), 0o600) } != 0 {
            return Err(format!(
                "cannot create {}: {}",
                pipe.display(),
                std::io::Error::last_os_error()
            ));
        }

        let helper = dir.join(HELPER);
        write_private(
            &helper,
            &format!(
                "#!/bin/bash\nexec {} --askpass {}\n",
                shell_quote(exe),
                shell_quote(&pipe)
            ),
        )?;

        let wrapper = dir.join(WRAPPER);
        write_private(
            &wrapper,
            &format!(
                "#!/bin/bash\nexport SUDO_ASKPASS={}\nexec /usr/bin/sudo -A \"$@\"\n",
                shell_quote(&helper)
            ),
        )?;

        let stop = Arc::new(AtomicBool::new(false));
        let worker = serve(pipe, secret.to_string(), Arc::clone(&stop));

        Ok(Self {
            dir,
            stop,
            worker: Some(worker),
        })
    }

    pub fn environment(&self) -> Vec<(&'static str, String)> {
        let path = match std::env::var("PATH") {
            Ok(existing) => format!("{}:{existing}", self.dir.display()),
            Err(_) => self.dir.display().to_string(),
        };
        vec![
            ("SUDO_ASKPASS", self.dir.join(HELPER).display().to_string()),
            ("PATH", path),
        ]
    }

    pub fn verify(&self) -> bool {
        let Some(sudo) = sudo_path() else {
            return true;
        };
        std::process::Command::new(sudo)
            .arg("-A")
            .arg("-v")
            .envs(self.environment())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

impl Drop for Credential {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

pub fn sudo_path() -> Option<PathBuf> {
    ["/usr/bin/sudo", "/bin/sudo"]
        .iter()
        .map(PathBuf::from)
        .find(|candidate| candidate.is_file())
}

pub fn is_root() -> bool {
    unsafe { libc::getuid() == 0 }
}

pub fn sudo_cached() -> bool {
    let Some(sudo) = sudo_path() else {
        return false;
    };
    std::process::Command::new(sudo)
        .arg("-n")
        .arg("-v")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn read_secret(pipe: &Path) -> Result<String, String> {
    let file =
        File::open(pipe).map_err(|error| format!("cannot read {}: {error}", pipe.display()))?;
    let mut secret = String::new();
    BufReader::new(file)
        .read_line(&mut secret)
        .map_err(|error| format!("cannot read {}: {error}", pipe.display()))?;
    Ok(secret.trim_end_matches('\n').to_string())
}

fn serve(pipe: PathBuf, secret: String, stop: Arc<AtomicBool>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            let Ok(mut writer) = OpenOptions::new().write(true).open(&pipe) else {
                break;
            };
            let _ = writer.write_all(secret.as_bytes());
            let _ = writer.write_all(b"\n");
        }
    })
}

fn private_dir() -> Result<PathBuf, String> {
    let uid = unsafe { libc::getuid() };
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        let runtime = PathBuf::from(runtime);
        if owned_by(&runtime, uid) {
            roots.push(runtime);
        }
    }
    roots.push(std::env::temp_dir());

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    for root in roots {
        let dir = root.join(format!("caelestia-install-{}-{unique}", std::process::id()));
        match fs::create_dir(&dir) {
            Ok(()) => {
                fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
                    .map_err(|error| format!("cannot protect {}: {error}", dir.display()))?;
                return Ok(dir);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("cannot create {}: {error}", dir.display())),
        }
    }
    Err("nowhere private to put the askpass helper".to_string())
}

fn owned_by(path: &Path, uid: u32) -> bool {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(path)
        .map(|meta| meta.is_dir() && meta.uid() == uid)
        .unwrap_or(false)
}

fn write_private(path: &Path, body: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    file.write_all(body.as_bytes())
        .map_err(|error| error.to_string())?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("cannot protect {}: {error}", path.display()))
}

fn shell_quote(value: &Path) -> String {
    let text = value.to_string_lossy();
    format!("'{}'", text.replace('\'', "'\\''"))
}

pub struct Inhibitor {
    pid: Option<libc::pid_t>,
    cookie: Option<String>,
}

impl Inhibitor {
    pub fn acquire() -> Self {
        let state = state_dir();
        let pid_file = state.join("inhibit.pid");
        let cookie_file = state.join("kde_inhibit.cookie");

        reap_previous(&pid_file);
        let _ = fs::remove_file(&cookie_file);

        let pid = spawn_inhibitor();
        if let Some(pid) = pid {
            let _ = fs::write(&pid_file, format!("{pid}\n"));
        }
        let cookie = inhibit_screensaver(&cookie_file);
        Self { pid, cookie }
    }
}

impl Drop for Inhibitor {
    fn drop(&mut self) {
        if let Some(pid) = self.pid {
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        if let Some(cookie) = self.cookie.take() {
            let _ = std::process::Command::new("qdbus6")
                .args([
                    "org.freedesktop.ScreenSaver",
                    "/ScreenSaver",
                    "org.freedesktop.ScreenSaver.UnInhibit",
                    &cookie,
                ])
                .status();
        }
        let state = state_dir();
        let _ = fs::remove_file(state.join("inhibit.pid"));
        let _ = fs::remove_file(state.join("kde_inhibit.cookie"));
    }
}

fn state_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .or_else(|| std::env::var_os("XDG_STATE_HOME"))
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state").into())
        })
        .unwrap_or_else(|| std::env::temp_dir().into());
    let dir = PathBuf::from(base).join("caelestia");
    let _ = fs::create_dir_all(&dir);
    let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    dir
}

fn reap_previous(pid_file: &Path) {
    let Ok(contents) = fs::read_to_string(pid_file) else {
        return;
    };
    let Ok(pid) = contents.trim().parse::<libc::pid_t>() else {
        return;
    };
    if pid <= 0 || pid == unsafe { libc::getpid() } || pid == unsafe { libc::getppid() } {
        return;
    }
    let Ok(cmdline) = fs::read_to_string(format!("/proc/{pid}/cmdline")) else {
        return;
    };
    if cmdline.contains("systemd-inhibit") {
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
}

fn spawn_inhibitor() -> Option<libc::pid_t> {
    std::process::Command::new("systemd-inhibit")
        .args([
            "--what=idle:sleep",
            "--who=Caelestia installer",
            "--why=Installation in progress",
            "bash",
            "-c",
            "while :; do sleep 600; done",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
        .map(|child| child.id() as libc::pid_t)
}

fn inhibit_screensaver(cookie_file: &Path) -> Option<String> {
    let output = std::process::Command::new("qdbus6")
        .args([
            "org.freedesktop.ScreenSaver",
            "/ScreenSaver",
            "org.freedesktop.ScreenSaver.Inhibit",
            "Caelestia installer",
            "Installation in progress",
        ])
        .output()
        .ok()?;
    let cookie = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if cookie.is_empty() {
        return None;
    }
    let _ = fs::write(cookie_file, &cookie);
    Some(cookie)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_quoted_so_a_generated_script_cannot_be_escaped_out_of() {
        assert_eq!(shell_quote(Path::new("/tmp/plain")), "'/tmp/plain'");
        assert_eq!(shell_quote(Path::new("/tmp/it's")), "'/tmp/it'\\''s'");
    }

    #[test]
    fn the_credential_publishes_a_helper_and_never_writes_the_password() {
        let exe = std::env::current_exe().expect("the test binary has a path");
        let credential = Credential::install("hunter2", &exe).expect("a credential");

        let helper = credential.dir.join(HELPER);
        let wrapper = credential.dir.join(WRAPPER);
        let pipe = credential.dir.join(PIPE);
        assert!(helper.is_file(), "the helper is published");
        assert!(wrapper.is_file(), "the sudo shim is published");
        assert!(pipe.exists(), "the pipe exists");

        for entry in fs::read_dir(&credential.dir).expect("the shim directory") {
            let path = entry.expect("an entry").path();
            if path == pipe {
                continue;
            }
            let body = fs::read_to_string(&path).unwrap_or_default();
            assert!(
                !body.contains("hunter2"),
                "{} holds the password",
                path.display()
            );
        }

        for path in [&helper, &wrapper] {
            let mode = fs::metadata(path).expect("metadata").permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{}", path.display());
        }

        let reader = std::thread::spawn(move || read_secret(&pipe));
        let served = reader
            .join()
            .expect("the reader thread finishes")
            .expect("the pipe is served");
        assert_eq!(served, "hunter2");
    }

    #[test]
    fn a_credential_cleans_up_after_itself() {
        let exe = std::env::current_exe().expect("the test binary has a path");
        let dir = {
            let credential = Credential::install("secret", &exe).expect("a credential");
            let dir = credential.dir.clone();
            assert!(dir.is_dir());
            dir
        };
        assert!(
            !dir.exists(),
            "the shim directory is removed when the credential drops"
        );
    }

    #[test]
    fn the_environment_points_the_scripts_at_the_helper() {
        let exe = std::env::current_exe().expect("the test binary has a path");
        let credential = Credential::install("secret", &exe).expect("a credential");
        let env = credential.environment();
        let askpass = env
            .iter()
            .find(|(key, _)| *key == "SUDO_ASKPASS")
            .expect("SUDO_ASKPASS");
        assert!(askpass.1.ends_with(HELPER), "{}", askpass.1);
        let path = env.iter().find(|(key, _)| *key == "PATH").expect("PATH");
        assert!(
            path.1.starts_with(&credential.dir.display().to_string()),
            "{}",
            path.1
        );
    }
}
