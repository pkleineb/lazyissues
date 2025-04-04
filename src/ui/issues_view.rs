use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::graphql_requests::github::issues_query;

use super::PanelElement;

pub const ISSUES_VIEW_NAME: &str = "issues_view";

pub struct IssuesView {
    layout_position: usize,

    issue_data: issues_query::IssuesQueryRepository,
    issue_amount: usize,
    selected_issue: usize,
}

impl IssuesView {
    pub fn new(layout_position: usize, data: issues_query::IssuesQueryRepository) -> Self {
        Self {
            layout_position,
            issue_amount: data.issues.nodes.as_ref().unwrap_or(&vec![]).len(),
            issue_data: data,
            selected_issue: 0,
        }
    }

    fn select_next_item(&mut self) {
        self.selected_issue += 1;
        if self.selected_issue >= self.issue_amount {
            self.selected_issue = 0;
        }
    }

    fn select_previous_item(&mut self) {
        if self.selected_issue == 0 {
            self.selected_issue = self.issue_amount - 1;
            return;
        }
        self.selected_issue -= 1;
    }

    fn display_issue(
        issue_data: &issues_query::IssuesQueryRepositoryIssuesNodes,
        render_frame: &mut ratatui::Frame,
        area: Rect,
        is_highlighted: bool,
    ) {
        let status_style = if issue_data.closed {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };
        let status = if issue_data.closed { "✓" } else { "○" };

        let issue_style = if is_highlighted {
            Style::default().fg(Color::LightGreen)
        } else {
            Style::default()
        };

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .style(issue_style)
            .title(format!(
                "[{} #{} {}]",
                status, issue_data.number, issue_data.title
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

        // this looks super goofy but we might get a misformed response due to some backend having
        // problems on their side. Same with the one underneath
        if let Some(author) = &issue_data.author {
            let author_paragraph = Paragraph::new(Span::styled(&author.login, Style::default()));
            render_frame.render_widget(author_paragraph, info_chunks[0]);
        }

        let time_paragraph = Paragraph::new(Span::styled(
            &issue_data.created_at.0,
            Style::default().fg(Color::Gray),
        ));
        render_frame.render_widget(time_paragraph, info_chunks[1]);

        if let Some(labels) = &issue_data.labels {
            if let Some(nodes) = &labels.nodes {
                let mut tags: Vec<Paragraph> = vec![];
                let mut constraints: Vec<Constraint> = vec![];

                for node in nodes {
                    if node.is_none() {
                        continue;
                    }

                    node.as_ref().map(|tag| {
                        constraints.push(Constraint::Length(tag.name.len() as u16 + 2));

                        tags.push(
                            Paragraph::new(Span::styled(&tag.name, Style::default()))
                                .block(Block::new().borders(Borders::ALL)),
                        );
                    });
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
}

impl PanelElement for IssuesView {
    fn handle_input(&mut self, key_event: ratatui::crossterm::event::KeyEvent) -> bool {
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

    fn render(
        &mut self,
        render_frame: &mut ratatui::Frame,
        layout: &std::rc::Rc<[ratatui::prelude::Rect]>,
    ) -> () {
        let render_area = layout[self.layout_position];

        render_frame.render_widget(Clear, render_area);

        if let Some(issue_nodes) = &self.issue_data.issues.nodes {
            let issues: Vec<_> = issue_nodes.iter().filter(|issue| issue.is_some()).collect();

            let mut constraints: Vec<Constraint> = vec![];
            for _ in 0..issues.len() {
                constraints.push(Constraint::Length(7));
            }

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(render_area);

            for (i, (issue, chunk)) in issues.iter().zip(chunks.iter()).enumerate() {
                if issue.is_none() {
                    continue;
                }

                let is_highlighted = i == self.selected_issue;

                issue
                    .as_ref()
                    .map(|node| Self::display_issue(node, render_frame, *chunk, is_highlighted));
            }
        }
    }

    fn update(&mut self, data: Box<dyn std::any::Any>) -> bool {
        if let Ok(repo_data) = data.downcast::<issues_query::IssuesQueryRepository>() {
            self.issue_data = *repo_data;
            self.issue_amount = self
                .issue_data
                .issues
                .nodes
                .as_ref()
                .unwrap_or(&vec![])
                .len();

            self.selected_issue = if self.selected_issue < self.issue_amount {
                self.selected_issue
            } else {
                self.issue_amount
            };

            return true;
        }

        false
    }

    fn wants_to_quit(&self) -> bool {
        false
    }
}
