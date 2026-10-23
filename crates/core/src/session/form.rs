//! Session Options' fields as typed, and back into a `Session` (checked).

use crate::session::{Forward, ForwardKind, Protocol, Session};

/// A forward as typed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FwdRow {
    pub kind: ForwardKind,
    pub listen: String,
    pub host: String,
    pub port: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Form {
    pub name: String,
    pub protocol: Protocol,
    pub host: String,
    pub port: String,
    pub user: String,
    pub identity: String,
    pub keepalive: String,
    pub extra: String,
    /// Credential set name, "" = none.
    pub credential: String,
    /// Button Bar name, "" = the one picked on the bar itself.
    pub button_bar: String,
    pub log_file: String,
    pub log_append: bool,
    pub log_raw: bool,
    pub jump: String,
    pub forwards: Vec<FwdRow>,
    pub reconnect: bool,
    pub reconnect_delay: String,
    pub reconnect_tries: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormError {
    Host,
    Port,
    Keepalive,
    Extra,
    Jump,
    /// The n-th forward.
    Forward(usize),
    Reconnect,
}

/// A host ssh takes as one: not empty, no spaces, not an option (`-oProxyCommand=…`).
pub fn valid_host(host: &str) -> bool {
    !host.is_empty() && !host.contains(char::is_whitespace) && !host.starts_with('-')
}

/// `[address:]port`: a port 1…65535, an address without spaces.
fn valid_listen(s: &str) -> bool {
    let s = s.trim();
    let (addr, port) = s.rsplit_once(':').map_or(("", s), |(a, p)| (a, p));
    port.parse::<u16>().is_ok_and(|p| p > 0)
        && !addr.contains(char::is_whitespace)
        && !addr.starts_with('-')
        && (!s.contains(':') || !addr.is_empty())
}

impl FwdRow {
    /// The forward, if what is typed makes one.
    fn checked(&self) -> Option<Forward> {
        if !valid_listen(&self.listen) {
            return None;
        }
        let listen = self.listen.trim().to_string();
        if self.kind == ForwardKind::Dynamic {
            return Some(Forward {
                kind: self.kind,
                listen,
                host: String::new(),
                port: 0,
            });
        }
        let host = self.host.trim();
        let port = self.port.trim().parse::<u16>().ok().filter(|&p| p > 0)?;
        valid_host(host).then(|| Forward {
            kind: self.kind,
            listen,
            host: host.to_string(),
            port,
        })
    }
}

impl Form {
    pub fn from_session(name: &str, s: &Session) -> Self {
        let text = |v: &Option<String>| v.clone().unwrap_or_default();
        Self {
            name: name.to_string(),
            protocol: s.protocol,
            host: s.host.clone(),
            port: s.port.map(|p| p.to_string()).unwrap_or_default(),
            user: text(&s.user),
            identity: text(&s.identity),
            keepalive: s.keepalive.map(|k| k.to_string()).unwrap_or_default(),
            extra: s.extra.clone(),
            credential: text(&s.credential),
            button_bar: text(&s.button_bar),
            log_file: s.log_file.clone(),
            log_append: s.log_append,
            log_raw: s.log_raw,
            jump: s.jump.clone(),
            forwards: (s.forwards.iter())
                .map(|f| FwdRow {
                    kind: f.kind,
                    listen: f.listen.clone(),
                    host: f.host.clone(),
                    port: match f.kind {
                        ForwardKind::Dynamic => String::new(),
                        _ => f.port.to_string(),
                    },
                })
                .collect(),
            reconnect: s.reconnect,
            reconnect_delay: s.reconnect_delay.map(|n| n.to_string()).unwrap_or_default(),
            reconnect_tries: s.reconnect_tries.map(|n| n.to_string()).unwrap_or_default(),
        }
    }

    /// `base` with the fields put in (what the form doesn't show kept). `need_host`: false for
    /// the Default template, which has none.
    pub fn apply(&self, base: &Session, need_host: bool) -> Result<Session, FormError> {
        let opt = |v: &str| Some(v.trim().to_string()).filter(|v| !v.is_empty());
        let host = self.host.trim();
        // A local shell has no host.
        let need_host = need_host && self.protocol == Protocol::Ssh2;
        if (need_host || !host.is_empty()) && !valid_host(host) {
            return Err(FormError::Host);
        }
        let port = match opt(&self.port) {
            None => None,
            Some(p) => Some(p.parse().ok().filter(|&p| p > 0).ok_or(FormError::Port)?),
        };
        let keepalive = match opt(&self.keepalive) {
            None => None,
            Some(k) => Some(k.parse().map_err(|_| FormError::Keepalive)?),
        };
        let jump = self.jump.trim().to_string();
        // `[user@]host[:port]`, comma between: each one a host ssh takes as one.
        if !jump.is_empty()
            && !jump
                .split(',')
                .all(|j| valid_host(j.trim()) && j.trim() == j)
        {
            return Err(FormError::Jump);
        }
        let forwards = (self.forwards.iter().enumerate())
            .map(|(i, r)| r.checked().ok_or(FormError::Forward(i)))
            .collect::<Result<Vec<_>, _>>()?;
        let count = |v: &str| -> Result<Option<u32>, FormError> {
            match opt(v) {
                None => Ok(None),
                Some(n) => n.parse().map(Some).map_err(|_| FormError::Reconnect),
            }
        };
        let (reconnect_delay, reconnect_tries) =
            (count(&self.reconnect_delay)?, count(&self.reconnect_tries)?);
        let extra = self.extra.trim().to_string();
        shlex::split(&extra).ok_or(FormError::Extra)?;
        Ok(Session {
            protocol: self.protocol,
            host: host.to_string(),
            port,
            user: opt(&self.user),
            identity: opt(&self.identity),
            keepalive,
            extra,
            credential: opt(&self.credential),
            button_bar: opt(&self.button_bar),
            log_file: self.log_file.trim().to_string(),
            log_append: self.log_append,
            log_raw: self.log_raw,
            jump,
            forwards,
            reconnect: self.reconnect,
            reconnect_delay,
            reconnect_tries,
            ..base.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(host: &str) -> Form {
        Form {
            host: host.into(),
            ..Default::default()
        }
    }

    #[test]
    fn round_trip() {
        let s = Session {
            host: "h".into(),
            port: Some(2222),
            user: Some("u".into()),
            identity: Some("~/.ssh/k".into()),
            keepalive: Some(30),
            extra: "-o A=1".into(),
            ..Default::default()
        };
        let f = Form::from_session("n", &s);
        assert_eq!(f.name, "n");
        assert_eq!(f.port, "2222");
        assert_eq!(f.apply(&Session::default(), true), Ok(s));
    }

    #[test]
    fn empty_fields_are_none() {
        let f = Form {
            user: "  ".into(),
            port: " ".into(),
            ..form(" h ")
        };
        assert_eq!(
            f.apply(&Session::default(), true),
            Ok(Session {
                host: "h".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn keeps_what_the_form_does_not_show() {
        let mut base = Session::default();
        base.rest.insert("future_field".into(), "x".into());
        assert_eq!(form("h").apply(&base, true).unwrap().rest, base.rest);
    }

    #[test]
    fn local_shell_needs_no_host() {
        let f = Form {
            protocol: Protocol::Local,
            ..Default::default()
        };
        let s = f.apply(&Session::default(), true).unwrap();
        assert_eq!(s.protocol, Protocol::Local);
        assert_eq!(
            Form::default().apply(&Session::default(), true),
            Err(FormError::Host)
        );
        let text = toml::to_string(&s).unwrap();
        assert!(text.contains("protocol = \"local\""), "{text}");
        assert!(
            !toml::to_string(&Session::default())
                .unwrap()
                .contains("protocol")
        );
    }

    #[test]
    fn jump_forwards_and_reconnect() {
        let row = |kind, listen: &str, host: &str, port: &str| FwdRow {
            kind,
            listen: listen.into(),
            host: host.into(),
            port: port.into(),
        };
        let f = Form {
            jump: "u@b:22,j2".into(),
            forwards: vec![
                row(ForwardKind::Local, "8080", "db", "5432"),
                row(ForwardKind::Dynamic, "127.0.0.1:1080", "", ""),
            ],
            reconnect: true,
            reconnect_delay: " 3 ".into(),
            ..form("h")
        };
        let s = f.apply(&Session::default(), true).unwrap();
        assert_eq!(s.jump, "u@b:22,j2");
        assert_eq!(s.forwards.len(), 2);
        assert_eq!(
            (s.reconnect, s.reconnect_delay, s.reconnect_tries),
            (true, Some(3), None)
        );
        assert_eq!(Form::from_session("n", &s).forwards[1].port, "");
        let text = toml::to_string(&s).unwrap();
        assert!(text.contains("[[forward]]"), "{text}");
        assert_eq!(toml::from_str::<Session>(&text).unwrap(), s);
        // each wrong thing is told apart
        let bad = |f: Form| f.apply(&Session::default(), true).unwrap_err();
        assert_eq!(
            bad(Form {
                jump: "-oX=1".into(),
                ..form("h")
            }),
            FormError::Jump
        );
        assert_eq!(
            bad(Form {
                jump: "a b".into(),
                ..form("h")
            }),
            FormError::Jump
        );
        for (l, h, p) in [
            ("0", "db", "1"),
            ("x", "db", "1"),
            ("8080", "", "1"),
            ("8080", "db", "0"),
            ("8080", "-x", "1"),
            (":80", "db", "1"),
        ] {
            let fw = vec![row(ForwardKind::Local, l, h, p)];
            assert_eq!(
                bad(Form {
                    forwards: fw,
                    ..form("h")
                }),
                FormError::Forward(0),
                "{l} {h} {p}"
            );
        }
        assert_eq!(
            bad(Form {
                reconnect_tries: "x".into(),
                ..form("h")
            }),
            FormError::Reconnect
        );
    }

    #[test]
    fn bad_port() {
        for p in ["0", "70000", "x", "-1"] {
            let f = Form {
                port: p.into(),
                ..form("h")
            };
            assert_eq!(
                f.apply(&Session::default(), true),
                Err(FormError::Port),
                "{p}"
            );
        }
    }

    #[test]
    fn bad_keepalive() {
        let f = Form {
            keepalive: "10s".into(),
            ..form("h")
        };
        assert_eq!(
            f.apply(&Session::default(), true),
            Err(FormError::Keepalive)
        );
    }

    #[test]
    fn bad_extra() {
        let f = Form {
            extra: "-o 'A=1".into(),
            ..form("h")
        };
        assert_eq!(f.apply(&Session::default(), true), Err(FormError::Extra));
    }

    #[test]
    fn host_required_unless_default() {
        assert_eq!(
            form("").apply(&Session::default(), true),
            Err(FormError::Host)
        );
        assert_eq!(
            form("").apply(&Session::default(), false),
            Ok(Session::default())
        );
        assert_eq!(
            form("a b").apply(&Session::default(), false),
            Err(FormError::Host)
        );
    }

    #[test]
    fn host_with_dash_rejected() {
        let f = form("-oProxyCommand=x");
        assert_eq!(f.apply(&Session::default(), true), Err(FormError::Host));
    }
}
