//! Transfer → Send / Receive: the file chooser, then the command to run (`core::zmodem`).

use shagshell_core::zmodem::{self, Command, Proto};
use std::path::PathBuf;

/// Files to send (Zmodem / Ymodem: several; Xmodem: one), or where an Xmodem file is to be
/// saved; Ymodem receives into `dir` as the server names its files. `None`: the user gave up.
pub async fn pick(proto: Proto, send: bool, dir: PathBuf) -> Option<Command> {
    use cosmic::dialog::file_chooser;
    match (send, proto) {
        (true, _) => {
            let r = file_chooser::open::Dialog::new()
                .title(crate::fl!("zm-pick-send"))
                .open_files()
                .await
                .ok()?;
            let files: Vec<PathBuf> = r
                .urls()
                .iter()
                .filter_map(|u| u.to_file_path().ok())
                .collect();
            (!files.is_empty()).then(|| zmodem::send(proto, &files))
        }
        (false, Proto::Xmodem) => {
            let r = file_chooser::save::Dialog::new()
                .title(crate::fl!("zm-pick-receive"))
                .save_file()
                .await
                .ok()?;
            let path = r.url()?.to_file_path().ok()?;
            Some(zmodem::receive(proto, &dir, Some(&path)))
        }
        (false, _) => Some(zmodem::receive(proto, &dir, None)),
    }
}
