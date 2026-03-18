//! eSkill manifest definition and parsing.

use serde::{Deserialize, Serialize};

/// Describes a packaged eSkill — its identity, trigger patterns, required tools, and mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// Unique skill name (e.g. "eskill-code-review").
    pub name: String,
    /// Semver version string.
    pub version: String,
    /// Human-readable description of what this skill does.
    pub description: String,
    /// Case-insensitive substring patterns that trigger this skill.
    /// Plain strings, not regex — simplicity over cleverness.
    pub trigger_patterns: Vec<String>,
    /// Tools that must be available for this skill to function.
    pub required_tools: Vec<String>,
    /// Which workstation mode(s) this skill applies to.
    pub mode: SkillMode,
}

/// The workstation mode(s) an eSkill is designed for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillMode {
    /// Only available in eCode (coding workstation).
    Ecode,
    /// Only available in eWork (general-purpose workstation).
    Ework,
    /// Available in both modes.
    Both,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_serde_roundtrip() {
        let manifest = SkillManifest {
            name: "eskill-code-review".into(),
            version: "1.0.0".into(),
            description: "Reviews code for bugs, security issues, and style".into(),
            trigger_patterns: vec![
                "review".into(),
                "code review".into(),
                "check my code".into(),
            ],
            required_tools: vec![
                "read_file".into(),
                "search_files".into(),
                "git_diff".into(),
            ],
            mode: SkillMode::Ecode,
        };

        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let back: SkillManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(back.name, "eskill-code-review");
        assert_eq!(back.version, "1.0.0");
        assert_eq!(back.trigger_patterns.len(), 3);
        assert_eq!(back.required_tools.len(), 3);
        assert_eq!(back.mode, SkillMode::Ecode);
    }

    #[test]
    fn skill_mode_serde() {
        assert_eq!(
            serde_json::to_string(&SkillMode::Ecode).unwrap(),
            "\"ecode\""
        );
        assert_eq!(
            serde_json::to_string(&SkillMode::Ework).unwrap(),
            "\"ework\""
        );
        assert_eq!(
            serde_json::to_string(&SkillMode::Both).unwrap(),
            "\"both\""
        );
    }
}
