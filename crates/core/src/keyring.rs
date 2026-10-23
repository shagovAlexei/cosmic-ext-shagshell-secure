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

/// Whose secret: a session's (by id) or a credential set's (by name).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Owner {
    Session(String),
    Credential(String),
}

impl Owner {
    fn kind(&self) -> &'static str {
        match self {
            Owner::Session(_) => "session",
            Owner::Credential(_) => "credential",
        }
    }

    fn id(&self) -> &str {
        match self {
            Owner::Session(id) | Owner::Credential(id) => id,
        }
    }

    fn label(&self, s: Secret) -> String {
        format!("ShagShell Secure: {} ({})", self.id(), s.name())
    }
}

/// What a secret is filed under.
pub fn attrs(owner: &Owner, s: Secret) -> Vec<(&'static str, String)> {
    vec![
        ("app", APP.to_string()),
        ("kind", owner.kind().to_string()),
        ("id", owner.id().to_string()),
        ("secret", s.name().to_string()),
    ]
}

/// The keyring of its own (the master password) is the one with this label.
const COLLECTION: &str = "ShagShell Secure";

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

    /// Our items matching `attrs`, locked or not (their attributes read either way): no unlock
    /// prompt for what only needs to know they are there.
    fn find(&self, attrs: &[(&'static str, String)]) -> zbus::Result<Vec<OwnedObjectPath>> {
        let (mut items, locked): (Vec<OwnedObjectPath>, Vec<OwnedObjectPath>) =
            self.service()?.call("SearchItems", &(map(attrs),))?;
        items.extend(locked);
        Ok(items)
    }

    /// Before reading or deleting them: the keyring may ask for its password (once).
    fn unlock(&self, items: &[OwnedObjectPath]) -> zbus::Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let (_, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) =
            self.service()?.call("Unlock", &(items,))?;
        answer(&self.conn, &prompt)
    }

    /// `find`, unlocked.
    fn search(&self, attrs: &[(&'static str, String)]) -> zbus::Result<Vec<OwnedObjectPath>> {
        let items = self.find(attrs)?;
        self.unlock(&items)?;
        Ok(items)
    }

    /// Our items whose session id `f` picks (with what to do with it), unlocked.
    fn ours<T>(
        &self,
        f: impl Fn(&str, Secret) -> Option<T>,
    ) -> zbus::Result<Vec<(OwnedObjectPath, Secret, T)>> {
        let mut picked = Vec::new();
        for item in self.find(&[("app", APP.to_string()), ("kind", "session".to_string())])? {
            if let Some((id, s)) = self.id_of(&item)?
                && let Some(t) = f(&id, s)
            {
                picked.push((item, s, t));
            }
        }
        let items: Vec<_> = picked.iter().map(|(i, ..)| i.clone()).collect();
        self.unlock(&items)?;
        Ok(picked)
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
        let collection = self.target()?;
        self.create(&collection, label, map(attrs), value)
    }

    /// Where new secrets go: our own keyring if there is one, else the default one.
    fn target(&self) -> zbus::Result<OwnedObjectPath> {
        if let Some(c) = self.own()? {
            return Ok(c);
        }
        let collection: OwnedObjectPath = self.service()?.call("ReadAlias", &("default",))?;
        match collection.as_str() {
            "/" => Err(zbus::Error::Failure("no default keyring".into())),
            _ => Ok(collection),
        }
    }

    /// Our own keyring (the master password), if made.
    fn own(&self) -> zbus::Result<Option<OwnedObjectPath>> {
        let all: Vec<OwnedObjectPath> = self.service()?.get_property("Collections")?;
        for c in all {
            let label: String = self
                .proxy(c.as_str(), "org.freedesktop.Secret.Collection")?
                .get_property("Label")?;
            if label == COLLECTION {
                return Ok(Some(c));
            }
        }
        Ok(None)
    }

    fn create(
        &self,
        collection: &OwnedObjectPath,
        label: &str,
        attrs: HashMap<&str, &str>,
        value: &str,
    ) -> zbus::Result<()> {
        let (_, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) =
            self.service()?.call("Unlock", &(vec![collection],))?;
        answer(&self.conn, &prompt)?;
        let props = HashMap::from([
            ("org.freedesktop.Secret.Item.Label", Value::from(label)),
            ("org.freedesktop.Secret.Item.Attributes", Value::from(attrs)),
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
pub fn lookup(owner: &Owner, s: Secret) -> zbus::Result<Option<String>> {
    let svc = Service::open()?;
    match svc.search(&attrs(owner, s))?.first() {
        Some(item) => svc.secret(item).map(Some),
        None => Ok(None),
    }
}

/// A password or passphrase is saved (no unlock asked).
pub fn exists(owner: &Owner) -> zbus::Result<bool> {
    let svc = Service::open()?;
    for s in [Secret::Password, Secret::Passphrase] {
        if !svc.find(&attrs(owner, s))?.is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Save it, replacing the one saved before.
pub fn store(owner: &Owner, s: Secret, value: &str) -> zbus::Result<()> {
    Service::open()?.store(&owner.label(s), &attrs(owner, s), value)
}

pub fn delete(owner: &Owner, s: Secret) -> zbus::Result<()> {
    let svc = Service::open()?;
    for item in svc.search(&attrs(owner, s))? {
        svc.delete(&item)?;
    }
    Ok(())
}

/// A credential set renamed: its secrets filed under the new name.
pub fn rename(from: &Owner, to: &Owner) -> zbus::Result<()> {
    let svc = Service::open()?;
    for s in [Secret::Password, Secret::Passphrase] {
        for item in svc.search(&attrs(from, s))? {
            let value = svc.secret(&item)?;
            svc.store(&to.label(s), &attrs(to, s), &value)?;
            svc.delete(&item)?;
        }
    }
    Ok(())
}

/// Passwords kept in a keyring of our own, with its own password (the master password).
pub fn separate() -> zbus::Result<bool> {
    Ok(Service::open()?.own()?.is_some())
}

/// Make our own keyring (gnome-keyring asks for its password) and move our secrets into it; or
/// move them back into the default keyring and delete ours.
pub fn set_separate(on: bool) -> zbus::Result<()> {
    let svc = Service::open()?;
    let own = svc.own()?;
    let (from, to) = match (on, own) {
        (true, Some(_)) | (false, None) => return Ok(()),
        (true, None) => {
            let props = HashMap::from([(
                "org.freedesktop.Secret.Collection.Label",
                Value::from(COLLECTION),
            )]);
            let (made, prompt): (OwnedObjectPath, OwnedObjectPath) =
                svc.service()?.call("CreateCollection", &(props, ""))?;
            let made = match made.as_str() {
                "/" => answer_value(&svc.conn, &prompt)?
                    .and_then(|v| OwnedObjectPath::try_from(v).ok())
                    .ok_or_else(|| zbus::Error::Failure("no keyring made".into()))?,
                _ => made,
            };
            (None, made)
        }
        (false, Some(own)) => {
            let default: OwnedObjectPath = svc.service()?.call("ReadAlias", &("default",))?;
            if default.as_str() == "/" {
                return Err(zbus::Error::Failure("no default keyring".into()));
            }
            (Some(own), default)
        }
    };
    let prefix = format!("{}/", to.as_str());
    let ours = svc.find(&[("app", APP.to_string())])?;
    let moving: Vec<_> = ours
        .into_iter()
        .filter(|i| !i.as_str().starts_with(&prefix))
        .collect();
    svc.unlock(&moving)?;
    for item in moving {
        let p = svc.proxy(item.as_str(), "org.freedesktop.Secret.Item")?;
        let attrs: HashMap<String, String> = p.get_property("Attributes")?;
        let label: String = p.get_property("Label")?;
        let value = svc.secret(&item)?;
        let a = attrs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        svc.create(&to, &label, a, &value)?;
        svc.delete(&item)?;
    }
    if let Some(own) = from {
        let prompt: OwnedObjectPath = svc
            .proxy(own.as_str(), "org.freedesktop.Secret.Collection")?
            .call("Delete", &())?;
        answer(&svc.conn, &prompt)?;
    }
    Ok(())
}

/// A session or folder renamed / moved: its secrets (and those under it) filed under the new id.
pub fn retarget(old: &str, new: &str) -> zbus::Result<()> {
    let svc = Service::open()?;
    let moving =
        svc.ours(|id, _| crate::session::store::retarget(id, old, new).filter(|to| to != id))?;
    for (item, s, to) in moving {
        let value = svc.secret(&item)?;
        let to = Owner::Session(to);
        svc.store(&to.label(s), &attrs(&to, s), &value)?;
        svc.delete(&item)?;
    }
    Ok(())
}

/// A session deleted (a folder: everything under it): its secrets go too.
pub fn forget_under(id: &str) -> zbus::Result<()> {
    let svc = Service::open()?;
    for (item, ..) in svc.ours(|sid, _| crate::session::store::retarget(sid, id, ""))? {
        svc.delete(&item)?;
    }
    Ok(())
}

/// A keyring prompt (unlock, delete): shown, and waited for. `/`: none needed.
fn answer(conn: &Connection, prompt: &OwnedObjectPath) -> zbus::Result<()> {
    answer_value(conn, prompt).map(drop)
}

/// `answer`, with what the prompt gives back (CreateCollection: the new keyring).
fn answer_value(conn: &Connection, prompt: &OwnedObjectPath) -> zbus::Result<Option<OwnedValue>> {
    if prompt.as_str() == "/" {
        return Ok(None);
    }
    let p = Proxy::new(conn, DEST, prompt.as_str(), "org.freedesktop.Secret.Prompt")?;
    let mut done = p.receive_signal("Completed")?;
    let _: () = p.call("Prompt", &("",))?;
    let msg = done
        .next()
        .ok_or_else(|| zbus::Error::Failure("no answer".into()))?;
    let (dismissed, result): (bool, OwnedValue) = msg.body().deserialize()?;
    match dismissed {
        true => Err(zbus::Error::Failure("dismissed".into())),
        false => Ok(Some(result)),
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
        let o = Owner::Session(id.into());
        store(&o, Secret::Password, "p1").unwrap();
        store(&o, Secret::Password, "p2").unwrap(); // replaces
        assert_eq!(lookup(&o, Secret::Password).unwrap().as_deref(), Some("p2"));
        assert_eq!(lookup(&o, Secret::Passphrase).unwrap(), None);
        retarget("zz-shagshell-test", "zz-shagshell-test2").unwrap();
        assert_eq!(lookup(&o, Secret::Password).unwrap(), None);
        let moved = Owner::Session("zz-shagshell-test2/probe".into());
        assert_eq!(
            lookup(&moved, Secret::Password).unwrap().as_deref(),
            Some("p2")
        );
        assert!(exists(&moved).unwrap() && !exists(&o).unwrap());
        forget_under("zz-shagshell-test2").unwrap();
        assert_eq!(lookup(&moved, Secret::Password).unwrap(), None);
        // A credential set: same id as a session, a different secret; renamed.
        let c = Owner::Credential("zz-shagshell-test2/probe".into());
        store(&c, Secret::Password, "c1").unwrap();
        assert_eq!(lookup(&moved, Secret::Password).unwrap(), None);
        let c2 = Owner::Credential("zz-shagshell-test-renamed".into());
        rename(&c, &c2).unwrap();
        assert_eq!(
            lookup(&c2, Secret::Password).unwrap().as_deref(),
            Some("c1")
        );
        assert!(!exists(&c).unwrap());
        delete(&c2, Secret::Password).unwrap();
        assert!(!exists(&c2).unwrap());
    }

    /// Our own keyring (the master password). Only on a throwaway bus: its prompt asks for a new
    /// password — `dbus-run-session` + `gnome-keyring-daemon`, the prompter on a headless display.
    #[test]
    #[ignore]
    fn live_separate() {
        let o = Owner::Session("zz-shagshell-test/sep".into());
        store(&o, Secret::Password, "p").unwrap();
        assert!(!separate().unwrap());
        set_separate(true).unwrap();
        assert!(separate().unwrap());
        assert_eq!(lookup(&o, Secret::Password).unwrap().as_deref(), Some("p"));
        let svc = Service::open().unwrap();
        let own = svc.own().unwrap().unwrap();
        let items = svc.find(&attrs(&o, Secret::Password)).unwrap();
        assert!(
            items.iter().all(|i| i.as_str().starts_with(own.as_str())),
            "{items:?}"
        );
        drop(svc);
        set_separate(false).unwrap();
        assert!(!separate().unwrap());
        assert_eq!(lookup(&o, Secret::Password).unwrap().as_deref(), Some("p"));
        delete(&o, Secret::Password).unwrap();
    }

    #[test]
    fn attrs_schema() {
        let a = attrs(&Owner::Session("lab/web".into()), Secret::Passphrase);
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
        let c = attrs(&Owner::Credential("adm".into()), Secret::Password);
        assert_eq!((c[1].1.as_str(), c[2].1.as_str()), ("credential", "adm"));
    }
}
