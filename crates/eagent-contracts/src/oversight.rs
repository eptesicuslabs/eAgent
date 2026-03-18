use eagent_protocol::messages::RiskLevel;
use serde::{Deserialize, Serialize};

/// Oversight mode controlling when agents must ask for approval.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OversightMode {
    /// Agents execute all tool calls without asking.
    FullAutonomy,
    /// Auto-proceed on Low risk, ask approval for Medium and High.
    #[default]
    ApproveRisky,
    /// Every tool call requires explicit approval.
    ApproveAll,
}

impl OversightMode {
    /// Whether a tool call at the given risk level requires human approval.
    pub fn requires_approval(&self, risk: RiskLevel) -> bool {
        match self {
            OversightMode::FullAutonomy => false,
            OversightMode::ApproveRisky => matches!(risk, RiskLevel::Medium | RiskLevel::High),
            OversightMode::ApproveAll => true,
        }
    }
}
