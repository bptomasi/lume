//! Pure validation shared by configuration, domain data and adapters.
//!
//! This module cannot access the library, application state, network or UI.

mod bindings;
mod names;
mod selector;
mod workspace;

pub(crate) use bindings::validate_binding;
pub(crate) use names::{valid_workspace_name, validate_collection_name};
pub use selector::validate_selector;
pub(crate) use workspace::{
    validate_panel_layout, validate_workspace_dimensions,
};
