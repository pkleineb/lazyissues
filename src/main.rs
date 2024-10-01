use std::{io, sync::mpsc, thread};

use ratatui::{crossterm::terminal::enable_raw_mode, prelude::CrosstermBackend, Terminal};
use rust_issue_handler::{EventLoop, TerminalApp};

fn main() {
    setup_terminal();
}

fn setup_terminal() {
    enable_raw_mode().expect("Can run in raw mode");

    let (sender, receiver) = mpsc::channel();
    let mut event_loop = EventLoop::new(sender);

    thread::spawn(move || event_loop.run());

    let app = TerminalApp::new(receiver);
    match app {
        Err(error) => println!("{error} occured during start of terminal app!"),
        Ok(mut app) => app.run(),
    }
}
