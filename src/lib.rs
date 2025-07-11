use std::{
    io,
    result::Result,
    sync::mpsc,
    time::{Duration, Instant},
};

use config::Config;
use ratatui::{
    crossterm::{
        event::{self, Event as CrossEvent},
        terminal::disable_raw_mode,
    },
    prelude::CrosstermBackend,
    Terminal,
};
use ui::{PanelElement, Ui};

mod config;
mod graphql_requests;
pub mod logging;
mod ui;

/// Sets tick rate(minimum intervall for a full redraw)
pub const TICK_RATE: Duration = Duration::from_millis(200);

/// Event enum to carry Input or Tick event to TerminalApp
pub enum Event<I> {
    Input(I),
    Tick,
}

/// Listens to input from user and captures that by sending it to the main application. Over a
/// multiproducer single consumer channel.
/// # Example
/// ```no_run
/// let (sender, receiver) = mpsc::channel();
/// let mut event_loop = EventLoop::new(sender);
///
/// thread::spawn(move || event_loop.run());
/// ```
pub struct EventLoop {
    sender: mpsc::Sender<Event<CrossEvent>>,
    last_tick: Instant,
}

impl EventLoop {
    /// Creates a new instance of EventLoop taking a sender of Event<CrossEvent>
    pub fn new(sender: mpsc::Sender<Event<CrossEvent>>) -> Self {
        Self {
            sender,
            last_tick: Instant::now(),
        }
    }

    /// Runs the Eventloop locking the current thread
    /// Therefore you should move this to a new thread:
    /// ```no_run
    /// let event_loop = EventLoop::new(sender);
    ///
    /// thread::spawn(move || event_loop.run());
    /// ```
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

    /// Reads the happened event and sends that if it is a key input through it's assigned channel.
    fn handle_event(&self) {
        match event::read() {
            Ok(CrossEvent::Key(key)) => {
                if let Err(error) = self.sender.send(Event::Input(CrossEvent::Key(key))) {
                    println!("{error} occured during sending!");
                }
            }
            Ok(_) => (),
            Err(error) => println!("{error} occured during reading of event!"),
        }
    }

    /// Sends a tick through it's assigned channel.
    fn send_tick(&mut self) {
        if self.last_tick.elapsed() <= TICK_RATE {
            return;
        }

        if self.sender.send(Event::Tick).is_ok() {
            self.last_tick = Instant::now();
        }
    }
}

/// Main application rendering ui and pushing input events to it's components.
/// ```no_run
/// let (sender, receiver) mpsc::channel();
///
/// let app = TerminalApp::new(receiver);
/// if app.is_ok() {
///     app.run();
/// }
/// ```
pub struct TerminalApp {
    input_receiver: mpsc::Receiver<Event<CrossEvent>>,

    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalApp {
    /// Creates a new Instance of TerminalApp taking a receiver of Event<CrossEvent>.
    /// Fetching the Terminal may error so we return a result.
    pub fn new(input_receiver: mpsc::Receiver<Event<CrossEvent>>) -> Result<Self, std::io::Error> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            input_receiver,
            terminal,
        })
    }

    /// Starts the main rendering loop displaying all widgets on the terminal.
    /// This runs until the client wants to exit.
    pub fn run(&mut self) {
        // clear terminal if this doesn't succed we can't really draw therefore we quit
        if let Err(error) = self.terminal.clear() {
            log::error!("{error} occured during terminal clearing");
            return;
        }

        let config = match Config::from_config_file() {
            Ok(config) => config,
            Err(error) => {
                log::error!("{}", error);
                Config::default()
            }
        };

        let mut ui = match Ui::new(config) {
            Ok(menu) => menu,
            Err(error) => {
                log::error!("{} occured during creation of TabMenu.", error);
                return;
            }
        };

        loop {
            // we call tick for the menu so it can try and receive data
            ui.tick();

            let draw_success = self.terminal.draw(|render_frame| {
                let layout = ui::layouts::create_base_layout(render_frame);

                ui.render(render_frame, layout[0])
            });

            if let Err(error) = draw_success {
                log::error!("Drawing to screen errored due to: {}", error);
            }

            // if we get an input event we pass that to the top most element and others
            // depending on if the panel doesn't want other panels to receive that input
            // we break the loop
            match self.input_receiver.recv() {
                Ok(event) => match event {
                    Event::Input(event) => {
                        if let CrossEvent::Key(key) = event {
                            ui.handle_input(key);
                        }
                    }
                    Event::Tick => {}
                },
                Err(error) => {
                    self.clean_up_terminal(Some(format!(
                        "{error} occured during receiving input!"
                    )));
                    break;
                }
            };

            if ui.wants_to_quit() {
                self.clean_up_terminal(None);
                break;
            }
        }
    }

    /// cleans up terminal after finish executing
    fn clean_up_terminal(&mut self, message: Option<String>) {
        if let Err(error) = self.terminal.clear() {
            log::error!("{error} occured during terminal clearing");
        }
        if let Err(error) = disable_raw_mode() {
            log::error!("{error} occured when trying to exit raw mode!");
        }
        if let Err(error) = self.terminal.show_cursor() {
            log::error!("{error} occured when trying to show cursor!");
        }

        if message.is_some() {
            log::error!("{}", message.unwrap());
        }
    }
}
