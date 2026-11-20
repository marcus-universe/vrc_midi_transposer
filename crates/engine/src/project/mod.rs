//! Projects: the model, the on-disk store and the v1 `config.json` importer.

pub mod legacy;
mod model;
mod store;

pub use model::*;
pub use store::{file_name_for, RecentFile, Settings, Startup, Store, EXTENSION};
