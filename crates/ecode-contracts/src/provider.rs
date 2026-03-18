//! Provider types for model and session management.

use serde::{Deserialize, Serialize};

/// The provider kind backing a thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    #[default]
    Codex,
    LlamaCpp,
}

/// Account type for the authenticated user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountType {
    ApiKey,
    Unknown,
}

/// Minimum required Codex CLI version.
pub const MIN_CODEX_VERSION: &str = "0.37.0";

/// Default Codex model.
pub const DEFAULT_CODEX_MODEL: &str = "gpt-5.4";

/// Fallback Codex model catalog used when live discovery is unavailable.
pub const FALLBACK_CODEX_MODELS: &[&str] = &[
    "gpt-5.2",
    "gpt-5.2-codex",
    "gpt-5.3-codex",
    "gpt-5.3-codex-spark",
    "gpt-5.4",
];

/// Default llama.cpp model label shown before a concrete local model is configured.
pub const DEFAULT_LLAMA_CPP_MODEL: &str = "local-model";

/// Spark model (for fast, lightweight tasks).
pub const SPARK_MODEL: &str = "codex-mini-latest";
