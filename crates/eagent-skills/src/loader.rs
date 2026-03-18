//! eSkill loader — loads skill directories, matches trigger patterns, provides access.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;
use tracing::{debug, info, warn};

use crate::manifest::SkillManifest;

/// A skill that has been loaded from disk.
#[derive(Debug, Clone)]
pub struct LoadedSkill {
    /// Parsed manifest.
    pub manifest: SkillManifest,
    /// The system prompt content (from `system_prompt.md`).
    pub system_prompt: String,
    /// Optional tools configuration (from `tools.json`).
    pub tools_config: Option<Value>,
}

/// Errors that can occur when loading eSkills.
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("manifest.json not found in skill directory: {0}")]
    ManifestNotFound(String),

    #[error("system_prompt.md not found in skill directory: {0}")]
    SystemPromptNotFound(String),

    #[error("failed to read file {path}: {source}")]
    IoError {
        path: String,
        source: std::io::Error,
    },

    #[error("failed to parse manifest.json in {path}: {source}")]
    ManifestParseError {
        path: String,
        source: serde_json::Error,
    },

    #[error("failed to parse tools.json in {path}: {source}")]
    ToolsParseError {
        path: String,
        source: serde_json::Error,
    },
}

/// Loads and manages eSkills from the filesystem.
pub struct SkillLoader {
    skills: HashMap<String, LoadedSkill>,
}

impl SkillLoader {
    /// Create a new empty skill loader.
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Load a skill from a directory.
    ///
    /// The directory must contain:
    /// - `manifest.json` — skill manifest (required)
    /// - `system_prompt.md` — system prompt text (required)
    /// - `tools.json` — optional tools configuration
    ///
    /// Returns the skill name on success.
    pub fn load_from_dir(&mut self, path: &Path) -> Result<String, SkillError> {
        let dir_str = path.display().to_string();

        // Read and parse manifest.json
        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            return Err(SkillError::ManifestNotFound(dir_str));
        }

        let manifest_content =
            std::fs::read_to_string(&manifest_path).map_err(|e| SkillError::IoError {
                path: manifest_path.display().to_string(),
                source: e,
            })?;

        let manifest: SkillManifest =
            serde_json::from_str(&manifest_content).map_err(|e| SkillError::ManifestParseError {
                path: manifest_path.display().to_string(),
                source: e,
            })?;

        // Read system_prompt.md
        let prompt_path = path.join("system_prompt.md");
        if !prompt_path.exists() {
            return Err(SkillError::SystemPromptNotFound(dir_str));
        }

        let system_prompt =
            std::fs::read_to_string(&prompt_path).map_err(|e| SkillError::IoError {
                path: prompt_path.display().to_string(),
                source: e,
            })?;

        // Optionally read tools.json
        let tools_path = path.join("tools.json");
        let tools_config = if tools_path.exists() {
            let content =
                std::fs::read_to_string(&tools_path).map_err(|e| SkillError::IoError {
                    path: tools_path.display().to_string(),
                    source: e,
                })?;
            let value: Value =
                serde_json::from_str(&content).map_err(|e| SkillError::ToolsParseError {
                    path: tools_path.display().to_string(),
                    source: e,
                })?;
            Some(value)
        } else {
            debug!(skill = %manifest.name, "no tools.json found, skipping");
            None
        };

        let name = manifest.name.clone();
        info!(skill = %name, version = %manifest.version, "loaded eSkill");

        self.skills.insert(
            name.clone(),
            LoadedSkill {
                manifest,
                system_prompt,
                tools_config,
            },
        );

        Ok(name)
    }

    /// Load all skills from a parent directory.
    ///
    /// Each immediate subdirectory of `skills_dir` is treated as a potential skill
    /// directory. Subdirectories that fail to load are warned about and skipped.
    ///
    /// Returns the list of successfully loaded skill names.
    pub fn load_all(&mut self, skills_dir: &Path) -> Result<Vec<String>, SkillError> {
        let mut loaded = Vec::new();

        let entries = std::fs::read_dir(skills_dir).map_err(|e| SkillError::IoError {
            path: skills_dir.display().to_string(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| SkillError::IoError {
                path: skills_dir.display().to_string(),
                source: e,
            })?;

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            match self.load_from_dir(&path) {
                Ok(name) => loaded.push(name),
                Err(e) => {
                    warn!(
                        dir = %path.display(),
                        error = %e,
                        "skipping skill directory that failed to load"
                    );
                }
            }
        }

        info!(count = loaded.len(), "loaded eSkills from directory");
        Ok(loaded)
    }

    /// Find skills whose trigger patterns match the query (case-insensitive substring).
    pub fn find_matching(&self, query: &str) -> Vec<&LoadedSkill> {
        let query_lower = query.to_lowercase();

        self.skills
            .values()
            .filter(|skill| {
                skill
                    .manifest
                    .trigger_patterns
                    .iter()
                    .any(|pattern| query_lower.contains(&pattern.to_lowercase()))
            })
            .collect()
    }

    /// Get a skill by exact name.
    pub fn get(&self, name: &str) -> Option<&LoadedSkill> {
        self.skills.get(name)
    }

    /// List manifests of all loaded skills.
    pub fn list(&self) -> Vec<&SkillManifest> {
        self.skills.values().map(|s| &s.manifest).collect()
    }
}

impl Default for SkillLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::SkillMode;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a valid skill directory with manifest + system_prompt.
    fn create_skill_dir(parent: &Path, name: &str, triggers: &[&str]) -> std::path::PathBuf {
        let dir = parent.join(name);
        fs::create_dir_all(&dir).unwrap();

        let manifest = serde_json::json!({
            "name": name,
            "version": "1.0.0",
            "description": format!("Test skill: {name}"),
            "trigger_patterns": triggers,
            "required_tools": ["read_file"],
            "mode": "ecode"
        });
        fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
        fs::write(
            dir.join("system_prompt.md"),
            format!("You are the {name} skill."),
        )
        .unwrap();

        dir
    }

    #[test]
    fn load_skill_from_dir() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = create_skill_dir(tmp.path(), "eskill-test", &["test", "check"]);

        let mut loader = SkillLoader::new();
        let name = loader.load_from_dir(&skill_dir).unwrap();

        assert_eq!(name, "eskill-test");

        let skill = loader.get("eskill-test").unwrap();
        assert_eq!(skill.manifest.version, "1.0.0");
        assert_eq!(skill.system_prompt, "You are the eskill-test skill.");
        assert!(skill.tools_config.is_none());
    }

    #[test]
    fn load_skill_with_tools_config() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = create_skill_dir(tmp.path(), "eskill-with-tools", &["review"]);

        let tools = serde_json::json!({
            "tools": [
                { "name": "read_file", "required": true },
                { "name": "git_diff", "required": false }
            ]
        });
        fs::write(skill_dir.join("tools.json"), tools.to_string()).unwrap();

        let mut loader = SkillLoader::new();
        loader.load_from_dir(&skill_dir).unwrap();

        let skill = loader.get("eskill-with-tools").unwrap();
        assert!(skill.tools_config.is_some());
        let config = skill.tools_config.as_ref().unwrap();
        assert!(config["tools"].is_array());
    }

    #[test]
    fn trigger_pattern_matching_case_insensitive() {
        let tmp = TempDir::new().unwrap();
        create_skill_dir(tmp.path(), "eskill-review", &["review", "code review"]);
        create_skill_dir(tmp.path(), "eskill-test", &["test", "run tests"]);

        let mut loader = SkillLoader::new();
        loader.load_all(tmp.path()).unwrap();

        // "review" should match eskill-review
        let matches = loader.find_matching("Please review my code");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].manifest.name, "eskill-review");

        // Case-insensitive: "REVIEW" should also match
        let matches = loader.find_matching("REVIEW this PR");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].manifest.name, "eskill-review");

        // "run tests" should match eskill-test
        let matches = loader.find_matching("Can you run tests?");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].manifest.name, "eskill-test");

        // No match
        let matches = loader.find_matching("deploy to production");
        assert!(matches.is_empty());
    }

    #[test]
    fn load_all_from_parent_dir() {
        let tmp = TempDir::new().unwrap();
        create_skill_dir(tmp.path(), "eskill-a", &["alpha"]);
        create_skill_dir(tmp.path(), "eskill-b", &["beta"]);

        // Also create a file (not a directory) — should be skipped
        fs::write(tmp.path().join("not-a-skill.txt"), "hi").unwrap();

        let mut loader = SkillLoader::new();
        let loaded = loader.load_all(tmp.path()).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loader.list().len(), 2);
    }

    #[test]
    fn missing_manifest_error() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("bad-skill");
        fs::create_dir_all(&dir).unwrap();
        // Only create system_prompt.md, no manifest.json
        fs::write(dir.join("system_prompt.md"), "prompt").unwrap();

        let mut loader = SkillLoader::new();
        let err = loader.load_from_dir(&dir).unwrap_err();
        assert!(matches!(err, SkillError::ManifestNotFound(_)));
    }

    #[test]
    fn missing_system_prompt_error() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("no-prompt-skill");
        fs::create_dir_all(&dir).unwrap();

        let manifest = serde_json::json!({
            "name": "no-prompt",
            "version": "0.1.0",
            "description": "Missing prompt",
            "trigger_patterns": [],
            "required_tools": [],
            "mode": "both"
        });
        fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
        // No system_prompt.md

        let mut loader = SkillLoader::new();
        let err = loader.load_from_dir(&dir).unwrap_err();
        assert!(matches!(err, SkillError::SystemPromptNotFound(_)));
    }

    #[test]
    fn list_returns_all_manifests() {
        let tmp = TempDir::new().unwrap();
        create_skill_dir(tmp.path(), "eskill-one", &["one"]);
        create_skill_dir(tmp.path(), "eskill-two", &["two"]);

        let mut loader = SkillLoader::new();
        loader.load_all(tmp.path()).unwrap();

        let manifests = loader.list();
        assert_eq!(manifests.len(), 2);

        let names: Vec<&str> = manifests.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"eskill-one"));
        assert!(names.contains(&"eskill-two"));
    }

    #[test]
    fn skill_mode_variants() {
        let tmp = TempDir::new().unwrap();

        for (name, mode) in [("ecode-skill", "ecode"), ("ework-skill", "ework"), ("both-skill", "both")] {
            let dir = tmp.path().join(name);
            fs::create_dir_all(&dir).unwrap();
            let manifest = serde_json::json!({
                "name": name,
                "version": "1.0.0",
                "description": "mode test",
                "trigger_patterns": [name],
                "required_tools": [],
                "mode": mode
            });
            fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
            fs::write(dir.join("system_prompt.md"), "prompt").unwrap();
        }

        let mut loader = SkillLoader::new();
        loader.load_all(tmp.path()).unwrap();

        assert_eq!(loader.get("ecode-skill").unwrap().manifest.mode, SkillMode::Ecode);
        assert_eq!(loader.get("ework-skill").unwrap().manifest.mode, SkillMode::Ework);
        assert_eq!(loader.get("both-skill").unwrap().manifest.mode, SkillMode::Both);
    }
}
