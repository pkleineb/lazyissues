use ratatui::widgets::Paragraph;

use super::{list_view::ListItem, PanelElement};

pub const DETAIL_VIEW_NAME: &str = "detail_view";

pub trait DetailItem: std::fmt::Debug {
    fn get_body(&self) -> &str;
    fn get_comments(&self) -> Vec<Box<dyn Comment>>;
}

pub trait Comment: std::fmt::Debug {
    fn get_author_login(&self) -> Option<&str>;
    fn get_created_at(&self) -> &str;
    fn get_body(&self) -> &str;
}

pub trait DetailListItem: DetailItem + ListItem {}

#[derive(Default)]
pub struct DetailView {
    item: Option<Box<dyn DetailListItem>>,

    is_focused: bool,
}

impl PanelElement for DetailView {
    fn tick(&mut self) {}

    fn render(&mut self, render_frame: &mut ratatui::Frame, rect: ratatui::prelude::Rect) {
        let Some(ref unwrapped_item) = self.item else {
            return;
        };

        let title_paragraph = Paragraph::new(unwrapped_item.get_title());
        render_frame.render_widget(title_paragraph, rect);
    }

    fn update(&mut self, data: Box<dyn std::any::Any>) -> bool {
        match data.downcast::<Box<dyn DetailListItem>>() {
            Ok(converted_data) => {
                self.item = Some(*converted_data);
                true
            }
            Err(other) => {
                log::debug!(
                    "Couldn't downcast to IntoDetailCollection implementing type. Other value was: {:?}",
                    other.type_id()
                );
                false
            }
        }
    }

    fn set_focus(&mut self, state: bool) -> bool {
        self.is_focused = state;
        true
    }

    fn handle_input(&mut self, key_event: ratatui::crossterm::event::KeyEvent) -> bool {
        false
    }

    fn wants_to_quit(&self) -> bool {
        false
    }
}
