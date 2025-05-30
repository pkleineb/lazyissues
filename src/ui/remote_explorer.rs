use std::{
    rc::Rc,
    sync::mpsc,
    time::{Duration, Instant},
};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListState},
    Frame,
};

use crate::{
    config::{self, git::get_git_remote_url_for_name},
    ui::create_floating_layout,
    ui::PanelElement,
};

use super::tab_menu::RepoData;

pub const REMOTE_EXPLORER_NAME: &str = "remote_explorer";

pub struct RemoteExplorer {
    remote_mask: String,
    items: Vec<String>,
    state: ListState,

    cursor_flicker_delay: Duration,
    last_cursor_flicker: Instant,
    cursor_rendered_last_flicker: bool,

    remote_sender: mpsc::Sender<RepoData>,

    quit: bool,
    is_focused: bool,
}

impl RemoteExplorer {
    pub fn new(remote_sender: mpsc::Sender<RepoData>) -> Result<Self, git2::Error> {
        let mut explorer = Self {
            remote_mask: String::from(""),
            items: Vec::new(),
            state: ListState::default(),

            cursor_flicker_delay: Duration::from_millis(300),
            last_cursor_flicker: Instant::now(),
            cursor_rendered_last_flicker: false,

            remote_sender,

            quit: false,
            is_focused: false,
        };
        explorer.update_items()?;
        Ok(explorer)
    }

    fn update_items(&mut self) -> Result<(), git2::Error> {
        self.items = config::git::get_remote_names()?
            .into_iter()
            .filter(|remote_name| self.compare_entry_to_mask(remote_name))
            .collect();

        self.items.sort();
        self.state.select(Some(0));
        Ok(())
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
        if entry.contains(&self.remote_mask) {
            return true;
        }

        false
    }

    fn add_to_mask(&mut self, char: char) -> Result<(), Box<dyn std::error::Error>> {
        self.remote_mask += &char.to_string();
        self.update_items()?;
        Ok(())
    }

    fn remove_from_mask(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.remote_mask.len() == 0 {
            return Ok(());
        }

        self.remote_mask.remove(self.remote_mask.len() - 1);

        self.update_items()?;
        Ok(())
    }

    fn clear_mask(&mut self) {
        self.remote_mask.clear();
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

    fn select_remote(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.state.selected() {
            Some(index) => match self.items.get(index) {
                Some(selected_remote) => {
                    let remote_url = get_git_remote_url_for_name(&selected_remote)?;

                    self.remote_sender
                        .send(RepoData::ActiveRemoteData(remote_url))?;

                    self.quit = true;

                    Ok(())
                }
                None => Err("Selected index of remote is out of bounds.".into()),
            },
            None => Err("Tried to select remote while there was no selection.".into()),
        }
    }
}

impl PanelElement for RemoteExplorer {
    fn handle_input(&mut self, key_event: KeyEvent) -> bool {
        match key_event {
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                ..
            } => match key_event.code {
                KeyCode::Tab => self.next_entry(),
                KeyCode::Enter => match self.select_remote() {
                    Err(error) => log::error!("{} occured on selecting remote!", error),
                    _ => (),
                },
                KeyCode::Char(char) => match self.add_to_mask(char) {
                    Err(error) => log::error!("{} occured during adding to mask!", error),
                    _ => (),
                },
                KeyCode::Backspace => match self.remove_from_mask() {
                    Err(error) => log::error!("{} occured on removing from mask!", error),
                    _ => (),
                },
                KeyCode::Esc => self.quit = true,
                _ => (),
            },
            KeyEvent {
                modifiers: KeyModifiers::SHIFT,
                ..
            } => match key_event.code {
                KeyCode::BackTab => self.previous_entry(),
                KeyCode::Char(char) => match self.add_to_mask(char) {
                    Err(error) => log::error!("{} occured during adding to mask!", error),
                    _ => (),
                },
                _ => (),
            },
            _ => (),
        }

        true
    }

    fn render(&mut self, render_frame: &mut Frame, rect: Rect) {
        let remotes = self.items.clone();

        let floating_area = create_floating_layout(20, 20, rect);
        render_frame.render_widget(Clear, floating_area);

        let display_rect = List::new(remotes)
            .highlight_style(Style::default().bg(Color::DarkGray))
            .block(
                Block::default()
                    .title(format!(
                        " Remotes: {}{} ",
                        self.remote_mask.to_owned(),
                        self.render_cursor()
                    ))
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White));

        render_frame.render_stateful_widget(display_rect, floating_area, &mut self.state);
    }

    fn tick(&mut self) -> () {
        ()
    }

    fn update(&mut self, _data: Box<dyn std::any::Any>) -> bool {
        false
    }

    fn wants_to_quit(&self) -> bool {
        self.quit
    }

    fn set_focus(&mut self, state: bool) -> bool {
        self.is_focused = state;
        true
    }
}
