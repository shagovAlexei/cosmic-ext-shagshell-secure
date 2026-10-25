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
}

/// The icons a session can pick, by the name kept in its file.
pub const PICKABLE: [(&str, Glyph); 12] = [
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
