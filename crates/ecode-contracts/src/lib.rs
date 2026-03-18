//! eCode Contracts — shared domain types, schemas, and protocol definitions.
//!
//! This crate defines all the types used across eCode:
//! - Domain IDs (ThreadId, TurnId, etc.)
//! - Codex JSON-RPC protocol types
//! - Orchestration domain (commands, events, read model)
//! - Terminal, Git, and configuration types

pub mod codex;
pub mod config;
pub mod git;
pub mod ids;
pub mod orchestration;
pub mod persistence;
pub mod provider;
pub mod provider_runtime;
pub mod terminal;
