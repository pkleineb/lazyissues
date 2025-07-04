use std::{ops::Deref, rc::Rc};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListState, Paragraph, Wrap},
    Frame,
};

use crate::{config::Config, graphql_requests::github::types::DateTime};

use super::{list_view::ListItem, PanelElement, RepoData};

#[derive(PartialEq)]
enum ScrollDirection {
    Up,
    Down,
}

impl Default for ScrollDirection {
    fn default() -> Self {
        Self::Up
    }
}

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
    fn get_created_at(&self) -> &DateTime;

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
    comment_list_state: ListState,
    draw_height: usize,
    last_scroll_direction: ScrollDirection,

    config: Rc<Config>,
}

impl DetailView {
    pub fn new(config: Rc<Config>) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    fn select_next_item(&mut self) {
        if self.last_scroll_direction == ScrollDirection::Down {
            self.comment_list_state.select_next();
        } else {
            let selected_index = self.comment_list_state.selected().unwrap_or_default();

            self.comment_list_state
                .select(Some(selected_index + self.draw_height + 1));
            self.last_scroll_direction = ScrollDirection::Down;
        }
    }

    fn select_previous_item(&mut self) {
        if self.last_scroll_direction == ScrollDirection::Up {
            self.comment_list_state.select_previous();
        } else {
            let selected_index = self.comment_list_state.selected().unwrap_or_default();

            self.comment_list_state
                .select(Some(selected_index - self.draw_height - 1));
            self.last_scroll_direction = ScrollDirection::Up;
        }
    }

    /// renders the title of a `DetailListItem` trait item
    fn render_title(&self, item: &dyn DetailListItem, render_frame: &mut Frame, area: Rect) {
        let status_style = if item.is_closed() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };
        let status = if item.is_closed() { "✓" } else { "○" };
        let item_number = item.get_number();
        let item_title = item.get_title();

        let title = format!("[{status}] #{item_number} - {item_title}");

        let vertical_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);

        let margin = 3;

        let title_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(margin),
                Constraint::Length(title.len().try_into().unwrap_or_default()),
                Constraint::Fill(1),
                Constraint::Min(0),
                Constraint::Length(margin),
            ])
            .split(vertical_split[0]);

        let title_paragraph = Paragraph::new(Line::from(vec![Span::styled(title, status_style)]));
        render_frame.render_widget(title_paragraph, title_layout[1]);

        let spacer_bar = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Gray));

        render_frame.render_widget(spacer_bar, vertical_split[1]);

        let labels = item.get_labels();
        if !labels.is_empty() {
            let mut tags: Vec<Paragraph> = vec![];
            let mut constraints: Vec<Constraint> = vec![];

            for label in labels {
                let label_fmt = format!("[{label}]");
                constraints.push(Constraint::Length(label_fmt.len() as u16 + 2));
                tags.push(Paragraph::new(Span::styled(
                    label_fmt,
                    self.config.get_tag_color(&label),
                )));
            }

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(constraints)
                .flex(Flex::End)
                .spacing(1)
                .split(title_layout[3]);

            for (tag, chunk) in tags.iter().zip(chunks.iter()) {
                render_frame.render_widget(tag, *chunk);
            }
        }
    }

    /// renders the body of a `Comment` trait item
    fn render_body(&self, item: &dyn Comment, render_frame: &mut Frame, area: Rect) {
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
            item.get_created_at().to_str(self.config.get_datetime_fmt())
        );

        let title_paragraph = Paragraph::new(Span::styled(title, Style::default()));
        render_frame.render_widget(title_paragraph, layout[0]);

        let body_paragraph = Paragraph::new(Text::styled(item.get_body(), Style::default()))
            .wrap(Wrap { trim: false });
        render_frame.render_widget(body_paragraph, layout[1]);
    }

    /// creates the title line of a `Comment` trait item as a seperate line for use in
    /// `ratatui::widgets::List`
    fn create_comment_title_line<'a>(
        item: &'a dyn Comment,
        time_fmt: &'a str,
        action_graph_width: usize,
        comment_width: usize,
        is_last_action: bool,
    ) -> Line<'a> {
        let title = format!(
            "{} commented on {}",
            item.get_author_login().unwrap_or_default(),
            item.get_created_at().to_str(time_fmt)
        );
        let title_connection = if is_last_action { "╰" } else { "├" };
        let title_padding = Self::calculate_padding_for_text(&title, comment_width - 2); // -2 for the borders

        let line = Line::from(vec![
            Span::styled(title_connection, Style::default().fg(Color::DarkGray)),
            Span::styled(
                "─".repeat(action_graph_width - 1), // -1 since we draw the graph first
                Style::default().fg(Color::DarkGray),
            ),
            Span::from("│"),
            Span::styled(title, Style::default()),
            Span::from(" ".repeat(title_padding)),
            Span::from("│"),
        ]);

        line
    }

    /// creates a body of a `Comment` trait item as a seperate lines for use in
    /// `ratatui::widgets::List`
    fn create_comment_body(
        item: &dyn Comment,
        action_graph_width: usize,
        comment_width: usize,
        is_last_action: bool,
    ) -> Vec<Line<'_>> {
        let mut body_lines: Vec<Line> = vec![];

        let lines: Vec<_> = item
            .get_body()
            .lines()
            .flat_map(|paragraph| {
                let length = paragraph.len();

                let mut real_lines = vec![];
                let mut i = 0;
                while i + comment_width < length {
                    real_lines.push(&paragraph[i..i + comment_width]);
                    i += comment_width;
                }
                real_lines.push(&paragraph[i..]);

                real_lines
            })
            .collect();

        let action_graph = if is_last_action { " " } else { "│" };

        for line in lines {
            // -2 for the borders
            let line_padding = Self::calculate_padding_for_text(line, comment_width - 2);

            body_lines.push(Line::from(vec![
                Span::styled(action_graph, Style::default().fg(Color::DarkGray)),
                Span::from(" ".repeat(action_graph_width - 1)), // -1 since we draw the
                // graph first
                Span::from("│"),
                Span::styled(line, Style::default()),
                Span::from(" ".repeat(line_padding)),
                Span::from("│"),
            ]));
        }

        body_lines
    }

    /// creates the upper border of a `Comment` trait item as a seperate line for use in
    /// `ratatui::widgets::List`
    fn create_comment_upper_border(
        action_graph_width: usize,
        comment_width: usize,
    ) -> Line<'static> {
        let line = Line::from(vec![
            Span::styled("│", Style::default().fg(Color::DarkGray)),
            Span::styled(
                " ".repeat(action_graph_width - 1), // -1 since we draw the graph first
                Style::default().fg(Color::DarkGray),
            ),
            Span::from("╭"),
            Span::styled("─".repeat(comment_width - 2), Style::default()), // -2 for the
            // corners
            Span::from("╮"),
        ]);

        line
    }

    /// creates the lower border of a `Comment` trait item as a seperate line for use in
    /// `ratatui::widgets::List`
    fn create_comment_lower_border(
        action_graph_width: usize,
        comment_width: usize,
        is_last_action: bool,
    ) -> Line<'static> {
        let action_graph = if is_last_action { " " } else { "│" };

        let line = Line::from(vec![
            Span::styled(action_graph, Style::default().fg(Color::DarkGray)),
            Span::styled(
                " ".repeat(action_graph_width - 1), // -1 since we draw the graph first
                Style::default().fg(Color::DarkGray),
            ),
            Span::from("╰"),
            Span::styled("─".repeat(comment_width - 2), Style::default()), // -2 for the
            // corners
            Span::from("╯"),
        ]);

        line
    }

    /// calculates the height in lines of a given text within a given width
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

    /// calculates the padding of a given text so that `text.len() + padding == width`
    fn calculate_padding_for_text(text: &str, width: usize) -> usize {
        if text.len() > width {
            return 0;
        }

        width - text.len()
    }
}

impl PanelElement for DetailView {
    fn tick(&mut self) {}

    fn render(&mut self, render_frame: &mut ratatui::Frame, rect: ratatui::prelude::Rect) {
        let Some(ref unwrapped_item) = self.item else {
            return;
        };

        let padding = 5;
        let padded_width = rect.width - 2 * padding;

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Fill(1)])
            .split(rect);

        self.render_title(unwrapped_item.deref(), render_frame, main_layout[0]);

        let center_comment_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(padded_width),
                Constraint::Fill(1),
            ])
            .split(main_layout[1]);

        let main_comment_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(
                    Self::calculate_body_height(
                        unwrapped_item.get_body(),
                        (padded_width + 2).into(),
                    ) as u16
                        + 1
                        + 2,
                ),
                Constraint::Fill(1),
            ])
            .split(center_comment_layout[1]);

        self.render_body(unwrapped_item.deref(), render_frame, main_comment_layout[0]);

        let action_graph_width = 5;
        let comments = unwrapped_item.get_comments();
        let comment_width = main_comment_layout[1].width - action_graph_width;

        self.draw_height = main_comment_layout[1].height as usize;

        let comment_list = List::new(comments.iter().enumerate().flat_map(|(i, comment)| {
            let is_last_action = i == comments.len() - 1;
            let upper_border =
                Self::create_comment_upper_border(action_graph_width.into(), comment_width.into());
            let title_line = Self::create_comment_title_line(
                *comment,
                self.config.get_datetime_fmt(),
                action_graph_width.into(),
                comment_width.into(),
                is_last_action,
            );
            let mut body_lines = Self::create_comment_body(
                *comment,
                action_graph_width.into(),
                comment_width.into(),
                is_last_action,
            );
            let lower_border = Self::create_comment_lower_border(
                action_graph_width.into(),
                comment_width.into(),
                is_last_action,
            );

            let mut result = vec![upper_border, title_line];
            result.append(&mut body_lines);
            result.push(lower_border);

            result
        }));

        render_frame.render_stateful_widget(
            comment_list,
            main_comment_layout[1],
            &mut self.comment_list_state,
        );
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
        match key_event {
            KeyEvent {
                modifiers: KeyModifiers::CONTROL,
                ..
            } => match key_event.code {
                KeyCode::Char('j') => {
                    self.select_next_item();
                    true
                }
                KeyCode::Char('k') => {
                    self.select_previous_item();
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn wants_to_quit(&self) -> bool {
        false
    }
}
