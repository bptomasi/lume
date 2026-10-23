//! Lume's reader core and adapters.
//!
//! Storage owns durable user data. Sources normalize feeds on a worker thread;
//! the application owns navigation and delegates all terminal drawing to `ui`.

pub mod app;
pub mod catalog;
pub mod clipboard;
pub mod config;
pub mod content;
pub mod demo;
pub mod exchange;
pub mod external_editor;
pub mod media;
pub mod model;
pub mod offline;
pub mod reader;
pub mod retention;
pub mod search;
pub mod source;
pub mod storage;
pub mod store;
pub mod ui;

mod http_body;
mod latest;

/// Local, explainable article recommendation policy.
pub mod recommendation;
