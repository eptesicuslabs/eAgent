//! eAgent Persistence — event store, config loading, and session state management.

pub mod config;
pub mod event_store;

pub use config::ConfigManager;
pub use event_store::{EventStore, StoredEvent};
