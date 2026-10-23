// Adapted from cosmic-term (https://github.com/pop-os/cosmic-term, GPL-3.0-only), src/terminal_theme.rs at 909a0d3. Via shagoff-commander (crates/app/src/term/).
//! The COSMIC dark / light terminal colors (scheme import / export left out).
use alacritty_terminal::{
    term::color::Colors,
    vte::ansi::{NamedColor, Rgb},
};

fn auto_colors() -> Colors {
    let mut colors = Colors::default();

    // These colors come from `ransid`: https://gitlab.redox-os.org/redox-os/ransid/-/blob/master/src/color.rs
    /* Indexed colors */
    for value in 16..=231 {
        let convert = |value: u8| -> u8 {
            match value {
                0 => 0,
                _ => value * 0x28 + 0x28,
            }
        };

        let r = convert((value - 16) / 36 % 6);
        let g = convert((value - 16) / 6 % 6);
        let b = convert((value - 16) % 6);
        colors[value as usize] = Some(Rgb { r, g, b });
    }

    /* Grays */
    for value in 232..=255 {
        let gray = (value - 232) * 10 + 8;
        colors[value as usize] = Some(Rgb {
            r: gray,
            g: gray,
            b: gray,
        });
    }

    colors
}

pub fn cosmic_dark() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x1B1B1B));
    colors[NamedColor::Red] = Some(encode_rgb(0xF16161));
    colors[NamedColor::Green] = Some(encode_rgb(0x7CB987));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xDDC74C));
    colors[NamedColor::Blue] = Some(encode_rgb(0x6296BE));
    colors[NamedColor::Magenta] = Some(encode_rgb(0xBE6DEE));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x49BAC8));
    colors[NamedColor::White] = Some(encode_rgb(0xBEBEBE));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x808080));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xFF8985));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x97D5A0));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0xFAE365));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x7DB1DA));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0xD68EFF));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x49BAC8));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xC4C4C4));

    colors[NamedColor::DimBlack] = Some(encode_rgb(0x000000));
    colors[NamedColor::DimRed] = Some(encode_rgb(0xA04040));
    colors[NamedColor::DimGreen] = Some(encode_rgb(0x5D7D62));
    colors[NamedColor::DimYellow] = Some(encode_rgb(0x9E914A));
    colors[NamedColor::DimBlue] = Some(encode_rgb(0x486073));
    colors[NamedColor::DimMagenta] = Some(encode_rgb(0x7F46A1));
    colors[NamedColor::DimCyan] = Some(encode_rgb(0x3F7F87));
    colors[NamedColor::DimWhite] = Some(encode_rgb(0x898989));

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::BrightWhite];
    // Background comes from theme settings: colors[NamedColor::Background] = colors[NamedColor::Black];
    colors[NamedColor::Cursor] = colors[NamedColor::BrightWhite];
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightWhite];
    colors[NamedColor::DimForeground] = colors[NamedColor::DimWhite];

    colors
}

pub fn cosmic_light() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x1B1B1B));
    colors[NamedColor::Red] = Some(encode_rgb(0xA24F50));
    colors[NamedColor::Green] = Some(encode_rgb(0x437E4F));
    colors[NamedColor::Yellow] = Some(encode_rgb(0x7D6E1E));
    colors[NamedColor::Blue] = Some(encode_rgb(0x516D94));
    colors[NamedColor::Magenta] = Some(encode_rgb(0x9A30CA));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x227A85));
    colors[NamedColor::White] = Some(encode_rgb(0xD7D7D7));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x000000));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0x890418));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x185529));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0x534800));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x2E496D));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0x6B0091));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x00525A));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xE8E8E8));

    colors[NamedColor::DimBlack] = Some(encode_rgb(0x262626));
    colors[NamedColor::DimRed] = Some(encode_rgb(0xB25D5E));
    colors[NamedColor::DimGreen] = Some(encode_rgb(0x528D5E));
    colors[NamedColor::DimYellow] = Some(encode_rgb(0x8C7D30));
    colors[NamedColor::DimBlue] = Some(encode_rgb(0x5F7CA4));
    colors[NamedColor::DimMagenta] = Some(encode_rgb(0xAA43DB));
    colors[NamedColor::DimCyan] = Some(encode_rgb(0x358994));
    colors[NamedColor::DimWhite] = Some(encode_rgb(0xC4C4C4));

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::Black];
    // Background comes from theme settings: colors[NamedColor::Background] = colors[NamedColor::BrightWhite];
    colors[NamedColor::Cursor] = colors[NamedColor::Black];
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightBlack];
    colors[NamedColor::DimForeground] = colors[NamedColor::DimBlack];

    colors
}

/// A named palette: foreground, background, the 8 normal and 8 bright colours.
struct Scheme {
    key: &'static str,
    name: &'static str,
    fg: u32,
    bg: u32,
    normal: [u32; 8],
    bright: [u32; 8],
}

const SCHEMES: [Scheme; 8] = [
    Scheme {
        key: "solarized-dark",
        name: "Solarized Dark",
        fg: 0x839496,
        bg: 0x002B36,
        normal: [
            0x073642, 0xDC322F, 0x859900, 0xB58900, 0x268BD2, 0xD33682, 0x2AA198, 0xEEE8D5,
        ],
        bright: [
            0x002B36, 0xCB4B16, 0x586E75, 0x657B83, 0x839496, 0x6C71C4, 0x93A1A1, 0xFDF6E3,
        ],
    },
    Scheme {
        key: "solarized-light",
        name: "Solarized Light",
        fg: 0x657B83,
        bg: 0xFDF6E3,
        normal: [
            0x073642, 0xDC322F, 0x859900, 0xB58900, 0x268BD2, 0xD33682, 0x2AA198, 0xEEE8D5,
        ],
        bright: [
            0x002B36, 0xCB4B16, 0x586E75, 0x657B83, 0x839496, 0x6C71C4, 0x93A1A1, 0xFDF6E3,
        ],
    },
    Scheme {
        key: "dracula",
        name: "Dracula",
        fg: 0xF8F8F2,
        bg: 0x282A36,
        normal: [
            0x21222C, 0xFF5555, 0x50FA7B, 0xF1FA8C, 0xBD93F9, 0xFF79C6, 0x8BE9FD, 0xF8F8F2,
        ],
        bright: [
            0x6272A4, 0xFF6E6E, 0x69FF94, 0xFFFFA5, 0xD6ACFF, 0xFF92DF, 0xA4FFFF, 0xFFFFFF,
        ],
    },
    Scheme {
        key: "nord",
        name: "Nord",
        fg: 0xD8DEE9,
        bg: 0x2E3440,
        normal: [
            0x3B4252, 0xBF616A, 0xA3BE8C, 0xEBCB8B, 0x81A1C1, 0xB48EAD, 0x88C0D0, 0xE5E9F0,
        ],
        bright: [
            0x4C566A, 0xBF616A, 0xA3BE8C, 0xEBCB8B, 0x81A1C1, 0xB48EAD, 0x8FBCBB, 0xECEFF4,
        ],
    },
    Scheme {
        key: "gruvbox-dark",
        name: "Gruvbox Dark",
        fg: 0xEBDBB2,
        bg: 0x282828,
        normal: [
            0x282828, 0xCC241D, 0x98971A, 0xD79921, 0x458588, 0xB16286, 0x689D6A, 0xA89984,
        ],
        bright: [
            0x928374, 0xFB4934, 0xB8BB26, 0xFABD2F, 0x83A598, 0xD3869B, 0x8EC07C, 0xEBDBB2,
        ],
    },
    Scheme {
        key: "monokai",
        name: "Monokai",
        fg: 0xF8F8F2,
        bg: 0x272822,
        normal: [
            0x272822, 0xF92672, 0xA6E22E, 0xF4BF75, 0x66D9EF, 0xAE81FF, 0xA1EFE4, 0xF8F8F2,
        ],
        bright: [
            0x75715E, 0xF92672, 0xA6E22E, 0xF4BF75, 0x66D9EF, 0xAE81FF, 0xA1EFE4, 0xF9F8F5,
        ],
    },
    Scheme {
        key: "classic",
        name: "Classic (green on black)",
        fg: 0x33FF33,
        bg: 0x000000,
        normal: [
            0x000000, 0xAA0000, 0x00AA00, 0xAA5500, 0x0000AA, 0xAA00AA, 0x00AAAA, 0xAAAAAA,
        ],
        bright: [
            0x555555, 0xFF5555, 0x55FF55, 0xFFFF55, 0x5555FF, 0xFF55FF, 0x55FFFF, 0xFFFFFF,
        ],
    },
    Scheme {
        key: "tango-light",
        name: "Tango Light",
        fg: 0x2E3436,
        bg: 0xFFFFFF,
        normal: [
            0x2E3436, 0xCC0000, 0x4E9A06, 0xC4A000, 0x3465A4, 0x75507B, 0x06989A, 0xD3D7CF,
        ],
        bright: [
            0x555753, 0xEF2929, 0x8AE234, 0xFCE94F, 0x729FCF, 0xAD7FA8, 0x34E2E2, 0xEEEEEC,
        ],
    },
];

/// `(key, name)` of the built-in schemes, for the pickers.
pub fn schemes() -> Vec<(&'static str, &'static str)> {
    SCHEMES.iter().map(|s| (s.key, s.name)).collect()
}

/// The colours of scheme `key`; `None` for "" or one that is gone (then the theme's own).
pub fn scheme(key: &str) -> Option<Colors> {
    use NamedColor::*;
    let s = SCHEMES.iter().find(|s| s.key == key)?;
    let rgb = |data: u32| {
        Some(Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        })
    };
    let mut colors = auto_colors();
    let normal = [Black, Red, Green, Yellow, Blue, Magenta, Cyan, White];
    let bright = [
        BrightBlack,
        BrightRed,
        BrightGreen,
        BrightYellow,
        BrightBlue,
        BrightMagenta,
        BrightCyan,
        BrightWhite,
    ];
    let dim = [
        DimBlack, DimRed, DimGreen, DimYellow, DimBlue, DimMagenta, DimCyan, DimWhite,
    ];
    for i in 0..8 {
        colors[normal[i]] = rgb(s.normal[i]);
        colors[bright[i]] = rgb(s.bright[i]);
        // Dim: the normal colour pulled toward the background.
        let mix = |a: u32, b: u32| {
            let ch = |sh: u32| (((a >> sh) & 0xFF) * 2 + ((b >> sh) & 0xFF)) / 3;
            (ch(16) << 16) | (ch(8) << 8) | ch(0)
        };
        colors[dim[i]] = rgb(mix(s.normal[i], s.bg));
    }
    colors[Foreground] = rgb(s.fg);
    colors[Background] = rgb(s.bg);
    colors[Cursor] = rgb(s.fg);
    colors[BrightForeground] = rgb(s.bright[7]);
    colors[DimForeground] = colors[DimWhite];
    Some(colors)
}
