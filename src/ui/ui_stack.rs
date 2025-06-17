use std::{
    any::Any,
    collections::{BTreeMap, HashMap},
};

use ratatui::{
    crossterm::event::KeyEvent,
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

use crate::ui::PanelElement;

/// keeps track of PanelElement trait objects while keeping them sorted by their priority
pub struct UiStack {
    panels: BTreeMap<u8, (Box<dyn PanelElement>, String)>,
    panel_names: HashMap<String, u8>,
}

impl UiStack {
    /// creates a new empty `UiStack`
    pub fn new() -> Self {
        Self {
            panels: BTreeMap::new(),
            panel_names: HashMap::new(),
        }
    }

    /// adds a `PanelElement` to the `UiStack`
    pub fn add_panel<P: PanelElement + 'static>(
        &mut self,
        panel: P,
        priority: u8,
        name: impl Into<String> + Copy,
    ) {
        self.panel_names.insert(name.into(), priority);
        self.panels.insert(priority, (Box::new(panel), name.into()));
    }

    /// clears the whole `UiStack` of all of its elements
    pub fn clear(&mut self) {
        self.panels.clear();
        self.panel_names.clear();
    }

    /// removes a panel based on it's priority and returns that element if an element with that
    /// priority was found
    pub fn remove_panel(&mut self, priority: u8) -> Option<(Box<dyn PanelElement>, String)> {
        self.panel_names.retain(|_, &mut p| p != priority);
        self.panels.remove(&priority)
    }

    /// removes the panel with the highest priority from the `UiStack` and returns that panel if
    /// there was any panel in the `UiStack`
    pub fn remove_highest_priority_panel(&mut self) -> Option<(Box<dyn PanelElement>, String)> {
        if let Some((&priority, _)) = self.panels.last_key_value() {
            return self.remove_panel(priority);
        }
        None
    }

    /// removes the panel with the lowest priority from the `UiStack` and returns that panel if
    /// there was any panel in the `UiStack`
    pub fn remove_lowest_priority_panel(&mut self) -> Option<(Box<dyn PanelElement>, String)> {
        if let Some((&priority, _)) = self.panels.first_key_value() {
            return self.remove_panel(priority);
        }
        None
    }

    /// removes a panel by it's name. If no panel with this name could be found return `None` other
    /// wise returns `Some(panel)`
    pub fn remove_panel_by_name(&mut self, name: &str) -> Option<(Box<dyn PanelElement>, String)> {
        if let Some(&priority) = self.panel_names.get(name) {
            self.panel_names.remove(name);
            return self.panels.remove(&priority);
        }

        None
    }

    /// returns the highest priorty currently in the `UiStack`
    pub fn get_highest_priority(&self) -> u8 {
        self.panels
            .last_key_value()
            .map_or(0, |(priority, _)| *priority)
    }

    /// returns the names of all panels that are currently registered in `UiStack`
    pub fn get_panel_names(&self) -> Vec<&String> {
        self.panel_names.keys().collect()
    }

    /// get a reference to a panel based on its name if the name exists in the `UiStack`
    pub fn get_panel_ref_by_name(&self, name: &str) -> Option<&(Box<dyn PanelElement>, String)> {
        if let Some(&priority) = self.panel_names.get(name) {
            return self.panels.get(&priority);
        }

        None
    }

    /// get a mutable reference to a panel based on its name if the name exists in the `UiStack`
    pub fn get_panel_mut_ref_by_name(
        &mut self,
        name: &str,
    ) -> Option<&mut (Box<dyn PanelElement>, String)> {
        if let Some(&priority) = self.panel_names.get(name) {
            return self.panels.get_mut(&priority);
        }

        None
    }

    /// iterates over all panels from lowest to highest priority
    /// use iter_rev if you want to iterate from highest to lowest priority
    pub fn iter(&mut self) -> impl Iterator<Item = &mut (Box<dyn PanelElement>, String)> {
        self.panels.values_mut()
    }

    /// iterates over all panles from highest to lowest priority
    /// use iter if you want to iterate from lowest to highest priority
    pub fn iter_rev(&mut self) -> impl Iterator<Item = &mut (Box<dyn PanelElement>, String)> {
        self.panels.values_mut().rev()
    }

    /// iterate over all panels from lowest ot highest priority
    /// returns both panel and it's associated priority
    pub fn iter_with_priority(
        &mut self,
    ) -> impl Iterator<Item = (&u8, &mut (Box<dyn PanelElement>, String))> {
        self.panels.iter_mut()
    }

    /// selects a panel based on it's name. Selecting means telling the panel it has focus and
    /// increasing the panels priority to n + 1, n being the former top priority. This also
    /// normalizes all the priorities in the `UiStack` since only the order is really important for
    /// us
    pub fn select_panel(&mut self, name: &str) -> bool {
        let success = match self.get_panel_mut_ref_by_name(name) {
            Some((panel, _)) => panel.set_focus(true),
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
            if let Some((old_panel, _)) = self.panels.get_mut(&self.get_highest_priority()) {
                old_panel.set_focus(false);
            }
        }

        self.set_panel_priority_by_name(self.get_highest_priority() + 1, name);
        self.normalize_priorities();

        true
    }

    /// sets the priority of a panel based on it's name. If there was no such panel with this name
    /// log a debug message.
    pub fn set_panel_priority_by_name(&mut self, new_priority: u8, name: &str) {
        if let Some(&priority) = self.panel_names.get(name) {
            if new_priority == priority {
                return;
            }

            if self.panels.contains_key(&new_priority) {
                log::debug!(
                    "Panel with name: {name} was set to the same priority as another panel."
                );
                return;
            }

            if let Some(panel) = self.panels.remove(&priority) {
                self.panel_names.insert(name.to_string(), new_priority);
                self.panels.insert(new_priority, panel);
            }

            return;
        }

        log::debug!("Panel with name: {name} was not in ui stack and can therefore not have it's priority changed.")
    }

    // this works but doesn't feel very well coded so I am not sure if I want to keep this
    /// normalises all priorities in `UiStack` this creates and populates new internal objects for
    /// storing panels.
    pub fn normalize_priorities(&mut self) {
        if self.panels.is_empty() {
            return;
        }

        let mut new_panels = BTreeMap::new();
        let mut new_panel_names = HashMap::new();
        let mut index: u8 = 0;

        let panel_names_copy = self.panel_names.clone();

        while !self.panels.is_empty() {
            let (old_priority, panel) = self.panels.pop_first().expect("self.panels is not empty");

            new_panels.insert(index, panel);

            for (panel_name, &priority) in &panel_names_copy {
                if priority == old_priority {
                    new_panel_names.insert(panel_name.clone(), index);
                }
            }

            index += 1;
        }

        self.panels = new_panels;
        self.panel_names = new_panel_names;
    }
}
