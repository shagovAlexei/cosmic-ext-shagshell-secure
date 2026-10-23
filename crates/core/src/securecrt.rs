//! Import from SecureCRT's configuration folder (`Config/`): the sessions (`Sessions/**.ini`)
//! and the Button Bar (`ButtonBarV5.ini`). Reading only. What cannot come across is reported, not
//! guessed: passwords and the login script rows are encrypted by SecureCRT, RDP has no meaning
//! here, scripts need SecureCRT's own `crt` object.

use crate::buttonbar::{Bar, Button, Kind, Target};
use crate::session::{Protocol, Session};
use std::collections::HashMap;
use std::path::Path;

/// One `.ini`: its `S:` / `D:` values by name, and its `Z:` lists (one line per entry).
#[derive(Debug, Default)]
pub struct Ini {
    pub values: HashMap<String, String>,
    pub lists: HashMap<String, Vec<String>>,
}

impl Ini {
    pub fn text(&self, key: &str) -> &str {
        self.values.get(key).map_or("", String::as_str)
    }

    /// A `D:` value (eight hex digits).
    pub fn dword(&self, key: &str) -> Option<u32> {
        u32::from_str_radix(self.values.get(key)?.trim(), 16).ok()
    }
}

/// Lines `S:"Name"=text`, `D:"Name"=0000001f`, `Z:"Name"=<count>` followed by that many lines
/// that start with a space, `B:"Name"=<size>` (binary, skipped with its lines).
pub fn parse_ini(text: &str) -> Ini {
    let mut ini = Ini::default();
    let mut list: Option<String> = None;
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim_end_matches('\r');
        if let Some(entry) = line.strip_prefix(' ') {
            if let Some(k) = &list {
                ini.lists
                    .entry(k.clone())
                    .or_default()
                    .push(entry.to_string());
            }
            continue;
        }
        list = None;
        let mut it = line.splitn(2, ':');
        let (Some(kind), Some(rest)) = (it.next(), it.next()) else {
            continue;
        };
        let Some(rest) = rest.strip_prefix('"') else {
            continue;
        };
        let Some((name, value)) = rest.split_once("\"=") else {
            continue;
        };
        match kind {
            "S" | "D" => {
                ini.values.insert(name.to_string(), value.to_string());
            }
            "Z" => {
                ini.lists.entry(name.to_string()).or_default();
                list = Some(name.to_string());
            }
            _ => {}
        }
    }
    ini
}

/// A session that was not imported, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Skip {
    /// RDP (or a protocol we don't have).
    Protocol(String),
    NoHost,
}

/// `COM3` → `/dev/ttyS2` (a guess: a USB adapter is `/dev/ttyUSB0`; the user can fix it).
fn serial_device(com: &str) -> String {
    match com
        .trim()
        .to_ascii_uppercase()
        .strip_prefix("COM")
        .and_then(|n| n.parse::<u32>().ok())
    {
        Some(n) if n >= 1 => format!("/dev/ttyS{}", n - 1),
        _ => com.trim().to_string(),
    }
}

/// The file-name part of a Windows or Unix path (`d:\logs\%H_%Y.log` → `%H_%Y.log`).
fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// The session in an `.ini`. `jump_of`: a session by SecureCRT's name → `[user@]host[:port]`
/// (for «Firewall: Session:name»).
pub fn session_from_ini(
    ini: &Ini,
    jump_of: &dyn Fn(&str) -> Option<String>,
) -> Result<Session, Skip> {
    let protocol = match ini.text("Protocol Name") {
        "SSH2" => Protocol::Ssh2,
        "Telnet" => Protocol::Telnet,
        "Local Shell" => Protocol::Local,
        "Serial" => Protocol::Serial,
        other => return Err(Skip::Protocol(other.to_string())),
    };
    let mut s = Session {
        protocol,
        ..Session::default()
    };
    let some = |v: &str| Some(v.trim().to_string()).filter(|v| !v.is_empty());
    match protocol {
        Protocol::Ssh2 | Protocol::Telnet => {
            s.host = ini.text("Hostname").trim().to_string();
            if s.host.is_empty() {
                return Err(Skip::NoHost);
            }
            let (key, default) = match protocol {
                Protocol::Ssh2 => ("[SSH2] Port", 22),
                _ => ("Port", 23),
            };
            s.port = ini
                .dword(key)
                .and_then(|p| u16::try_from(p).ok())
                .filter(|&p| p != 0 && p != default);
            s.user = some(ini.text("Username"));
            if protocol == Protocol::Ssh2 {
                s.identity = some(ini.text("Identity Filename V2"));
                // «Firewall: Session:name»: another session to go through.
                let fw = ini.text("Firewall Name");
                if let Some(name) = fw.strip_prefix("Session:") {
                    s.jump = jump_of(name.trim()).unwrap_or_default();
                }
            }
        }
        Protocol::Serial => {
            s.serial.device = serial_device(ini.text("Com Port"));
            s.serial.baud = ini.dword("Baud Rate").filter(|&b| b != 0 && b != 9600);
        }
        Protocol::Local => {}
    }
    // Log: the name's template is the same language (`%H %S %Y %M %D %h %m %s`); the folder was
    // a Windows one, so only the file name comes across (the log folder is ours to choose).
    if ini.dword("Start Log Upon Connect") == Some(1) {
        let name = file_name(ini.text("Log Filename V2")).trim().to_string();
        if !name.is_empty() {
            s.log_file = name;
            s.log_append = ini.dword("Log Mode") == Some(1);
            s.log_raw = ini.dword("Raw Log") == Some(1);
        }
    }
    s.button_bar = some(ini.text("User Button Bar Name"));
    Ok(s)
}

/// A name as a file name of ours: no `/`, no leading dot.
pub fn clean_name(name: &str) -> String {
    let n: String = name
        .trim()
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    match n.strip_prefix('.') {
        Some(rest) => format!("_{rest}"),
        None => n,
    }
}

#[derive(Debug, Default)]
pub struct SessionsFound {
    /// `(folder, name, session)`, folders as our ids (`a/b`).
    pub sessions: Vec<(String, String, Session)>,
    /// `(path under Sessions/, why)`.
    pub skipped: Vec<(String, Skip)>,
}

/// Every session under `root` (`Config/Sessions`). The `Default*.ini` templates and the
/// `__FolderData__.ini` files are not sessions. Unreadable files are left out.
pub fn read_sessions(root: &Path) -> SessionsFound {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<(String, String, Ini)>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            if path.is_dir() {
                let sub = match rel.is_empty() {
                    true => clean_name(&name),
                    false => format!("{rel}/{}", clean_name(&name)),
                };
                walk(&path, &sub, out);
            } else if let Some(stem) = name.strip_suffix(".ini")
                && !name.starts_with("__")
                && !(rel.is_empty() && (stem == "Default" || stem.starts_with("Default_")))
                && let Ok(bytes) = std::fs::read(&path)
            {
                out.push((
                    rel.to_string(),
                    clean_name(stem),
                    parse_ini(&String::from_utf8_lossy(&bytes)),
                ));
            }
        }
    }
    let mut all = Vec::new();
    walk(root, "", &mut all);
    // «Firewall: Session:name» names a session by its SecureCRT name (path included).
    let by_name: HashMap<String, (String, u16, String)> = all
        .iter()
        .map(|(folder, name, ini)| {
            let key = match folder.is_empty() {
                true => name.clone(),
                false => format!("{folder}/{name}"),
            };
            let port = ini
                .dword("[SSH2] Port")
                .and_then(|p| u16::try_from(p).ok())
                .unwrap_or(22);
            (
                key,
                (
                    ini.text("Hostname").trim().to_string(),
                    port,
                    ini.text("Username").trim().to_string(),
                ),
            )
        })
        .collect();
    let jump_of = |name: &str| {
        let (host, port, user) = by_name.get(&name.replace('\\', "/"))?;
        let at = match user.is_empty() {
            true => String::new(),
            false => format!("{user}@"),
        };
        let p = match *port {
            22 => String::new(),
            p => format!(":{p}"),
        };
        (!host.is_empty()).then(|| format!("{at}{host}{p}"))
    };
    let mut found = SessionsFound::default();
    for (folder, name, ini) in &all {
        match session_from_ini(ini, &jump_of) {
            Ok(s) => found.sessions.push((folder.clone(), name.clone(), s)),
            Err(why) => found.skipped.push((
                format!("{folder}/{name}")
                    .trim_start_matches('/')
                    .to_string(),
                why,
            )),
        }
    }
    found
}

/// `\,` is a comma, `\\` a backslash, `\r` / `\n` the characters (an entry stays one line).
fn unescape_field(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('r') => out.push('\r'),
            Some('n') => out.push('\n'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
    out
}

/// A line's fields: split at commas that are not escaped (`\,`), each field read.
fn split_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\\' => {
                cur.push('\\');
                if let Some(n) = it.next() {
                    cur.push(n);
                }
            }
            ',' => fields.push(unescape_field(&std::mem::take(&mut cur))),
            c => cur.push(c),
        }
    }
    fields.push(unescape_field(&cur));
    fields
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct BarsFound {
    pub bars: Vec<Bar>,
    /// Buttons that did not come across: `(bar, label, kind)`.
    pub skipped: Vec<(String, String, String)>,
}

/// `ButtonBarV5.ini`: `Z:"bar name"=<count>` and that many button lines
/// ` KIND,arg,arg,,label,0,n,tooltip,`; for `SEND` the string is field 1 and the label field 2,
/// for `RUN` the script is field 1, its arguments field 2 and the label field 4. `SEND` is
/// imported as Send String (to the current tab), `RUN` of a `.py` as Run Script (its Windows path
/// put under the wine `prefix`, if there is one); the rest, with the bar settings lines (`D:`,
/// `S:`), are reported.
pub fn read_button_bars(text: &str, prefix: Option<&Path>) -> BarsFound {
    let mut found = BarsFound::default();
    let mut cur: Option<usize> = None;
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim_end_matches('\r');
        if let Some(entry) = line.strip_prefix(' ') {
            let Some(i) = cur else { continue };
            let f = split_fields(entry);
            let kind = f.first().map_or("", String::as_str);
            let get = |n: usize| f.get(n).map_or("", String::as_str);
            match kind {
                "SEND" => {
                    let label = match get(2).trim() {
                        "" => (get(1).trim().chars())
                            .take_while(|c| !c.is_control())
                            .take(24)
                            .collect::<String>(),
                        l => l.to_string(),
                    };
                    found.bars[i].buttons.push(Button {
                        label,
                        kind: Kind::Send,
                        arg: get(1).to_string(),
                        args: String::new(),
                        target: Target::Current,
                    });
                }
                "RUN" if is_python(get(1)) => {
                    let label = match get(4).trim() {
                        "" => get(1)
                            .trim()
                            .rsplit('\\')
                            .next()
                            .unwrap_or("")
                            .rsplit('/')
                            .next()
                            .unwrap_or("")
                            .to_string(),
                        l => l.to_string(),
                    };
                    found.bars[i].buttons.push(Button {
                        label,
                        kind: Kind::Script,
                        arg: windows_path(get(1), prefix),
                        args: get(2).trim().to_string(),
                        target: Target::Current,
                    });
                }
                other => {
                    let label = match other {
                        "RUN" => get(4),
                        _ => get(2),
                    };
                    found.skipped.push((
                        found.bars[i].name.clone(),
                        label.to_string(),
                        other.to_string(),
                    ));
                }
            }
            continue;
        }
        cur = None;
        if let Some(rest) = line.strip_prefix("Z:\"")
            && let Some((name, _)) = rest.split_once("\"=")
            && name != "---"
            && !name.is_empty()
        {
            found.bars.push(Bar {
                name: name.to_string(),
                buttons: Vec::new(),
            });
            cur = Some(found.bars.len() - 1);
        }
    }
    found
}

fn is_python(path: &str) -> bool {
    path.trim().to_ascii_lowercase().ends_with(".py")
}

/// `C:\dir\a.py` (single or doubled backslashes) as a path under the wine `prefix`
/// (`<prefix>/drive_c/dir/a.py`); no prefix or not a drive path: slashes only.
fn windows_path(path: &str, prefix: Option<&Path>) -> String {
    let p = path.trim();
    let mut chars = p.chars();
    let drive = match (chars.next(), chars.next(), chars.next()) {
        (Some(d), Some(':'), Some('\\' | '/')) if d.is_ascii_alphabetic() => Some(d),
        _ => None,
    };
    let (Some(d), Some(prefix)) = (drive, prefix) else {
        return p.replace('\\', "/");
    };
    let mut out = prefix.join(format!("drive_{}", d.to_ascii_lowercase()));
    for seg in p[2..].split(['\\', '/']).filter(|s| !s.is_empty()) {
        out.push(seg);
    }
    out.display().to_string()
}

/// The wine prefix (the folder holding `drive_c`) that `root` lies in.
fn wine_prefix(root: &Path) -> Option<std::path::PathBuf> {
    root.ancestors()
        .find(|a| a.join("drive_c").is_dir())
        .map(Path::to_path_buf)
}

/// What an import did.
#[derive(Debug, Default)]
pub struct Report {
    pub sessions_added: usize,
    /// Already there under that name: left as they are.
    pub sessions_existing: usize,
    /// RDP and the like.
    pub sessions_skipped: usize,
    /// Bars that were not there yet, with their buttons (the app merges them into its file).
    pub bars: Vec<Bar>,
    pub bars_existing: usize,
    /// `RUN` and other buttons that need SecureCRT itself.
    pub buttons_skipped: usize,
}

/// The default places of SecureCRT's `Config` folder: under wine (the user's `AppData`), and a
/// Windows home mounted somewhere is for the user to pick.
pub fn find_config(home: &Path) -> Option<std::path::PathBuf> {
    let users = home.join(".wine/drive_c/users");
    let mut found = Vec::new();
    for u in std::fs::read_dir(users).ok()?.flatten() {
        for rel in [
            "AppData/Local/VanDyke Software/SecureCRT/Config",
            "AppData/Roaming/VanDyke/Config",
        ] {
            let c = u.path().join(rel);
            if c.join("Sessions").is_dir() || c.join("ButtonBarV5.ini").is_file() {
                found.push(c);
            }
        }
    }
    found.sort();
    found.into_iter().next()
}

/// Import from `root` (`Config/`): sessions into `store` (the folder tree kept, existing names left
/// alone), the Button Bar as `Report::bars` (new names only). A session names its bar only if that
/// bar exists (`have_bars`, or imported now).
pub fn import(
    root: &Path,
    store: &crate::session::Store,
    have_bars: &[String],
    sessions: bool,
    bars: bool,
) -> std::io::Result<Report> {
    let mut r = Report::default();
    let mut known: Vec<String> = have_bars.to_vec();
    if bars {
        // No Button Bar file: no bars (the sessions are still imported).
        let text = match std::fs::read_to_string(root.join("ButtonBarV5.ini")) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let f = read_button_bars(&text, wine_prefix(root).as_deref());
        r.buttons_skipped = f.skipped.len();
        for b in f.bars {
            match known.contains(&b.name) {
                true => r.bars_existing += 1,
                false => {
                    known.push(b.name.clone());
                    r.bars.push(b);
                }
            }
        }
    }
    if sessions {
        let f = read_sessions(&root.join("Sessions"));
        r.sessions_skipped = f.skipped.len();
        for (folder, name, mut s) in f.sessions {
            if s.button_bar.as_ref().is_some_and(|n| !known.contains(n)) {
                s.button_bar = None;
            }
            match store.add(&folder, &name, &s) {
                Ok(_) => r.sessions_added += 1,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => r.sessions_existing += 1,
                // A name we can't use as a file name: that session only is left out.
                Err(e)
                    if e.kind() == std::io::ErrorKind::InvalidFilename
                        || e.kind() == std::io::ErrorKind::InvalidInput =>
                {
                    r.sessions_skipped += 1
                }
                Err(e) => return Err(e),
            }
        }
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SSH: &str = "\u{feff}Z:\"Login Script V4\"=00000000\r\nS:\"Username\"=root\r\nS:\"Protocol Name\"=SSH2\r\nS:\"Hostname\"=host.example\r\nD:\"[SSH2] Port\"=000008ae\r\nS:\"Identity Filename V2\"=\r\nS:\"Firewall Name\"=Session:jump/bastion\r\nD:\"Start Log Upon Connect\"=00000001\r\nS:\"Log Filename V2\"=d:\\logs\\%S_%Y-%M-%D.log\r\nD:\"Log Mode\"=00000001\r\nD:\"Raw Log\"=00000000\r\nS:\"User Button Bar Name\"=Kube\r\nZ:\"Description\"=00000001\r\n my note\r\nB:\"Normal Font v2\"=00000060\r\n f3 ff ff\r\n";

    #[test]
    fn ini_values_lists_and_binary() {
        let i = parse_ini(SSH);
        assert_eq!(i.text("Hostname"), "host.example");
        assert_eq!(i.dword("[SSH2] Port"), Some(2222));
        assert_eq!(i.lists["Description"], ["my note"]);
        assert!(i.lists["Login Script V4"].is_empty());
        assert!(!i.values.contains_key("Normal Font v2")); // binary: not a value
    }

    #[test]
    fn an_ssh_session() {
        let jump = |n: &str| (n == "jump/bastion").then(|| "u@b.example:2200".to_string());
        let s = session_from_ini(&parse_ini(SSH), &jump).unwrap();
        assert_eq!(
            (s.protocol, s.host.as_str(), s.port, s.user.as_deref()),
            (Protocol::Ssh2, "host.example", Some(2222), Some("root"))
        );
        assert_eq!(s.jump, "u@b.example:2200");
        assert_eq!(
            (s.log_file.as_str(), s.log_append, s.log_raw),
            ("%S_%Y-%M-%D.log", true, false)
        );
        assert_eq!(s.button_bar.as_deref(), Some("Kube"));
        assert_eq!(s.identity, None);
    }

    #[test]
    fn other_protocols_and_what_is_skipped() {
        let ini = |t: &str| parse_ini(t);
        let no = |_: &str| None;
        let t = session_from_ini(
            &ini("S:\"Protocol Name\"=Telnet\nS:\"Hostname\"=sw1\nD:\"Port\"=00000017\n"),
            &no,
        )
        .unwrap();
        assert_eq!(
            (t.protocol, t.host.as_str(), t.port),
            (Protocol::Telnet, "sw1", None)
        ); // 23: the default
        let ser = session_from_ini(
            &ini("S:\"Protocol Name\"=Serial\nS:\"Com Port\"=COM3\nD:\"Baud Rate\"=0001c200\n"),
            &no,
        )
        .unwrap();
        assert_eq!(
            (ser.serial.device.as_str(), ser.serial.baud),
            ("/dev/ttyS2", Some(115200))
        );
        assert_eq!(
            session_from_ini(&ini("S:\"Protocol Name\"=Local Shell\n"), &no)
                .unwrap()
                .protocol,
            Protocol::Local
        );
        assert_eq!(
            session_from_ini(&ini("S:\"Protocol Name\"=RDP\n"), &no),
            Err(Skip::Protocol("RDP".into()))
        );
        assert_eq!(
            session_from_ini(&ini("S:\"Protocol Name\"=SSH2\nS:\"Hostname\"=\n"), &no),
            Err(Skip::NoHost)
        );
    }

    #[test]
    fn a_folder_of_sessions() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::create_dir_all(r.join("AI")).unwrap();
        std::fs::write(r.join("Default.ini"), SSH).unwrap();
        std::fs::write(
            r.join("__FolderData__.ini"),
            "Z:\"Session List V2\"=00000000\n",
        )
        .unwrap();
        std::fs::write(r.join("a.b.ini"), SSH).unwrap();
        std::fs::write(r.join("AI/gpu1.ini"), SSH).unwrap();
        std::fs::write(r.join("rdp.ini"), "S:\"Protocol Name\"=RDP\n").unwrap();
        let f = read_sessions(r);
        let ids: Vec<String> = f
            .sessions
            .iter()
            .map(|(d, n, _)| format!("{d}/{n}"))
            .collect();
        assert_eq!(ids, ["AI/gpu1", "/a.b"]); // by file name, "AI" first
        assert_eq!(
            f.skipped,
            [("rdp".to_string(), Skip::Protocol("RDP".into()))]
        );
    }

    #[test]
    fn firewall_by_session_name_through_the_folder() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("jump")).unwrap();
        std::fs::write(d.path().join("jump/bastion.ini"), "S:\"Protocol Name\"=SSH2\nS:\"Hostname\"=b.example\nS:\"Username\"=u\nD:\"[SSH2] Port\"=00000016\n").unwrap();
        std::fs::write(d.path().join("t.ini"), SSH).unwrap();
        let f = read_sessions(d.path());
        let t = f.sessions.iter().find(|(_, n, _)| n == "t").unwrap();
        assert_eq!(t.2.jump, "u@b.example");
    }

    #[test]
    fn button_bar_lines() {
        let text = "\u{feff}D:\"Sort Alphabetically\"=00000000\nZ:\"Main\"=00000003\n SEND,conn\\\\n,conn,,,0,5,,\n RUN,C:\\\\x\\\\a.py,arg,,runit,0,5,tip,\n RUN,C:\\\\x\\\\b.vbs,,,vb,0,5,,\n SEND,ls\\\\n\\r\\npwd\\\\n\\r\\n,,,,0,1,,\nZ:\"---\"=00000000\nZ:\"Other\"=00000001\n SEND,a\\,b,comma,,,0,1,,\n";
        let f = read_button_bars(text, None);
        assert_eq!(f.bars.len(), 2);
        let main = &f.bars[0].buttons;
        assert_eq!(main.len(), 3);
        assert_eq!(
            (main[0].label.as_str(), main[0].arg.as_str()),
            ("conn", "conn\\n")
        ); // `\\n` in the ini is the escape `\n`
        assert_eq!(main[1].kind, Kind::Script);
        assert_eq!(
            (main[1].label.as_str(), main[1].arg.as_str()),
            ("runit", "C:/x/a.py")
        );
        assert_eq!(main[1].args, "arg");
        assert_eq!(main[2].arg, "ls\\n\r\npwd\\n\r\n"); // the line ends are real ones
        assert_eq!(main[2].label, "ls\\n"); // no line end in a label
        assert_eq!(f.bars[1].buttons[0].arg, "a,b");
        assert_eq!(
            f.skipped,
            [("Main".to_string(), "vb".to_string(), "RUN".to_string())] // not Python
        );
    }

    #[test]
    fn windows_paths_go_under_the_wine_prefix() {
        let pre = Path::new("/home/u/.wine");
        assert_eq!(
            windows_path("C:\\Users\\u\\a b.py", Some(pre)),
            "/home/u/.wine/drive_c/Users/u/a b.py"
        );
        assert_eq!(
            windows_path("d:/x//y.py", Some(pre)),
            "/home/u/.wine/drive_d/x/y.py"
        );
        assert_eq!(windows_path("C:\\x\\a.py", None), "C:/x/a.py");
        assert_eq!(windows_path("/opt/a.py", Some(pre)), "/opt/a.py");
    }

    /// Against a real SecureCRT folder (set `SECURECRT_CONFIG`); prints only counts.
    #[test]
    #[ignore]
    fn live_config() {
        let Some(root) = std::env::var_os("SECURECRT_CONFIG") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let s = read_sessions(&root.join("Sessions"));
        let mut by: HashMap<String, usize> = HashMap::new();
        for (_, _, x) in &s.sessions {
            *by.entry(format!("{:?}", x.protocol)).or_default() += 1;
        }
        println!(
            "sessions {} {:?}, skipped {}",
            s.sessions.len(),
            by,
            s.skipped.len()
        );
        let with_jump = s
            .sessions
            .iter()
            .filter(|(_, _, x)| !x.jump.is_empty())
            .count();
        let with_log = s
            .sessions
            .iter()
            .filter(|(_, _, x)| !x.log_file.is_empty())
            .count();
        let with_bar = s
            .sessions
            .iter()
            .filter(|(_, _, x)| x.button_bar.is_some())
            .count();
        println!("jump {with_jump}, log {with_log}, bar {with_bar}");
        let text = std::fs::read_to_string(root.join("ButtonBarV5.ini")).unwrap();
        let b = read_button_bars(&text, None);
        let n: usize = b.bars.iter().map(|x| x.buttons.len()).sum();
        println!(
            "bars {} buttons {} skipped {}",
            b.bars.len(),
            n,
            b.skipped.len()
        );
        // every session's bar name exists among the imported bars
        let names: Vec<&str> = b.bars.iter().map(|x| x.name.as_str()).collect();
        let missing = s
            .sessions
            .iter()
            .filter(|(_, _, x)| x.button_bar.as_deref().is_some_and(|n| !names.contains(&n)))
            .count();
        println!("sessions naming a bar that is not there: {missing}");
    }

    #[test]
    fn import_into_a_store() {
        let (cfg, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        std::fs::create_dir_all(cfg.path().join("Sessions/AI")).unwrap();
        std::fs::write(cfg.path().join("Sessions/AI/gpu1.ini"), SSH).unwrap();
        std::fs::write(
            cfg.path().join("Sessions/rdp.ini"),
            "S:\"Protocol Name\"=RDP\n",
        )
        .unwrap();
        std::fs::write(
            cfg.path().join("ButtonBarV5.ini"),
            "Z:\"Kube\"=00000002\n SEND,get pods\\\\n,pods,,,0,1,,\n RUN,x.vbs,,,r,0,1,,\nZ:\"Mine\"=00000000\n",
        )
        .unwrap();
        let store = crate::session::Store::new(out.path().to_path_buf());
        // «Mine» exists already: not imported again
        let r = import(cfg.path(), &store, &["Mine".to_string()], true, true).unwrap();
        assert_eq!(
            (
                r.sessions_added,
                r.sessions_skipped,
                r.buttons_skipped,
                r.bars_existing
            ),
            (1, 1, 1, 1)
        );
        assert_eq!(r.bars.len(), 1);
        assert_eq!(r.bars[0].name, "Kube");
        let s = store.get("AI/gpu1").unwrap();
        assert_eq!(s.button_bar.as_deref(), Some("Kube")); // imported with this import
        // again: nothing new, the session is left as it is
        let again = import(cfg.path(), &store, &["Kube".to_string()], true, false).unwrap();
        assert_eq!((again.sessions_added, again.sessions_existing), (0, 1));
        // a session that names a bar that is nowhere loses the name
        std::fs::write(
            cfg.path().join("Sessions/n.ini"),
            SSH.replace("Kube", "Nowhere"),
        )
        .unwrap();
        import(cfg.path(), &store, &[], true, false).unwrap();
        assert_eq!(store.get("n").unwrap().button_bar, None);
        assert_eq!(find_config(cfg.path()), None);
    }
}
