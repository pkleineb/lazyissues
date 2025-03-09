use std::{collections::HashMap, rc::Rc, sync::mpsc, thread};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, Tabs},
    Frame,
};
use tokio::runtime::Runtime;

use crate::{
    config::Config,
    graphql_requests::github::{issue_query, perform_issue_query},
    ui::PanelElement,
    Signal,
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

pub enum QueryData {
    IssuesData(issue_query::ResponseData),
}

pub struct TabMenu {
    active_menu_item: MenuItem,

    layout_position: usize,

    query_receiver: mpsc::Receiver<(MenuItem, QueryData)>,
    query_clone_sender: mpsc::Sender<(MenuItem, QueryData)>,

    // this might be a stupid way to store this
    query_response_data: HashMap<MenuItem, QueryData>,

    signal_sender: mpsc::Sender<Signal>,

    config: Config,
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
            query_response_data: HashMap::new(),
            signal_sender,
            config,
        }
    }

    fn render_issues_view(
        render_frame: &mut Frame,
        chunk: Rect,
        query_response_data: &Vec<QueryData>,
    ) {
        for response_data in query_response_data.iter() {
            match response_data {
                QueryData::IssuesData(data) => {
                    if let Some(repo) = &data.repository {
                        Self::display_issues(&repo, render_frame, chunk)
                    }
                }
            }
        }
    }

    fn display_issues(
        repo_data: &issue_query::IssueQueryRepository,
        render_frame: &mut Frame,
        chunk: Rect,
    ) {
        if let Some(issues) = &repo_data.issues.nodes {
            let issue_items: Vec<Span> = issues
                .iter()
                .map(|issue| {
                    if let Some(node) = issue {
                        return Span::styled(&node.title, Style::default());
                    }
                    Span::default()
                })
                .collect();

            let issue_list = List::new(issue_items)
                .block(Block::default().title("Issues").borders(Borders::ALL))
                .style(Style::default().fg(Color::White));

            render_frame.render_widget(issue_list, chunk);
        }
    }

    async fn fetch_issues(sender: mpsc::Sender<(MenuItem, QueryData)>) {
        let variables = issue_query::Variables {
            repo_name: "test_repo".to_string(),
            repo_owner: "pkleineb".to_string(),
        };

        let response_data = perform_issue_query(variables).await;

        match response_data {
            Ok(ok) => match ok {
                Some(data) => match sender.send((MenuItem::Issues, QueryData::IssuesData(data))) {
                    Err(error) => log::error!("{error} occured during sending of query data!"),
                    _ => (),
                },
                None => log::debug!("No data fetched from server!"),
            },
            Err(error) => log::error!("{:?} occured during fetching data from server!", error),
        };
    }
}

impl PanelElement for TabMenu {
    fn handle_input(&mut self, key_event: KeyEvent) -> bool {
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
                    let cloned_sender = self.query_clone_sender.clone();
                    thread::spawn(move || {
                        let runtime = Runtime::new();
                        match runtime {
                            Ok(runtime) => runtime.block_on(async {
                                Self::fetch_issues(cloned_sender).await;
                            }),
                            Err(error) => log::error!("{error} occured while creating runtime"),
                        };
                    });
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
    }

    fn tick(&mut self) -> () {
        // try_recv does not block the current thread which is nice here because we don't
        // have a tick signal recv() would block the thread until we receive a message from
        // the sender I am ignoring the error here but that may not be best practice
        if let Ok(data) = self.query_receiver.try_recv() {
            let (key, value) = data;
            self.query_response_data.insert(key, value);
        }
    }
}
