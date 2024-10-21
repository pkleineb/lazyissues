// started porting everything over to use traits so that i can generalise and also attribute the
// key inputs to any panel; next up fix all the errors

use std::{
    io,
    rc::Rc,
    result::Result,
    sync::mpsc,
    time::{Duration, Instant},
};

use ratatui::{
    crossterm::{
        event::{self, Event as CrossEvent, KeyCode},
        terminal::disable_raw_mode,
    },
    layout::{Constraint, Direction, Layout, Rect},
    prelude::CrosstermBackend,
    Frame, Terminal,
};
use ui::UiStack;

mod config;
mod file_explorer;
mod graphql_requests;
mod tab_menu;
mod ui;

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
}

impl TerminalApp {
    pub fn new(input_receiver: mpsc::Receiver<Event<CrossEvent>>) -> Result<Self, std::io::Error> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            input_receiver,
            terminal,
        })
    }

    pub fn run(&mut self) {
        if let Err(error) = self.terminal.clear() {
            println!("{error} occured during terminal clearing");
            return;
        }

        let mut config = match config::read_config() {
            Ok(Some(config)) => config,
            Ok(None) => config::Config::new(),
            Err(error) => {
                println!("{error} occured while reading config! Using default config.");
                config::Config::new()
            }
        };

        let mut ui_stack = Self::create_ui(UiStack::new());

        loop {
            let _ = self.terminal.draw(|render_frame| {
                let layout = Self::create_base_layout(render_frame);

                for panel in ui_stack.iter() {
                    panel.render(render_frame, &layout)
                }
            });

            match self.input_receiver.recv() {
                Ok(event) => match event {
                    Event::Input(event) => match event {
                        CrossEvent::Key(key) => match key.code {
                            KeyCode::Char('q') => {
                                self.clean_up_terminal(None);
                                break;
                            }
                            _ => (), // use event here
                        },
                        _ => (),
                    },
                    Event::Tick => {
                        for panel in ui_stack.iter_rev() {
                            panel.tick();
                        }
                    }
                },
                Err(error) => {
                    self.clean_up_terminal(Some(format!(
                        "{error} occured during receiving input!"
                    )));
                    break;
                }
            };
        }
    }

    fn create_ui(mut ui_stack: UiStack) -> UiStack {
        let tab_menu = tab_menu::TabMenu::new(0);
        ui_stack.add_panel(tab_menu, 10);

        match file_explorer::FileExplorer::new(1) {
            Ok(explorer) => ui_stack.add_panel(explorer, 0),
            Err(error) => println!("{error} occured during creation of file explorer!"),
        }

        ui_stack
    }

    fn create_base_layout(render_frame: &mut Frame) -> Rc<[Rect]> {
        let size = render_frame.area();
        Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(2)].as_ref())
            .split(size)
    }

    fn clean_up_terminal(&mut self, message: Option<String>) {
        if let Err(error) = disable_raw_mode() {
            println!("{error} occured when trying to exit raw mode!");
        }
        if let Err(error) = self.terminal.show_cursor() {
            println!("{error} occured when trying to show cursor!");
        }

        match message {
            Some(message) => println!("{message}"),
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
