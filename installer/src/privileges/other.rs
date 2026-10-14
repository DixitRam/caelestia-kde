//! The installer only ever runs on Linux. This keeps the crate buildable
//! elsewhere so that everything that *is* portable — the CLI, the data, the
//! screens, the runner — can still be developed and tested on any host.

use std::path::Path;

pub struct Credential;

impl Credential {
    pub fn install(_secret: &str, _exe: &Path) -> Result<Self, String> {
        Err("privilege escalation needs Linux".to_string())
    }

    pub fn environment(&self) -> Vec<(&'static str, String)> {
        Vec::new()
    }

    pub fn verify(&self) -> bool {
        false
    }
}

pub struct Inhibitor;

impl Inhibitor {
    pub fn acquire() -> Self {
        Inhibitor
    }
}

pub fn read_secret(_pipe: &Path) -> Result<String, String> {
    Err("the askpass helper needs Linux".to_string())
}

pub fn is_root() -> bool {
    false
}

pub fn sudo_cached() -> bool {
    false
}
