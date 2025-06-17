use std::rc::Rc;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

/// creates a centered floating layout in the drawable area
pub fn create_floating_layout(width: u16, height: u16, base_chunk: Rect) -> Rect {
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

/// creates the base rendering layout
pub fn create_base_layout(render_frame: &mut Frame) -> Rc<[Rect]> {
    let size = render_frame.area();
    Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Min(2)].as_ref())
        .split(size)
}
