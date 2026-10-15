use std::path::{Path, PathBuf};

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Option<Self> {
        let base = std::env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        for attempt in 0..64u32 {
            let candidate = base.join(format!("{prefix}.{}.{attempt}", std::process::id()));
            match std::fs::create_dir(&candidate) {
                Ok(()) => return Some(Self { path: candidate }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return None,
            }
        }
        None
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::TempDir;

    #[test]
    fn a_temporary_directory_is_created_and_removed_again() {
        let path = {
            let temporary = TempDir::new("caelestia-test").expect("a temporary directory");
            assert!(temporary.path().is_dir());
            temporary.path().to_path_buf()
        };
        assert!(!path.exists(), "the directory should be gone");
    }

    #[test]
    fn two_temporary_directories_do_not_collide() {
        let first = TempDir::new("caelestia-test").expect("a temporary directory");
        let second = TempDir::new("caelestia-test").expect("a temporary directory");
        assert_ne!(first.path(), second.path());
    }
}
