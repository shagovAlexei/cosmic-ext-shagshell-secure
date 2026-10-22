//! What a session file holds. Every field has a default, unknown ones are ignored: files from
//! older and newer versions both load.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Private key file; `~` allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
    /// ServerAliveInterval, seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keepalive: Option<u32>,
    /// More ssh options, split as a shell would.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub extra: String,
}
