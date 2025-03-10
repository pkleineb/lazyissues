use std::{error::Error, sync::mpsc, thread};

use lazyissues::{enable_logging, EventLoop, TerminalApp};
use ratatui::crossterm::terminal::enable_raw_mode;

fn main() -> Result<(), Box<dyn Error>> {
    enable_logging()?;
    setup_terminal();
    Ok(())
}

fn setup_terminal() {
    enable_raw_mode().expect("Can run in raw mode");

    let (sender, receiver) = mpsc::channel();
    let mut event_loop = EventLoop::new(sender);

    thread::spawn(move || event_loop.run());

    let app = TerminalApp::new(receiver);
    match app {
        Err(error) => log::error!("{error} occured during start of terminal app!"),
        Ok(mut app) => app.run(),
    }
}
