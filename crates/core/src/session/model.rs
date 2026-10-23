//! What a session file holds. Every field has a default, unknown ones are kept as they are: files
//! from older and newer versions both load, and saving one doesn't lose what a newer version wrote.

use serde::{Deserialize, Serialize};

/// What a session connects with (Session Options → Protocol).
fn is_false(b: &bool) -> bool {
    !*b
}

fn is_ssh2(p: &Protocol) -> bool {
    *p == Protocol::Ssh2
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Ssh2,
    /// The user's `$SHELL` in a tab: no host.
    Local,
}

impl Protocol {
    pub const ALL: [Protocol; 2] = [Protocol::Ssh2, Protocol::Local];
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    #[serde(skip_serializing_if = "is_ssh2")]
    pub protocol: Protocol,
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
    /// The Button Bar shown while this session's tab is in front (`buttonbars.toml`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button_bar: Option<String>,
    /// Log file name template (`core::logfile`); empty: no log on connect.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub log_file: String,
    /// Add to the file instead of starting it over.
    #[serde(skip_serializing_if = "is_false")]
    pub log_append: bool,
    /// The bytes as they came, escape sequences and all.
    #[serde(skip_serializing_if = "is_false")]
    pub log_raw: bool,
    /// Fields this version doesn't know, written back unchanged.
    #[serde(flatten)]
    pub rest: toml::Table,
}
