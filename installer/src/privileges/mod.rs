//! Privilege escalation: the installer supplies a credential, the shell library
//! owns the policy.
//!
//! `scripts/lib/privileges.sh` already decides *how* to elevate — it picks
//! between root, a cached credential, an askpass helper, a terminal and pkexec,
//! and it is used by the step scripts and by `update.sh`/`uninstall.sh` alike.
//! The one thing it cannot do is ask the user for a password on a terminal the
//! wizard is holding in raw mode. That is this module's whole job.
//!
//! The password is never written to disk. A named pipe carries it from the
//! wizard to a helper that `sudo -A` runs on demand, and the helper is told
//! where that pipe is by argument rather than by environment, because `sudo`
//! resets the environment before it runs an askpass program.
//!
//! The installer is Linux-only; the portable build exists so the rest of the
//! crate can be developed and tested on any host.

#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub use unix::*;

#[cfg(not(unix))]
mod other;

#[cfg(not(unix))]
pub use other::*;
