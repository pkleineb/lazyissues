use std::{fs, io, path::PathBuf};

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, List, ListState},
    Frame,
};

use crate::create_floating_layout;

pub struct FileExplorer {
    current_path: PathBuf,
    items: Vec<PathBuf>,
    state: ListState,
}

impl FileExplorer {
    pub fn new() -> Self {
        // i dont really like this error handling since it should be passed to the TerminalApp but
        // mhm
        let current_path = match std::env::current_dir() {
            Ok(path) => path,
            Err(error) => {
                println!("{error} occured while getting cwd!");
                PathBuf::from("")
            }
        };
        let mut explorer = Self {
            current_path,
            items: Vec::new(),
            state: ListState::default(),
        };
        // i dont really like this error handling since it should be passed to the TerminalApp but
        // mhm
        match explorer.update_items() {
            Ok(_) => (),
            Err(error) => println!("{error} occured while updating items!"),
        }
        explorer
    }

    pub fn render(&self, render_frame: &mut Frame, chunk: Rect) {
        let directory_items = self.items_as_str();

        let display_rect = List::new(directory_items)
            .block(
                Block::default()
                    .title(self.current_path.to_str().unwrap_or("path"))
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White));

        render_frame.render_widget(display_rect, create_floating_layout(50, 50, chunk));
    }

    fn items_as_str(&self) -> Vec<&str> {
        self.items
            .iter()
            .map(|path| path.to_str().unwrap_or(""))
            .collect()
    }

    fn update_items(&mut self) -> io::Result<()> {
        self.items = fs::read_dir(&self.current_path)?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .collect();
        self.items.sort();
        self.items.insert(0, "..".into());
        self.state.select(Some(0));
        Ok(())
    }

    fn enter_dir(&mut self) -> io::Result<()> {
        match self.state.selected() {
            Some(selected) => {
                let path = &self.items[selected];
                if path.is_dir() {
                    self.current_path = path.clone();
                    self.update_items()?;
                }
                Ok(())
            }
            None => Ok(()),
        }
    }

    fn next_entry(&mut self) {
        let entry_index = match self.state.selected() {
            Some(index) => {
                if index >= self.items.len() - 1 {
                    0
                } else {
                    index + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(entry_index));
    }

    fn previous_entry(&mut self) {
        let entry_index = match self.state.selected() {
            Some(index) => {
                if index < 0 {
                    self.items.len() - 1
                } else {
                    index - 1
                }
            }
            None => self.items.len() - 1,
        };
        self.state.select(Some(entry_index));
    }
}
