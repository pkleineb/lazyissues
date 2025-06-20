use std::{fmt::format, ops::Deref};

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::{list_view::ListItem, PanelElement, RepoData};

/// detail view name for `UiStack`
pub const DETAIL_VIEW_NAME: &str = "detail_view";

/// trait implementing some special functions for a detailed item
pub trait DetailItem: std::fmt::Debug {
    fn get_comments(&self) -> Vec<Box<dyn Comment>>;
}

/// trait for comments
pub trait Comment: std::fmt::Debug {
    fn get_author_login(&self) -> Option<&str>;
    fn get_created_at(&self) -> &str;
    fn get_body(&self) -> &str;
}

/// super trait, combining Detail and ListItem
pub trait DetailListItem: DetailItem + ListItem + Comment + Send {}

/// Widget for displaying details about an item(issue, pr or project)
#[derive(Default)]
pub struct DetailView {
    item: Option<Box<dyn DetailListItem>>,

    is_focused: bool,
}

impl DetailView {
    fn render_title(item: &dyn DetailListItem, render_frame: &mut Frame, area: Rect) {
        let title = item.get_title();
        let number_text = format!(" #{}", item.get_number());
        let vertical_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);

        let centered = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(
                    (title.len() + number_text.len())
                        .try_into()
                        .unwrap_or_default(),
                ),
                Constraint::Fill(1),
            ])
            .split(vertical_split[0]);

        let title_paragraph = Paragraph::new(Line::from(vec![
            Span::styled(title, Style::default().add_modifier(Modifier::UNDERLINED)),
            Span::styled(number_text, Style::default().fg(Color::DarkGray)),
        ]));
        render_frame.render_widget(title_paragraph, centered[1]);

        let spacer_bar = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Gray));

        render_frame.render_widget(spacer_bar, vertical_split[1]);
    }
}

impl PanelElement for DetailView {
    fn tick(&mut self) {}

    fn render(&mut self, render_frame: &mut ratatui::Frame, rect: ratatui::prelude::Rect) {
        let Some(ref unwrapped_item) = self.item else {
            return;
        };

        Self::render_title(unwrapped_item.deref(), render_frame, rect);
    }

    fn update(&mut self, data: RepoData) -> bool {
        match data {
            RepoData::ItemDetails(data) => {
                self.item = Some(data);
                true
            }
            other => {
                log::debug!(
                    "Received data wasn't of type RepoData::ItemDetails. Other value was: {other:?}",
                );
                false
            }
        }
    }

    fn set_focus(&mut self, state: bool) -> bool {
        self.is_focused = state;
        true
    }

    fn handle_input(&mut self, key_event: ratatui::crossterm::event::KeyEvent) -> bool {
        false
    }

    fn wants_to_quit(&self) -> bool {
        false
    }
}
