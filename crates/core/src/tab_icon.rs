//! A tab's icon, SecureCRT style: a glyph for what the tab is (ssh, telnet, serial, local shell,
//! SFTP, note, console), drawn in the colour of its state. Full-colour SVG, so it looks the same
//! under any icon theme.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Ssh,
    Telnet,
    Serial,
    Local,
    Sftp,
    Note,
    Console,
    // Picked for a session (Session Options → Appearance):
    Server,
    Router,
    Switch,
    Firewall,
    Database,
    Cloud,
    Desktop,
    Container,
    Laptop,
    Mobile,
    Printer,
    Camera,
    Wifi,
    Storage,
    Chip,
    Power,
    Antenna,
    Web,
    Mail,
    Vpn,
    Key,
    Service,
    Monitoring,
    Code,
    Deploy,
    Office,
    Tools,
    Lab,
    Bug,
    Vm,
    Star,
    Flag,
}

/// The icons a session can pick, by the name kept in its file.
pub const PICKABLE: [(&str, Glyph); 36] = [
    ("ssh", Glyph::Ssh),
    ("terminal", Glyph::Telnet),
    ("serial", Glyph::Serial),
    ("local", Glyph::Local),
    ("server", Glyph::Server),
    ("router", Glyph::Router),
    ("switch", Glyph::Switch),
    ("firewall", Glyph::Firewall),
    ("database", Glyph::Database),
    ("cloud", Glyph::Cloud),
    ("desktop", Glyph::Desktop),
    ("container", Glyph::Container),
    ("laptop", Glyph::Laptop),
    ("mobile", Glyph::Mobile),
    ("printer", Glyph::Printer),
    ("camera", Glyph::Camera),
    ("wifi", Glyph::Wifi),
    ("storage", Glyph::Storage),
    ("chip", Glyph::Chip),
    ("power", Glyph::Power),
    ("antenna", Glyph::Antenna),
    ("web", Glyph::Web),
    ("mail", Glyph::Mail),
    ("vpn", Glyph::Vpn),
    ("key", Glyph::Key),
    ("service", Glyph::Service),
    ("monitoring", Glyph::Monitoring),
    ("code", Glyph::Code),
    ("deploy", Glyph::Deploy),
    ("office", Glyph::Office),
    ("tools", Glyph::Tools),
    ("lab", Glyph::Lab),
    ("bug", Glyph::Bug),
    ("vm", Glyph::Vm),
    ("star", Glyph::Star),
    ("flag", Glyph::Flag),
];

impl Glyph {
    /// The session's own icon (`Session::icon`), else the one of its protocol.
    pub fn for_session(icon: &str, p: crate::session::Protocol) -> Self {
        (PICKABLE.iter())
            .find(|(n, _)| *n == icon)
            .map_or_else(|| Self::of(p), |(_, g)| *g)
    }

    /// A saved session's (the Session Manager's icon).
    pub fn of(p: crate::session::Protocol) -> Self {
        use crate::session::Protocol as P;
        match p {
            P::Ssh2 => Glyph::Ssh,
            P::Telnet => Glyph::Telnet,
            P::Serial => Glyph::Serial,
            P::Local => Glyph::Local,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Started, nothing received yet (ssh still connecting / logging in).
    Connecting,
    Connected,
    /// New output while the tab was not in front.
    Output,
    /// That output stopped a while ago (SecureCRT's Inactivity): the command is likely done.
    Quiet,
    Disconnected,
    /// `crt.Session.Lock`: the keyboard is the script's.
    Locked,
    /// A note or the console: no connection.
    Plain,
}

/// How long unread output must have stopped for a tab to show `Quiet`.
pub const QUIET_SECS: u64 = 5;

/// The state of a tab whose program `alive` is (or SFTP is connected), that has `seen` output,
/// has output the user has not looked at (`unread`), whose output stopped `QUIET_SECS` ago
/// (`quiet`), and is `locked` by a script.
pub fn state(alive: bool, seen: bool, unread: bool, quiet: bool, locked: bool) -> State {
    match (alive, locked, unread, seen) {
        (false, ..) => State::Disconnected,
        (true, true, ..) => State::Locked,
        (true, _, true, _) if quiet => State::Quiet,
        (true, _, true, _) => State::Output,
        (true, _, _, true) => State::Connected,
        _ => State::Connecting,
    }
}

pub fn colour(s: State) -> &'static str {
    match s {
        State::Connecting => "#d29922",
        State::Locked => "#a371f7",
        State::Connected => "#2ea043",
        State::Output => "#388bfd",
        State::Quiet => "#2bb5a5",
        State::Disconnected => "#da3633",
        State::Plain => "#8b949e",
    }
}

/// The 16×16 SVG of `g` in the colour of `s` (a locked tab: purple, with a padlock).
pub fn svg(g: Glyph, s: State) -> String {
    let c = colour(s);
    // A screen with a ">_" prompt.
    let screen = format!(
        r#"<rect x="1" y="2" width="14" height="12" rx="2" fill="none" stroke="{c}" stroke-width="1.5"/><path d="M4 6l2.5 2L4 10M8 10.5h4" fill="none" stroke="{c}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>"#
    );
    let lock = |fill: &str| {
        format!(
            r##"<rect x="8.5" y="9.5" width="7" height="6" rx="1" fill="{fill}" stroke="#1e1e1e" stroke-width="0.8"/><path d="M10 9.5V8a2 2 0 0 1 4 0v1.5" fill="none" stroke="{fill}" stroke-width="1.3"/>"##
        )
    };
    let body = match g {
        Glyph::Ssh => format!("{screen}{}", lock(c)),
        Glyph::Telnet | Glyph::Console => screen,
        Glyph::Serial => format!(
            r#"<path d="M1.5 4h13l-2 8h-9z" fill="none" stroke="{c}" stroke-width="1.5" stroke-linejoin="round"/><circle cx="5" cy="7" r="1" fill="{c}"/><circle cx="8" cy="7" r="1" fill="{c}"/><circle cx="11" cy="7" r="1" fill="{c}"/><circle cx="6.5" cy="9.5" r="1" fill="{c}"/><circle cx="9.5" cy="9.5" r="1" fill="{c}"/>"#
        ),
        Glyph::Local => format!(
            r#"<path d="M2 8l6-5.5L14 8M4 7v7h3v-4h2v4h3V7" fill="none" stroke="{c}" stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round"/>"#
        ),
        Glyph::Sftp => format!(
            r#"<path d="M1.5 4.5a1 1 0 0 1 1-1h3.5l1.5 1.5h6a1 1 0 0 1 1 1V13a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z" fill="none" stroke="{c}" stroke-width="1.5" stroke-linejoin="round"/><path d="M8 7v5M6 10l2 2 2-2" fill="none" stroke="{c}" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#
        ),
        Glyph::Server => format!(
            r#"<rect x="2" y="1.5" width="12" height="5.5" rx="1" fill="none" stroke="{c}" stroke-width="1.4"/><rect x="2" y="9" width="12" height="5.5" rx="1" fill="none" stroke="{c}" stroke-width="1.4"/><circle cx="5" cy="4.25" r="1" fill="{c}"/><circle cx="5" cy="11.75" r="1" fill="{c}"/>"#
        ),
        Glyph::Router => format!(
            r#"<rect x="1.5" y="8.5" width="13" height="5" rx="1.2" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M8 8.5V5M5 4.5a4.2 4.2 0 0 1 6 0M3.3 2.8a6.6 6.6 0 0 1 9.4 0" fill="none" stroke="{c}" stroke-width="1.3" stroke-linecap="round"/><circle cx="4.5" cy="11" r=".9" fill="{c}"/>"#
        ),
        Glyph::Switch => format!(
            r#"<rect x="1.5" y="4" width="13" height="8" rx="1.2" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M4 7h1.6M7.2 7h1.6M10.4 7H12M4 9.5h1.6M7.2 9.5h1.6M10.4 9.5H12" stroke="{c}" stroke-width="1.3" stroke-linecap="round"/>"#
        ),
        Glyph::Firewall => format!(
            r#"<path d="M8 1.5l5.5 2v4.3c0 3.2-2.3 5.6-5.5 6.7-3.2-1.1-5.5-3.5-5.5-6.7V3.5z" fill="none" stroke="{c}" stroke-width="1.4" stroke-linejoin="round"/><path d="M5 6.5h6M5 9.5h6M8 6.5v3" stroke="{c}" stroke-width="1.2"/>"#
        ),
        Glyph::Database => format!(
            r#"<ellipse cx="8" cy="3.5" rx="5.5" ry="2" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M2.5 3.5v9c0 1.1 2.5 2 5.5 2s5.5-.9 5.5-2v-9M2.5 8c0 1.1 2.5 2 5.5 2s5.5-.9 5.5-2" fill="none" stroke="{c}" stroke-width="1.4"/>"#
        ),
        Glyph::Cloud => format!(
            r#"<path d="M4.5 13h7.5a3 3 0 0 0 .3-6 4.2 4.2 0 0 0-8.1-.8A3.4 3.4 0 0 0 4.5 13z" fill="none" stroke="{c}" stroke-width="1.4" stroke-linejoin="round"/>"#
        ),
        Glyph::Desktop => format!(
            r#"<rect x="1.5" y="2" width="13" height="9" rx="1.2" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M6 14h4M8 11v3" stroke="{c}" stroke-width="1.4" stroke-linecap="round"/>"#
        ),
        Glyph::Container => format!(
            r#"<path d="M8 1.5l6 3v7l-6 3-6-3v-7z" fill="none" stroke="{c}" stroke-width="1.4" stroke-linejoin="round"/><path d="M2 4.5l6 3 6-3M8 7.5v7" fill="none" stroke="{c}" stroke-width="1.2" stroke-linejoin="round"/>"#
        ),
        Glyph::Laptop => format!(
            r#"<rect x="3" y="3" width="10" height="7" rx="1" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M1.5 12.5h13" stroke="{c}" stroke-width="1.5" stroke-linecap="round"/>"#
        ),
        Glyph::Mobile => format!(
            r#"<rect x="4.5" y="1.5" width="7" height="13" rx="1.5" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M7 12.2h2" stroke="{c}" stroke-width="1.3" stroke-linecap="round"/>"#
        ),
        Glyph::Printer => format!(
            r#"<path d="M4.5 6V2h7v4M4.5 11.5H2.5V6.5h11v5h-2" fill="none" stroke="{c}" stroke-width="1.4" stroke-linejoin="round"/><rect x="4.5" y="9.5" width="7" height="5" fill="none" stroke="{c}" stroke-width="1.4"/>"#
        ),
        Glyph::Camera => format!(
            r#"<rect x="1.5" y="4.5" width="9" height="7" rx="1.2" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M10.5 7l4-2v6l-4-2z" fill="none" stroke="{c}" stroke-width="1.3" stroke-linejoin="round"/><circle cx="6" cy="8" r="1.4" fill="{c}"/>"#
        ),
        Glyph::Wifi => format!(
            r#"<path d="M2 6.2a8.5 8.5 0 0 1 12 0M4.2 8.6a5.3 5.3 0 0 1 7.6 0M6.4 11a2.2 2.2 0 0 1 3.2 0" fill="none" stroke="{c}" stroke-width="1.4" stroke-linecap="round"/><circle cx="8" cy="13.2" r="1" fill="{c}"/>"#
        ),
        Glyph::Storage => format!(
            r#"<rect x="1.5" y="3" width="13" height="10" rx="1.5" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M1.5 9.5h13" stroke="{c}" stroke-width="1.2"/><circle cx="11.5" cy="11.3" r=".9" fill="{c}"/><path d="M4 11.3h3.5" stroke="{c}" stroke-width="1.2" stroke-linecap="round"/>"#
        ),
        Glyph::Chip => format!(
            r#"<rect x="4" y="4" width="8" height="8" rx="1" fill="none" stroke="{c}" stroke-width="1.4"/><rect x="6.5" y="6.5" width="3" height="3" fill="{c}"/><path d="M6 1.5V4M10 1.5V4M6 12v2.5M10 12v2.5M1.5 6H4M1.5 10H4M12 6h2.5M12 10h2.5" stroke="{c}" stroke-width="1.2" stroke-linecap="round"/>"#
        ),
        Glyph::Power => format!(
            r#"<path d="M5 3.5a5.5 5.5 0 1 0 6 0" fill="none" stroke="{c}" stroke-width="1.5" stroke-linecap="round"/><path d="M8 1.5v6" stroke="{c}" stroke-width="1.5" stroke-linecap="round"/>"#
        ),
        Glyph::Antenna => format!(
            r#"<path d="M8 6v8.5M5.5 14.5h5" stroke="{c}" stroke-width="1.4" stroke-linecap="round"/><circle cx="8" cy="5" r="1.2" fill="{c}"/><path d="M4.8 2a4.5 4.5 0 0 0 0 6M11.2 2a4.5 4.5 0 0 1 0 6" fill="none" stroke="{c}" stroke-width="1.3" stroke-linecap="round"/>"#
        ),
        Glyph::Web => format!(
            r#"<circle cx="8" cy="8" r="6.3" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M1.7 8h12.6M8 1.7c2.2 2.4 2.2 10.2 0 12.6M8 1.7c-2.2 2.4-2.2 10.2 0 12.6" fill="none" stroke="{c}" stroke-width="1.1"/>"#
        ),
        Glyph::Mail => format!(
            r#"<rect x="1.5" y="3.5" width="13" height="9" rx="1.2" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M2 4.5l6 4.5 6-4.5" fill="none" stroke="{c}" stroke-width="1.3" stroke-linejoin="round"/>"#
        ),
        Glyph::Vpn => format!(
            r#"<rect x="3" y="7" width="10" height="7.5" rx="1.2" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" fill="none" stroke="{c}" stroke-width="1.4"/><circle cx="8" cy="10.7" r="1.1" fill="{c}"/>"#
        ),
        Glyph::Key => format!(
            r#"<circle cx="5" cy="8" r="3" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M8 8h6.5M12 8v2.5M14 8v2" stroke="{c}" stroke-width="1.4" stroke-linecap="round"/>"#
        ),
        Glyph::Service => format!(
            r#"<circle cx="8" cy="8" r="2.2" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" stroke="{c}" stroke-width="1.4" stroke-linecap="round"/>"#
        ),
        Glyph::Monitoring => format!(
            r#"<rect x="1.5" y="2" width="13" height="10" rx="1.2" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M3.5 9l2.5-3 2 2 2.5-3.5L12.5 7" fill="none" stroke="{c}" stroke-width="1.3" stroke-linejoin="round" stroke-linecap="round"/><path d="M6 14.5h4" stroke="{c}" stroke-width="1.4" stroke-linecap="round"/>"#
        ),
        Glyph::Code => format!(
            r#"<path d="M5.5 4L2 8l3.5 4M10.5 4L14 8l-3.5 4M9 2.5L7 13.5" fill="none" stroke="{c}" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>"#
        ),
        Glyph::Deploy => format!(
            r#"<path d="M8 1.5c2.5 1.5 3.5 4 3.5 6.5l-1.5 2.5H6L4.5 8c0-2.5 1-5 3.5-6.5z" fill="none" stroke="{c}" stroke-width="1.3" stroke-linejoin="round"/><circle cx="8" cy="6" r="1.2" fill="{c}"/><path d="M6.5 12.5L8 14.5l1.5-2" fill="none" stroke="{c}" stroke-width="1.3" stroke-linejoin="round"/>"#
        ),
        Glyph::Office => format!(
            r#"<path d="M2.5 14.5V2.5h7v12M9.5 6.5h4v8M1.5 14.5h13" fill="none" stroke="{c}" stroke-width="1.4" stroke-linejoin="round"/><path d="M4.5 5h2.5M4.5 8h2.5M4.5 11h2.5M11 9h1M11 12h1" stroke="{c}" stroke-width="1.2" stroke-linecap="round"/>"#
        ),
        Glyph::Tools => format!(
            r#"<path d="M10.5 1.8a3.2 3.2 0 0 0-3.9 4.1L2 10.5 4.5 13l4.6-4.6a3.2 3.2 0 0 0 4.1-3.9l-2 2-1.8-.5-.5-1.8z" fill="none" stroke="{c}" stroke-width="1.3" stroke-linejoin="round"/>"#
        ),
        Glyph::Lab => format!(
            r#"<path d="M6 1.5h4M6.5 1.5v4.5L2.5 13a1 1 0 0 0 .9 1.5h9.2a1 1 0 0 0 .9-1.5l-4-7V1.5" fill="none" stroke="{c}" stroke-width="1.4" stroke-linejoin="round"/><path d="M4.5 10h7" stroke="{c}" stroke-width="1.2"/>"#
        ),
        Glyph::Bug => format!(
            r#"<rect x="4.5" y="4.5" width="7" height="9" rx="3.5" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M8 7.5v6M4.5 8H2M11.5 8H14M4.5 11.5H2.5M11.5 11.5h2M6 4.5L5 2.5M10 4.5l1-2" stroke="{c}" stroke-width="1.2" stroke-linecap="round"/>"#
        ),
        Glyph::Vm => format!(
            r#"<rect x="4" y="1.5" width="10.5" height="8" rx="1" fill="none" stroke="{c}" stroke-width="1.3"/><rect x="1.5" y="6" width="10.5" height="8.5" rx="1" fill="none" stroke="{c}" stroke-width="1.4"/><path d="M4 9.5l1.8 1.5L4 12.5" fill="none" stroke="{c}" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>"#
        ),
        Glyph::Star => format!(
            r#"<path d="M8 1.8l1.9 3.9 4.3.6-3.1 3 .7 4.3L8 11.6l-3.8 2 .7-4.3-3.1-3 4.3-.6z" fill="none" stroke="{c}" stroke-width="1.3" stroke-linejoin="round"/>"#
        ),
        Glyph::Flag => format!(
            r#"<path d="M3.5 14.5V2" stroke="{c}" stroke-width="1.5" stroke-linecap="round"/><path d="M3.5 2.5h9l-2 3 2 3h-9" fill="none" stroke="{c}" stroke-width="1.4" stroke-linejoin="round"/>"#
        ),
        Glyph::Note => format!(
            r#"<path d="M3 1.5h7l3 3V14.5H3z" fill="none" stroke="{c}" stroke-width="1.5" stroke-linejoin="round"/><path d="M5.5 7h5M5.5 9.5h5M5.5 12h3" stroke="{c}" stroke-width="1.2" stroke-linecap="round"/>"#
        ),
    };
    let over = match (s, g) {
        (State::Locked, g) if g != Glyph::Ssh => lock(colour(State::Locked)),
        _ => String::new(),
    };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">{body}{over}</svg>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tab_state_follows_its_connection_and_output() {
        assert_eq!(state(true, false, false, false, false), State::Connecting);
        assert_eq!(state(true, true, false, false, false), State::Connected);
        assert_eq!(state(true, true, true, false, false), State::Output);
        assert_eq!(state(true, true, true, true, false), State::Quiet);
        assert_eq!(state(true, true, false, true, false), State::Connected); // read: not quiet
        assert_eq!(state(true, true, true, false, true), State::Locked);
        assert_eq!(state(false, true, true, true, true), State::Disconnected);
    }

    #[test]
    fn a_session_may_pick_its_icon() {
        use crate::session::Protocol as P;
        assert_eq!(Glyph::for_session("router", P::Ssh2), Glyph::Router);
        assert_eq!(Glyph::for_session("", P::Serial), Glyph::Serial);
        assert_eq!(Glyph::for_session("gone", P::Ssh2), Glyph::Ssh); // unknown: by protocol
        for (_, g) in PICKABLE {
            assert!(svg(g, State::Connected).contains(colour(State::Connected)));
        }
    }

    #[test]
    fn a_session_gets_the_glyph_of_its_protocol() {
        use crate::session::Protocol as P;
        assert_eq!(Glyph::of(P::Ssh2), Glyph::Ssh);
        assert_eq!(Glyph::of(P::Serial), Glyph::Serial);
        assert_eq!(Glyph::of(P::Local), Glyph::Local);
    }

    #[test]
    fn the_icon_is_drawn_in_its_state_colour() {
        for g in [
            Glyph::Ssh,
            Glyph::Telnet,
            Glyph::Serial,
            Glyph::Local,
            Glyph::Sftp,
            Glyph::Note,
            Glyph::Console,
        ] {
            let s = svg(g, State::Disconnected);
            assert!(s.starts_with("<svg") && s.ends_with("</svg>"));
            assert!(s.contains(colour(State::Disconnected)));
            assert!(!s.contains(colour(State::Connected)));
        }
        assert!(svg(Glyph::Local, State::Locked).matches("<rect").count() >= 1); // the padlock
        // Locked and connecting must not look alike (the ssh glyph has a padlock already).
        assert_ne!(
            svg(Glyph::Ssh, State::Locked),
            svg(Glyph::Ssh, State::Connecting)
        );
    }
}
