//! What a session file holds. Every field has a default, unknown ones are kept as they are: files
//! from older and newer versions both load, and saving one doesn't lose what a newer version wrote.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    /// A credential set (`credentials.toml`): its user and password instead of the session's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
    /// Fields this version doesn't know, written back unchanged.
    #[serde(flatten)]
    pub rest: toml::Table,
}
