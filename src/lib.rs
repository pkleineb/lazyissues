use std::{
    error::Error,
    io,
    result::Result,
    sync::mpsc,
    time::{Duration, Instant},
};

use ratatui::{
    backend,
    crossterm::event::{self, Event as CrossEvent},
    prelude::CrosstermBackend,
    Terminal,
};

pub const TICK_RATE: Duration = Duration::from_millis(200);

pub enum Event<I> {
    Input(I),
    Tick,
}

pub enum MenuItem {
    Issues,
    IssueView,
    PullRequests,
    PullRequestView,
    Actions,
    Projects,
    ProjectsView,
}

impl From<MenuItem> for usize {
    fn from(input: MenuItem) -> usize {
        match input {
            MenuItem::Issues | MenuItem::IssueView => 0,
            MenuItem::PullRequests | MenuItem::PullRequestView => 1,
            MenuItem::Actions => 2,
            MenuItem::Projects | MenuItem::ProjectsView => 3,
        }
    }
}

pub struct EventLoop {
    sender: mpsc::Sender<Event<CrossEvent>>,
    last_tick: Instant,
}

impl EventLoop {
    pub fn new(sender: mpsc::Sender<Event<CrossEvent>>) -> Self {
        Self {
            sender,
            last_tick: Instant::now(),
        }
    }

    pub fn run(&mut self) {
        self.last_tick = Instant::now();

        loop {
            let timeout = TICK_RATE
                .checked_sub(self.last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            let poll = event::poll(timeout);
            match poll {
                Ok(found_event) => {
                    if found_event {
                        self.handle_event();
                    } else {
                        self.send_tick();
                    }
                }
                Err(error) => println!("{error} occured during polling!"),
            }
        }
    }

    fn handle_event(&self) {
        match event::read() {
            Ok(CrossEvent::Key(key)) => {
                match self.sender.send(Event::Input(CrossEvent::Key(key))) {
                    Err(error) => println!("{error} occured during sending!"),
                    _ => (),
                }
            }
            Ok(_) => (),
            Err(error) => println!("{error} occured during reading of event!"),
        }
    }

    fn send_tick(&mut self) {
        if self.last_tick.elapsed() >= TICK_RATE {
            if let Ok(_) = self.sender.send(Event::Tick) {
                self.last_tick = Instant::now();
            }
        }
    }
}

pub struct TerminalApp {
    receiver: mpsc::Receiver<Event<CrossEvent>>,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalApp {
    pub fn new(receiver: mpsc::Receiver<Event<CrossEvent>>) -> Result<Self, std::io::Error> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend);

        match terminal {
            Ok(terminal) => Ok(Self { receiver, terminal }),
            Err(error) => Err(error),
        }
    }

    pub fn run(&mut self) {
        match self.terminal.clear() {
            Err(error) => println!("{error} occured during terminal clearing"),
            _ => (),
        }
    }
}
