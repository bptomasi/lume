//! Personal navigation ordering is durable workspace data.

use super::LayoutState;
use crate::model::View;
use anyhow::{Context, Result};

impl LayoutState {
    pub(in crate::app) fn toggle_pin(&mut self, target: &View) {
        if let Some(index) = self
            .workspace
            .pins
            .iter()
            .position(|view| view.same_destination(target))
        {
            self.workspace.pins.remove(index);
        } else {
            self.workspace.pins.push(target.clone());
        }
    }

    /// Returns whether the section is collapsed after this operation.
    pub(in crate::app) fn toggle_group(&mut self, group: &str) -> bool {
        if let Some(index) = self
            .workspace
            .collapsed_groups
            .iter()
            .position(|item| item == group)
        {
            self.workspace.collapsed_groups.remove(index);
            false
        } else {
            self.workspace.collapsed_groups.push(group.into());
            true
        }
    }

    pub(in crate::app) fn expand_group(&mut self, group: &str) {
        self.workspace.collapsed_groups.retain(|item| item != group);
    }

    pub(in crate::app) fn remove_destination(&mut self, target: &View) {
        self.workspace
            .pins
            .retain(|view| !view.same_destination(target));
        self.workspace
            .navigation_order
            .retain(|view| !view.same_destination(target));
    }

    pub(in crate::app) fn reorder_destination(
        &mut self,
        target: &View,
        destinations: Vec<View>,
        forward: bool,
    ) -> Result<()> {
        let pinned = self
            .workspace
            .pins
            .iter()
            .any(|view| view.same_destination(target));
        let order = if pinned {
            &mut self.workspace.pins
        } else {
            self.workspace.navigation_order = destinations;
            &mut self.workspace.navigation_order
        };
        let index = order
            .iter()
            .position(|view| view.same_destination(target))
            .context("Select a destination to reorder")?;
        let next = index
            .saturating_add_signed(if forward { 1 } else { -1 })
            .min(order.len().saturating_sub(1));
        order.swap(index, next);
        Ok(())
    }
}
