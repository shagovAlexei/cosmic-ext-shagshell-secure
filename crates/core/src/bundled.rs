//! The program's own files, built into the binary and written under `~/.config/shagshell/scripts`
//! at start (one place for everything; no `/usr/share`). `.lib/` is ours and rewritten when it
//! differs; examples are written only when missing, so the user's edits stay.

use std::path::Path;

/// `scripts/.lib/`: what `python3` imports (`crt.py`) and runs (`shag_parse.py`).
pub const LIB: &[(&str, &str)] = &[
    ("crt.py", include_str!("../../../res/python/crt.py")),
    (
        "shag_parse.py",
        include_str!("../../../res/python/shag_parse.py"),
    ),
];

/// `scripts/examples/`: `Example/` of the repository.
pub const EXAMPLES: &[(&str, &str)] = &[
    (
        "01_dialogs_and_arguments.py",
        include_str!("../../../Example/01_dialogs_and_arguments.py"),
    ),
    (
        "02_send_and_read.py",
        include_str!("../../../Example/02_send_and_read.py"),
    ),
    (
        "03_connect_saved_session.py",
        include_str!("../../../Example/03_connect_saved_session.py"),
    ),
    (
        "04_many_hosts_to_file.py",
        include_str!("../../../Example/04_many_hosts_to_file.py"),
    ),
    (
        "05_parse_output.py",
        include_str!("../../../Example/05_parse_output.py"),
    ),
    (
        "06_output_to_clipboard.py",
        include_str!("../../../Example/06_output_to_clipboard.py"),
    ),
    (
        "07_save_result_to_file.py",
        include_str!("../../../Example/07_save_result_to_file.py"),
    ),
    (
        "08_send_to_all_tabs.py",
        include_str!("../../../Example/08_send_to_all_tabs.py"),
    ),
    (
        "09_auto_responder.py",
        include_str!("../../../Example/09_auto_responder.py"),
    ),
    (
        "10_comment_in_log.py",
        include_str!("../../../Example/10_comment_in_log.py"),
    ),
    (
        "11_network_device.py",
        include_str!("../../../Example/11_network_device.py"),
    ),
    (
        "12_dialogs_gallery.py",
        include_str!("../../../Example/12_dialogs_gallery.py"),
    ),
    (
        "13_pick_file_and_hosts.py",
        include_str!("../../../Example/13_pick_file_and_hosts.py"),
    ),
    (
        "14_pre_script.py",
        include_str!("../../../Example/14_pre_script.py"),
    ),
    (
        "15_new_session.py",
        include_str!("../../../Example/15_new_session.py"),
    ),
    (
        "16_scheduled.py",
        include_str!("../../../Example/16_scheduled.py"),
    ),
    (
        "17_transfer.py",
        include_str!("../../../Example/17_transfer.py"),
    ),
    ("README.md", include_str!("../../../Example/README.md")),
];

/// Writes `LIB` into `scripts/.lib` (when it differs) and `EXAMPLES` into `scripts/examples`
/// (when missing).
pub fn install(scripts: &Path) -> std::io::Result<()> {
    let lib = scripts.join(".lib");
    std::fs::create_dir_all(&lib)?;
    for (name, text) in LIB {
        let p = lib.join(name);
        if std::fs::read_to_string(&p).ok().as_deref() != Some(*text) {
            std::fs::write(p, text)?;
        }
    }
    let ex = scripts.join("examples");
    std::fs::create_dir_all(&ex)?;
    for (name, text) in EXAMPLES {
        let p = ex.join(name);
        if !p.exists() {
            std::fs::write(p, text)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_writes_lib_and_examples() {
        let d = tempfile::tempdir().unwrap();
        install(d.path()).unwrap();
        for (n, text) in LIB {
            assert_eq!(
                std::fs::read_to_string(d.path().join(".lib").join(n)).unwrap(),
                *text
            );
        }
        for (n, text) in EXAMPLES {
            assert_eq!(
                std::fs::read_to_string(d.path().join("examples").join(n)).unwrap(),
                *text
            );
        }
    }

    #[test]
    fn install_refreshes_lib_keeps_user_edits() {
        let d = tempfile::tempdir().unwrap();
        install(d.path()).unwrap();
        let lib = d.path().join(".lib/crt.py");
        let mine = d.path().join("examples").join(EXAMPLES[0].0);
        std::fs::write(&lib, "old").unwrap();
        std::fs::write(&mine, "mine").unwrap();
        install(d.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&lib).unwrap(), LIB[0].1);
        assert_eq!(std::fs::read_to_string(&mine).unwrap(), "mine");
    }

    #[test]
    fn examples_cover_repo() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Example");
        let mut repo: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        repo.sort();
        let ours: Vec<&str> = EXAMPLES.iter().map(|(n, _)| *n).collect();
        assert_eq!(ours, repo);
    }
}
