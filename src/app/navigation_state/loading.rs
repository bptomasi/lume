//! Foreground replacement and background projection have distinct delivery states.
//!
//! Foreground loading blocks source-bound mutations. Background projection keeps
//! the current selection visible until a replacement can be published safely.
//! Worker transport and read admission remain application coordination effects.

use super::NavigationState;

impl NavigationState {
    pub(in crate::app) fn invalidate_library(&mut self) {
        self.library_revision = self.library_revision.wrapping_add(1);
    }

    pub(in crate::app) fn begin_catalog(&mut self) {
        self.catalog_loading = true;
    }

    pub(in crate::app) fn complete_catalog(&mut self) {
        self.catalog_loading = false;
    }

    pub(in crate::app) fn begin_navigation(&mut self) {
        self.projecting_navigation = false;
        self.navigation_loading = true;
    }

    pub(in crate::app) fn begin_projection(&mut self) {
        self.projecting_navigation = true;
    }

    pub(in crate::app) fn complete_navigation(&mut self) {
        self.navigation_loading = false;
        self.projecting_navigation = false;
    }

    pub(in crate::app) fn cancel_loading(&mut self) {
        self.navigation_loading = false;
    }
}
