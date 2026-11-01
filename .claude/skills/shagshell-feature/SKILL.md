---
name: shagshell-feature
description: Use when starting any new feature, roadmap item, or bug fix in ShagShell Secure — the project cycle talk → (spec) → branch → test-first → verify → review → PR → tag.
---

# Feature cycle for ShagShell Secure

1. **Talk it through.** Read the ROADMAP item (`grep` it, not the whole file), the module's section in `.claude/docs/modules.md` and, if it should behave "like SecureCRT", the matching part of `.claude/docs/securecrt-reference.md` (update it when you learn more). Ask only what changes what gets built, one question at a time; otherwise pick a sensible default and say it. Small change: describe it in chat and go. Big feature: write `.claude/docs/specs/NN-<topic>.md` (next free NN, no dates) and get the user's OK before code. No separate plan file unless the user asks.
2. **Branch.** `git switch main && git pull && git switch -c feat/<topic>` (`fix/` for bugs, `docs/` for docs). Never commit to `main`.
3. **Implement, test first.** Logic in `crates/core`: write the test, see it fail, make it pass. `crates/app` stays thin, split by feature module; `app.rs` only routes. Every UI string in `fl!` with `en` and `ru`. Secrets never in files or logs.
   Bug: find the root cause first; add a `regression_<what>` test in core.
4. **Verify.** `just verify > $SCRATCH/verify.log 2>&1; tail -30 $SCRATCH/verify.log`. For UI changes look at the real UI with `.claude/skills/shagshell-feature/headless.sh` (invisible sway + `wtype` + `grim`; never the user's desktop or config): `start`, keys, `shot name`, Read the png. Clicks on header menu items never arrive there: use the item's key or leave it to the manual check. Connections: local `sshd` (headless.sh) on 127.0.0.1:2222.
   Add the manual check to `TESTING.md` as `- [ ]` under the phase, in Russian: bold name + version, numbered steps (preparation, exact menu paths from `i18n/ru`, what to click / press, a test `.py` written out in full if needed), a final «Должно быть:» line. Shared setup is in «Подготовка».
5. **Review.** `/code-review low` (medium for big features); fix what it confirms.
6. **PR.** Tick the item in `ROADMAP.md`; update `CLAUDE.md` / `modules.md` if the architecture changed. Bump `version` in the root `Cargo.toml` (feature → minor, fix → patch), add the `CHANGELOG.md` entry, run `just verify` so `Cargo.lock` follows.
   ```sh
   git push -u origin <branch>
   gh pr create --fill
   gh pr checks --watch > $SCRATCH/checks.log 2>&1; tail -5 $SCRATCH/checks.log
   gh pr merge --merge --delete-branch
   git switch main && git pull   # tag only when the user asks for a release (see memory no-tags-until-asked)
   ```
   Tag + `just sign` only when the user asks for a release; suggest `/clear` after a finished task.
