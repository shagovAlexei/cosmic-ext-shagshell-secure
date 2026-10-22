//! Saved passwords in the Secret Service keyring (gnome-keyring, KWallet…), over D-Bus (zbus), no
//! libsecret / glib in the build. Adapted from shagoff-commander (`crates/core/src/keyring.rs`).
//! Everything here blocks (the keyring may ask to be unlocked): call it off the UI thread.

use std::collections::HashMap;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const APP: &str = "io.github.shagovalexei.ShagShellSecure";
const DEST: &str = "org.freedesktop.secrets";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Secret {
    Password,
    Passphrase,
}

impl Secret {
    fn name(self) -> &'static str {
        match self {
            Secret::Password => "password",
            Secret::Passphrase => "passphrase",
        }
    }
}

/// What a session's secret is filed under.
pub fn attrs(session: &str, s: Secret) -> Vec<(&'static str, String)> {
    vec![
        ("app", APP.to_string()),
        ("kind", "session".to_string()),
        ("id", session.to_string()),
        ("secret", s.name().to_string()),
    ]
}

fn map<'a>(attrs: &'a [(&'static str, String)]) -> HashMap<&'static str, &'a str> {
    attrs.iter().map(|(k, v)| (*k, v.as_str())).collect()
}

struct Service {
    conn: Connection,
    session: OwnedObjectPath,
}

impl Service {
    fn open() -> zbus::Result<Self> {
        let conn = Connection::session()?;
        // "plain": the secret crosses the session bus as is, as with `secret-tool` without libgcrypt.
        let (_, session): (OwnedValue, OwnedObjectPath) = Self::proxy_on(
            &conn,
            "/org/freedesktop/secrets",
            "org.freedesktop.Secret.Service",
        )?
        .call("OpenSession", &("plain", Value::from("")))?;
        Ok(Self { conn, session })
    }

    fn proxy_on<'a>(conn: &Connection, path: &str, iface: &'static str) -> zbus::Result<Proxy<'a>> {
        Proxy::new(conn, DEST, path.to_string(), iface)
    }

    fn proxy(&self, path: &str, iface: &'static str) -> zbus::Result<Proxy<'_>> {
        Self::proxy_on(&self.conn, path, iface)
    }

    fn service(&self) -> zbus::Result<Proxy<'_>> {
        self.proxy("/org/freedesktop/secrets", "org.freedesktop.Secret.Service")
    }

    /// Our items matching `attrs`, unlocked (the keyring may ask for its password).
    fn search(&self, attrs: &[(&'static str, String)]) -> zbus::Result<Vec<OwnedObjectPath>> {
        let (mut items, locked): (Vec<OwnedObjectPath>, Vec<OwnedObjectPath>) =
            self.service()?.call("SearchItems", &(map(attrs),))?;
        if !locked.is_empty() {
            let (done, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) =
                self.service()?.call("Unlock", &(&locked,))?;
            items.extend(done);
            if prompt.as_str() != "/" {
                answer(&self.conn, &prompt)?;
                items.extend(locked); // unlocked by the prompt
            }
        }
        items.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        items.dedup();
        Ok(items)
    }

    fn secret(&self, item: &OwnedObjectPath) -> zbus::Result<String> {
        let (_, _, value, _): (OwnedObjectPath, Vec<u8>, Vec<u8>, String) = self
            .proxy(item.as_str(), "org.freedesktop.Secret.Item")?
            .call("GetSecret", &(&self.session,))?;
        String::from_utf8(value).map_err(|_| zbus::Error::Failure("not text".into()))
    }

    fn delete(&self, item: &OwnedObjectPath) -> zbus::Result<()> {
        let prompt: OwnedObjectPath = self
            .proxy(item.as_str(), "org.freedesktop.Secret.Item")?
            .call("Delete", &())?;
        answer(&self.conn, &prompt)
    }

    fn store(
        &self,
        label: &str,
        attrs: &[(&'static str, String)],
        value: &str,
    ) -> zbus::Result<()> {
        let collection: OwnedObjectPath = self.service()?.call("ReadAlias", &("default",))?;
        if collection.as_str() == "/" {
            return Err(zbus::Error::Failure("no default keyring".into()));
        }
        let (_, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) =
            self.service()?.call("Unlock", &(vec![&collection],))?;
        answer(&self.conn, &prompt)?;
        let props = HashMap::from([
            ("org.freedesktop.Secret.Item.Label", Value::from(label)),
            (
                "org.freedesktop.Secret.Item.Attributes",
                Value::from(map(attrs)),
            ),
        ]);
        let secret = (
            &self.session,
            Vec::<u8>::new(),
            value.as_bytes(),
            "text/plain",
        );
        let (_, prompt): (OwnedObjectPath, OwnedObjectPath) = self
            .proxy(collection.as_str(), "org.freedesktop.Secret.Collection")?
            .call("CreateItem", &(props, secret, true))?;
        answer(&self.conn, &prompt)
    }

    fn id_of(&self, item: &OwnedObjectPath) -> zbus::Result<Option<(String, Secret)>> {
        let attrs: HashMap<String, String> = self
            .proxy(item.as_str(), "org.freedesktop.Secret.Item")?
            .get_property("Attributes")?;
        let secret = match attrs.get("secret").map(String::as_str) {
            Some("password") => Secret::Password,
            Some("passphrase") => Secret::Passphrase,
            _ => return Ok(None),
        };
        Ok(attrs.get("id").map(|id| (id.clone(), secret)))
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        if let Ok(p) = self.proxy(self.session.as_str(), "org.freedesktop.Secret.Session") {
            let _: zbus::Result<()> = p.call("Close", &());
        }
    }
}

/// A Secret Service answers (`--check`).
pub fn available() -> bool {
    Service::open().is_ok()
}

/// The saved secret, if any.
pub fn lookup(session: &str, s: Secret) -> zbus::Result<Option<String>> {
    let svc = Service::open()?;
    match svc.search(&attrs(session, s))?.first() {
        Some(item) => svc.secret(item).map(Some),
        None => Ok(None),
    }
}

/// Save it, replacing the one saved before.
pub fn store(session: &str, s: Secret, value: &str) -> zbus::Result<()> {
    let label = format!("ShagShell Secure: {session} ({})", s.name());
    Service::open()?.store(&label, &attrs(session, s), value)
}

pub fn delete(session: &str, s: Secret) -> zbus::Result<()> {
    let svc = Service::open()?;
    for item in svc.search(&attrs(session, s))? {
        svc.delete(&item)?;
    }
    Ok(())
}

/// A session or folder renamed / moved: its secrets (and those under it) filed under the new id.
pub fn retarget(old: &str, new: &str) -> zbus::Result<()> {
    let svc = Service::open()?;
    let ours = [("app", APP.to_string()), ("kind", "session".to_string())];
    for item in svc.search(&ours)? {
        let Some((id, s)) = svc.id_of(&item)? else {
            continue;
        };
        if let Some(to) = crate::session::store::retarget(&id, old, new).filter(|to| *to != id) {
            let value = svc.secret(&item)?;
            let label = format!("ShagShell Secure: {to} ({})", s.name());
            svc.store(&label, &attrs(&to, s), &value)?;
            svc.delete(&item)?;
        }
    }
    Ok(())
}

/// A session deleted (a folder: everything under it): its secrets go too.
pub fn forget_under(id: &str) -> zbus::Result<()> {
    let svc = Service::open()?;
    let ours = [("app", APP.to_string()), ("kind", "session".to_string())];
    for item in svc.search(&ours)? {
        if let Some((sid, _)) = svc.id_of(&item)?
            && crate::session::store::retarget(&sid, id, "").is_some()
        {
            svc.delete(&item)?;
        }
    }
    Ok(())
}

/// A keyring prompt (unlock, delete): shown, and waited for. `/`: none needed.
fn answer(conn: &Connection, prompt: &OwnedObjectPath) -> zbus::Result<()> {
    if prompt.as_str() == "/" {
        return Ok(());
    }
    let p = Proxy::new(conn, DEST, prompt.as_str(), "org.freedesktop.Secret.Prompt")?;
    let mut done = p.receive_signal("Completed")?;
    let _: () = p.call("Prompt", &("",))?;
    let msg = done
        .next()
        .ok_or_else(|| zbus::Error::Failure("no answer".into()))?;
    let (dismissed, _): (bool, OwnedValue) = msg.body().deserialize()?;
    match dismissed {
        true => Err(zbus::Error::Failure("dismissed".into())),
        false => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real keyring: `cargo test -p shagshell-core keyring -- --ignored` (writes, then removes,
    /// `zz-shagshell-test/…`).
    #[test]
    #[ignore]
    fn live_round_trip() {
        let id = "zz-shagshell-test/probe";
        store(id, Secret::Password, "p1").unwrap();
        store(id, Secret::Password, "p2").unwrap(); // replaces
        assert_eq!(lookup(id, Secret::Password).unwrap().as_deref(), Some("p2"));
        assert_eq!(lookup(id, Secret::Passphrase).unwrap(), None);
        retarget("zz-shagshell-test", "zz-shagshell-test2").unwrap();
        assert_eq!(lookup(id, Secret::Password).unwrap(), None);
        let moved = "zz-shagshell-test2/probe";
        assert_eq!(
            lookup(moved, Secret::Password).unwrap().as_deref(),
            Some("p2")
        );
        forget_under("zz-shagshell-test2").unwrap();
        assert_eq!(lookup(moved, Secret::Password).unwrap(), None);
    }

    #[test]
    fn attrs_schema() {
        let a = attrs("lab/web", Secret::Passphrase);
        let got: Vec<(&str, &str)> = a.iter().map(|(k, v)| (*k, v.as_str())).collect();
        assert_eq!(
            got,
            [
                ("app", "io.github.shagovalexei.ShagShellSecure"),
                ("kind", "session"),
                ("id", "lab/web"),
                ("secret", "passphrase"),
            ]
        );
    }
}
