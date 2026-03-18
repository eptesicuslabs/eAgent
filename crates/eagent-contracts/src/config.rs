use crate::oversight::OversightMode;
use crate::provider::ProviderKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level eAgent application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub agent_defaults: AgentDefaults,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub projects: ProjectsConfig,
}

/// General UI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self { theme: default_theme(), font_size: default_font_size() }
    }
}

fn default_theme() -> String { "dark".into() }
fn default_font_size() -> f32 { 14.0 }

/// Default settings for agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefaults {
    #[serde(default = "default_planner_provider")]
    pub planner_provider: String,
    #[serde(default = "default_worker_provider")]
    pub worker_provider: String,
    #[serde(default)]
    pub fallback_provider: Option<String>,
    #[serde(default)]
    pub oversight_mode: OversightMode,
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: u32,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

impl Default for AgentDefaults {
    fn default() -> Self {
        Self {
            planner_provider: default_planner_provider(),
            worker_provider: default_worker_provider(),
            fallback_provider: None,
            oversight_mode: OversightMode::default(),
            max_concurrency: default_max_concurrency(),
            max_retries: default_max_retries(),
        }
    }
}

fn default_planner_provider() -> String { "codex".into() }
fn default_worker_provider() -> String { "codex".into() }
fn default_max_concurrency() -> u32 { 4 }
fn default_max_retries() -> u32 { 2 }

/// Configuration for a single provider instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_concurrent_sessions")]
    pub max_concurrent_sessions: u32,
    #[serde(default)]
    pub default_model: String,
    #[serde(flatten)]
    pub specific: ProviderSpecificConfig,
}

fn default_true() -> bool { true }
fn default_concurrent_sessions() -> u32 { 4 }

/// Provider-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider_type", rename_all = "snake_case")]
pub enum ProviderSpecificConfig {
    Codex {
        #[serde(default)]
        binary_path: String,
        #[serde(default)]
        home_dir: String,
    },
    LlamaCpp {
        #[serde(default)]
        server_binary_path: String,
        #[serde(default)]
        model_path: String,
        #[serde(default = "default_llama_host")]
        host: String,
        #[serde(default = "default_llama_port")]
        port: u16,
        #[serde(default = "default_ctx_size")]
        ctx_size: u32,
        #[serde(default = "default_llama_threads")]
        threads: u16,
        #[serde(default)]
        gpu_layers: i32,
    },
    ApiKey {
        endpoint: String,
        #[serde(default)]
        api_key: String,
        #[serde(default)]
        models: Vec<String>,
        #[serde(default = "default_api_max_context")]
        max_context: u32,
    },
}

fn default_llama_host() -> String { "127.0.0.1".into() }
fn default_llama_port() -> u16 { 8012 }
fn default_ctx_size() -> u32 { 4096 }
fn default_llama_threads() -> u16 {
    std::thread::available_parallelism()
        .map(|p| p.get().saturating_sub(2).clamp(1, 8) as u16)
        .unwrap_or(4)
}
fn default_api_max_context() -> u32 { 128_000 }

/// Projects configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectsConfig {
    #[serde(default)]
    pub entries: Vec<ProjectEntry>,
}

/// A single project entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub default_provider: Option<String>,
}
