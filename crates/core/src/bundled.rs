//! The program's own files, built into the binary and written under `~/.config/shagshell/scripts`
//! at start (one place for everything; no `/usr/share`). `.lib/` and `shag_blocks.py` are ours and
//! rewritten when they differ; examples and templates are written only when missing, so the user's
//! edits stay.

use std::path::Path;

/// `scripts/.lib/`: what `python3` imports (`crt.py`) and runs (`shag_parse.py`).
pub const LIB: &[(&str, &str)] = &[
    ("crt.py", include_str!("../../../res/python/crt.py")),
    (
        "shag_parse.py",
        include_str!("../../../res/python/shag_parse.py"),
    ),
];

/// `scripts/shag_blocks.py`: the Script Builder's big blocks, next to the scripts (ours: rewritten).
pub const BLOCKS: &str = include_str!("../../../res/python/shag_blocks.py");

/// `scripts/templates/`: the Script Builder's ready templates (written when missing).
pub const TEMPLATES: &[(&str, &str)] = &[
    (
        "01_output_to_clipboard.py",
        include_str!("../../../res/builder-templates/01_output_to_clipboard.py"),
    ),
    (
        "02_hosts_to_file.py",
        include_str!("../../../res/builder-templates/02_hosts_to_file.py"),
    ),
    (
        "03_parallel_logs.py",
        include_str!("../../../res/builder-templates/03_parallel_logs.py"),
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

/// `scripts/examples-short/`: `Example-short/` of the repository (the same scripts in short blocks).
pub const EXAMPLES_SHORT: &[(&str, &str)] = &[
    (
        "01_dialogs_and_arguments.py",
        include_str!("../../../Example-short/01_dialogs_and_arguments.py"),
    ),
    (
        "02_send_and_read.py",
        include_str!("../../../Example-short/02_send_and_read.py"),
    ),
    (
        "03_connect_saved_session.py",
        include_str!("../../../Example-short/03_connect_saved_session.py"),
    ),
    (
        "04_many_hosts_to_file.py",
        include_str!("../../../Example-short/04_many_hosts_to_file.py"),
    ),
    (
        "05_parse_output.py",
        include_str!("../../../Example-short/05_parse_output.py"),
    ),
    (
        "06_output_to_clipboard.py",
        include_str!("../../../Example-short/06_output_to_clipboard.py"),
    ),
    (
        "07_save_result_to_file.py",
        include_str!("../../../Example-short/07_save_result_to_file.py"),
    ),
    (
        "08_send_to_all_tabs.py",
        include_str!("../../../Example-short/08_send_to_all_tabs.py"),
    ),
    (
        "09_auto_responder.py",
        include_str!("../../../Example-short/09_auto_responder.py"),
    ),
    (
        "10_comment_in_log.py",
        include_str!("../../../Example-short/10_comment_in_log.py"),
    ),
    (
        "11_network_device.py",
        include_str!("../../../Example-short/11_network_device.py"),
    ),
    (
        "12_dialogs_gallery.py",
        include_str!("../../../Example-short/12_dialogs_gallery.py"),
    ),
    (
        "13_pick_file_and_hosts.py",
        include_str!("../../../Example-short/13_pick_file_and_hosts.py"),
    ),
    (
        "14_pre_script.py",
        include_str!("../../../Example-short/14_pre_script.py"),
    ),
    (
        "15_new_session.py",
        include_str!("../../../Example-short/15_new_session.py"),
    ),
    (
        "16_scheduled.py",
        include_str!("../../../Example-short/16_scheduled.py"),
    ),
    (
        "17_transfer.py",
        include_str!("../../../Example-short/17_transfer.py"),
    ),
    (
        "README.md",
        include_str!("../../../Example-short/README.md"),
    ),
];

/// Writes `LIB` into `scripts/.lib` (when it differs), `EXAMPLES` / `EXAMPLES_SHORT` / `TEMPLATES`
/// into their folders (when missing).
pub fn install(scripts: &Path) -> std::io::Result<()> {
    let lib = scripts.join(".lib");
    std::fs::create_dir_all(&lib)?;
    for (name, text) in LIB {
        let p = lib.join(name);
        if std::fs::read_to_string(&p).ok().as_deref() != Some(*text) {
            std::fs::write(p, text)?;
        }
    }
    let blocks = scripts.join("shag_blocks.py");
    if std::fs::read_to_string(&blocks).ok().as_deref() != Some(BLOCKS) {
        std::fs::write(blocks, BLOCKS)?;
    }
    for (dir, files) in [
        ("examples", EXAMPLES),
        ("examples-short", EXAMPLES_SHORT),
        ("templates", TEMPLATES),
    ] {
        let d = scripts.join(dir);
        std::fs::create_dir_all(&d)?;
        for (name, text) in files {
            let p = d.join(name);
            if !p.exists() {
                std::fs::write(p, text)?;
            }
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
    fn install_writes_blocks_and_templates() {
        let d = tempfile::tempdir().unwrap();
        install(d.path()).unwrap();
        for (n, text) in TEMPLATES {
            assert_eq!(
                std::fs::read_to_string(d.path().join("templates").join(n)).unwrap(),
                *text
            );
        }
        let blocks = d.path().join("shag_blocks.py");
        assert_eq!(std::fs::read_to_string(&blocks).unwrap(), BLOCKS);
        std::fs::write(&blocks, "old").unwrap();
        install(d.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(&blocks).unwrap(),
            BLOCKS,
            "ours: rewritten"
        );
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
        for (dir, files) in [("Example", EXAMPLES), ("Example-short", EXAMPLES_SHORT)] {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(dir);
            let mut repo: Vec<String> = std::fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            repo.sort();
            let ours: Vec<&str> = files.iter().map(|(n, _)| *n).collect();
            assert_eq!(ours, repo);
        }
    }

    #[test]
    fn regression_examples_use_only_known_names() {
        // Example/14 used ICON_WARNING, which crt.py does not have (ICON_WARN): a NameError.
        use std::process::Command;
        if Command::new("python3").arg("-V").output().is_err() {
            return;
        }
        const SNIPPET: &str = r#"
import ast, builtins, glob, sys
root = sys.argv[1]
sys.path.insert(0, root + "/res/python")
import shag_blocks, crt
known = set(dir(builtins)) | set(shag_blocks.__all__) | {"crt", "ScriptError"}
known |= {n for n in dir(crt) if n.isupper()}
bad = []
for f in sorted(glob.glob(root + "/Example*/*.py")):
    t = ast.parse(open(f, encoding="utf-8").read())
    have = {a.asname or a.name for n in ast.walk(t) if isinstance(n, (ast.Import, ast.ImportFrom)) for a in n.names}
    for n in ast.walk(t):
        if isinstance(n, ast.Name) and isinstance(n.ctx, ast.Store):
            have.add(n.id)
        elif isinstance(n, ast.FunctionDef):
            have.add(n.name)
            have |= {a.arg for a in n.args.args}
        elif isinstance(n, ast.ExceptHandler) and n.name:
            have.add(n.name)
    used = {n.id for n in ast.walk(t) if isinstance(n, ast.Name) and isinstance(n.ctx, ast.Load)}
    bad += ["%s: %s" % (f, u) for u in sorted(used - have - known)]
assert not bad, bad
"#;
        let o = Command::new("python3")
            .args(["-I", "-c", SNIPPET])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }

    #[test]
    fn install_writes_short_examples() {
        let d = tempfile::tempdir().unwrap();
        install(d.path()).unwrap();
        for (n, text) in EXAMPLES_SHORT {
            assert_eq!(
                std::fs::read_to_string(d.path().join("examples-short").join(n)).unwrap(),
                *text
            );
        }
    }
}
