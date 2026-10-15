//! Termination signals, so a `kill` leaves the terminal as it found it.
//!
//! Ctrl+C does not need this: the terminal is in raw mode while the wizard runs,
//! so it arrives as a key event instead. These are the signals that can still
//! arrive from outside.

/// Set by the handler, read by the render loop. An atomic store is all a signal
/// handler is allowed to do, and all this needs.
#[cfg(unix)]
static TERMINATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn on_signal(_signal: libc::c_int) {
    TERMINATED.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(unix)]
pub fn install() {
    // Bind the function to a pointer first: a function *item* cannot be cast to
    // an integer directly, and `signal` wants the address.
    let handler: extern "C" fn(libc::c_int) = on_signal;
    unsafe {
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT] {
            libc::signal(signal, handler as libc::sighandler_t);
        }
    }
}

#[cfg(unix)]
pub fn terminated() -> bool {
    TERMINATED.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(not(unix))]
pub fn install() {}

#[cfg(not(unix))]
pub fn terminated() -> bool {
    false
}
