use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::graphql_requests::github::{
    issues_query, pull_requests_query, IssuesCollection, PullRequestsCollection,
};

use super::PanelElement;

pub const ISSUES_VIEW_NAME: &str = "issues_view";
pub const PULL_REQUESTS_VIEW_NAME: &str = "pull_requests_view";

pub trait ListItem: std::fmt::Debug {
    fn get_title(&self) -> &str;
    fn get_number(&self) -> i64;
    fn is_closed(&self) -> bool;
    fn get_author_login(&self) -> Option<&str>;
    fn get_created_at(&self) -> &str;
    fn get_labels(&self) -> Vec<String>;
}

pub trait ListCollection {
    fn get_items(&self) -> Vec<Box<dyn ListItem>>;
}

pub struct ListView<T: ListCollection + 'static> {
    layout_position: usize,
    collection: T,
    item_amount: usize,
    selected_item: usize,
}

impl<T: ListCollection + 'static> ListView<T> {
    pub fn new(layout_position: usize, collection: T) -> Self {
        let item_amount = collection.get_items().len();
        Self {
            layout_position,
            collection,
            item_amount,
            selected_item: 0,
        }
    }

    fn select_next_item(&mut self) {
        self.selected_item += 1;
        if self.selected_item >= self.item_amount {
            self.selected_item = 0;
        }
    }

    fn select_previous_item(&mut self) {
        if self.selected_item == 0 {
            self.selected_item = self.item_amount.saturating_sub(1);
            return;
        }
        self.selected_item -= 1;
    }

    fn display_item(
        item: &dyn ListItem,
        render_frame: &mut Frame,
        area: Rect,
        is_highlighted: bool,
    ) {
        let status_style = if item.is_closed() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };
        let status = if item.is_closed() { "✓" } else { "○" };

        let item_style = if is_highlighted {
            Style::default().fg(Color::LightGreen)
        } else {
            Style::default()
        };

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .style(item_style)
            .title(format!(
                "[{} #{} {}]",
                status,
                item.get_number(),
                item.get_title()
            ))
            .title_style(status_style);
        let inner_area = outer_block.inner(area);
        render_frame.render_widget(outer_block, area);

        let info_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(5),
            ])
            .split(inner_area);

        if let Some(author) = item.get_author_login() {
            let author_paragraph = Paragraph::new(Span::styled(author, Style::default()));
            render_frame.render_widget(author_paragraph, info_chunks[0]);
        }

        let time_paragraph = Paragraph::new(Span::styled(
            item.get_created_at(),
            Style::default().fg(Color::Gray),
        ));
        render_frame.render_widget(time_paragraph, info_chunks[1]);

        let labels = item.get_labels();
        if !labels.is_empty() {
            let mut tags: Vec<Paragraph> = vec![];
            let mut constraints: Vec<Constraint> = vec![];

            for label in labels {
                constraints.push(Constraint::Length(label.len() as u16 + 2));
                tags.push(
                    Paragraph::new(Span::styled(label, Style::default()))
                        .block(Block::new().borders(Borders::ALL)),
                );
            }

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(constraints)
                .flex(Flex::Start)
                .split(info_chunks[2]);

            for (tag, chunk) in tags.iter().zip(chunks.iter()) {
                render_frame.render_widget(tag, *chunk);
            }
        }
    }
}

impl<T: ListCollection + 'static> PanelElement for ListView<T> {
    fn handle_input(&mut self, key_event: KeyEvent) -> bool {
        match key_event {
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                ..
            } => match key_event.code {
                KeyCode::Tab => {
                    self.select_next_item();
                    false
                }
                _ => false,
            },
            KeyEvent {
                modifiers: KeyModifiers::SHIFT,
                ..
            } => match key_event.code {
                KeyCode::BackTab => {
                    self.select_previous_item();
                    false
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn tick(&mut self) -> () {}

    fn render(&mut self, render_frame: &mut Frame, layout: &std::rc::Rc<[Rect]>) -> () {
        let render_area = layout[self.layout_position];

        render_frame.render_widget(Clear, render_area);

        let items = self.collection.get_items();

        if items.is_empty() {
            return;
        }

        let mut constraints: Vec<Constraint> = vec![];
        for _ in 0..items.len() {
            constraints.push(Constraint::Length(7));
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(render_area);

        for (i, (item, chunk)) in items.iter().zip(chunks.iter()).enumerate() {
            let is_highlighted = i == self.selected_item;
            Self::display_item(item.as_ref(), render_frame, *chunk, is_highlighted);
        }
    }

    fn update(&mut self, data: Box<dyn std::any::Any>) -> bool {
        if let Ok(collection) = data.downcast::<T>() {
            self.collection = *collection;
            self.item_amount = self.collection.get_items().len();

            self.selected_item = if self.selected_item < self.item_amount {
                self.selected_item
            } else if self.item_amount > 0 {
                self.item_amount - 1
            } else {
                0
            };

            return true;
        }

        false
    }

    fn wants_to_quit(&self) -> bool {
        false
    }
}

pub fn create_issues_view(
    layout_position: usize,
    data: issues_query::IssuesQueryRepository,
) -> impl PanelElement {
    let collection = IssuesCollection::new(data);
    ListView::new(layout_position, collection)
}

// Example of creating a pull requests view
pub fn create_pull_requests_view(
    layout_position: usize,
    data: pull_requests_query::PullRequestsQueryRepository,
) -> impl PanelElement {
    let collection = PullRequestsCollection::new(data);
    ListView::new(layout_position, collection)
}
