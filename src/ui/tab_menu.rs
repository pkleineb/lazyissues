use std::{collections::HashMap, error::Error, path::PathBuf, sync::mpsc, thread};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear},
    Frame,
};
use regex::Regex;
use tokio::runtime::Runtime;

use crate::{
    config::{git::get_git_repo_root, Config, State},
    graphql_requests::github::{
        issue_detail_query, issues_query, perform_issues_query, perform_projects_query,
        perform_pull_requests_query, projects_query, pull_requests_query, VariableStore,
    },
    ui::PanelElement,
};

use super::{
    list_view::{
        create_issues_view, create_projects_view, create_pull_requests_view, ISSUES_VIEW_NAME,
        PROJECTS_VIEW_NAME, PULL_REQUESTS_VIEW_NAME,
    },
    remote_explorer::{RemoteExplorer, REMOTE_EXPLORER_NAME},
    UiStack,
};

pub const ISSUES_LAYOUT_POSITION: usize = 0;
pub const PULL_REQUESTS_LAYOUT_POSITION: usize = 1;
pub const PROJECTS_LAYOUT_POSITION: usize = 2;
pub const DETAIL_LAYOUT_POSITION: usize = 0;
pub const STATUS_LAYOUT_POSITION: usize = 1;

#[derive(Hash, PartialEq, Eq)]
pub enum MenuItem {
    Issues,
    PullRequests,
    Projects,
}

impl From<&MenuItem> for usize {
    fn from(input: &MenuItem) -> usize {
        match input {
            MenuItem::Issues => 0,
            MenuItem::PullRequests => 1,
            MenuItem::Projects => 2,
        }
    }
}

impl From<&MenuItem> for String {
    fn from(input: &MenuItem) -> String {
        match input {
            MenuItem::Issues => "Issues".to_string(),
            MenuItem::PullRequests => "Pull requests".to_string(),
            MenuItem::Projects => "Projects".to_string(),
        }
    }
}

impl MenuItem {
    fn to_main_menu_points_str() -> [&'static str; 3] {
        return ["Issues", "Pull requests", "Projects"];
    }

    fn to_main_menu_points() -> [MenuItem; 3] {
        return [MenuItem::Issues, MenuItem::PullRequests, MenuItem::Projects];
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RequestType {
    IssuesRequest,
    PullRequestsRequest,
    ProjectsRequest,
}

impl RequestType {
    fn iter() -> impl Iterator<Item = &'static RequestType> {
        [
            RequestType::IssuesRequest,
            RequestType::PullRequestsRequest,
            RequestType::ProjectsRequest,
        ]
        .iter()
    }

    fn to_str(&self) -> &'static str {
        match self {
            RequestType::IssuesRequest => "IssuesRequest",
            RequestType::PullRequestsRequest => "PullRequestsRequest",
            RequestType::ProjectsRequest => "ProjectsRequest",
        }
    }
}

pub enum RepoData {
    ActiveRemoteData(String),

    IssuesData(issues_query::ResponseData),
    PullRequestsData(pull_requests_query::ResponseData),
    ProjectsData(projects_query::ResponseData),

    IssueInspectData(issue_detail_query::ResponseData),
    PullRequestInspectData(issue_detail_query::ResponseData),
    ProjectInspectData(issue_detail_query::ResponseData),
}

pub struct TabMenu {
    active_menu_item: MenuItem,

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
    pub fn new(config: Config) -> Result<Self, git2::Error> {
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

        tab_menu.add_menu_panels();

        if tab_menu.active_remote.is_some() {
            tab_menu.request_all();
        } else {
            tab_menu.open_remote_explorer()?;
        }

        Ok(tab_menu)
    }

    fn request_all(&self) {
        for request_type in RequestType::iter() {
            let request_type_string = request_type.to_str();
            match self.send_request(*request_type) {
                Err(error) => log::error!(
                    "{} occured during initial {:?} request",
                    error,
                    request_type_string
                ),
                _ => (),
            }
        }
    }

    fn add_menu_panels(&mut self) {
        self.ui_stack.add_panel(
            create_issues_view(
                issues_query::IssuesQueryRepository {
                    issues: issues_query::IssuesQueryRepositoryIssues { nodes: None },
                },
                self.config.clone(),
                self.data_clone_sender.clone(),
            ),
            2,
            ISSUES_VIEW_NAME,
        );

        self.ui_stack.add_panel(
            create_pull_requests_view(
                pull_requests_query::PullRequestsQueryRepository {
                    pull_requests: pull_requests_query::PullRequestsQueryRepositoryPullRequests {
                        nodes: None,
                    },
                },
                self.config.clone(),
                self.data_clone_sender.clone(),
            ),
            0,
            PULL_REQUESTS_VIEW_NAME,
        );

        self.ui_stack.add_panel(
            create_projects_view(
                projects_query::ProjectsQueryRepository {
                    projects_v2: projects_query::ProjectsQueryRepositoryProjectsV2 { nodes: None },
                },
                self.config.clone(),
                self.data_clone_sender.clone(),
            ),
            1,
            PROJECTS_VIEW_NAME,
        );

        self.ui_stack.select_panel(ISSUES_VIEW_NAME);
    }

    fn open_remote_explorer(&mut self) -> Result<(), git2::Error> {
        self.ui_stack.add_panel(
            RemoteExplorer::new(self.data_clone_sender.clone())?,
            self.ui_stack.get_highest_priority() + 1,
            REMOTE_EXPLORER_NAME,
        );

        Ok(())
    }

    fn display_menu_item(
        menu_item: &MenuItem,
        render_frame: &mut Frame,
        area: Rect,
        is_highlighted: bool,
    ) -> Rect {
        let item_style = if is_highlighted {
            Style::default().fg(Color::LightGreen)
        } else {
            Style::default()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .style(item_style)
            .title(format!("[{}]", String::from(menu_item)));

        let block_inner = block.inner(area);

        render_frame.render_widget(block, area);

        block_inner
    }

    fn send_request(&self, request_type: RequestType) -> Result<(), Box<dyn Error>> {
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
            return Err("Couldn't capture owner or name for request".into());
        };

        let variables = VariableStore::new(
            repo_captures["name"].to_string(),
            repo_captures["owner"].to_string(),
        );

        let cloned_sender = self.data_clone_sender.clone();
        let cloned_access_token = self
            .config
            .github_token
            .clone()
            .expect("Access token already checked");

        thread::spawn(move || match Runtime::new() {
            Ok(runtime) => {
                runtime.block_on(async {
                    match request_type {
                        RequestType::IssuesRequest => match perform_issues_query(
                            cloned_sender,
                            variables.into(),
                            cloned_access_token,
                        )
                        .await
                        {
                            Err(error) => {
                                log::error!("issues_query returned an error. {}", error)
                            }
                            _ => (),
                        },

                        RequestType::PullRequestsRequest => match perform_pull_requests_query(
                            cloned_sender,
                            variables.into(),
                            cloned_access_token,
                        )
                        .await
                        {
                            Err(error) => {
                                log::error!("pull_requests_query returned an error. {}", error)
                            }
                            _ => (),
                        },
                        RequestType::ProjectsRequest => match perform_projects_query(
                            cloned_sender,
                            variables.into(),
                            cloned_access_token,
                        )
                        .await
                        {
                            Err(error) => {
                                log::error!("projects_query returned an error. {}", error)
                            }
                            _ => (),
                        },
                    }
                });
            }
            Err(error) => log::error!("Couldn't spawn runtime for issues_query. {}", error),
        });
        Ok(())
    }

    fn select_next_menu_item(&mut self) {
        match self.active_menu_item {
            MenuItem::Issues => self.select_pull_requests_view(),
            MenuItem::PullRequests => self.select_projects_view(),
            MenuItem::Projects => self.select_issues_view(),
        }
    }

    fn select_previous_menu_item(&mut self) {
        match self.active_menu_item {
            MenuItem::Issues => self.select_projects_view(),
            MenuItem::PullRequests => self.select_issues_view(),
            MenuItem::Projects => self.select_pull_requests_view(),
        }
    }

    fn select_issues_view(&mut self) {
        self.active_menu_item = MenuItem::Issues;
        self.ui_stack.select_panel(ISSUES_VIEW_NAME);

        match self.send_request(RequestType::IssuesRequest) {
            Err(error) => {
                log::error!("{} occured during sending of issue request", error);
            }
            _ => (),
        }
    }

    fn select_pull_requests_view(&mut self) {
        self.active_menu_item = MenuItem::PullRequests;
        self.ui_stack.select_panel(PULL_REQUESTS_VIEW_NAME);

        match self.send_request(RequestType::PullRequestsRequest) {
            Err(error) => {
                log::error!("{} occured during sending of pull requests request", error);
            }
            _ => (),
        }
    }

    fn select_projects_view(&mut self) {
        self.active_menu_item = MenuItem::Projects;
        self.ui_stack.select_panel(PROJECTS_VIEW_NAME);

        match self.send_request(RequestType::ProjectsRequest) {
            Err(error) => {
                log::error!("{} occured during sending of projects request", error);
            }
            _ => (),
        }
    }
}

impl PanelElement for TabMenu {
    fn handle_input(&mut self, key_event: KeyEvent) -> bool {
        for (panel, _) in self.ui_stack.iter_rev() {
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
                KeyCode::Tab => self.select_next_menu_item(),
                _ => (),
            },
            KeyEvent {
                modifiers: KeyModifiers::SHIFT,
                ..
            } => match key_event.code {
                KeyCode::BackTab => self.select_previous_menu_item(),
                KeyCode::Char('I') => self.select_issues_view(),
                KeyCode::Char('P') => self.select_pull_requests_view(),
                KeyCode::Char('R') => self.select_projects_view(),
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

    fn render(&mut self, render_frame: &mut Frame, rect: Rect) -> () {
        render_frame.render_widget(Clear, rect);

        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(rect);

        let menu_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(34), // Issues
                Constraint::Percentage(33), // PullRequests
                Constraint::Percentage(33), // Projects
            ])
            .split(horizontal_chunks[0]);

        let mut inner_menu_chunks: Vec<Rect> = vec![];

        let menu_items = MenuItem::to_main_menu_points();
        for (item, chunk) in menu_items.iter().zip(menu_chunks.iter()) {
            let is_highlighted = *item == self.active_menu_item;
            let inner_chunk = Self::display_menu_item(item, render_frame, *chunk, is_highlighted);
            inner_menu_chunks.push(inner_chunk);
        }

        let detail_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(80), Constraint::Percentage(20)])
            .split(horizontal_chunks[1]);

        let mut inner_detail_chunks: Vec<Rect> = vec![];
        for chunk in detail_chunks.iter() {
            let block = Block::default().borders(Borders::ALL);

            let block_inner = block.inner(*chunk);

            render_frame.render_widget(block, *chunk);
            inner_detail_chunks.push(block_inner);
        }

        let panel_layout = HashMap::from([
            (ISSUES_VIEW_NAME, inner_menu_chunks[ISSUES_LAYOUT_POSITION]), // Issues
            (
                PULL_REQUESTS_VIEW_NAME,
                inner_menu_chunks[PULL_REQUESTS_LAYOUT_POSITION],
            ), // Pull Requests
            (
                PROJECTS_VIEW_NAME,
                inner_menu_chunks[PROJECTS_LAYOUT_POSITION],
            ), // Projects
            (REMOTE_EXPLORER_NAME, rect),
            ("", inner_detail_chunks[DETAIL_LAYOUT_POSITION]),
            ("", inner_detail_chunks[STATUS_LAYOUT_POSITION]),
        ]);

        for (panel, name) in self.ui_stack.iter() {
            panel.render(render_frame, panel_layout[name.as_str()]);
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
                RepoData::IssuesData(data) => match data.repository {
                    Some(repo_data) => {
                        let top_priority = self.ui_stack.get_highest_priority() + 1;
                        if let Some((panel, _)) =
                            self.ui_stack.get_panel_mut_ref_by_name(ISSUES_VIEW_NAME)
                        {
                            panel.update(Box::new(repo_data));
                        } else {
                            self.ui_stack.add_panel(
                                create_issues_view(
                                    repo_data,
                                    self.config.clone(),
                                    self.data_clone_sender.clone(),
                                ),
                                top_priority,
                                ISSUES_VIEW_NAME,
                            );
                        }
                    }
                    None => {
                        log::debug!("Couldn't display issues since there was no repository in response data")
                    }
                },
                RepoData::PullRequestsData(data) => match data.repository {
                    Some(repo_data) => {
                        let top_priority = self.ui_stack.get_highest_priority() + 1;
                        if let Some((panel, _)) = self
                            .ui_stack
                            .get_panel_mut_ref_by_name(PULL_REQUESTS_VIEW_NAME)
                        {
                            panel.update(Box::new(repo_data));
                        } else {
                            self.ui_stack.add_panel(
                                create_pull_requests_view(
                                    repo_data,
                                    self.config.clone(),
                                    self.data_clone_sender.clone(),
                                ),
                                top_priority,
                                PULL_REQUESTS_VIEW_NAME,
                            );
                        }
                    }
                    None => {
                        log::debug!("Couldn't display issues since there was no repository in response data")
                    }
                },
                RepoData::ProjectsData(data) => match data.repository {
                    Some(repo_data) => {
                        let top_priority = self.ui_stack.get_highest_priority() + 1;
                        if let Some((panel, _)) =
                            self.ui_stack.get_panel_mut_ref_by_name(PROJECTS_VIEW_NAME)
                        {
                            panel.update(Box::new(repo_data));
                        } else {
                            self.ui_stack.add_panel(
                                create_projects_view(
                                    repo_data,
                                    self.config.clone(),
                                    self.data_clone_sender.clone(),
                                ),
                                top_priority,
                                PROJECTS_VIEW_NAME,
                            );
                        }
                    }
                    None => {
                        log::debug!("Couldn't display issues since there was no repository in response data")
                    }
                },
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

                    should_refresh_issues = true;
                }
                RepoData::IssueInspectData(_data) => (),
                RepoData::PullRequestInspectData(_data) => (),
                RepoData::ProjectInspectData(_data) => (),
            }
        }

        if should_refresh_issues {
            self.request_all();
        }

        let mut priorities_to_quit: Vec<u8> = vec![];

        for (priority, (panel, _)) in self.ui_stack.iter_with_priority() {
            if panel.wants_to_quit() {
                priorities_to_quit.push(*priority);
            }
        }

        for priority in priorities_to_quit.iter() {
            self.ui_stack.remove_panel(*priority);
        }
    }

    fn update(&mut self, _data: Box<dyn std::any::Any>) -> bool {
        false
    }

    fn wants_to_quit(&self) -> bool {
        self.quit
    }

    fn set_focus(&mut self, _state: bool) -> bool {
        false
    }
}
