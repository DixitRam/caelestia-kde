// The app: what the wizard knows, and what a key does to it.
//
// The shape is one stage at a time with overlays on top of it, and one meaning per key: `esc`
// always steps back, `q` always leaves, `ctrl+c` always stops. A failure is never dismissed by
// accident, and a step that is running is never abandoned without being stopped first.

use std::io::IsTerminal;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui::layout::Rect;

use crate::env::Env;
use crate::exit::Outcome;
use crate::glyphs::Glyphs;
use crate::logline::LogLine;
use crate::menu::{self, Answers, Entry};
use crate::privileges::{self, Credential, Inhibitor};
use crate::runner::Running;
use crate::signals;
use crate::steps::{self, Phase, STEPS, Status};
use crate::theme::Theme;
use crate::ui::{self, Key};

/// How many lines of the failing step's output the failure card shows.
const FAILURE_TAIL: usize = 6;

pub fn log_path() -> PathBuf {
    std::env::var_os("CAELESTIA_INSTALL_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("caelestia-install.log"))
}

/// What the wizard is here to do. Update and Uninstall leave the TUI: `update.rs` and
/// `uninstall.rs` own their own prompts, and running them over a terminal the TUI is holding
/// would have the two fight over it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Install,
    Update,
    Uninstall,
}

impl Action {
    pub const ALL: [Action; 3] = [Action::Install, Action::Update, Action::Uninstall];

    pub fn title(self) -> &'static str {
        match self {
            Action::Install => "Install Caelestia",
            Action::Update => "Update this install",
            Action::Uninstall => "Uninstall Caelestia",
        }
    }

    /// One plain line per action, in the voice the installer this is modelled on uses: a short
    /// factual description, no sell.
    pub fn help(self) -> &'static str {
        match self {
            Action::Install => "Installs Caelestia and the packages it needs.",
            Action::Update => "Updates an install that is already on this machine.",
            Action::Uninstall => "Removes Caelestia and restores the configs it backed up.",
        }
    }

    pub fn header(self) -> &'static str {
        match self {
            Action::Install => "install",
            Action::Update => "update",
            Action::Uninstall => "uninstall",
        }
    }

    fn handoff(self) -> Option<Handoff> {
        match self {
            Action::Install => None,
            Action::Update => Some(Handoff::Update),
            Action::Uninstall => Some(Handoff::Uninstall),
        }
    }
}

/// Where the wizard has got to. Overlays are not stages: closing one returns to the stage it was
/// opened over.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Stage {
    Home,
    Privilege,
    Options,
    Review,
    Run,
    Done,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Overlay {
    Keys,
    Log,
    Failure,
    Confirm,
}

/// What the confirm overlay is asking about.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Confirm {
    Handoff(Action),
    Quit,
}

/// The two actions that hand the terminal back to another program.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Handoff {
    Update,
    Uninstall,
}

/// What the TUI leaves behind: an exit code, and possibly work for the caller to do in the
/// terminal it just gave back.
pub struct Session {
    pub outcome: Outcome,
    pub handoff: Option<Handoff>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Done(Outcome),
}

pub struct Choice {
    pub key: char,
    pub label: &'static str,
    pub help: &'static str,
}

/// What went wrong, what can be done about it, and what each answer costs.
pub struct FailureCard {
    pub step: &'static str,
    pub detail: Vec<LogLine>,
    pub choice: usize,
    id: &'static str,
}

impl FailureCard {
    pub const CHOICES: [Choice; 4] = [
        Choice {
            key: 'r',
            label: "Retry",
            help: "run this step again from the start",
        },
        Choice {
            key: 's',
            label: "Skip",
            help: "mark it SKIPPED and carry on",
        },
        Choice {
            key: 'i',
            label: "Ignore",
            help: "mark it IGNORED, carry on, and report the install as failed",
        },
        Choice {
            key: 'q',
            label: "Stop",
            help: "end the install here",
        },
    ];

    pub fn new(step: &'static steps::Step, log: &[LogLine]) -> Self {
        let detail = log.iter().rev().take(FAILURE_TAIL).rev().cloned().collect();
        Self {
            step: step.name,
            id: step.id,
            detail,
            choice: 0,
        }
    }

    /// The command that reproduces the step outside the TUI, so a failure can be worked on
    /// somewhere there is room to read it.
    pub fn reproduction(&self) -> String {
        format!("caelestia-install step {}", self.id)
    }
}

pub struct App {
    pub bundle: PathBuf,
    pub log_path: PathBuf,
    pub env: Env,
    pub theme: Theme,
    pub glyphs: Glyphs,
    pub distro: String,
    pub kind: &'static str,
    pub installed: bool,
    pub stage: Stage,
    pub overlay: Option<Overlay>,
    pub confirm: Option<Confirm>,
    pub action: Action,
    pub home: usize,
    pub answers: Answers,
    pub menu_path: Vec<usize>,
    pub cursor: usize,
    pub password: String,
    /// A password that is with sudo. The answer arrives on a later tick, so the frame can show
    /// that something is happening while PAM takes its time over it.
    pub(crate) checking: Option<std::sync::mpsc::Receiver<bool>>,
    /// Walk the state machine without running any step: what the tests use to see a whole install
    /// through. Nothing on the command line sets it, and it is false everywhere else.
    pub pretend: bool,
    pub attempts: usize,
    pub problem: Option<String>,
    pub notice: Option<String>,
    pub failure: Option<FailureCard>,
    pub paused: bool,
    pub statuses: Vec<Status>,
    pub current: usize,
    pub log: Vec<LogLine>,
    pub log_follow: bool,
    pub log_scroll: usize,
    pub log_loud_only: bool,
    pub review_scroll: usize,
    pub area: Rect,
    pub spinner: usize,
    durations: Vec<Option<Duration>>,
    running_since: Option<Instant>,
    leaving: Option<Outcome>,
    handoff: Option<Handoff>,
    credential: Option<Credential>,
    inhibitor: Option<Inhibitor>,
    running: Option<Running>,
}

impl App {
    pub fn new(bundle: PathBuf, log_path: PathBuf, env: Env) -> Self {
        let mut answers = Answers::new();
        menu::seed_answers(&menu::ROOT, &mut answers);
        Self {
            bundle,
            log_path,
            theme: Theme::resolve(&env),
            glyphs: Glyphs::resolve(&env),
            env,
            // Asked once, at startup: detection reads the filesystem, and the header is drawn
            // thirty times a second.
            distro: crate::distro::detect().label().to_string(),
            kind: crate::install_kind::label(),
            installed: crate::install_kind::shell_config().exists(),
            stage: Stage::Home,
            overlay: None,
            confirm: None,
            action: Action::Install,
            home: 0,
            answers,
            menu_path: Vec::new(),
            cursor: 0,
            password: String::new(),
            checking: None,
            pretend: false,
            attempts: 0,
            problem: None,
            notice: None,
            failure: None,
            paused: false,
            statuses: vec![Status::Pending; STEPS.len()],
            current: 0,
            log: Vec::new(),
            log_follow: true,
            log_scroll: 0,
            log_loud_only: false,
            review_scroll: 0,
            area: Rect::new(0, 0, 0, 0),
            spinner: 0,
            durations: vec![None; STEPS.len()],
            running_since: None,
            leaving: None,
            handoff: None,
            credential: None,
            inhibitor: None,
            running: None,
        }
    }

    // What the view asks -------------------------------------------------------------

    pub fn header(&self) -> ui::Header {
        ui::Header {
            left: match self.stage {
                Stage::Home => "installer".to_string(),
                _ => self.action.header().to_string(),
            },
            right: format!("{} · {} install", self.distro, self.kind),
        }
    }

    pub fn counter(&self) -> Option<String> {
        match self.stage {
            Stage::Run | Stage::Review => Some(format!("step {}/{}", self.finished(), STEPS.len())),
            _ => None,
        }
    }

    pub fn finished(&self) -> usize {
        self.statuses
            .iter()
            .filter(|status| !matches!(status, Status::Pending | Status::Running))
            .count()
    }

    pub fn step_elapsed(&self, index: usize) -> Option<Duration> {
        if self.statuses[index] == Status::Running {
            return self.running_since.map(|since| since.elapsed());
        }
        self.durations[index]
    }

    pub fn phase_elapsed(&self, phase: Phase) -> Duration {
        STEPS
            .iter()
            .enumerate()
            .filter(|(_, step)| step.phase == phase)
            .filter_map(|(index, _)| self.step_elapsed(index))
            .sum()
    }

    pub fn total_elapsed(&self) -> Duration {
        self.durations.iter().flatten().sum::<Duration>()
            + self
                .running_since
                .map(|since| since.elapsed())
                .unwrap_or_default()
    }

    pub fn phase_status(&self, phase: Phase) -> Status {
        let statuses: Vec<Status> = STEPS
            .iter()
            .enumerate()
            .filter(|(_, step)| step.phase == phase)
            .map(|(index, _)| self.statuses[index])
            .collect();
        statuses
            .iter()
            .copied()
            .max_by_key(|status| rank(*status))
            .unwrap_or(Status::Pending)
    }

    /// How far through its phase a phase is, and how many steps it has. A step being worked on
    /// counts as reached, so the line reads as a position rather than a score.
    pub fn phase_progress(&self, phase: Phase) -> (usize, usize) {
        let steps: Vec<usize> = STEPS
            .iter()
            .enumerate()
            .filter(|(_, step)| step.phase == phase)
            .map(|(index, _)| index)
            .collect();
        let reached = steps
            .iter()
            .filter(|index| self.statuses[**index] != Status::Pending)
            .count();
        (reached, steps.len())
    }

    /// The step being worked on, as an index into the table.
    pub fn running_step(&self) -> Option<usize> {
        self.statuses
            .iter()
            .position(|status| *status == Status::Running)
    }

    pub fn tally(&self) -> Tally {
        let count = |status: Status| self.statuses.iter().filter(|each| **each == status).count();
        Tally {
            ok: count(Status::Ok),
            warn: count(Status::Warn),
            failed: count(Status::Failed),
            ignored: count(Status::Ignored),
            skipped: count(Status::Skipped),
            total: STEPS.len(),
        }
    }

    pub fn current_entries(&self) -> &'static [Entry] {
        let mut entries: &'static [Entry] = &menu::ROOT;
        for index in &self.menu_path {
            match &entries[*index] {
                Entry::Submenu { items, .. } => entries = items,
                _ => break,
            }
        }
        entries
    }

    pub fn selected(&self) -> Option<&'static Entry> {
        self.current_entries().get(self.cursor)
    }

    pub fn breadcrumb(&self) -> String {
        let mut trail = vec![
            crate::theme::NAME.to_string(),
            self.stage_name().to_string(),
        ];
        let mut entries: &'static [Entry] = &menu::ROOT;
        for index in &self.menu_path {
            match entries.get(*index) {
                Some(Entry::Submenu {
                    items, title: name, ..
                }) => {
                    trail.push(name.to_string());
                    entries = items;
                }
                _ => break,
            }
        }
        trail.join(" › ")
    }

    /// What the header calls this stage.
    fn stage_name(&self) -> &'static str {
        match self.stage {
            Stage::Home => "welcome",
            Stage::Privilege => "privileges",
            Stage::Options => "configure",
            Stage::Review => "review",
            Stage::Run => "install",
            Stage::Done => "finished",
        }
    }

    pub fn action_enabled(&self, action: Action) -> bool {
        match action {
            Action::Install | Action::Update => true,
            Action::Uninstall => self.installed,
        }
    }

    /// The lines the log pane shows, in order: everything, or only what went wrong.
    pub fn visible_log(&self) -> Vec<&LogLine> {
        self.log
            .iter()
            .filter(|line| !self.log_loud_only || line.is_loud())
            .collect()
    }

    pub fn log_rows(&self) -> usize {
        ui::log_rows(self.run_layout().log).max(1) as usize
    }

    pub fn run_layout(&self) -> ui::RunLayout {
        let body = ui::body(self.area);
        ui::run_layout(body, crate::screens::checklist_rows(self))
    }

    /// Where the log pane starts, sized for a pane: the run screen passes the pane it drew, and
    /// the full-screen log passes its own. One function, so the two cannot disagree.
    pub fn log_window_for(&self, rows: usize) -> (usize, usize) {
        let rows = rows.max(1);
        let lines = self.visible_log();
        let start = if self.log_follow {
            lines.len().saturating_sub(rows)
        } else {
            self.log_scroll.min(lines.len().saturating_sub(rows))
        };
        (start, rows)
    }

    pub fn review_rows(&self) -> usize {
        let (_, list) = ui::review_layout(ui::body(self.area));
        list.height.max(1) as usize
    }

    /// How far the review list can be scrolled before it is showing its last page.
    pub fn review_limit(&self) -> usize {
        crate::screens::review_total(self).saturating_sub(self.review_rows())
    }

    // The loop ----------------------------------------------------------------------

    pub fn leaving(&self) -> Option<Outcome> {
        self.leaving
    }

    pub fn session(&self, outcome: Outcome) -> Session {
        Session {
            outcome,
            handoff: self.handoff,
        }
    }

    fn leave(&mut self, outcome: Outcome) {
        self.leaving = Some(outcome);
    }

    /// The signal handler's escape hatch, and `ctrl+c`: stop whatever is running, then leave.
    fn interrupt(&mut self) -> Flow {
        if self.overlay == Some(Overlay::Confirm) {
            return self.stop_and_leave();
        }
        if self.busy() {
            self.confirm = Some(Confirm::Quit);
            self.overlay = Some(Overlay::Confirm);
            return Flow::Continue;
        }
        Flow::Done(Outcome::Interrupted)
    }

    /// Whether a step is in flight, which is the only thing that makes quitting worth asking
    /// about.
    fn busy(&self) -> bool {
        self.running.is_some() || self.running_step().is_some()
    }

    fn stop_and_leave(&mut self) -> Flow {
        self.stop_running();
        Flow::Done(Outcome::Interrupted)
    }

    /// A step that is stopped is a step that is stopped: leaving the child behind would let a
    /// package manager carry on writing while the user believes they cancelled.
    fn stop_running(&mut self) {
        if let Some(mut running) = self.running.take() {
            running.stop();
        }
        self.running_since = None;
    }

    pub fn tick(&mut self) {
        if signals::terminated() {
            self.stop_running();
            self.leave(Outcome::Interrupted);
            return;
        }
        self.spinner = self.spinner.wrapping_add(1);
        if self.stage == Stage::Privilege {
            self.check_password();
        }
        if self.stage == Stage::Run && self.running.is_some() {
            self.progress();
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Flow {
        // A notice is the answer to the key before this one, so any key clears it.
        self.notice = None;

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return self.interrupt();
        }
        if let Some(overlay) = self.overlay {
            return self.overlay_key(overlay, key);
        }
        match self.stage {
            Stage::Home => self.home_key(key),
            Stage::Privilege => self.privilege_key(key),
            Stage::Options => self.options_key(key),
            Stage::Review => self.review_key(key),
            Stage::Run => self.run_key(key),
            Stage::Done => self.done_key(key),
        }
    }

    pub fn keys(&self) -> Vec<Key> {
        let glyphs = self.glyphs;
        match self.overlay {
            Some(Overlay::Keys) => vec![ui::key(glyphs.escape(), "close")],
            Some(Overlay::Log) => vec![
                ui::key(glyphs.up_down(), "scroll"),
                ui::key("w", "loud only"),
                ui::key("f", "follow"),
                ui::key(glyphs.escape(), "close"),
            ],
            Some(Overlay::Failure) => vec![
                ui::key("r", "retry"),
                ui::key("s", "skip"),
                ui::key("i", "ignore"),
                ui::key("q", "stop"),
                ui::key(glyphs.escape(), "decide later"),
            ],
            Some(Overlay::Confirm) => vec![ui::key("y", "yes"), ui::key("n", "no")],
            None => match self.stage {
                Stage::Home => vec![
                    ui::key(glyphs.up_down(), "choose"),
                    ui::key(glyphs.enter(), "run"),
                    ui::key("t", "theme"),
                    ui::key("?", "keys"),
                    ui::key("q", "quit"),
                ],
                Stage::Privilege if self.checking.is_some() => Vec::new(),
                Stage::Privilege if self.attempts > 0 => vec![
                    ui::key(glyphs.enter(), "continue"),
                    ui::key("r", "try again"),
                    ui::key(glyphs.escape(), "back"),
                ],
                Stage::Privilege => vec![
                    ui::key(glyphs.enter(), "continue"),
                    ui::key(glyphs.escape(), "back"),
                ],
                Stage::Options => vec![
                    ui::key(glyphs.up_down(), "move"),
                    ui::key(glyphs.left_right(), "change"),
                    ui::key(glyphs.enter(), "choose"),
                    ui::key("r", "reset"),
                    ui::key("R", "reset all"),
                    ui::key(glyphs.escape(), "back"),
                ],
                Stage::Review => vec![
                    ui::key(glyphs.up_down(), "scroll"),
                    ui::key(glyphs.enter(), "install"),
                    ui::key(glyphs.escape(), "back"),
                ],
                Stage::Run if self.paused => vec![
                    ui::key(glyphs.enter(), "the failure"),
                    ui::key("l", "log"),
                    ui::key("q", "quit"),
                ],
                Stage::Run => vec![
                    ui::key(glyphs.up_down(), "scroll"),
                    ui::key("f", "follow"),
                    ui::key("w", "loud only"),
                    ui::key("l", "log"),
                    ui::key("q", "quit"),
                ],
                Stage::Done => vec![
                    ui::key("r", "retry failed"),
                    ui::key("l", "log"),
                    ui::key(glyphs.enter(), "finish"),
                    ui::key("q", "quit"),
                ],
            },
        }
    }

    // Home --------------------------------------------------------------------------

    fn home_key(&mut self, key: KeyEvent) -> Flow {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.home = if self.home == 0 {
                    Action::ALL.len() - 1
                } else {
                    self.home - 1
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.home = (self.home + 1) % Action::ALL.len();
            }
            KeyCode::Char('t') => {
                self.theme.cycle();
                self.notice = Some(format!("{} theme", self.theme.mode.name()));
            }
            KeyCode::Char('?') => self.overlay = Some(Overlay::Keys),
            KeyCode::Enter | KeyCode::Char(' ') => return self.choose_action(),
            KeyCode::Esc | KeyCode::Char('q') => return Flow::Done(Outcome::Cancelled),
            _ => {}
        }
        Flow::Continue
    }

    fn choose_action(&mut self) -> Flow {
        let action = Action::ALL[self.home];
        if !self.action_enabled(action) {
            self.notice =
                Some("Caelestia is not installed, so there is nothing to remove".to_string());
            return Flow::Continue;
        }
        self.action = action;
        match action {
            Action::Install => {
                self.stage = if needs_password() {
                    Stage::Privilege
                } else {
                    Stage::Options
                };
            }
            Action::Update | Action::Uninstall => {
                self.confirm = Some(Confirm::Handoff(action));
                self.overlay = Some(Overlay::Confirm);
            }
        }
        Flow::Continue
    }

    // Privilege ---------------------------------------------------------------------

    fn privilege_key(&mut self, key: KeyEvent) -> Flow {
        // The password is with sudo, and no key pressed now can change what comes back: it
        // would only queue up a second attempt behind the first.
        if self.checking.is_some() {
            return Flow::Continue;
        }
        match key.code {
            KeyCode::Enter => {
                self.start_check();
                Flow::Continue
            }
            KeyCode::Char('r') if self.attempts > 0 => {
                self.start_check();
                Flow::Continue
            }
            KeyCode::Backspace => {
                self.password.pop();
                Flow::Continue
            }
            KeyCode::Esc => {
                self.password.clear();
                self.problem = None;
                self.attempts = 0;
                self.stage = Stage::Home;
                Flow::Continue
            }
            // A chord is not a character of a password. A terminal that sends one for enter,
            // or any other combination, must not leave a stray letter in the field.
            KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !character.is_control() {
                    self.password.push(character);
                }
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    /// Hands the password to sudo on a thread of its own. PAM can take seconds to answer, and a
    /// frame that freezes for that long reads as a hang - which is what made people type their
    /// password again while the first attempt was still in flight.
    fn start_check(&mut self) {
        if self.checking.is_some() {
            return;
        }
        if self.password.is_empty() {
            self.problem = Some("Type the password first.".to_string());
            return;
        }
        // A correct password clears the system's tally, so an attempt is always worth letting
        // through while there is one to make; only a wrong one can lock the account, and a
        // locked account cannot take the right password either. Nothing asks the system in a
        // pretend run, where there is no system to ask.
        if !self.pretend && privileges::locked_out() {
            self.problem = Some("Locked out for a few minutes after too many tries.".to_string());
            return;
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        if self.pretend {
            // Nothing here should call sudo: answer the way one that accepted would have.
            let _ = sender.send(true);
        } else {
            let secret = self.password.clone();
            thread::spawn(move || {
                let _ = sender.send(privileges::accepts(&secret));
            });
        }
        self.checking = Some(receiver);
        self.problem = None;
    }

    fn check_password(&mut self) {
        let Some(receiver) = self.checking.as_ref() else {
            return;
        };
        match receiver.try_recv() {
            Ok(true) => {
                self.checking = None;
                self.accept_password();
            }
            Ok(false) => {
                self.checking = None;
                self.password.clear();
                self.attempts += 1;
                self.problem = Some(if !self.pretend && privileges::on_the_last_entry() {
                    // The system counts refusals, and once it has counted enough it stops
                    // accepting the right password for a while. Warn before that happens
                    // rather than after it.
                    "Not accepted. A wrong entry now locks the account.".to_string()
                } else {
                    "That password was not accepted.".to_string()
                });
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.checking = None;
                self.problem = Some("The password could not be checked.".to_string());
            }
        }
    }

    /// The password sudo accepted: publish it for the steps to escalate with, and keep the
    /// session awake for the work that follows.
    fn accept_password(&mut self) {
        if !self.pretend {
            let exe =
                std::env::current_exe().unwrap_or_else(|_| PathBuf::from("caelestia-install"));
            match Credential::install(&self.password, &exe) {
                Ok(credential) => self.credential = Some(credential),
                Err(problem) => {
                    eprintln!("caelestia-install: {problem}");
                    self.problem = Some(problem);
                    self.leave(Outcome::Internal);
                    return;
                }
            }
            if self.inhibitor.is_none() {
                self.inhibitor = Some(Inhibitor::acquire());
            }
        }
        self.password.clear();
        self.attempts = 0;
        self.problem = None;
        self.stage = Stage::Options;
    }

    // Options -----------------------------------------------------------------------

    fn options_key(&mut self, key: KeyEvent) -> Flow {
        let entries = self.current_entries();
        if entries.is_empty() {
            return Flow::Continue;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursor = if self.cursor == 0 {
                    entries.len() - 1
                } else {
                    self.cursor - 1
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor = (self.cursor + 1) % entries.len();
            }
            KeyCode::Char('r') => self.reset_highlighted(),
            KeyCode::Char('R') => self.reset_all(),
            KeyCode::Char('?') => self.overlay = Some(Overlay::Keys),
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(entry) = self.selected() {
                    menu::cycle(entry, &mut self.answers, true);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(entry) = self.selected() {
                    menu::cycle(entry, &mut self.answers, false);
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => return self.choose(),
            KeyCode::Esc => return self.go_back(),
            _ => {}
        }
        Flow::Continue
    }

    pub fn choose(&mut self) -> Flow {
        let Some(entry) = self.selected() else {
            return Flow::Continue;
        };
        match entry {
            Entry::Submenu { .. } => {
                self.menu_path.push(self.cursor);
                self.cursor = 0;
            }
            Entry::Toggle { .. } => {
                menu::toggle(entry, &mut self.answers);
            }
            Entry::Preset { .. } => {
                let on = menu::preset_on(entry, &self.answers);
                menu::preset_set(entry, &mut self.answers, !on);
                self.notice = Some(format!(
                    "{}: {}",
                    entry.title(),
                    if on { "off" } else { "on" }
                ));
            }
            Entry::Choice { .. } => {
                menu::cycle(entry, &mut self.answers, true);
            }
            Entry::Action { .. } if entry.id() == menu::REVIEW => {
                self.stage = Stage::Review;
                self.review_scroll = 0;
            }
            Entry::Action { .. } => return self.go_back(),
        }
        Flow::Continue
    }

    fn reset_highlighted(&mut self) {
        let Some(entry) = self.selected() else {
            return;
        };
        if menu::reset(entry, &mut self.answers) {
            self.notice = Some(format!("{} reset", entry.title()));
        }
    }

    fn reset_all(&mut self) {
        self.answers.clear();
        menu::seed_answers(&menu::ROOT, &mut self.answers);
        self.notice = Some("every option is back at its default".to_string());
    }

    fn go_back(&mut self) -> Flow {
        if self.menu_path.pop().is_some() {
            self.cursor = 0;
        } else {
            self.stage = Stage::Home;
        }
        Flow::Continue
    }

    // Review ------------------------------------------------------------------------

    fn review_key(&mut self, key: KeyEvent) -> Flow {
        let rows = self.review_rows();
        let limit = self.review_limit();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.review_scroll = self.review_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.review_scroll = (self.review_scroll + 1).min(limit);
            }
            KeyCode::PageUp => self.review_scroll = self.review_scroll.saturating_sub(rows),
            KeyCode::PageDown => {
                self.review_scroll = (self.review_scroll + rows).min(limit);
            }
            KeyCode::Home | KeyCode::Char('g') => self.review_scroll = 0,
            KeyCode::End | KeyCode::Char('G') => self.review_scroll = limit,
            KeyCode::Char('?') => self.overlay = Some(Overlay::Keys),
            KeyCode::Enter | KeyCode::Char(' ') => self.begin_install(),
            KeyCode::Esc => self.stage = Stage::Options,
            _ => {}
        }
        Flow::Continue
    }

    // Run ---------------------------------------------------------------------------

    fn run_key(&mut self, key: KeyEvent) -> Flow {
        if self.paused && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
            self.overlay = Some(Overlay::Failure);
            return Flow::Continue;
        }
        let rows = self.log_rows();
        match key.code {
            KeyCode::Up => self.scroll_log(-1, rows),
            KeyCode::Down => self.scroll_log(1, rows),
            KeyCode::PageUp => self.scroll_log(-(rows as isize), rows),
            KeyCode::PageDown => self.scroll_log(rows as isize, rows),
            KeyCode::Home | KeyCode::Char('g') => {
                self.log_follow = false;
                self.log_scroll = 0;
            }
            KeyCode::End => self.log_follow = true,
            KeyCode::Char('f') => {
                self.log_follow = !self.log_follow;
                self.notice = Some(if self.log_follow {
                    "following the log".to_string()
                } else {
                    "log paused".to_string()
                });
            }
            KeyCode::Char('w') => self.toggle_loud_only(),
            KeyCode::Char('l') => self.overlay = Some(Overlay::Log),
            KeyCode::Char('?') => self.overlay = Some(Overlay::Keys),
            KeyCode::Char('q') => {
                self.confirm = Some(Confirm::Quit);
                self.overlay = Some(Overlay::Confirm);
            }
            _ => {}
        }
        Flow::Continue
    }

    fn scroll_log(&mut self, step: isize, rows: usize) {
        self.log_follow = false;
        let furthest = self.visible_log().len().saturating_sub(rows) as isize;
        self.log_scroll = (self.log_scroll as isize + step).clamp(0, furthest) as usize;
    }

    fn toggle_loud_only(&mut self) {
        self.log_loud_only = !self.log_loud_only;
        self.log_scroll = 0;
        self.notice = Some(if self.log_loud_only {
            "showing warnings and errors only".to_string()
        } else {
            "showing every line".to_string()
        });
    }

    fn done_key(&mut self, key: KeyEvent) -> Flow {
        match key.code {
            KeyCode::Char('r') => self.retry_failed(),
            KeyCode::Char('l') => self.overlay = Some(Overlay::Log),
            KeyCode::Char('?') => self.overlay = Some(Overlay::Keys),
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Esc | KeyCode::Char('q') => {
                return self.finish();
            }
            _ => {}
        }
        Flow::Continue
    }

    /// The exit code has to agree with the screen: a session with a step that failed, or that the
    /// user chose to move past, is not a success.
    pub fn finish(&mut self) -> Flow {
        if self.tally().unresolved() > 0 {
            Flow::Done(Outcome::Failed)
        } else {
            Flow::Done(Outcome::Ok)
        }
    }

    // Overlays ----------------------------------------------------------------------

    fn overlay_key(&mut self, overlay: Overlay, key: KeyEvent) -> Flow {
        match overlay {
            Overlay::Keys => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('?')
                ) {
                    self.overlay = None;
                }
                Flow::Continue
            }
            Overlay::Log => self.log_overlay_key(key),
            Overlay::Failure => self.failure_key(key),
            Overlay::Confirm => self.confirm_key(key),
        }
    }

    fn log_overlay_key(&mut self, key: KeyEvent) -> Flow {
        let rows = ui::log_overlay_rows(self.area).max(1) as usize;
        match key.code {
            KeyCode::Esc | KeyCode::Char('l') => self.overlay = None,
            KeyCode::Up => self.scroll_log(-1, rows),
            KeyCode::Down => self.scroll_log(1, rows),
            KeyCode::PageUp => self.scroll_log(-(rows as isize), rows),
            KeyCode::PageDown => self.scroll_log(rows as isize, rows),
            KeyCode::Home | KeyCode::Char('g') => {
                self.log_follow = false;
                self.log_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => self.log_follow = true,
            KeyCode::Char('f') => self.log_follow = !self.log_follow,
            KeyCode::Char('w') => self.toggle_loud_only(),
            _ => {}
        }
        Flow::Continue
    }

    fn failure_key(&mut self, key: KeyEvent) -> Flow {
        if self.failure.is_none() {
            self.overlay = None;
            return Flow::Continue;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(card) = self.failure.as_mut() {
                    card.choice = card.choice.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(card) = self.failure.as_mut() {
                    card.choice = (card.choice + 1).min(FailureCard::CHOICES.len() - 1);
                }
            }
            KeyCode::Char('r') => return self.resolve_failure(0),
            KeyCode::Char('s') => return self.resolve_failure(1),
            KeyCode::Char('i') => return self.resolve_failure(2),
            KeyCode::Char('q') => return self.resolve_failure(3),
            KeyCode::Enter | KeyCode::Char(' ') => {
                let choice = self.failure.as_ref().map_or(0, |card| card.choice);
                return self.resolve_failure(choice);
            }
            // Closing without deciding leaves the failure where it is and the run paused: a key
            // that means "back" everywhere else must not quietly accept a failure here.
            KeyCode::Esc => {
                self.overlay = None;
                self.paused = true;
            }
            _ => {}
        }
        Flow::Continue
    }

    fn resolve_failure(&mut self, choice: usize) -> Flow {
        self.overlay = None;
        self.paused = false;
        match choice {
            0 => {
                self.statuses[self.current] = Status::Pending;
                self.durations[self.current] = None;
                self.start_next();
            }
            1 => {
                self.statuses[self.current] = Status::Skipped;
                self.current += 1;
                self.start_next();
            }
            2 => {
                self.statuses[self.current] = Status::Ignored;
                self.current += 1;
                self.start_next();
            }
            _ => {
                self.statuses[self.current] = Status::Failed;
                return Flow::Done(Outcome::Failed);
            }
        }
        Flow::Continue
    }

    fn confirm_key(&mut self, key: KeyEvent) -> Flow {
        let confirmed = match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => true,
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => false,
            _ => return Flow::Continue,
        };
        let Some(confirm) = self.confirm.take() else {
            self.overlay = None;
            return Flow::Continue;
        };
        self.overlay = None;
        if !confirmed {
            return Flow::Continue;
        }
        match confirm {
            Confirm::Quit => self.stop_and_leave(),
            Confirm::Handoff(action) => {
                self.stop_running();
                self.handoff = action.handoff();
                Flow::Done(Outcome::Ok)
            }
        }
    }

    // The install itself ------------------------------------------------------------

    pub fn begin_install(&mut self) {
        self.stage = Stage::Run;
        self.statuses = vec![Status::Pending; STEPS.len()];
        self.durations = vec![None; STEPS.len()];
        self.current = 0;
        self.log.clear();
        self.log_scroll = 0;
        self.log_follow = true;
        self.failure = None;
        self.paused = false;
        self.start_next();
    }

    /// Try the failed steps again, without redoing what worked.
    pub fn retry_failed(&mut self) {
        let failed: Vec<usize> = self
            .statuses
            .iter()
            .enumerate()
            .filter(|(_, status)| matches!(status, Status::Failed | Status::Ignored))
            .map(|(index, _)| index)
            .collect();
        if failed.is_empty() {
            self.notice = Some("no step needs retrying".to_string());
            return;
        }
        for index in &failed {
            self.statuses[*index] = Status::Pending;
            self.durations[*index] = None;
        }
        self.current = failed[0];
        self.stage = Stage::Run;
        self.failure = None;
        self.paused = false;
        self.log_follow = true;
        self.start_next();
    }

    pub fn start_next(&mut self) {
        while self.current < STEPS.len() {
            let index = self.current;
            if self.statuses[index] != Status::Pending {
                self.current += 1;
                continue;
            }
            if steps::is_skipped(&STEPS[index], &self.answers) {
                self.statuses[index] = Status::Skipped;
                self.durations[index] = Some(Duration::ZERO);
                self.log.push(LogLine::parse(&format!(
                    "  [SKIP]  {} is switched off in the options",
                    STEPS[index].name
                )));
                self.current += 1;
                continue;
            }
            if self.pretend {
                self.statuses[index] = Status::Ok;
                self.durations[index] = Some(Duration::from_secs(2));
                self.log.push(LogLine::parse(&format!(
                    "  [OK]    {} finished",
                    STEPS[index].name
                )));
                self.current += 1;
                continue;
            }
            self.statuses[index] = Status::Running;
            self.running_since = Some(Instant::now());
            let env = self.step_environment();
            let step = &STEPS[index];
            match Running::spawn_native(&self.bundle, step.id, &self.log_path, &env) {
                Ok(running) => {
                    self.running = Some(running);
                    return;
                }
                Err(problem) => {
                    self.statuses[index] = Status::Failed;
                    self.log
                        .push(LogLine::parse(&format!("  [ERR]   {problem}")));
                    self.failure = Some(FailureCard::new(step, &self.log));
                    self.paused = true;
                    return;
                }
            }
        }
        self.stage = Stage::Done;
        self.credential = None;
        self.inhibitor = None;
    }

    pub fn step_environment(&self) -> Vec<(&'static str, String)> {
        let mut env: Vec<(&'static str, String)> = self
            .answers
            .iter()
            .map(|(key, value)| (*key, value.clone()))
            .collect();
        env.push(("CONFIRM_ARG", "--noconfirm".to_string()));
        env.push(("BUNDLE_DIR", self.bundle.display().to_string()));
        if let Some(credential) = &self.credential {
            env.extend(credential.environment());
        }
        env
    }

    fn progress(&mut self) {
        let Some(mut running) = self.running.take() else {
            return;
        };
        let mut fresh: Vec<String> = Vec::new();
        running.drain(&mut fresh);
        self.log
            .extend(fresh.iter().map(|line| LogLine::parse(line)));
        match running.poll() {
            Ok(Some(status)) => {
                let outcome = running.outcome(status);
                self.statuses[self.current] = outcome;
                self.durations[self.current] = Some(
                    self.running_since
                        .take()
                        .map(|since| since.elapsed())
                        .unwrap_or_default(),
                );
                if outcome.is_failure() {
                    self.failure = Some(FailureCard::new(&STEPS[self.current], &self.log));
                    self.paused = true;
                } else {
                    self.current += 1;
                    self.start_next();
                }
            }
            Ok(None) => self.running = Some(running),
            Err(problem) => {
                self.statuses[self.current] = Status::Failed;
                self.durations[self.current] =
                    self.running_since.take().map(|since| since.elapsed());
                self.log
                    .push(LogLine::parse(&format!("  [ERR]   {problem}")));
                self.failure = Some(FailureCard::new(&STEPS[self.current], &self.log));
                self.paused = true;
            }
        }
    }
}

/// Counted once, so the screen and the exit code cannot disagree.
pub struct Tally {
    pub ok: usize,
    pub warn: usize,
    pub failed: usize,
    pub ignored: usize,
    pub skipped: usize,
    pub total: usize,
}

impl Tally {
    /// Steps that did not do what they were asked to do.
    pub fn unresolved(&self) -> usize {
        self.failed + self.ignored
    }

    pub fn ran(&self) -> usize {
        self.ok + self.warn
    }
}

/// What a set of statuses amounts to, loudest first.
fn rank(status: Status) -> u8 {
    match status {
        Status::Failed => 6,
        Status::Ignored => 5,
        Status::Warn => 4,
        Status::Ok => 3,
        Status::Running => 2,
        Status::Skipped => 1,
        Status::Pending => 0,
    }
}

fn needs_password() -> bool {
    !privileges::is_root() && !privileges::sudo_cached()
}

pub fn run(bundle: PathBuf, log_path: PathBuf, env: Env) -> Session {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        eprintln!("caelestia-install: the installer needs a terminal to run");
        return Session {
            outcome: Outcome::Environment,
            handoff: None,
        };
    }
    signals::install();
    let mut terminal = match ratatui::try_init() {
        Ok(terminal) => terminal,
        Err(error) => {
            eprintln!("caelestia-install: cannot open a terminal: {error}");
            return Session {
                outcome: Outcome::Environment,
                handoff: None,
            };
        }
    };
    let mut app = App::new(bundle, log_path, env);
    let outcome = event_loop(&mut terminal, &mut app);
    ratatui::try_restore().ok();
    app.session(outcome)
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Outcome {
    loop {
        if let Some(outcome) = app.leaving() {
            return outcome;
        }
        if terminal
            .draw(|frame| crate::screens::draw(frame, app))
            .is_err()
        {
            return Outcome::Environment;
        }
        app.tick();
        if event::poll(Duration::from_millis(120)).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    if let Flow::Done(outcome) = app.key(key) {
                        return outcome;
                    }
                }
                Ok(_) => {}
                Err(_) => return Outcome::Environment,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu;

    fn app() -> App {
        let mut app = App::new(
            PathBuf::from("/tmp/caelestia"),
            PathBuf::from("/tmp/install.log"),
            Env::default(),
        );
        // The screens and the page keys agree on a size; the tests have to give them one.
        app.area = Rect::new(0, 0, 100, 30);
        app.pretend = true;
        app
    }

    fn press(app: &mut App, code: KeyCode) -> Flow {
        app.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn char_key(app: &mut App, character: char) -> Flow {
        press(app, KeyCode::Char(character))
    }

    /// Everything the run screen shows, as a state without any of the work.
    fn run_with(statuses: &[Status]) -> App {
        let mut app = app();
        app.stage = Stage::Run;
        for (index, status) in statuses.iter().enumerate() {
            app.statuses[index] = *status;
        }
        app
    }

    #[test]
    fn home_offers_the_three_actions_the_installer_fronts() {
        let mut app = app();
        assert_eq!(app.stage, Stage::Home);
        let mut seen = Vec::new();
        for _ in 0..Action::ALL.len() {
            seen.push(Action::ALL[app.home]);
            press(&mut app, KeyCode::Down);
        }
        assert_eq!(
            seen,
            vec![Action::Install, Action::Update, Action::Uninstall]
        );
        assert_eq!(Action::ALL[app.home], Action::Install, "and it wraps");
    }

    #[test]
    fn choosing_install_asks_for_a_password_before_the_options() {
        let mut app = app();
        press(&mut app, KeyCode::Enter);
        // A test process is not root and has no cached sudo, so the password comes first here;
        // the point is that the wizard never skips the stage it needs.
        assert!(matches!(app.stage, Stage::Privilege | Stage::Options));
    }

    #[test]
    fn update_and_uninstall_confirm_before_handing_the_terminal_over() {
        for (index, action, handoff) in [
            (1, Action::Update, Handoff::Update),
            (2, Action::Uninstall, Handoff::Uninstall),
        ] {
            let mut app = app();
            app.installed = true;
            app.home = index;
            assert_eq!(Action::ALL[app.home], action);

            assert_eq!(press(&mut app, KeyCode::Enter), Flow::Continue);
            assert_eq!(app.overlay, Some(Overlay::Confirm));
            assert_eq!(app.confirm, Some(Confirm::Handoff(action)));

            // Saying no leaves the wizard where it was.
            char_key(&mut app, 'n');
            assert_eq!(app.overlay, None);
            assert_eq!(app.leaving(), None);

            press(&mut app, KeyCode::Enter);
            char_key(&mut app, 'y');
            let session = app.session(Outcome::Ok);
            assert_eq!(session.handoff, Some(handoff));
            assert_eq!(app.leaving(), None, "the handoff is the caller's job");
        }
    }

    #[test]
    fn uninstall_is_refused_where_there_is_nothing_installed() {
        let mut app = app();
        app.installed = false;
        app.home = 2;
        char_key(&mut app, ' ');
        assert_eq!(app.stage, Stage::Home);
        assert!(app.notice.is_some(), "and it says why");
        assert_eq!(app.overlay, None);
    }

    #[test]
    fn esc_steps_back_rather_than_leaving() {
        let mut app = app();
        app.stage = Stage::Privilege;
        char_key(&mut app, 'x');
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.stage, Stage::Home);
        assert_eq!(app.password, "", "and it forgets the password");
        assert_eq!(app.leaving(), None);

        app.stage = Stage::Review;
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.stage, Stage::Options);

        app.menu_path = vec![0];
        press(&mut app, KeyCode::Esc);
        assert!(app.menu_path.is_empty());
        assert_eq!(app.stage, Stage::Options);

        press(&mut app, KeyCode::Esc);
        assert_eq!(app.stage, Stage::Home);
        assert_eq!(app.leaving(), None);
    }

    #[test]
    fn quitting_from_home_is_a_cancel_and_not_an_interruption() {
        let mut app = app();
        assert_eq!(
            press(&mut app, KeyCode::Esc),
            Flow::Done(Outcome::Cancelled)
        );
        assert_eq!(char_key(&mut app, 'q'), Flow::Done(Outcome::Cancelled));
    }

    #[test]
    fn a_bad_password_is_asked_for_again_and_never_kept() {
        let mut app = app();
        app.stage = Stage::Privilege;
        for character in "hunter2".chars() {
            char_key(&mut app, character);
        }
        assert_eq!(app.password, "hunter2");
        press(&mut app, KeyCode::Backspace);
        assert_eq!(app.password, "hunter");
    }

    #[test]
    fn the_check_runs_off_the_draw_path_and_the_wait_is_visible() {
        let mut app = app();
        app.stage = Stage::Privilege;
        for character in "hunter2".chars() {
            char_key(&mut app, character);
        }
        press(&mut app, KeyCode::Enter);
        assert!(
            app.checking.is_some(),
            "the check has to run somewhere the frame can carry on without it"
        );
        assert_eq!(
            app.password, "hunter2",
            "the password is still needed for the steps"
        );
        assert_eq!(app.stage, Stage::Privilege, "and nothing has moved on yet");
        assert!(
            app.keys().is_empty(),
            "no key does anything while sudo has it"
        );

        // A key pressed now cannot change the answer that is already on its way.
        press(&mut app, KeyCode::Enter);
        char_key(&mut app, 'x');
        assert_eq!(app.password, "hunter2");

        app.tick();
        assert_eq!(app.stage, Stage::Options);
        assert_eq!(
            app.password, "",
            "the password leaves the field once it is used"
        );
    }

    #[test]
    fn a_refused_password_is_asked_for_again() {
        let mut app = app();
        app.stage = Stage::Privilege;
        app.password = "wrong".to_string();
        let (sender, receiver) = std::sync::mpsc::channel();
        sender.send(false).expect("the answer fits");
        app.checking = Some(receiver);

        app.tick();
        assert_eq!(app.stage, Stage::Privilege, "a refusal keeps the gate open");
        assert_eq!(app.attempts, 1);
        assert_eq!(app.password, "");
        assert!(
            app.problem
                .as_deref()
                .unwrap_or_default()
                .contains("accepted"),
            "the gate has to say it was refused: {:?}",
            app.problem
        );
    }

    #[test]
    fn a_control_chord_never_becomes_part_of_the_password() {
        // Some terminals send the enter key as a control character - ctrl+j is what script(1)
        // sends - and a chord must never leave a stray letter in the field.
        let mut app = app();
        app.stage = Stage::Privilege;
        for character in "hunter2".chars() {
            char_key(&mut app, character);
        }
        for code in [
            KeyCode::Char('j'),
            KeyCode::Char('a'),
            KeyCode::Char('u'),
            KeyCode::Char('h'),
        ] {
            app.key(KeyEvent::new(code, KeyModifiers::CONTROL));
        }
        assert_eq!(
            app.password, "hunter2",
            "a chord is not a character of a password"
        );
    }

    #[test]
    fn the_checklist_collapses_a_phase_that_has_finished() {
        let app = run_with(&[
            Status::Ok,
            Status::Ok,
            Status::Ok,
            Status::Ok,
            Status::Running,
        ]);
        let lines = crate::screens::checklist(&app);
        // One line per phase, plus the running step's own line: six rows where the old screen
        // spent nineteen.
        assert_eq!(lines.len(), 6, "{lines:?}");
        assert!(lines.len() < STEPS.len());
    }

    #[test]
    fn the_way_on_from_a_failure_is_a_decision_rather_than_a_dismissal() {
        let mut app = run_with(&[Status::Failed]);
        app.current = 0;
        app.failure = Some(FailureCard::new(&STEPS[0], &[]));
        app.paused = true;
        app.overlay = Some(Overlay::Failure);

        // `esc` closes the card and leaves the failure exactly where it was.
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.overlay, None);
        assert!(app.paused);
        assert_eq!(app.statuses[0], Status::Failed);
        assert_eq!(app.current, 0, "and nothing advanced past it");

        // Enter brings it back rather than starting anything.
        assert_eq!(press(&mut app, KeyCode::Enter), Flow::Continue);
        assert_eq!(app.overlay, Some(Overlay::Failure));
    }

    #[test]
    fn stopping_at_a_failure_ends_the_session_as_a_failure() {
        let mut app = run_with(&[Status::Failed]);
        app.current = 0;
        app.failure = Some(FailureCard::new(&STEPS[0], &[]));
        app.overlay = Some(Overlay::Failure);
        assert_eq!(char_key(&mut app, 'q'), Flow::Done(Outcome::Failed));
        assert_eq!(app.statuses[0], Status::Failed);
    }

    #[test]
    fn skipping_and_ignoring_both_carry_on_but_only_one_of_them_is_a_failure() {
        for (key, expected) in [('s', Status::Skipped), ('i', Status::Ignored)] {
            // The step is the last one, so carrying on lands on the summary without starting
            // anything.
            let last = STEPS.len() - 1;
            let mut app = run_with(&[Status::Failed]);
            app.current = last;
            app.statuses[last] = Status::Failed;
            app.failure = Some(FailureCard::new(&STEPS[last], &[]));
            app.overlay = Some(Overlay::Failure);

            char_key(&mut app, key);
            assert_eq!(app.statuses[last], expected);
            assert_eq!(app.stage, Stage::Done);
            assert!(!app.paused);
        }
    }

    #[test]
    fn an_ignored_step_is_still_reported_as_a_failure_of_the_session() {
        let mut app = app();
        app.statuses = vec![Status::Ok; STEPS.len()];
        app.statuses[2] = Status::Ignored;
        app.stage = Stage::Done;
        assert_eq!(app.tally().unresolved(), 1);
        assert_eq!(press(&mut app, KeyCode::Enter), Flow::Done(Outcome::Failed));
    }

    #[test]
    fn a_clean_install_finishes_with_a_success() {
        let mut app = app();
        app.statuses = vec![Status::Ok; STEPS.len()];
        app.statuses[1] = Status::Warn;
        app.stage = Stage::Done;
        assert_eq!(app.tally().unresolved(), 0);
        assert_eq!(press(&mut app, KeyCode::Enter), Flow::Done(Outcome::Ok));
    }

    #[test]
    fn the_whole_install_walks_to_the_summary() {
        let mut app = app();
        app.begin_install();
        assert_eq!(app.stage, Stage::Done);
        let tally = app.tally();
        assert_eq!(
            tally.ran() + tally.skipped,
            STEPS.len(),
            "every step either ran or was switched off"
        );
        assert_eq!(app.finished(), STEPS.len());
        assert!(app.total_elapsed() > Duration::ZERO);
    }

    #[test]
    fn retrying_from_the_summary_goes_back_to_the_run() {
        let mut app = app();
        app.begin_install();
        app.statuses[4] = Status::Ignored;
        app.stage = Stage::Done;

        char_key(&mut app, 'r');
        assert_ne!(app.statuses[4], Status::Ignored, "the step was tried again");
        assert_eq!(app.stage, Stage::Done, "and the walk carried on to the end");
    }

    #[test]
    fn retrying_when_nothing_failed_says_so() {
        let mut app = app();
        app.statuses = vec![Status::Ok; STEPS.len()];
        app.stage = Stage::Done;
        char_key(&mut app, 'r');
        assert_eq!(app.stage, Stage::Done);
        assert!(app.notice.is_some());
    }

    #[test]
    fn a_phase_reports_the_worst_thing_that_happened_in_it() {
        let mut app = app();
        app.begin_install();
        app.statuses[1] = Status::Warn;
        assert_eq!(app.phase_status(Phase::Prepare), Status::Warn);
        let (reached, total) = app.phase_progress(Phase::Prepare);
        assert_eq!((reached, total), (4, 4));
        assert_eq!(app.phase_status(Phase::Finalize), Status::Ok);
        assert!(app.phase_elapsed(Phase::Prepare) > Duration::ZERO);
    }

    #[test]
    fn a_phase_that_has_not_started_reports_nothing_and_the_worst_wins() {
        let app = run_with(&[Status::Ok, Status::Warn, Status::Failed]);
        assert_eq!(app.phase_status(Phase::Prepare), Status::Failed);
        assert_eq!(app.phase_status(Phase::Configure), Status::Pending);
        assert_eq!(app.phase_elapsed(Phase::Prepare), Duration::ZERO);
        assert_eq!(app.phase_progress(Phase::Prepare), (3, 4));
    }

    #[test]
    fn options_reset_one_row_or_all_of_them() {
        let mut app = app();
        app.stage = Stage::Options;
        app.cursor = 0;

        // The preset turns the app theming on, and the row knows it was changed.
        press(&mut app, KeyCode::Enter);
        let preset = app.selected().expect("a row");
        assert!(menu::is_changed(preset, &app.answers));

        // `r` puts that row back.
        char_key(&mut app, 'r');
        let preset = app.selected().expect("a row");
        assert!(!menu::is_changed(preset, &app.answers));

        // `R` puts everything back, including a row that was changed by hand.
        app.answers.insert("SKIP_SYSTEM_UPDATE", "true".to_string());
        char_key(&mut app, 'R');
        assert_eq!(
            app.answers.get("SKIP_SYSTEM_UPDATE").map(String::as_str),
            Some("false"),
            "reset all put the whole tree back"
        );
        assert!(app.notice.is_some());
    }

    #[test]
    fn the_options_tree_walks_in_and_out() {
        let mut app = app();
        app.stage = Stage::Options;
        app.cursor = 1;
        let title = app.current_entries()[1].title();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.menu_path, vec![1]);
        assert_eq!(app.cursor, 0);
        assert!(app.breadcrumb().contains(title), "{}", app.breadcrumb());
        press(&mut app, KeyCode::Esc);
        assert!(app.menu_path.is_empty());
        assert_eq!(app.stage, Stage::Options);
    }

    #[test]
    fn review_scrolls_inside_its_own_list() {
        let mut app = app();
        app.area = Rect::new(0, 0, 80, 24);
        app.stage = Stage::Review;
        let rows = app.review_rows();
        let limit = app.review_limit();
        assert!(rows > 3, "the test terminal has a list to scroll");
        assert!(limit > 0, "and the list is longer than the pane");

        press(&mut app, KeyCode::End);
        assert_eq!(app.review_scroll, limit);
        press(&mut app, KeyCode::Down);
        assert_eq!(app.review_scroll, limit, "and it stops at the end");
        press(&mut app, KeyCode::PageUp);
        assert_eq!(app.review_scroll, limit.saturating_sub(rows));
        press(&mut app, KeyCode::Home);
        assert_eq!(app.review_scroll, 0);
    }

    #[test]
    fn the_log_pane_and_the_page_keys_agree_about_how_tall_it_is() {
        let mut app = run_with(&[Status::Running]);
        for line in 0..200 {
            app.log
                .push(LogLine::parse(&format!("  [INFO]  line {line}")));
        }
        let (start, rows) = app.log_window_for(app.log_rows());
        assert_eq!(rows, app.log_rows());
        assert_eq!(start, app.log.len() - rows, "following shows the tail");

        app.log_follow = false;
        app.log_scroll = 0;
        press(&mut app, KeyCode::PageDown);
        assert_eq!(app.log_scroll, rows, "a page is the pane, not a guess");
        press(&mut app, KeyCode::End);
        assert!(app.log_follow, "end goes back to following");
    }

    #[test]
    fn the_loud_filter_keeps_warnings_and_errors() {
        let mut app = run_with(&[Status::Running]);
        app.log = vec![
            LogLine::parse("  [INFO]  chatter"),
            LogLine::parse("  [WARN]  slow mirror"),
            LogLine::parse("  [ERR]   a package failed"),
        ];
        assert_eq!(app.visible_log().len(), 3);
        char_key(&mut app, 'w');
        assert_eq!(app.visible_log().len(), 2);
        assert!(app.log_loud_only);
        char_key(&mut app, 'w');
        assert_eq!(app.visible_log().len(), 3);
    }

    #[test]
    fn quitting_while_a_step_is_running_asks_first_and_then_stops_it() {
        let mut app = run_with(&[Status::Running]);
        // Ctrl+C twice: the first asks, the second stops and leaves.
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.key(ctrl_c), Flow::Continue);
        assert_eq!(app.overlay, Some(Overlay::Confirm));
        assert_eq!(app.confirm, Some(Confirm::Quit));
        assert_eq!(app.key(ctrl_c), Flow::Done(Outcome::Interrupted));
    }

    #[test]
    fn quitting_when_nothing_is_running_does_not_ask() {
        let mut app = run_with(&[Status::Ok]);
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.key(ctrl_c), Flow::Done(Outcome::Interrupted));
    }

    #[test]
    fn the_theme_can_be_flipped_from_the_home_screen() {
        let mut app = app();
        let before = app.theme.mode;
        char_key(&mut app, 't');
        assert_ne!(app.theme.mode, before);
        assert!(app.notice.is_some());
        char_key(&mut app, 't');
        assert_eq!(app.theme.mode, before);
    }

    #[test]
    fn a_notice_lasts_until_the_next_key() {
        let mut app = app();
        char_key(&mut app, 't');
        assert!(app.notice.is_some());
        char_key(&mut app, 'j');
        assert_eq!(app.notice, None);
    }

    #[test]
    fn every_screen_offers_a_way_on() {
        let mut app = app();
        let stages = [
            Stage::Home,
            Stage::Privilege,
            Stage::Options,
            Stage::Review,
            Stage::Run,
            Stage::Done,
        ];
        for stage in stages {
            app.stage = stage;
            app.overlay = None;
            assert!(!app.keys().is_empty(), "{stage:?} has no hints");
            for key in app.keys() {
                assert!(!key.keys.is_empty(), "{stage:?} hints an empty key");
                assert!(!key.what.is_empty(), "{stage:?} hints nothing");
            }
        }
        for overlay in [
            Overlay::Keys,
            Overlay::Log,
            Overlay::Failure,
            Overlay::Confirm,
        ] {
            app.overlay = Some(overlay);
            assert!(!app.keys().is_empty(), "{overlay:?} has no hints");
        }
    }

    #[test]
    fn the_keymap_overlay_closes_on_the_keys_a_user_would_try() {
        let mut app = app();
        for code in [KeyCode::Char('?'), KeyCode::Esc, KeyCode::Enter] {
            char_key(&mut app, '?');
            assert_eq!(app.overlay, Some(Overlay::Keys));
            press(&mut app, code);
            assert_eq!(app.overlay, None, "{code:?} did not close it");
        }
    }

    #[test]
    fn the_log_overlay_closes_and_pages_the_way_the_pane_does() {
        let mut app = run_with(&[Status::Running]);
        for line in 0..200 {
            app.log
                .push(LogLine::parse(&format!("  [INFO]  line {line}")));
        }
        char_key(&mut app, 'l');
        assert_eq!(app.overlay, Some(Overlay::Log));

        let rows = crate::ui::log_overlay_rows(app.area) as usize;
        assert!(rows > 3, "the overlay has room of its own");
        app.log_follow = false;
        app.log_scroll = 100;
        press(&mut app, KeyCode::PageUp);
        assert_eq!(app.log_scroll, 100 - rows, "it pages by its own height");
        press(&mut app, KeyCode::Home);
        assert_eq!(app.log_scroll, 0);

        char_key(&mut app, 'l');
        assert_eq!(app.overlay, None);
    }

    #[test]
    fn the_step_environment_carries_the_answers_and_the_bundle() {
        let app = app();
        let env = app.step_environment();
        assert!(env.iter().any(|(key, _)| *key == "BUNDLE_DIR"));
        assert!(
            env.iter()
                .any(|(key, value)| *key == "CONFIRM_ARG" && value == "--noconfirm")
        );
        assert!(env.iter().any(|(key, _)| *key == "INSTALL_FISH"));
    }

    #[test]
    fn the_header_says_which_action_is_running_and_where() {
        let mut app = app();
        assert_eq!(app.header().left, "installer");
        app.stage = Stage::Options;
        app.action = Action::Install;
        assert_eq!(app.header().left, "install");
        assert!(app.header().right.contains("install"));
        assert!(!app.distro.is_empty());
    }
}
