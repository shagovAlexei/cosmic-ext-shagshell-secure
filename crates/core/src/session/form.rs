//! Session Options' fields as typed, and back into a `Session` (checked).

use crate::session::Session;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Form {
    pub name: String,
    pub host: String,
    pub port: String,
    pub user: String,
    pub identity: String,
    pub keepalive: String,
    pub extra: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormError {
    Host,
    Port,
    Keepalive,
    Extra,
}

/// A host ssh takes as one: not empty, no spaces, not an option (`-oProxyCommand=…`).
pub fn valid_host(host: &str) -> bool {
    !host.is_empty() && !host.contains(char::is_whitespace) && !host.starts_with('-')
}

impl Form {
    pub fn from_session(name: &str, s: &Session) -> Self {
        let text = |v: &Option<String>| v.clone().unwrap_or_default();
        Self {
            name: name.to_string(),
            host: s.host.clone(),
            port: s.port.map(|p| p.to_string()).unwrap_or_default(),
            user: text(&s.user),
            identity: text(&s.identity),
            keepalive: s.keepalive.map(|k| k.to_string()).unwrap_or_default(),
            extra: s.extra.clone(),
        }
    }

    /// `base` with the fields put in (what the form doesn't show kept). `need_host`: false for
    /// the Default template, which has none.
    pub fn apply(&self, base: &Session, need_host: bool) -> Result<Session, FormError> {
        let opt = |v: &str| Some(v.trim().to_string()).filter(|v| !v.is_empty());
        let host = self.host.trim();
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
        let extra = self.extra.trim().to_string();
        shlex::split(&extra).ok_or(FormError::Extra)?;
        Ok(Session {
            host: host.to_string(),
            port,
            user: opt(&self.user),
            identity: opt(&self.identity),
            keepalive,
            extra,
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
        base.rest.insert("jump".into(), "bastion".into());
        assert_eq!(form("h").apply(&base, true).unwrap().rest, base.rest);
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
