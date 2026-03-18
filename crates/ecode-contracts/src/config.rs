//! Configuration types for eCode.

use crate::ids::ProjectId;
use crate::orchestration::{CodexReasoningEffort, InteractionMode, RuntimeMode, ThreadSettings};
use crate::provider::{DEFAULT_CODEX_MODEL, DEFAULT_LLAMA_CPP_MODEL, ProviderKind};
use serde::{Deserialize, Serialize};

/// Top-level application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub codex: CodexConfig,
    #[serde(default)]
    pub llama_cpp: LlamaCppConfig,
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
        Self {
            theme: default_theme(),
            font_size: default_font_size(),
        }
    }
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_font_size() -> f32 {
    14.0
}

/// Codex CLI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexConfig {
    /// Path to the codex binary (empty = find on PATH).
    #[serde(default)]
    pub binary_path: String,
    /// Override for CODEX_HOME directory.
    #[serde(default)]
    pub home_dir: String,
    /// Default model to use.
    #[serde(default = "default_model")]
    pub default_model: String,
    /// Default reasoning effort exposed in per-thread settings.
    #[serde(default)]
    pub default_reasoning_effort: CodexReasoningEffort,
    /// Whether new threads should prefer the fast Codex model.
    #[serde(default)]
    pub default_fast_mode: bool,
    /// Default interaction mode for new threads.
    #[serde(default)]
    pub default_interaction_mode: InteractionMode,
    /// Default runtime mode for new threads.
    #[serde(default)]
    pub default_runtime_mode: RuntimeMode,
}

impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            binary_path: String::new(),
            home_dir: String::new(),
            default_model: default_model(),
            default_reasoning_effort: CodexReasoningEffort::default(),
            default_fast_mode: false,
            default_interaction_mode: InteractionMode::default(),
            default_runtime_mode: RuntimeMode::default(),
        }
    }
}

fn default_model() -> String {
    DEFAULT_CODEX_MODEL.to_string()
}

/// Managed llama.cpp provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaCppConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub llama_server_binary_path: String,
    #[serde(default)]
    pub model_path: String,
    #[serde(default = "default_llama_host")]
    pub host: String,
    #[serde(default = "default_llama_port")]
    pub port: u16,
    #[serde(default = "default_ctx_size")]
    pub ctx_size: u32,
    #[serde(default = "default_llama_threads")]
    pub threads: u16,
    #[serde(default)]
    pub gpu_layers: i32,
    #[serde(default)]
    pub flash_attention: bool,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_top_p")]
    pub top_p: f32,
    #[serde(default = "default_llama_model_label")]
    pub default_model: String,
    #[serde(default)]
    pub default_runtime_mode: RuntimeMode,
    #[serde(default)]
    pub default_interaction_mode: InteractionMode,
    #[serde(default = "default_local_web_search")]
    pub default_local_agent_web_search_enabled: bool,
}

impl Default for LlamaCppConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            llama_server_binary_path: String::new(),
            model_path: String::new(),
            host: default_llama_host(),
            port: default_llama_port(),
            ctx_size: default_ctx_size(),
            threads: default_llama_threads(),
            gpu_layers: 0,
            flash_attention: false,
            temperature: default_temperature(),
            top_p: default_top_p(),
            default_model: default_llama_model_label(),
            default_runtime_mode: RuntimeMode::default(),
            default_interaction_mode: InteractionMode::default(),
            default_local_agent_web_search_enabled: default_local_web_search(),
        }
    }
}

fn default_llama_host() -> String {
    "127.0.0.1".to_string()
}

fn default_llama_port() -> u16 {
    8012
}

fn default_ctx_size() -> u32 {
    4096
}

fn default_llama_threads() -> u16 {
    std::thread::available_parallelism()
        .map(|parallelism| parallelism.get().saturating_sub(2).clamp(1, 8) as u16)
        .unwrap_or(4)
}

fn default_temperature() -> f32 {
    0.2
}

fn default_top_p() -> f32 {
    0.95
}

fn default_llama_model_label() -> String {
    DEFAULT_LLAMA_CPP_MODEL.to_string()
}

fn default_local_web_search() -> bool {
    true
}

impl AppConfig {
    /// Build the default per-thread settings for a newly created thread.
    pub fn default_thread_settings(&self, provider: ProviderKind) -> ThreadSettings {
        match provider {
            ProviderKind::Codex => ThreadSettings {
                provider,
                model: self.codex.default_model.clone(),
                runtime_mode: self.codex.default_runtime_mode,
                interaction_mode: self.codex.default_interaction_mode,
                codex_reasoning_effort: self.codex.default_reasoning_effort,
                codex_fast_mode: self.codex.default_fast_mode,
                local_agent_web_search_enabled: false,
            },
            ProviderKind::LlamaCpp => ThreadSettings {
                provider,
                model: self.llama_cpp.default_model.clone(),
                runtime_mode: self.llama_cpp.default_runtime_mode,
                interaction_mode: self.llama_cpp.default_interaction_mode,
                codex_reasoning_effort: CodexReasoningEffort::default(),
                codex_fast_mode: false,
                local_agent_web_search_enabled: self
                    .llama_cpp
                    .default_local_agent_web_search_enabled,
            },
        }
    }
}

/// Projects configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectsConfig {
    #[serde(default)]
    pub entries: Vec<ProjectEntry>,
}

/// A single project entry in the configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub id: ProjectId,
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub scripts: Vec<ProjectScript>,
}

/// A custom script for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectScript {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub icon: Option<String>,
}

/// Keybinding definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub action: String,
    pub key: String,
    #[serde(default)]
    pub modifiers: Vec<String>,
}
