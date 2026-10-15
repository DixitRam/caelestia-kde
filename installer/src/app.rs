use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;

use crate::exit::Outcome;
use crate::menu::{self, Answers, Entry};
use crate::privileges::{self, Credential, Inhibitor};
use crate::runner::Running;
use crate::signals;
use crate::steps::{self, STEPS, Status};

pub fn log_path() -> PathBuf {
    std::env::var_os("CAELESTIA_INSTALL_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("caelestia-install.log"))
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Password,
    Theming,
    Configure,
    Review,
    Install,
    Complete,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Done(Outcome),
}

pub struct ErrorDialog {
    pub step: String,
    pub detail: Vec<String>,
    pub choice: usize,
}

impl ErrorDialog {
    pub const CHOICES: [&'static str; 3] = ["Retry", "Ignore", "Exit"];

    pub fn new(step: &str, log: &[String]) -> Self {
        let detail: Vec<String> = log.iter().rev().take(6).rev().cloned().collect();
        Self {
            step: step.to_string(),
            detail,
            choice: 0,
        }
    }
}

pub struct App {
    pub bundle: PathBuf,
    pub log_path: PathBuf,
    pub screen: Screen,
    pub answers: Answers,
    pub menu_path: Vec<usize>,
    pub cursor: usize,
    pub theming: usize,
    pub password: String,
    pub problem: Option<String>,
    pub error: Option<ErrorDialog>,
    pub statuses: Vec<Status>,
    pub current: usize,
    pub log: Vec<String>,
    pub log_scroll: usize,
    pub log_follow: bool,
    pub finished: bool,
    credential: Option<Credential>,
    inhibitor: Option<Inhibitor>,
    running: Option<Running>,
    leaving: Option<Outcome>,
}

impl App {
    pub fn new(bundle: PathBuf, log_path: PathBuf) -> Self {
        let mut answers = Answers::new();
        menu::seed_answers(&menu::ROOT, &mut answers);
        Self {
            bundle,
            log_path,
            screen: Screen::Welcome,
            answers,
            menu_path: Vec::new(),
            cursor: 0,
            theming: 0,
            password: String::new(),
            problem: None,
            error: None,
            statuses: vec![Status::Pending; STEPS.len()],
            current: 0,
            log: Vec::new(),
            log_scroll: 0,
            log_follow: true,
            finished: false,
            credential: None,
            inhibitor: None,
            running: None,
            leaving: None,
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

    fn leave(&mut self, outcome: Outcome) {
        self.leaving = Some(outcome);
    }

    pub fn tick(&mut self) {
        if signals::terminated() {
            self.leave(Outcome::Interrupted);
            return;
        }
        if self.screen == Screen::Install {
            self.progress();
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Flow {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Flow::Done(Outcome::Interrupted);
        }
        match self.screen {
            Screen::Welcome => self.welcome_key(key),
            Screen::Password => self.password_key(key),
            Screen::Theming => self.theming_key(key),
            Screen::Configure => self.configure_key(key),
            Screen::Review => self.review_key(key),
            Screen::Install => self.install_key(key),
            Screen::Complete => self.complete_key(key),
        }
    }

    fn welcome_key(&mut self, key: KeyEvent) -> Flow {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.problem = None;
                if needs_password() {
                    self.screen = Screen::Password;
                } else {
                    self.screen = Screen::Theming;
                }
                Flow::Continue
            }
            KeyCode::Esc | KeyCode::Char('q') => Flow::Done(Outcome::Cancelled),
            _ => Flow::Continue,
        }
    }

    fn password_key(&mut self, key: KeyEvent) -> Flow {
        match key.code {
            KeyCode::Enter => {
                let exe =
                    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("caelestia-install"));
                match Credential::install(&self.password, &exe) {
                    Ok(credential) => {
                        if credential.verify() {
                            self.credential = Some(credential);
                            self.password.clear();
                            self.problem = None;
                            self.inhibitor = Some(Inhibitor::acquire());
                            self.screen = Screen::Theming;
                        } else {
                            self.password.clear();
                            self.problem = Some("That password was not accepted.".to_string());
                        }
                    }
                    Err(problem) => {
                        eprintln!("caelestia-install: {problem}");
                        self.problem = Some(problem);
                        self.leave(Outcome::Internal);
                    }
                }
                Flow::Continue
            }
            KeyCode::Backspace => {
                self.password.pop();
                Flow::Continue
            }
            KeyCode::Esc => Flow::Done(Outcome::Cancelled),
            KeyCode::Char(ch) => {
                self.password.push(ch);
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    fn theming_key(&mut self, key: KeyEvent) -> Flow {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.theming = self.theming.saturating_sub(1);
                Flow::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.theming = (self.theming + 1).min(1);
                Flow::Continue
            }
            KeyCode::Char('y') => {
                self.theming = 0;
                Flow::Continue
            }
            KeyCode::Char('n') => {
                self.theming = 1;
                Flow::Continue
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.theming == 0 {
                    enable_app_theming(&mut self.answers);
                }
                self.screen = Screen::Configure;
                self.menu_path.clear();
                self.cursor = 0;
                Flow::Continue
            }
            KeyCode::Esc => {
                self.screen = Screen::Welcome;
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    fn configure_key(&mut self, key: KeyEvent) -> Flow {
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
            Entry::Choice { .. } => {
                menu::cycle(entry, &mut self.answers, true);
            }
            Entry::Action { .. } if entry.id() == menu::REVIEW => {
                self.screen = Screen::Review;
            }
            Entry::Action { .. } => return self.go_back(),
        }
        Flow::Continue
    }

    fn go_back(&mut self) -> Flow {
        if self.menu_path.pop().is_some() {
            self.cursor = 0;
        } else {
            self.screen = Screen::Theming;
        }
        Flow::Continue
    }

    fn review_key(&mut self, key: KeyEvent) -> Flow {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.begin_install();
                Flow::Continue
            }
            KeyCode::Esc => {
                self.screen = Screen::Configure;
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    fn install_key(&mut self, key: KeyEvent) -> Flow {
        if self.error.is_some() {
            return self.error_key(key);
        }
        let visible = 12;
        match key.code {
            KeyCode::PageUp => {
                self.log_follow = false;
                self.log_scroll = self.log_scroll.saturating_sub(visible);
            }
            KeyCode::PageDown => {
                let max = self.log.len().saturating_sub(visible);
                self.log_scroll = (self.log_scroll + visible).min(max);
            }
            KeyCode::End | KeyCode::Char('f') => self.log_follow = true,
            KeyCode::Home => {
                self.log_follow = false;
                self.log_scroll = 0;
            }
            _ => {}
        }
        Flow::Continue
    }

    fn error_key(&mut self, key: KeyEvent) -> Flow {
        let Some(error) = self.error.as_mut() else {
            return Flow::Continue;
        };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                error.choice = error.choice.saturating_sub(1);
                Flow::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                error.choice = (error.choice + 1).min(ErrorDialog::CHOICES.len() - 1);
                Flow::Continue
            }
            KeyCode::Enter | KeyCode::Char(' ') => match error.choice {
                0 => {
                    self.error = None;
                    self.statuses[self.current] = Status::Pending;
                    self.start_next();
                    Flow::Continue
                }
                1 => {
                    self.error = None;
                    self.statuses[self.current] = Status::Ignored;
                    self.current += 1;
                    self.start_next();
                    Flow::Continue
                }
                _ => Flow::Done(Outcome::Failed),
            },
            KeyCode::Esc => {
                self.error = None;
                self.statuses[self.current] = Status::Ignored;
                self.current += 1;
                self.start_next();
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    fn complete_key(&mut self, key: KeyEvent) -> Flow {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Esc => Flow::Done(Outcome::Ok),
            _ => Flow::Continue,
        }
    }

    pub fn begin_install(&mut self) {
        self.screen = Screen::Install;
        self.statuses = vec![Status::Pending; STEPS.len()];
        self.current = 0;
        self.log.clear();
        self.log_scroll = 0;
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
                self.current += 1;
                continue;
            }
            self.statuses[index] = Status::Running;
            let env = self.step_environment();
            let step = &STEPS[index];
            let started = match step.body {
                steps::Body::Script(script) => {
                    Running::spawn(&self.bundle, script, &self.log_path, &env)
                }
                steps::Body::Native(_) => {
                    Running::spawn_native(&self.bundle, step.id, &self.log_path, &env)
                }
            };
            match started {
                Ok(running) => {
                    self.running = Some(running);
                    return;
                }
                Err(problem) => {
                    self.statuses[index] = Status::Failed;
                    self.error = Some(ErrorDialog::new(
                        STEPS[index].name,
                        std::slice::from_ref(&problem),
                    ));
                    self.problem = Some(problem);
                    return;
                }
            }
        }
        self.screen = Screen::Complete;
        self.finished = true;
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
        env.push(("CAELESTIA_FRESH_INSTALL", "1".to_string()));
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
        running.drain(&mut self.log);
        if self.log_follow {
            self.log_scroll = self.log.len().saturating_sub(12);
        }
        match running.poll() {
            Ok(Some(status)) => {
                let outcome = running.outcome(status);
                self.statuses[self.current] = outcome;
                if outcome.is_failure() {
                    self.error = Some(ErrorDialog::new(STEPS[self.current].name, &self.log));
                } else {
                    self.current += 1;
                    self.start_next();
                }
            }
            Ok(None) => self.running = Some(running),
            Err(problem) => {
                self.statuses[self.current] = Status::Failed;
                self.error = Some(ErrorDialog::new(
                    STEPS[self.current].name,
                    std::slice::from_ref(&problem),
                ));
                self.problem = Some(problem);
            }
        }
    }

    pub fn leaving(&self) -> Option<Outcome> {
        self.leaving
    }

    pub fn tally(&self) -> (usize, usize, usize, usize) {
        let count = |status: Status| self.statuses.iter().filter(|each| **each == status).count();
        (
            count(Status::Ok),
            count(Status::Warn),
            count(Status::Ignored) + count(Status::Failed),
            count(Status::Skipped),
        )
    }
}

fn enable_app_theming(answers: &mut Answers) {
    for id in [
        "INSTALL_VSCODE",
        "INSTALL_ZED",
        "INSTALL_SPICETIFY",
        "INSTALL_DISCORD",
        "INSTALL_TODOIST",
        "INSTALL_FIREFOX_THEME",
    ] {
        answers.insert(id, "true".to_string());
    }
}

fn needs_password() -> bool {
    !privileges::is_root() && !privileges::sudo_cached()
}

pub fn run(bundle: PathBuf, log_path: PathBuf) -> Outcome {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        eprintln!("caelestia-install: the installer needs a terminal to run");
        return Outcome::Environment;
    }
    signals::install();
    let mut terminal = match ratatui::try_init() {
        Ok(terminal) => terminal,
        Err(error) => {
            eprintln!("caelestia-install: cannot open a terminal: {error}");
            return Outcome::Environment;
        }
    };
    let mut app = App::new(bundle, log_path);
    let outcome = event_loop(&mut terminal, &mut app);
    ratatui::try_restore().ok();
    outcome
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
