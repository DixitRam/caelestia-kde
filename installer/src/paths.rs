use std::path::PathBuf;

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn cache_dir() -> PathBuf {
    option("XDG_CACHE_HOME").unwrap_or_else(|| home().join(".cache"))
}

pub fn state_dir() -> PathBuf {
    option("XDG_STATE_HOME").unwrap_or_else(|| home().join(".local").join("state"))
}

fn option(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}
