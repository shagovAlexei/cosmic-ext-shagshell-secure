# 5a — SFTP tab: Implementation Plan

**Spec:** `.claude/docs/specs/05-sftp.md`. Branch `feat/sftp` (on `feat/credentials`), version 0.7.0.

## Global Constraints

- Remote paths and names are bytes (`PathBuf` / `OsString` built from bytes): never `to_str().unwrap()`; shown with `to_string_lossy`.
- No network on the UI thread: one worker thread per SFTP tab (commands in, events out).
- Every UI string `fl!` en + ru.

## Task 1: `core::sftp::proto` — SFTP v3 client (TDD against `/usr/lib/openssh/sftp-server`)

- `Client<R: Read, W: Write>::new(r, w) -> io::Result<Self>` (INIT 3 / VERSION).
- `realpath(&Path) -> PathBuf`, `read_dir(&Path) -> Vec<RemoteEntry { name: OsString, attrs: Attrs }>` (`.` / `..` dropped), `stat` / `lstat -> Attrs { size, perm, mtime }` (`is_dir`), `mkdir`, `rmdir`, `remove`, `rename`.
- `download(&Path, &mut impl Write, progress: impl FnMut(u64) -> bool) -> io::Result<u64>` and `upload(&mut impl Read, &Path, progress) -> io::Result<u64>`: 64 KiB chunks, up to 16 requests in flight, out-of-order answers, short reads re-requested; `progress` false = cancel (`ErrorKind::Interrupted`).
- Status → `io::Error` (NoSuchFile → NotFound, PermissionDenied → PermissionDenied, else Other with the server's message).
- Tests: `list_non_utf8_names`, `stat_dir_and_file`, `mkdir_rename_remove`, `round_trip_5mb` (bytes equal), `download_cancel`, `missing_is_not_found`, `realpath_dot`.

## Task 2: `core::sftp::queue` — pure state machine

- `Job { id, dir: Up|Down, src, dst, size, done, state: Waiting|Running|Done|Failed(String)|Cancelled|Skipped, is_move, batch }`.
- `Queue::add(batch, jobs) / next() / progress(id, n) / finish(id, Result) / cancel(id) / retry(id) / clear_done()`; per-batch conflict answer `answer(batch, Choice, for_all)` / `choice(batch)`.
- Tests: order, retry resets, cancel waiting vs running, clear keeps active, for-all per batch.

## Task 3: `core::panel` + `core::format` (after shagoff-commander, cut down)

- `Entry { name, dir, size, mtime }`; `Panel { cwd, entries, cursor, marked }`: `set_listing(cwd, entries, focus)` (dirs first, by name, `..` unless `/`), move / home / end / page, `toggle_mark`, `mark_all`, `targets()` (marked, else cursor; never `..`), `current()`.
- `format::size(u64)`, `format::time(i64, now)`.
- Tests: sort and `..`, focus after `cd ..`, targets.

## Task 4: app — the SFTP tab

- `launch::sftp_argv` (the session's ssh options, `-s -- host sftp`), tested.
- `sftp_view.rs`: `SftpView { panels: [Panel; 2], active, queue, conflict dialog, status }`, `SftpMsg`; worker thread (`sftp_worker`): `Cmd::{List, Mkdir, Rename, Delete, Transfer(batch), Answer, Cancel}` → `Event::{Listed, Done, Progress, Conflict, Failed, Disconnected}`.
- `tabs`: a tab is `Body::Terminal | Body::Sftp`; status bar, close, groups shared; keys for the active SFTP tab routed by the app.
- Entry points: tab menu «Connect SFTP Tab», toolbar SFTP, Transfer menu, Alt+P, tree menu «Открыть SFTP».

## Review Focus

- A non-UTF-8 name: listed, entered, downloaded, renamed.
- The connection dies mid-transfer: the job fails, the panel says why, Reconnect works.
- Cancel mid-file: partial target removed, the next job runs.
- Copying a folder onto an existing one: per-file conflicts, «for all» honoured.
- F6 of a file that failed to copy: the source stays.
