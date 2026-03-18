//! Strongly-typed domain identifiers.
//!
//! All IDs are newtype wrappers around `uuid::Uuid` providing type safety
//! so you can't accidentally pass a ThreadId where a TurnId is expected.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Generate a new random ID.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Create from an existing UUID.
            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Parse from a string.
            pub fn parse(s: &str) -> Result<Self, uuid::Error> {
                Ok(Self(Uuid::parse_str(s)?))
            }

            /// Get the inner UUID.
            pub fn inner(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

define_id!(ThreadId, "Unique identifier for a conversation thread.");
define_id!(
    TurnId,
    "Unique identifier for a single turn within a thread."
);
define_id!(
    ProjectId,
    "Unique identifier for a registered project/workspace."
);
define_id!(
    SessionId,
    "Unique identifier for a Codex CLI provider session."
);
define_id!(
    ApprovalRequestId,
    "Unique identifier for a pending approval request."
);
define_id!(TerminalId, "Unique identifier for a terminal instance.");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let a = ThreadId::new();
        let b = ThreadId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn id_roundtrip_serde() {
        let id = ThreadId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: ThreadId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn id_display() {
        let id = ThreadId::new();
        let s = id.to_string();
        let parsed = ThreadId::parse(&s).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn different_id_types_not_mixable() {
        // This is a compile-time check — if it compiles, the types are distinct.
        let thread_id = ThreadId::new();
        let turn_id = TurnId::new();
        // These have different types, so you can't accidentally swap them.
        assert_ne!(thread_id.inner(), turn_id.inner());
    }
}
