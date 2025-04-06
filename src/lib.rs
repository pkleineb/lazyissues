// started porting everything over to use traits so that i can generalise and also attribute the
// key inputs to any panel; next up fix all the errors

use std::{
    fs::{self, OpenOptions},
    io,
    path::PathBuf,
    rc::Rc,
    result::Result,
    sync::mpsc,
    time::{Duration, Instant},
};

use config::Config;
use dirs::data_local_dir;
use env_logger::{Builder, Env};
use ratatui::{
    crossterm::{
        event::{self, Event as CrossEvent},
        terminal::disable_raw_mode,
    },
    layout::{Constraint, Direction, Layout, Rect},
    prelude::CrosstermBackend,
    Frame, Terminal,
};
use ui::{tab_menu::TabMenu, PanelElement};

mod config;
mod graphql_requests;
mod ui;

pub const LOG_FILE_NAME: &str = "lazyissues.log";
pub const LOG_DIR_NAME: &str = "lazyissues";

pub fn enable_logging() -> Result<(), std::io::Error> {
    let log_dir = data_local_dir()
        .unwrap_or(PathBuf::new())
        .join(LOG_DIR_NAME);

    fs::create_dir_all(&log_dir)?;

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join(LOG_FILE_NAME))?;

    let default_level = if cfg!(debug_assertions) {
        "debug"
    } else {
        "info"
    };

    Builder::from_env(Env::default().default_filter_or(default_level))
        .target(env_logger::Target::Pipe(Box::new(file)))
        .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Seconds))
        .init();

    Ok(())
}

pub const TICK_RATE: Duration = Duration::from_millis(200);

pub enum Event<I> {
    Input(I),
    Tick,
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
    input_receiver: mpsc::Receiver<Event<CrossEvent>>,

    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    config: Config,
}

impl TerminalApp {
    pub fn new(input_receiver: mpsc::Receiver<Event<CrossEvent>>) -> Result<Self, std::io::Error> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let config = match Config::from_config_file() {
            Ok(config) => config,
            Err(error) => {
                log::error!("{}", error);
                Config::new()
            }
        };

        Ok(Self {
            input_receiver,
            terminal,
            config,
        })
    }

    // main render loop
    pub fn run(&mut self) {
        // clear terminal if this doesn't succed we can't really draw therefore we quit
        if let Err(error) = self.terminal.clear() {
            log::error!("{error} occured during terminal clearing");
            return;
        }

        let mut menu = match TabMenu::new(0, self.config.clone()) {
            Ok(menu) => menu,
            Err(error) => {
                log::error!("{} occured during creation of TabMenu.", error);
                return;
            }
        };

        loop {
            // we call tick for the menu so it can try and receive data
            menu.tick();

            let draw_success = self.terminal.draw(|render_frame| {
                let layout = Self::create_base_layout(render_frame);

                menu.render(render_frame, &layout)
            });

            if let Err(error) = draw_success {
                log::error!("Drawing to screen errored due to: {}", error);
            }

            // if we get an input event we pass that to the top most element and others
            // depending on if the panel doesn't want other panels to receive that input
            // we break the loop
            match self.input_receiver.recv() {
                Ok(event) => match event {
                    Event::Input(event) => match event {
                        CrossEvent::Key(key) => {
                            menu.handle_input(key);
                        }
                        _ => (),
                    },
                    Event::Tick => {}
                },
                Err(error) => {
                    self.clean_up_terminal(Some(format!(
                        "{error} occured during receiving input!"
                    )));
                    break;
                }
            };

            if menu.wants_to_quit() {
                self.clean_up_terminal(None);
                break;
            }
        }
    }

    fn create_base_layout(render_frame: &mut Frame) -> Rc<[Rect]> {
        let size = render_frame.area();
        Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Min(2)].as_ref())
            .split(size)
    }

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

        match message {
            Some(message) => log::error!("{message}"),
            None => (),
        }
    }
}

fn create_floating_layout(width: u16, height: u16, base_chunk: Rect) -> Rect {
    let y_offset = 50 - height / 2;
    let x_offset = 50 - width / 2;

    let vertical_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(y_offset),
            Constraint::Percentage(height),
            Constraint::Percentage(y_offset),
        ])
        .split(base_chunk);

    let horizontal_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(x_offset),
            Constraint::Percentage(width),
            Constraint::Percentage(x_offset),
        ])
        .split(vertical_layout[1]);

    horizontal_layout[1]
}
