use std::{
    fs::{self, File},
    io::{self, Write},
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, List, ListState},
    Frame,
};

use crate::{create_floating_layout, ui::PanelElement};

pub struct FileExplorer {
    current_path: PathBuf,
    path_mask: String,
    items: Vec<PathBuf>,
    state: ListState,

    layout_position: usize,

    cursor_flicker_delay: Duration,
    last_cursor_flicker: Instant,
    cursor_rendered_last_flicker: bool,
}

impl FileExplorer {
    pub fn new(layout_position: usize) -> io::Result<Self> {
        let current_path = std::env::current_dir()?;
        let mut explorer = Self {
            current_path,
            path_mask: String::from(""),
            items: Vec::new(),
            state: ListState::default(),

            layout_position,

            cursor_flicker_delay: Duration::from_millis(300),
            last_cursor_flicker: Instant::now(),
            cursor_rendered_last_flicker: false,
        };
        explorer.update_items()?;
        Ok(explorer)
    }

    fn items_as_str(&self) -> Vec<String> {
        self.items
            .iter()
            .map(|path| String::from(path.to_str().unwrap_or("")))
            .collect()
    }

    fn update_items(&mut self) -> io::Result<()> {
        self.items = fs::read_dir(&self.current_path)?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|entry| self.compare_entry_to_mask(entry.to_str().unwrap_or_default()))
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
                if path.to_str().unwrap_or_default() == ".." {
                    self.go_down_dir()?;
                } else if path.is_dir() {
                    self.current_path = path.clone();
                    self.clear_mask();
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
                if index == 0 {
                    self.items.len() - 1
                } else {
                    index - 1
                }
            }
            None => self.items.len() - 1,
        };
        self.state.select(Some(entry_index));
    }

    fn compare_entry_to_mask(&self, entry: &str) -> bool {
        if entry.contains(
            &(self.current_path.to_str().unwrap_or_default().to_owned() + "/" + &self.path_mask),
        ) {
            return true;
        }

        false
    }

    fn add_to_mask(&mut self, char: char) -> io::Result<()> {
        self.path_mask += &char.to_string();

        self.update_items()?;
        Ok(())
    }

    fn remove_from_mask(&mut self) -> io::Result<()> {
        if self.path_mask.len() == 0 {
            self.go_down_dir()?;
            return Ok(());
        }

        self.path_mask.remove(self.path_mask.len() - 1);

        self.update_items()?;
        Ok(())
    }

    fn go_down_dir(&mut self) -> io::Result<()> {
        match self.current_path.parent() {
            Some(parent_path) => {
                let new_mask = self
                    .current_path
                    .to_str()
                    .unwrap_or_default()
                    .split("/")
                    .last()
                    .unwrap_or_default();
                self.path_mask = String::from(new_mask);

                self.current_path = parent_path.to_path_buf();
            }
            _ => (),
        }

        self.update_items()?;

        Ok(())
    }

    fn clear_mask(&mut self) {
        self.path_mask.clear();
    }

    fn render_cursor(&mut self) -> &str {
        let should_switch_mode =
            Instant::now() - self.last_cursor_flicker > self.cursor_flicker_delay;

        if should_switch_mode {
            self.cursor_rendered_last_flicker = !self.cursor_rendered_last_flicker;
            self.last_cursor_flicker = Instant::now();
        }

        if self.cursor_rendered_last_flicker {
            return "_";
        } else {
            return " ";
        }
    }
}

impl PanelElement for FileExplorer {
    fn handle_input(&mut self, key_event: KeyEvent) -> bool {
        match key_event {
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                ..
            } => match key_event.code {
                KeyCode::Tab => self.next_entry(),
                KeyCode::Enter => match self.enter_dir() {
                    Err(error) => println!("{error} occured during switching directory!"),
                    _ => (),
                },
                KeyCode::Char(char) => match self.add_to_mask(char) {
                    Err(error) => println!("{error} occured during adding to mask!"),
                    _ => (),
                },
                KeyCode::Backspace => match self.remove_from_mask() {
                    Err(error) => println!("{error} occured during removing from mask!"),
                    _ => (),
                },
                _ => (),
            },
            KeyEvent {
                modifiers: KeyModifiers::SHIFT,
                ..
            } => match key_event.code {
                KeyCode::BackTab => self.previous_entry(),
                _ => (),
            },
            _ => (),
        }

        false
    }

    fn render(&mut self, render_frame: &mut Frame, layout: &Rc<[Rect]>) {
        let directory_items = self.items_as_str();

        let display_rect = List::new(directory_items)
            .highlight_style(Style::default().bg(Color::DarkGray))
            .block(
                Block::default()
                    .title(
                        self.current_path.to_str().unwrap_or("path").to_owned()
                            + "/"
                            + &self.path_mask
                            + self.render_cursor(),
                    )
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White));

        render_frame.render_stateful_widget(
            display_rect,
            create_floating_layout(50, 50, layout[self.layout_position]),
            &mut self.state,
        );
    }

    fn tick(&mut self) -> () {
        ()
    }
}
