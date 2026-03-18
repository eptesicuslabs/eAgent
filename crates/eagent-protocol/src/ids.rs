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
            pub fn new() -> Self { Self(Uuid::new_v4()) }
            pub fn from_uuid(uuid: Uuid) -> Self { Self(uuid) }
            pub fn parse(s: &str) -> Result<Self, uuid::Error> { Ok(Self(Uuid::parse_str(s)?)) }
            pub fn inner(&self) -> Uuid { self.0 }
        }

        impl Default for $name {
            fn default() -> Self { Self::new() }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

define_id!(TaskId, "Unique identifier for a task within a TaskGraph.");
define_id!(TaskGraphId, "Unique identifier for a TaskGraph (one user request).");
define_id!(AgentId, "Unique identifier for an agent instance.");
define_id!(ProviderId, "Unique identifier for a configured provider.");
define_id!(TerminalId, "Unique identifier for a terminal instance.");
define_id!(ThreadId, "Unique identifier for a conversation thread (legacy compat).");
define_id!(SessionId, "Unique identifier for a provider session.");
