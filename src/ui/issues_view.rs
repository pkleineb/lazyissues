use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame,
};

use crate::graphql_requests::github::issue_query;

use super::{tab_menu::MenuItem, PanelElement};

pub struct IssuesView {
    layout_position: usize,

    issue_data: issue_query::IssueQueryRepository,
}

impl IssuesView {
    pub fn new(layout_position: usize, data: issue_query::IssueQueryRepository) -> Self {
        Self {
            layout_position,
            issue_data: data,
        }
    }

    fn display_issue(
        issue_data: &issue_query::IssueQueryRepositoryIssuesNodes,
        render_frame: &mut ratatui::Frame,
        area: Rect,
    ) {
        let status_style = if issue_data.closed {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };
        let status = if issue_data.closed { "✓" } else { "○" };

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default())
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
    fn handle_input(&mut self, _key_event: ratatui::crossterm::event::KeyEvent) -> bool {
        false
    }

    fn tick(&mut self) -> () {}

    fn render(
        &mut self,
        render_frame: &mut ratatui::Frame,
        layout: &std::rc::Rc<[ratatui::prelude::Rect]>,
    ) -> () {
        if let Some(issue_nodes) = &self.issue_data.issues.nodes {
            let issues: Vec<_> = issue_nodes.iter().filter(|issue| issue.is_some()).collect();

            let mut constraints: Vec<Constraint> = vec![];
            for _ in 0..issues.len() {
                constraints.push(Constraint::Length(7));
            }

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(layout[self.layout_position]);

            for (issue, chunk) in issues.iter().zip(chunks.iter()) {
                if issue.is_none() {
                    continue;
                }

                issue
                    .as_ref()
                    .map(|node| Self::display_issue(node, render_frame, *chunk));
            }
        }
    }
}
