use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, Tabs},
    Frame,
};

use crate::graphql_requests::github::issue_query;

use super::PanelElement;

pub struct IssuesView {
    layout_position: usize,

    data: issue_query::IssueQueryRepository,
}

impl IssuesView {
    pub fn new(layout_position: usize, data: issue_query::IssueQueryRepository) -> Self {
        Self {
            layout_position,
            data,
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
        if let Some(issues) = &self.data.issues.nodes {
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

            render_frame.render_widget(issue_list, layout[self.layout_position]);
        }
    }
}
