use std::{
    fs::File,
    io,
    io::Write,
    rc::Rc,
    result::Result,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use graphql_requests::github::issue_query;
use ratatui::{
    crossterm::{
        event::{self, Event as CrossEvent, KeyCode},
        terminal::disable_raw_mode,
    },
    layout::{Constraint, Direction, Layout, Rect},
    prelude::CrosstermBackend,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, Tabs},
    Frame, Terminal,
};
use tokio::runtime::Runtime;

mod config;
mod file_explorer;
mod graphql_requests;

pub const TICK_RATE: Duration = Duration::from_millis(200);

pub enum Event<I> {
    Input(I),
    Tick,
}

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

pub struct EventLoop {
    sender: mpsc::Sender<Event<CrossEvent>>,
    last_tick: Instant,
}

impl EventLoop {
    pub fn new(sender: mpsc::Sender<Event<CrossEvent>>) -> Self {
        Self {
            sender,
            last_tick: Instant::now(),
        }
    }

    pub fn run(&mut self) {
        self.last_tick = Instant::now();

        loop {
            let timeout = TICK_RATE
                .checked_sub(self.last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            let poll = event::poll(timeout);
            match poll {
                Ok(found_event) => {
                    if found_event {
                        self.handle_event();
                    } else {
                        self.send_tick();
                    }
                }
                Err(error) => println!("{error} occured during polling!"),
            }
        }
    }

    fn handle_event(&self) {
        match event::read() {
            Ok(CrossEvent::Key(key)) => {
                match self.sender.send(Event::Input(CrossEvent::Key(key))) {
                    Err(error) => println!("{error} occured during sending!"),
                    _ => (),
                }
            }
            Ok(_) => (),
            Err(error) => println!("{error} occured during reading of event!"),
        }
    }

    fn send_tick(&mut self) {
        if self.last_tick.elapsed() >= TICK_RATE {
            if let Ok(_) = self.sender.send(Event::Tick) {
                self.last_tick = Instant::now();
            }
        }
    }
}

pub struct TerminalApp {
    input_receiver: mpsc::Receiver<Event<CrossEvent>>,
    query_receiver: mpsc::Receiver<QueryData>,
    query_clone_sender: mpsc::Sender<QueryData>,

    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalApp {
    pub fn new(input_receiver: mpsc::Receiver<Event<CrossEvent>>) -> Result<Self, std::io::Error> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend);

        let (query_clone_sender, query_receiver): (
            mpsc::Sender<QueryData>,
            mpsc::Receiver<QueryData>,
        ) = mpsc::channel();

        match terminal {
            Ok(terminal) => Ok(Self {
                input_receiver,
                query_receiver,
                query_clone_sender,
                terminal,
            }),
            Err(error) => Err(error),
        }
    }

    pub fn run(&mut self) {
        if let Err(error) = self.terminal.clear() {
            println!("{error} occured during terminal clearing");
            return;
        }

        let mut active_menu_item = MenuItem::Issues;
        let mut query_response_data: Vec<QueryData> = Vec::new();
        let mut config = match config::read_config() {
            Ok(Some(config)) => config,
            Ok(None) => config::Config::new(),
            Err(error) => {
                println!("{error} occured while reading config! Using default config.");
                config::Config::new()
            }
        };

        let explorer = file_explorer::FileExplorer::new();

        loop {
            let _ = self.terminal.draw(|render_frame| {
                let layout = Self::create_base_layout(render_frame);
                Self::create_tab_menu(render_frame, layout[0], &active_menu_item);

                match active_menu_item {
                    MenuItem::Issues => {
                        Self::render_issues_view(render_frame, layout[1], &query_response_data)
                    }
                    _ => (),
                }

                if config.is_default() {
                    explorer.render(render_frame, layout[1])
                }
            });

            match self.input_receiver.recv() {
                Ok(event) => {
                    let close = self.handle_input(event, &mut active_menu_item);
                    if close {
                        break;
                    }
                }
                Err(error) => {
                    self.clean_up_terminal(Some(format!("{error} occured during receiving input!")))
                }
            };

            // try_recv does not block the current thread which is nice here because we don't
            // have a tick signal recv() would block the thread until we receive a message from
            // the sender I am ignoring the error here but that may not be best practice
            if let Ok(data) = self.query_receiver.try_recv() {
                // this vector might get quite big so I need to implement garbage collection which
                // looks if there is two of the same QueryData since we can then delete the older
                // one
                query_response_data.push(data);
            }
        }
    }

    fn create_base_layout(render_frame: &mut Frame) -> Rc<[Rect]> {
        let size = render_frame.area();
        Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(2)].as_ref())
            .split(size)
    }

    fn create_tab_menu(render_frame: &mut Frame, chunk: Rect, active_menu_item: &MenuItem) {
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
            .select(active_menu_item.into())
            .block(
                Block::default()
                    .title(String::from(active_menu_item))
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Red))
            .divider(Span::raw("|"));

        render_frame.render_widget(tabs, chunk);
    }

    fn ask_for_token_files(render_frame: &mut Frame, chunk: Rect, config: &mut config::Config) {}

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

    fn handle_input(&mut self, event: Event<CrossEvent>, active_menu_item: &mut MenuItem) -> bool {
        match event {
            Event::Input(event) => match event {
                CrossEvent::Key(key) => match key.code {
                    KeyCode::Char('q') => {
                        self.clean_up_terminal(None);
                        return true;
                    }
                    KeyCode::Char('I') => {
                        *active_menu_item = MenuItem::Issues;
                        let cloned_sender = self.query_clone_sender.clone();
                        thread::spawn(move || {
                            let runtime = Runtime::new();
                            match runtime {
                                Ok(runtime) => runtime.block_on(async {
                                    Self::fetch_issues(cloned_sender).await;
                                }),
                                Err(error) => println!("{error} occured while creating runtime"),
                            };
                        });
                    }
                    KeyCode::Char('P') => *active_menu_item = MenuItem::PullRequests,
                    KeyCode::Char('A') => *active_menu_item = MenuItem::Actions,
                    KeyCode::Char('r') => *active_menu_item = MenuItem::Projects,
                    _ => (),
                },
                _ => (),
            },
            Event::Tick => (),
        }

        false
    }

    async fn fetch_issues(sender: mpsc::Sender<QueryData>) {
        let variables = graphql_requests::github::issue_query::Variables {
            repo_name: "test_repo".to_string(),
            repo_owner: "pkleineb".to_string(),
        };

        let response_data = graphql_requests::github::perform_issue_query(variables).await;

        match response_data {
            Ok(ok) => match ok {
                Some(data) => match sender.send(QueryData::IssuesData(data)) {
                    Err(error) => println!("{error} occured during sending of query data!"),
                    _ => (),
                },
                None => println!("No data fetched from server!"),
            },
            Err(error) => println!("{:?} occured during fetching data from server!", error),
        };
    }

    fn clean_up_terminal(&mut self, message: Option<String>) {
        if let Err(error) = disable_raw_mode() {
            println!("{error} occured when trying to exit raw mode!");
        }
        if let Err(error) = self.terminal.show_cursor() {
            println!("{error} occured when trying to show cursor!");
        }

        match message {
            Some(message) => println!("{message}"),
            None => (),
        }
    }
}

fn create_floating_layout(width: u16, height: u16, base_chunk: Rect) -> Rect {
    let y_offset = 50 - height / 2;
    let x_offset = 50 - width / 2;

    let vertical_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(y_offset),
            Constraint::Percentage(height),
            Constraint::Percentage(y_offset),
        ])
        .split(base_chunk);

    let horizontal_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(x_offset),
            Constraint::Percentage(width),
            Constraint::Percentage(x_offset),
        ])
        .split(vertical_layout[1]);

    horizontal_layout[1]
}
