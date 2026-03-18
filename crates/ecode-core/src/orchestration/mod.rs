//! Orchestration engine — decider, projector, and engine.

mod decider;
mod engine;
mod projector;

pub use decider::decide;
pub use engine::OrchestrationEngine;
pub use projector::apply;
