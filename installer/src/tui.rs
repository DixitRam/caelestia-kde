//! The screen the wizard draws on, and the one thing it must not hand to anyone else: the
//! process's own standard output.
//!
//! The wizard draws on the terminal, and everything it starts inherits that terminal as its
//! output. A program a step leaves behind - a browser, a Qt tool that warns about the locale -
//! then writes wherever the cursor happened to be: over the frame, and, five lines later, far
//! enough down the screen to scroll it. The wizard's own record of the screen still believes those
//! cells say what it drew, so a drawing pass that only writes what changed never repairs them.
//!
//! So the wizard takes a handle on the terminal for itself, and hands the process's own output to
//! the install log. The drawing goes on through the handle; a stray warning from a stray program
//! lands in the log, which is where a warning belongs; and the terminal is given back before
//! anything is printed on it again.

#[cfg(unix)]
use std::fs::{File, OpenOptions};
use std::io::{self, Stdout, Write};
use std::path::Path;
#[cfg(unix)]
use std::sync::atomic::{AtomicI32, Ordering};

use ratatui::Frame;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

/// The wizard's own handle on the terminal, which stays the terminal whatever the process's own
/// output is pointed at.
pub struct Screen(Handle);

enum Handle {
    /// A descriptor of the wizard's own, taken before the output was handed over.
    #[cfg(unix)]
    Terminal(File),
    Stdout(Stdout),
}

impl Screen {
    fn take() -> Self {
        #[cfg(unix)]
        {
            use std::os::fd::FromRawFd;

            let handle = unsafe { libc::dup(libc::STDOUT_FILENO) };
            if handle >= 0 {
                return Self(Handle::Terminal(unsafe { File::from_raw_fd(handle) }));
            }
        }
        Self(Handle::Stdout(io::stdout()))
    }
}

impl Write for Screen {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        match &mut self.0 {
            #[cfg(unix)]
            Handle::Terminal(file) => file.write(buffer),
            Handle::Stdout(stdout) => stdout.write(buffer),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.0 {
            #[cfg(unix)]
            Handle::Terminal(file) => file.flush(),
            Handle::Stdout(stdout) => stdout.flush(),
        }
    }
}

/// The descriptors this process had before it handed them over, so they can be handed back. Held
/// as atomics rather than on the `Tui` because a panic has to hand them back from a hook, which
/// cannot reach it.
#[cfg(unix)]
static LISTENING_OUT: AtomicI32 = AtomicI32::new(-1);
#[cfg(unix)]
static LISTENING_ERR: AtomicI32 = AtomicI32::new(-1);

/// Point this process's own output at the log.
#[cfg(unix)]
fn hand_over(log: &Path) {
    use std::os::fd::AsRawFd;

    // A log that cannot be opened is not worth refusing to start over: the terminal is the worse
    // place for stray output, but a wizard that will not run is worse than either.
    let Ok(sink) = OpenOptions::new().create(true).append(true).open(log) else {
        return;
    };
    // Nothing to hand over if this has already happened, and nothing to give back afterwards.
    if LISTENING_OUT.load(Ordering::SeqCst) >= 0 {
        return;
    }
    let out = unsafe { libc::dup(libc::STDOUT_FILENO) };
    let err = unsafe { libc::dup(libc::STDERR_FILENO) };
    if out < 0 || err < 0 {
        return;
    }
    LISTENING_OUT.store(out, Ordering::SeqCst);
    LISTENING_ERR.store(err, Ordering::SeqCst);
    unsafe {
        libc::dup2(sink.as_raw_fd(), libc::STDOUT_FILENO);
        libc::dup2(sink.as_raw_fd(), libc::STDERR_FILENO);
    }
}

/// Give this process its own output back. Safe when it was never handed over, and safe to call
/// twice, which is what lets the panic hook below use it.
#[cfg(unix)]
fn give_back() {
    let out = LISTENING_OUT.swap(-1, Ordering::SeqCst);
    let err = LISTENING_ERR.swap(-1, Ordering::SeqCst);
    if out >= 0 {
        unsafe {
            libc::dup2(out, libc::STDOUT_FILENO);
            libc::close(out);
        }
    }
    if err >= 0 {
        unsafe {
            libc::dup2(err, libc::STDERR_FILENO);
            libc::close(err);
        }
    }
}

#[cfg(not(unix))]
fn hand_over(_log: &Path) {}

#[cfg(not(unix))]
fn give_back() {}

/// A panic has to leave the alternate screen through the terminal rather than through the log, and
/// nothing else is going to do it: a restore that writes to the process's own output writes to the
/// log while the wizard is drawing.
fn keep_the_terminal() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        give_back();
        let _ = disable_raw_mode();
        let mut terminal = io::stdout();
        let _ = terminal.write_all(b"\x1b[?25h\x1b[?1049l");
        let _ = terminal.flush();
        previous(info);
    }));
}

/// The wizard's terminal: what it draws on, and what it owes back when it is done.
pub struct Tui {
    terminal: ratatui::Terminal<CrosstermBackend<Screen>>,
}

impl Tui {
    /// Take the terminal, and point the process's own output at `log`.
    pub fn start(log: &Path) -> io::Result<Self> {
        let mut screen = Screen::take();
        enable_raw_mode()?;
        execute!(screen, EnterAlternateScreen)?;
        let terminal = ratatui::Terminal::new(CrosstermBackend::new(screen))?;
        hand_over(log);
        keep_the_terminal();
        Ok(Self { terminal })
    }

    pub fn draw<F>(&mut self, render: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame),
    {
        self.terminal.draw(render).map(|_| ())
    }

    /// Forget what is on the screen and paint the next frame whole. Whatever was written over the
    /// frame left cells the wizard's record still believes it drew, and a diffing pass would never
    /// touch them again.
    pub fn repaint(&mut self) {
        let _ = self.terminal.clear();
    }

    /// Leave the screen clean, and give the terminal back to the process's own output.
    pub fn finish(mut self) {
        let _ = self.terminal.show_cursor();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = disable_raw_mode();
        give_back();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::io::Write;
    use std::os::fd::FromRawFd;
    use std::sync::{Mutex, OnceLock};

    use super::*;

    /// The handing over is process-wide, so two tests doing it at once would each be checking the
    /// other's log. One at a time, and in the order the story reads.
    fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    /// What a program the wizard started does with the output it was given.
    fn write_to_inherited_output(line: &str) -> io::Result<()> {
        let mut inherited = unsafe { File::from_raw_fd(libc::dup(libc::STDOUT_FILENO)) };
        inherited.write_all(line.as_bytes())?;
        inherited.flush()
    }

    #[test]
    fn what_a_step_started_writes_lands_in_the_log_rather_than_on_the_screen() {
        let _turn = one_at_a_time();
        let directory = crate::tmp::TempDir::new("caelestia-tui-test").expect("a temporary dir");
        let log = directory.path().join("install.log");

        hand_over(&log);
        let written = write_to_inherited_output("Detected locale \"C\"\n");
        give_back();

        assert!(written.is_ok(), "the inherited output is writable");
        let logged = std::fs::read_to_string(&log).expect("the log was written");
        assert!(logged.contains("Detected locale"), "{logged}");
    }

    #[test]
    fn the_output_is_handed_back_so_what_prints_next_is_not_a_log_line() {
        let _turn = one_at_a_time();
        let directory =
            crate::tmp::TempDir::new("caelestia-tui-give-back").expect("a temporary dir");
        let log = directory.path().join("install.log");

        hand_over(&log);
        hand_over(&log);
        give_back();
        // A marker rather than an empty file: another test's own printing can land in a log that
        // is open on the whole process, and this is about where *this* output goes.
        let written = write_to_inherited_output("this line belongs on the screen\n");

        assert!(written.is_ok(), "the terminal is back: {written:?}");
        let logged = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(
            !logged.contains("this line belongs on the screen"),
            "the output is still going to the log after giving it back: {logged}"
        );
    }

    #[test]
    fn the_screen_the_wizard_draws_on_outlives_the_output_being_handed_over() {
        let _turn = one_at_a_time();
        let directory = crate::tmp::TempDir::new("caelestia-tui-screen").expect("a temp dir");
        let log = directory.path().join("install.log");

        let mut screen = Screen::take();
        hand_over(&log);
        let drawn = screen.write_all(b"the frame");
        give_back();

        assert!(drawn.is_ok(), "the wizard keeps its own handle: {drawn:?}");
    }
}
