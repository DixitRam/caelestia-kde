//! Ending the session, for the paths that finish by asking the reader to log out.
//!
//! Asking the desktop to do it is the same call its own menu makes, so anything that is open gets
//! the warning it would normally get rather than being killed from under the person using it.

use crate::process;

/// Plasma 6 answers to `org.kde.Shutdown` on the session bus. A session that predates it, or a
/// machine where the Qt 6 tools are named differently, still has the same call under `qdbus`.
const ASK: [&str; 3] = ["org.kde.Shutdown", "/Shutdown", "org.kde.Shutdown.logout"];

pub fn now() -> Result<(), String> {
    let asked = ["qdbus6", "qdbus"]
        .into_iter()
        .filter(|program| process::available(program))
        .any(|program| process::succeeds(program, &ASK, None));
    if asked {
        Ok(())
    } else {
        Err(
            "Could not ask the session to log out. Log out from the desktop's own menu."
                .to_string(),
        )
    }
}
