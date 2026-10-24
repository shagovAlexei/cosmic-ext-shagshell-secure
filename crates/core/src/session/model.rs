//! What a session file holds. Every field has a default, unknown ones are kept as they are: files
//! from older and newer versions both load, and saving one doesn't lose what a newer version wrote.

use serde::{Deserialize, Serialize};

/// A port forward (`ssh -L / -R / -D`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ForwardKind {
    /// `-L listen:host:port`: a local port to a host reached from the server.
    #[default]
    Local,
    /// `-R listen:host:port`: a server's port to a host reached from here.
    Remote,
    /// `-D listen`: a local SOCKS proxy through the server.
    Dynamic,
}

impl ForwardKind {
    pub const ALL: [ForwardKind; 3] = [
        ForwardKind::Local,
        ForwardKind::Remote,
        ForwardKind::Dynamic,
    ];
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Forward {
    pub kind: ForwardKind,
    /// `[address:]port` that listens.
    pub listen: String,
    /// Where it leads (not for `Dynamic`).
    pub host: String,
    pub port: u16,
}

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
    /// `telnet host [port]`.
    Telnet,
    /// `picocom` on a device: no host.
    Serial,
}

impl Protocol {
    pub const ALL: [Protocol; 4] = [
        Protocol::Ssh2,
        Protocol::Local,
        Protocol::Telnet,
        Protocol::Serial,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Parity {
    #[default]
    None,
    Even,
    Odd,
}

impl Parity {
    pub const ALL: [Parity; 3] = [Parity::None, Parity::Even, Parity::Odd];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Flow {
    #[default]
    None,
    Hardware,
    Software,
}

impl Flow {
    pub const ALL: [Flow; 3] = [Flow::None, Flow::Hardware, Flow::Software];
}

/// A serial line (`[serial]` in the session file). Absent numbers: 9600 8N1.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Serial {
    pub device: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baud: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_bits: Option<u8>,
    #[serde(skip_serializing_if = "is_default_parity")]
    pub parity: Parity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_bits: Option<u8>,
    #[serde(skip_serializing_if = "is_default_flow")]
    pub flow: Flow,
}

fn is_default_parity(p: &Parity) -> bool {
    *p == Parity::None
}

fn is_default_flow(f: &Flow) -> bool {
    *f == Flow::None
}

impl Serial {
    fn is_default(&self) -> bool {
        *self == Serial::default()
    }
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
    #[serde(skip_serializing_if = "Serial::is_default")]
    pub serial: Serial,
    /// The tab's text instead of the program's long title; empty: the program's, else the session name.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// Terminal colour scheme (a key of the built-in ones); empty: the global one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub color_scheme: String,
    /// Hosts to go through (`ssh -J`): `[user@]host[:port]`, comma between several.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub jump: String,
    #[serde(rename = "forward", skip_serializing_if = "Vec::is_empty")]
    pub forwards: Vec<Forward>,
    /// Connect again by itself when the connection drops (after it had been up for a while).
    #[serde(skip_serializing_if = "is_false")]
    pub reconnect: bool,
    /// Seconds before that (default 5) and how many times in a row (default 10).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconnect_delay: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconnect_tries: Option<u32>,
    /// Logon Actions: what to answer to what the session says after connecting.
    #[serde(rename = "logon", skip_serializing_if = "Vec::is_empty")]
    pub logon: Vec<crate::logon::Step>,
    /// A Python script run on the tab when the session is opened (`core::script`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub script: String,
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
