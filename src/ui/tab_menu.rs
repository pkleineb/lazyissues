use std::{error::Error, path::PathBuf, rc::Rc, sync::mpsc, thread};

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
    config::{self, git::get_git_repo_root, Config, State},
    graphql_requests::github::{issue_query, perform_issue_query},
    ui::PanelElement,
};

use super::{
    issues_view::{IssuesView, ISSUES_VIEW_NAME},
    remote_explorer::{RemoteExplorer, REMOTE_EXPLORER_NAME},
    UiStack,
};

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

pub enum RepoData {
    ActiveRemoteData(String),

    IssuesData(issue_query::ResponseData),
}

pub struct TabMenu {
    active_menu_item: MenuItem,

    layout_position: usize,

    data_receiver: mpsc::Receiver<RepoData>,
    data_clone_sender: mpsc::Sender<RepoData>,

    // this might be a stupid way to store this
    data_response_data: Vec<RepoData>,

    config: Config,
    state: State,

    repo_root: PathBuf,
    active_remote: Option<String>,

    ui_stack: UiStack,

    quit: bool,
}

impl TabMenu {
    pub fn new(layout_position: usize, config: Config) -> Result<Self, git2::Error> {
        let (data_clone_sender, data_receiver) = mpsc::channel();

        let state = match State::read() {
            Ok(state) => state,
            Err(error) => {
                log::error!(
                    "Error {} occured while fetching state. Using default state",
                    error
                );
                State::default()
            }
        };

        let repo_root = get_git_repo_root()?;
        let active_remote = state.get_repository_data(&repo_root);

        let mut tab_menu = Self {
            active_menu_item: MenuItem::Issues,
            layout_position,
            data_receiver,
            data_clone_sender,
            data_response_data: vec![],
            config,
            state,
            repo_root,
            active_remote,
            ui_stack: UiStack::new(),
            quit: false,
        };

        if tab_menu.active_remote.is_some() {
            match tab_menu.send_issue_request() {
                Err(error) => log::error!("{} occured during initial issue fetch request.", error),
                _ => (),
            }
        } else {
            tab_menu.open_remote_explorer()?;
        }

        Ok(tab_menu)
    }

    fn open_remote_explorer(&mut self) -> Result<(), git2::Error> {
        self.ui_stack.add_panel(
            RemoteExplorer::new(1, self.data_clone_sender.clone())?,
            self.ui_stack.get_highest_priority() + 1,
            REMOTE_EXPLORER_NAME,
        );

        Ok(())
    }

    fn send_issue_request(&self) -> Result<(), Box<dyn Error>> {
        if self.config.github_token.is_none() {
            log::info!("Github token not set.");
            return Ok(());
        }

        if self.active_remote.is_none() {
            log::info!("No active remote set for repository.");
            return Ok(());
        }

        let repo_regex = Regex::new(":(?<owner>.*)/(?<name>.*).git$")?;
        let active_remote = self
            .active_remote
            .as_ref()
            .expect("active_remote already checked");
        let Some(repo_captures) = repo_regex.captures(active_remote) else {
            return Err("Couldn't capture owner or name for issue_query".into());
        };

        let variables = issue_query::Variables {
            repo_name: repo_captures["name"].to_string(),
            repo_owner: repo_captures["owner"].to_string(),
        };

        let cloned_sender = self.data_clone_sender.clone();
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
                KeyCode::Char('q') => self.quit = true,
                _ => (),
            },
            KeyEvent {
                modifiers: KeyModifiers::SHIFT,
                ..
            } => match key_event.code {
                KeyCode::Char('I') => {
                    self.active_menu_item = MenuItem::Issues;
                    match self.send_issue_request() {
                        Err(error) => {
                            log::error!("{} occured during sending of issue request", error);
                        }
                        _ => (),
                    }
                }
                KeyCode::Char('P') => self.active_menu_item = MenuItem::PullRequests,
                KeyCode::Char('A') => self.active_menu_item = MenuItem::Actions,
                KeyCode::Char('R') => self.active_menu_item = MenuItem::Projects,
                _ => (),
            },
            KeyEvent {
                modifiers: KeyModifiers::CONTROL,
                ..
            } => match key_event.code {
                KeyCode::Char('n') => match self.open_remote_explorer() {
                    Err(error) => log::error!("{} occured while opening remote explorer!", error),
                    _ => (),
                },
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
        if let Ok(data) = self.data_receiver.try_recv() {
            self.data_response_data.push(data);
        }

        let mut should_refresh_issues = false;

        for data in self.data_response_data.drain(..) {
            match data {
                RepoData::IssuesData(data) => {
                    if self.active_menu_item != MenuItem::Issues {
                        continue;
                    };

                    match data.repository {
                        Some(repo_data) => {
                            let top_priority = self.ui_stack.get_highest_priority() + 1;
                            if let Some(panel) =
                                self.ui_stack.get_panel_mut_ref_by_name(ISSUES_VIEW_NAME)
                            {
                                panel.update(Box::new(repo_data));
                            } else {
                                self.ui_stack.add_panel(
                                    IssuesView::new(1, repo_data),
                                    top_priority,
                                    ISSUES_VIEW_NAME,
                                );
                            }
                        }
                        None => {
                            log::debug!("Couldn't display issues since there was no repository in response data")
                        }
                    }
                }
                RepoData::ActiveRemoteData(remote) => {
                    match self
                        .state
                        .set_repository_data(self.repo_root.clone(), remote.clone())
                    {
                        Err(error) => {
                            log::error!("{} occured during setting of active remote", error)
                        }
                        _ => (),
                    }
                    self.active_remote = Some(remote);
                    self.ui_stack.remove_panel_by_name(REMOTE_EXPLORER_NAME);

                    should_refresh_issues = true;
                }
            }
        }

        if should_refresh_issues {
            match self.send_issue_request() {
                Err(error) => log::error!(
                    "{} occured on issue fetch request after remote explorer closed.",
                    error
                ),
                _ => (),
            }
        }
    }

    fn update(&mut self, data: Box<dyn std::any::Any>) -> bool {
        false
    }

    fn wants_to_quit(&self) -> bool {
        self.quit
    }
}
