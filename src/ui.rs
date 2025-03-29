use std::{collections::BTreeMap, rc::Rc};

use ratatui::{crossterm::event::KeyEvent, layout::Rect, Frame};

pub mod issues_view;
pub mod remote_explorer;
pub mod tab_menu;

pub trait PanelElement {
    fn handle_input(&mut self, key_event: KeyEvent) -> bool;
    fn render(&mut self, render_frame: &mut Frame, layout: &Rc<[Rect]>) -> ();
    fn tick(&mut self) -> ();
}

pub struct UiStack {
    panels: BTreeMap<u8, Box<dyn PanelElement>>,
}

impl UiStack {
    pub fn new() -> Self {
        Self {
            panels: BTreeMap::new(),
        }
    }

    pub fn add_panel<P: PanelElement + 'static>(&mut self, panel: P, priority: u8) {
        self.panels.insert(priority, Box::new(panel));
    }

    pub fn remove_panel(&mut self, priority: u8) -> Option<Box<dyn PanelElement>> {
        self.panels.remove(&priority)
    }

    pub fn remove_highest_priority_panel(&mut self) -> Option<Box<dyn PanelElement>> {
        if let Some((&priority, _)) = self.panels.last_key_value() {
            return self.panels.remove(&priority);
        }
        None
    }

    pub fn remove_lowest_priority_panel(&mut self) -> Option<Box<dyn PanelElement>> {
        if let Some((&priority, _)) = self.panels.first_key_value() {
            return self.panels.remove(&priority);
        }
        None
    }

    pub fn get_highest_priority(&self) -> u8 {
        self.panels
            .last_key_value()
            .map_or(0, |(priority, _)| *priority)
    }

    pub fn iter(&mut self) -> impl Iterator<Item = &mut Box<dyn PanelElement>> {
        self.panels.values_mut()
    }

    pub fn iter_rev(&mut self) -> impl Iterator<Item = &mut Box<dyn PanelElement>> {
        self.panels.values_mut().rev()
    }
}
