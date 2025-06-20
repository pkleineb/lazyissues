use std::{fmt::format, ops::Deref};

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::{list_view::ListItem, PanelElement, RepoData};

/// detail view name for `UiStack`
pub const DETAIL_VIEW_NAME: &str = "detail_view";

/// trait implementing some special functions for a detailed item
pub trait DetailItem: std::fmt::Debug {
    /// returns the number of comments on a DetailItem(this is at the max 100 since the request
    /// only allows 100 fetches here)
    fn get_num_comments(&self) -> usize;

    /// returns all fetched comments on the trait
    fn get_comments(&self) -> Vec<&dyn Comment>;
}

/// trait for comments
pub trait Comment: std::fmt::Debug {
    /// returns the login of the author of the `Comment`
    fn get_author_login(&self) -> Option<&str>;

    /// returns the time the `Comment` got created
    fn get_created_at(&self) -> &str;

    /// returns the body(text) of the `Comment`
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

    fn render_body(item: &dyn Comment, render_frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray));

        let inner_area = block.inner(area);
        render_frame.render_widget(block, area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Fill(1)])
            .split(inner_area);

        let title = format!(
            "{} commented on {}",
            item.get_author_login().unwrap_or_default(),
            item.get_created_at()
        );

        let title_paragraph = Paragraph::new(Span::styled(title, Style::default()));
        render_frame.render_widget(title_paragraph, layout[0]);

        let body_paragraph = Paragraph::new(Span::styled(item.get_body(), Style::default()))
            .wrap(Wrap { trim: false });
        render_frame.render_widget(body_paragraph, layout[1]);
    }

    fn calculate_body_height(text: &str, width: usize) -> usize {
        let mut lines = 0;

        for paragraph in text.lines() {
            if paragraph.is_empty() {
                lines += 1;
                continue;
            }

            let line_amount = paragraph.len().div_ceil(width);
            lines += line_amount;
        }

        lines
    }
}

impl PanelElement for DetailView {
    fn tick(&mut self) {}

    fn render(&mut self, render_frame: &mut ratatui::Frame, rect: ratatui::prelude::Rect) {
        let Some(ref unwrapped_item) = self.item else {
            return;
        };

        let padding = 10;
        let padded_width = rect.width - 2 * padding;

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Fill(1)])
            .split(rect);

        Self::render_title(unwrapped_item.deref(), render_frame, main_layout[0]);

        let center_comment_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(padded_width),
                Constraint::Fill(1),
            ])
            .split(main_layout[1]);

        let mut comments = unwrapped_item.get_comments();
        comments.insert(0, unwrapped_item.deref() as &dyn Comment);

        let constraints: Vec<_> = comments
            .iter()
            .map(|comment| {
                Constraint::Length(
                    Self::calculate_body_height(comment.get_body(), (padded_width + 2).into()) // +2 for the borders left and right
                        as u16
                        + 1  // +1 for the title where created at and author goes and 
                        + 2, // +2 for the borders up and down
                )
            })
            .collect();

        let comment_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .spacing(1)
            .split(center_comment_layout[1]);

        for (comment, area) in comments.iter().zip(comment_layout.iter()) {
            Self::render_body(*comment, render_frame, *area);
        }
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
