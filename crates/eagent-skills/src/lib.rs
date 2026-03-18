//! eAgent Skills — eSkill loading, manifest parsing, and agent capability packaging.
//!
//! eSkills are packaged prompts + tool configurations that give agents specialized
//! capabilities. Each skill lives in a directory containing:
//!
//! - `manifest.json` — identity, trigger patterns, required tools, mode
//! - `system_prompt.md` — the skill's system prompt
//! - `tools.json` (optional) — tool-specific configuration
//!
//! Skills are auto-triggered by case-insensitive substring matching against
//! `trigger_patterns`, or explicitly selected by the planner.

pub mod manifest;
pub mod loader;

pub use manifest::{SkillManifest, SkillMode};
pub use loader::{LoadedSkill, SkillLoader, SkillError};
