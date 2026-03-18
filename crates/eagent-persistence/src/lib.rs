//! eAgent Persistence — event store, config loading, and session state management.

pub mod event_store;

pub use event_store::{EventStore, StoredEvent};
