//! eAgent Contracts — shared non-protocol domain types.
//!
//! Configuration, provider metadata, oversight model, and UI DTOs.
//! Does NOT contain AgentMessage or TaskGraph types (those live in eagent-protocol).

pub mod config;
pub mod oversight;
pub mod provider;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oversight::OversightMode;
    use eagent_protocol::messages::RiskLevel;

    #[test]
    fn oversight_full_autonomy_never_requires_approval() {
        let mode = OversightMode::FullAutonomy;
        assert!(!mode.requires_approval(RiskLevel::Low));
        assert!(!mode.requires_approval(RiskLevel::Medium));
        assert!(!mode.requires_approval(RiskLevel::High));
    }

    #[test]
    fn oversight_approve_risky_skips_low() {
        let mode = OversightMode::ApproveRisky;
        assert!(!mode.requires_approval(RiskLevel::Low));
        assert!(mode.requires_approval(RiskLevel::Medium));
        assert!(mode.requires_approval(RiskLevel::High));
    }

    #[test]
    fn oversight_approve_all_always_requires() {
        let mode = OversightMode::ApproveAll;
        assert!(mode.requires_approval(RiskLevel::Low));
        assert!(mode.requires_approval(RiskLevel::Medium));
        assert!(mode.requires_approval(RiskLevel::High));
    }

    #[test]
    fn agent_config_default_serde_roundtrip() {
        let config = config::AgentConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let back: config::AgentConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.general.theme, "dark");
        assert_eq!(back.agent_defaults.max_concurrency, 4);
    }

    #[test]
    fn provider_event_serde() {
        let evt = provider::ProviderEvent::TokenDelta { text: "hello".into() };
        let json = serde_json::to_value(&evt).unwrap();
        assert_eq!(json["type"], "token_delta");
    }
}
