use std::{cmp::max, rc::Rc, sync::mpsc};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::{
    config::Config,
    graphql_requests::github::{
        issues_query, projects_query, pull_requests_query, IssuesCollection, ProjectsCollection,
        PullRequestsCollection,
    },
};

use super::{ItemDetailFunc, PanelElement, RepoData};

/// issues view name for the `UiStack`
pub const ISSUES_VIEW_NAME: &str = "issues_view";
/// pull requests view name for the `UiStack`
pub const PULL_REQUESTS_VIEW_NAME: &str = "pull_requests_view";
/// projects view name for the `UiStack`
pub const PROJECTS_VIEW_NAME: &str = "projects_view";

/// trait for items to be displayed in a `ListView`
pub trait ListItem: std::fmt::Debug {
    /// returns the title of that item
    fn get_title(&self) -> &str;
    /// returns the number of that item
    fn get_number(&self) -> i64;
    /// check wether or not the item is closed
    fn is_closed(&self) -> bool;
    /// returns the author login(username) of that item
    fn get_author_login(&self) -> Option<&str>;
    /// returns the timestamp of creation of that item
    fn get_created_at(&self) -> &str;
    /// returns all labels of that item
    fn get_labels(&self) -> Vec<String>;
}

/// trait for remote data to be used as a collection of `ListItem`s
pub trait ListCollection {
    /// converts the data given to a `ListCollection` implementing struct
    fn from_repository_data(data: RepoData) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized;
    /// returns all items of the `ListCollection`
    fn get_items(&self) -> Vec<Box<dyn ListItem>>;

    /// fetches the detail info for the specific type for displaying
    fn get_detail_func() -> ItemDetailFunc;
}

/// displays `ListItem`s
pub struct ListView<T: ListCollection> {
    collection: T,
    item_amount: usize,
    selected_item: usize,
    config: Rc<Config>,

    is_focused: bool,

    changed_selected_item: bool,
    data_sender_cloner: mpsc::Sender<RepoData>,
}

impl<T: ListCollection> ListView<T> {
    /// creates a new `ListView<T>` based on the ListCollection given
    pub fn new(
        collection: T,
        config: Rc<Config>,
        data_sender_cloner: mpsc::Sender<RepoData>,
    ) -> Self {
        let item_amount = collection.get_items().len();
        Self {
            collection,
            item_amount,
            selected_item: 0,
            config,

            is_focused: false,

            changed_selected_item: false,
            data_sender_cloner,
        }
    }

    /// selects the next item, wrapping on the edges
    fn select_next_item(&mut self) {
        // usize will probably not be exceeded
        self.selected_item = self.selected_item.saturating_add(1);
        if self.selected_item >= self.item_amount {
            self.selected_item = 0;
        }

        self.changed_selected_item = true;
    }

    /// selects the previous item, wrapping on the edges
    fn select_previous_item(&mut self) {
        // usize will probably not be exceeded
        if self.selected_item == 0 {
            self.selected_item = self.item_amount.saturating_sub(1);
        } else {
            self.selected_item -= 1;
        }

        self.changed_selected_item = true;
    }

    /// displays a singular item on it's asigned area
    fn display_item(
        &self,
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
        let item_number = item.get_number();
        let item_title = item.get_title();

        let item_style = if is_highlighted && self.is_focused {
            Style::default().bg(Color::Rgb(120, 120, 120))
        } else {
            Style::default()
        };

        let outer_block = Block::default().borders(Borders::NONE).style(item_style);

        let inner_area = outer_block.inner(area);
        render_frame.render_widget(outer_block, area);

        let title = format!("[{status}] #{item_number} - {item_title}");

        let created_at = item.get_created_at();
        let author_name = item.get_author_login().unwrap_or("");
        let lower_issue_info = format!("{author_name} @ {created_at}");

        let horizontal_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(
                    max(title.len(), lower_issue_info.len())
                        .try_into()
                        .unwrap_or(30), // default here should be fine might create a seperate
                                        // constant
                ),
                Constraint::Length(2), // spacer
                Constraint::Fill(1),
            ])
            .split(inner_area);

        let info_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(horizontal_split[0]);

        let title_paragraph = Paragraph::new(Span::styled(title, status_style));
        render_frame.render_widget(title_paragraph, info_chunks[0]);

        let lower_issue_info_paragraph =
            Paragraph::new(Span::styled(lower_issue_info, Style::default()));
        render_frame.render_widget(lower_issue_info_paragraph, info_chunks[1]);

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
                .flex(Flex::Start)
                .spacing(1)
                .split(horizontal_split[2]);

            for (tag, chunk) in tags.iter().zip(chunks.iter()) {
                render_frame.render_widget(tag, *chunk);
            }
        }
    }
}

impl<T: ListCollection> PanelElement for ListView<T> {
    fn handle_input(&mut self, key_event: KeyEvent) -> bool {
        match key_event {
            KeyEvent {
                modifiers: KeyModifiers::NONE,
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

    fn tick(&mut self) {
        if self.changed_selected_item {
            if let Err(error) = self.data_sender_cloner.send(RepoData::ViewItemDetails(
                self.collection.get_items()[self.selected_item]
                    .get_number()
                    .try_into()
                    .unwrap_or_default(),
                T::get_detail_func(),
            )) {
                log::error!("While sending view detail request to ui experienced error: {error}");
            }
            self.changed_selected_item = false;
        }
    }

    fn render(&mut self, render_frame: &mut Frame, rect: Rect) {
        render_frame.render_widget(Clear, rect);

        let items = self.collection.get_items();

        if items.is_empty() {
            return;
        }

        let mut constraints: Vec<Constraint> = vec![];
        for _ in 0..items.len() {
            constraints.push(Constraint::Length(2));
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(rect);

        for (i, (item, chunk)) in items.iter().zip(chunks.iter()).enumerate() {
            let is_highlighted = i == self.selected_item;
            self.display_item(item.as_ref(), render_frame, *chunk, is_highlighted);
        }
    }

    fn update(&mut self, data: RepoData) -> bool {
        // try to construct the generic T from data received from the git remote
        if let Ok(collection) = T::from_repository_data(data) {
            self.collection = collection;
            self.item_amount = self.collection.get_items().len();

            // we expect the git remotes to return items ordered so we can keep the selected item
            // if there isn't less for some reason at any point
            self.selected_item = if self.selected_item < self.item_amount {
                self.selected_item
            } else if self.item_amount > 0 {
                self.item_amount - 1
            } else {
                0
            };

            self.changed_selected_item = true;

            return true;
        }

        false
    }

    // since this panel should never close we always return false
    fn wants_to_quit(&self) -> bool {
        false
    }

    fn set_focus(&mut self, state: bool) -> bool {
        self.is_focused = state;
        true
    }
}

/// quickly creates an widgets where you can view issues on
pub fn create_issues_view(
    data: issues_query::IssuesQueryRepository,
    config: Rc<Config>,
    data_sender: mpsc::Sender<RepoData>,
) -> impl PanelElement {
    let collection = IssuesCollection::new(data);
    ListView::new(collection, config, data_sender)
}

/// quickly creates an widgets where you can view pull requests on
pub fn create_pull_requests_view(
    data: pull_requests_query::PullRequestsQueryRepository,
    config: Rc<Config>,
    data_sender: mpsc::Sender<RepoData>,
) -> impl PanelElement {
    let collection = PullRequestsCollection::new(data);
    ListView::new(collection, config, data_sender)
}

/// quickly creates an widgets where you can view projects on
pub fn create_projects_view(
    data: projects_query::ProjectsQueryRepository,
    config: Rc<Config>,
    data_sender: mpsc::Sender<RepoData>,
) -> impl PanelElement {
    let collection = ProjectsCollection::new(data);
    ListView::new(collection, config, data_sender)
}
