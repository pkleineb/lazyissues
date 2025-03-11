use std::{error::Error, rc::Rc, sync::mpsc, thread};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Tabs},
    Frame,
};
use regex::Regex;
use tokio::runtime::Runtime;

use crate::{
    config::{self, Config},
    graphql_requests::github::{issue_query, perform_issue_query},
    ui::PanelElement,
    Signal,
};

use super::{issues_view::IssuesView, UiStack};

#[derive(Hash, PartialEq, Eq)]
pub enum MenuItem {
    Issues,
    IssueView,
    PullRequests,
    PullRequestView,
    Actions,
    Projects,
    ProjectsView,
}

impl From<&MenuItem> for usize {
    fn from(input: &MenuItem) -> usize {
        match input {
            MenuItem::Issues | MenuItem::IssueView => 0,
            MenuItem::PullRequests | MenuItem::PullRequestView => 1,
            MenuItem::Actions => 2,
            MenuItem::Projects | MenuItem::ProjectsView => 3,
        }
    }
}

impl From<&MenuItem> for String {
    fn from(input: &MenuItem) -> String {
        match input {
            MenuItem::Issues | MenuItem::IssueView => "Issues".to_string(),
            MenuItem::PullRequests | MenuItem::PullRequestView => "Pull requests".to_string(),
            MenuItem::Actions => "Actions".to_string(),
            MenuItem::Projects | MenuItem::ProjectsView => "Projects".to_string(),
        }
    }
}

impl MenuItem {
    fn to_string_array() -> [String; 4] {
        return [
            "Issues".to_string(),
            "Pull requests".to_string(),
            "Actions".to_string(),
            "Projects".to_string(),
        ];
    }
}

pub enum QueryData {
    IssuesData(issue_query::ResponseData),
}

pub struct TabMenu {
    active_menu_item: MenuItem,

    layout_position: usize,

    query_receiver: mpsc::Receiver<(MenuItem, QueryData)>,
    query_clone_sender: mpsc::Sender<(MenuItem, QueryData)>,

    // this might be a stupid way to store this
    query_response_data: Vec<(MenuItem, QueryData)>,

    signal_sender: mpsc::Sender<Signal>,

    config: Config,

    ui_stack: UiStack,
}

impl TabMenu {
    pub fn new(
        layout_position: usize,
        signal_sender: mpsc::Sender<Signal>,
        config: Config,
    ) -> Self {
        let (query_clone_sender, query_receiver) = mpsc::channel();

        Self {
            active_menu_item: MenuItem::Issues,
            layout_position,
            query_receiver,
            query_clone_sender,
            query_response_data: vec![],
            signal_sender,
            config,
            ui_stack: UiStack::new(),
        }
    }

    fn send_issue_request(&self) -> Result<(), Box<dyn Error>> {
        if self.config.github_token.is_none() {
            log::info!("Github token not set.");
            return Ok(());
        }

        let active_remote = config::git::get_active_remote()?;

        let repo_regex = Regex::new(":(?<owner>.*)/(?<name>.*).git$")?;
        let Some(repo_captures) = repo_regex.captures(&active_remote) else {
            return Err("Couldn't capture owner or name for issue_query".into());
        };

        let variables = issue_query::Variables {
            repo_name: repo_captures["name"].to_string(),
            repo_owner: repo_captures["owner"].to_string(),
        };

        let cloned_sender = self.query_clone_sender.clone();
        let cloned_access_token = self
            .config
            .github_token
            .clone()
            .expect("Access token already checked");

        thread::spawn(move || match Runtime::new() {
            Ok(runtime) => {
                runtime.block_on(async {
                    match perform_issue_query(cloned_sender, variables, cloned_access_token).await {
                        Err(error) => log::error!("issue_query returned an error. {}", error),
                        _ => (),
                    }
                });
            }
            Err(error) => log::error!("Couldn't spawn runtime for issue_query. {}", error),
        });
        Ok(())
    }
}

impl PanelElement for TabMenu {
    fn handle_input(&mut self, key_event: KeyEvent) -> bool {
        for panel in self.ui_stack.iter_rev() {
            if panel.handle_input(key_event) {
                return true;
            }
        }

        match key_event {
            KeyEvent {
                modifiers: KeyModifiers::NONE,
                ..
            } => match key_event.code {
                KeyCode::Char('q') => {
                    let _ = self.signal_sender.send(Signal::Quit);
                }
                _ => (),
            },
            KeyEvent {
                modifiers: KeyModifiers::SHIFT,
                ..
            } => match key_event.code {
                KeyCode::Char('I') => {
                    self.active_menu_item = MenuItem::Issues;
                    self.send_issue_request();
                }
                KeyCode::Char('P') => self.active_menu_item = MenuItem::PullRequests,
                KeyCode::Char('A') => self.active_menu_item = MenuItem::Actions,
                KeyCode::Char('R') => self.active_menu_item = MenuItem::Projects,
                _ => (),
            },
            _ => (),
        }

        false
    }

    fn render(&mut self, render_frame: &mut Frame, layout: &Rc<[Rect]>) -> () {
        let menu_string_items = MenuItem::to_string_array();
        let menu: Vec<Line> = menu_string_items
            .iter()
            .map(|title| {
                let (first, rest) = title.split_at(1);
                Line::from(vec![
                    Span::styled(
                        first,
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::UNDERLINED),
                    ),
                    Span::styled(rest, Style::default().fg(Color::White)),
                ])
            })
            .collect();

        let tabs = Tabs::new(menu)
            .select((&self.active_menu_item).into())
            .block(
                Block::default()
                    .title(String::from(&self.active_menu_item))
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Red))
            .divider(Span::raw("|"));

        render_frame.render_widget(tabs, layout[self.layout_position]);

        for panel in self.ui_stack.iter() {
            panel.render(render_frame, &layout)
        }
    }

    fn tick(&mut self) -> () {
        // try_recv does not block the current thread which is nice here because we don't
        // have a tick signal recv() would block the thread until we receive a message from
        // the sender I am ignoring the error here but that may not be best practice
        if let Ok(data) = self.query_receiver.try_recv() {
            let (key, value) = data;
            self.query_response_data.push((key, value));
        }

        for (menu_item, query_data) in self.query_response_data.drain(..) {
            if menu_item != self.active_menu_item {
                continue;
            }

            match query_data {
                QueryData::IssuesData(data) => match data.repository {
                    Some(repo_data) => {
                        let top_priority = self.ui_stack.get_highest_priority() + 1;
                        self.ui_stack
                            .add_panel(IssuesView::new(1, repo_data), top_priority);
                    }
                    None => log::debug!("Couldn't display issues since there was no repository"),
                },
            }
        }
    }
}
