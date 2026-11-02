//! Icon names a toolbar button can use: the symbolic icons of the given themes.
//! From shagoff-commander (`crates/core/src/icons.rs`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Names of `*-symbolic` icons (svg / png) under `<root>/<theme>/` for every root and theme,
/// sorted, each once. Missing dirs are skipped.
pub fn symbolic(roots: &[PathBuf], themes: &[&str]) -> Vec<String> {
    let mut names = BTreeSet::new();
    for root in roots {
        for theme in themes {
            walk(&root.join(theme), &mut names);
        }
    }
    names.into_iter().collect()
}

/// Where icon themes live: `~/.icons`, `$XDG_DATA_HOME/icons`, then `$XDG_DATA_DIRS/*/icons`.
pub fn roots() -> Vec<PathBuf> {
    let home = std::env::home_dir();
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    (home.map(|h| h.join(".icons")).into_iter())
        .chain(data_home.map(|d| d.join("icons")))
        .chain(data_dirs.split(':').map(|d| Path::new(d).join("icons")))
        .collect()
}

/// The icon theme to give libcosmic outside COSMIC: it looks only in «Cosmic», and without it
/// most icons (the window's minimize / maximize too) draw blank. Adwaita comes with GTK.
pub fn fallback_theme(roots: &[PathBuf]) -> Option<&'static str> {
    (!roots.iter().any(|r| r.join("Cosmic").is_dir())).then_some("Adwaita")
}

fn walk(dir: &Path, names: &mut BTreeSet<String>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        // file_type: a theme's symlinked dirs (`scalable -> ...`) are not followed, no loops.
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            walk(&p, names);
        } else if let Some(stem) = p.file_stem().and_then(|s| s.to_str())
            && stem.ends_with("-symbolic")
            && p.extension().is_some_and(|x| x == "svg" || x == "png")
        {
            names.insert(stem.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbolic_icons_of_the_themes_once_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let file = |p: &str| {
            let p = tmp.path().join(p);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, "").unwrap();
        };
        file("a/Cosmic/scalable/actions/edit-copy-symbolic.svg");
        file("a/Cosmic/scalable/actions/edit-copy.svg"); // not symbolic
        file("a/Other/x/zzz-symbolic.svg"); // other theme
        file("b/Adwaita/16x16/go-up-symbolic.png");
        file("b/Adwaita/symbolic/edit-copy-symbolic.svg"); // twice
        file("b/Adwaita/symbolic/readme-symbolic.txt");
        let roots = [
            tmp.path().join("a"),
            tmp.path().join("b"),
            tmp.path().join("none"),
        ];
        assert_eq!(
            symbolic(&roots, &["Cosmic", "Adwaita"]),
            ["edit-copy-symbolic", "go-up-symbolic"]
        );
    }

    #[test]
    fn regression_no_cosmic_icon_theme_falls_back_to_adwaita() {
        // Linux Lite (XFCE): no Cosmic theme, the window's minimize / maximize drew blank.
        let tmp = tempfile::tempdir().unwrap();
        let roots = [tmp.path().join("a"), tmp.path().join("b")];
        fs::create_dir_all(roots[0].join("Papirus")).unwrap();
        assert_eq!(fallback_theme(&roots), Some("Adwaita"));
        fs::create_dir_all(roots[1].join("Cosmic")).unwrap();
        assert_eq!(fallback_theme(&roots), None);
    }
}
