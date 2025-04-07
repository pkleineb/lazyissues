use std::{
    any::Any,
    collections::{BTreeMap, HashMap},
    net::ToSocketAddrs,
    rc::Rc,
};

use ratatui::{crossterm::event::KeyEvent, layout::Rect, Frame};

pub mod list_view;
pub mod remote_explorer;
pub mod tab_menu;

pub trait PanelElement {
    fn handle_input(&mut self, key_event: KeyEvent) -> bool;
    fn render(&mut self, render_frame: &mut Frame, layout: &Rc<[Rect]>) -> ();
    fn tick(&mut self) -> ();
    fn update(&mut self, data: Box<dyn Any>) -> bool;
    fn wants_to_quit(&self) -> bool;
    fn set_focus(&mut self, state: bool) -> bool;
}

pub struct UiStack {
    panels: BTreeMap<u8, Box<dyn PanelElement>>,
    panel_names: HashMap<String, u8>,
}

impl UiStack {
    pub fn new() -> Self {
        Self {
            panels: BTreeMap::new(),
            panel_names: HashMap::new(),
        }
    }

    pub fn add_panel<P: PanelElement + 'static>(
        &mut self,
        panel: P,
        priority: u8,
        name: impl Into<String>,
    ) {
        let name = name.into();
        self.panel_names.insert(name, priority);
        self.panels.insert(priority, Box::new(panel));
    }

    pub fn clear(&mut self) {
        self.panels.clear();
        self.panel_names.clear();
    }

    pub fn remove_panel(&mut self, priority: u8) -> Option<Box<dyn PanelElement>> {
        self.panel_names.retain(|_, &mut p| p != priority);
        self.panels.remove(&priority)
    }

    pub fn remove_highest_priority_panel(&mut self) -> Option<Box<dyn PanelElement>> {
        if let Some((&priority, _)) = self.panels.last_key_value() {
            return self.remove_panel(priority);
        }
        None
    }

    pub fn remove_lowest_priority_panel(&mut self) -> Option<Box<dyn PanelElement>> {
        if let Some((&priority, _)) = self.panels.first_key_value() {
            return self.remove_panel(priority);
        }
        None
    }

    pub fn remove_panel_by_name(&mut self, name: &str) -> Option<Box<dyn PanelElement>> {
        if let Some(&priority) = self.panel_names.get(name) {
            self.panel_names.remove(name);
            return self.panels.remove(&priority);
        }

        None
    }
    pub fn get_highest_priority(&self) -> u8 {
        self.panels
            .last_key_value()
            .map_or(0, |(priority, _)| *priority)
    }

    pub fn get_panel_names(&self) -> Vec<&String> {
        self.panel_names.keys().collect()
    }

    pub fn get_panel_ref_by_name(&self, name: &str) -> Option<&Box<dyn PanelElement>> {
        if let Some(&priority) = self.panel_names.get(name) {
            return self.panels.get(&priority);
        }

        None
    }

    pub fn get_panel_mut_ref_by_name(&mut self, name: &str) -> Option<&mut Box<dyn PanelElement>> {
        if let Some(&priority) = self.panel_names.get(name) {
            return self.panels.get_mut(&priority);
        }

        None
    }

    pub fn iter(&mut self) -> impl Iterator<Item = &mut Box<dyn PanelElement>> {
        self.panels.values_mut()
    }

    pub fn iter_rev(&mut self) -> impl Iterator<Item = &mut Box<dyn PanelElement>> {
        self.panels.values_mut().rev()
    }

    pub fn iter_with_priority(
        &mut self,
    ) -> impl Iterator<Item = (&u8, &mut Box<dyn PanelElement>)> {
        self.panels.iter_mut()
    }

    pub fn select_panel(&mut self, name: &str) -> bool {
        let success = match self.get_panel_mut_ref_by_name(name) {
            Some(panel) => panel.set_focus(true),
            None => {
                log::debug!(
                    "Panel with name: {} was not in ui stack and therefore can't be selected.",
                    name
                );
                false
            }
        };

        if !success {
            return false;
        }

        if self
            .panel_names
            .get(name)
            .expect("Has to exist because otherwise we would have returned ealier.")
            != &self.get_highest_priority()
        {
            if let Some(old_panel) = self.panels.get_mut(&self.get_highest_priority()) {
                old_panel.set_focus(false);
            }
        }

        self.set_panel_priority_by_name(self.get_highest_priority() + 1, name);
        self.normalize_priorities();

        true
    }

    pub fn set_panel_priority_by_name(&mut self, new_priority: u8, name: &str) {
        if let Some(&priority) = self.panel_names.get(name) {
            if new_priority == priority {
                return;
            }

            if let Some(panel) = self.panels.remove(&priority) {
                self.panel_names.insert(name.to_string(), new_priority);
                self.panels.insert(new_priority, panel);
                log::debug!("Set panel {} priority to {}", name, new_priority);
            }
        } else {
            log::debug!("Panel with name: {} was not in ui stack and can therefore not have it's priority changed.", name)
        }
    }

    pub fn normalize_priorities(&mut self) {
        if self.panels.is_empty() {
            return;
        }

        let mut new_panels = BTreeMap::new();
        let mut index: u8 = 0;
        while !self.panels.is_empty() {
            let (_, panel) = self.panels.pop_first().expect("self.panels is not empty");

            new_panels.insert(index, panel);
            index += 1;
        }

        self.panels = new_panels;
    }
}
