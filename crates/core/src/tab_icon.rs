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
}

impl Glyph {
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
    Disconnected,
    /// `crt.Session.Lock`: the keyboard is the script's.
    Locked,
    /// A note or the console: no connection.
    Plain,
}

/// The state of a tab whose program `alive` is (or SFTP is connected), that has `seen` output,
/// has output the user has not looked at (`unread`), and is `locked` by a script.
pub fn state(alive: bool, seen: bool, unread: bool, locked: bool) -> State {
    match (alive, locked, unread, seen) {
        (false, ..) => State::Disconnected,
        (true, true, ..) => State::Locked,
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
        assert_eq!(state(true, false, false, false), State::Connecting);
        assert_eq!(state(true, true, false, false), State::Connected);
        assert_eq!(state(true, true, true, false), State::Output);
        assert_eq!(state(true, true, true, true), State::Locked);
        assert_eq!(state(false, true, true, true), State::Disconnected);
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
