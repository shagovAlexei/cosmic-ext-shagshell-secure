---
name: shagshell-feature
description: Use when starting any new feature, roadmap phase, or non-trivial change in ShagShell Secure — runs the project cycle brainstorm → spec → plan → branch → TDD → verify → review → PR, with docs under .claude/docs.
---

# Feature cycle for ShagShell Secure

Every feature and every roadmap phase goes through these steps in order. Do not skip a step. Each step's gate is the user's approval.

1. **Brainstorm.** Invoke `superpowers:brainstorming`. Before asking anything, read `CLAUDE.md`, `.claude/docs/ROADMAP.md` and the matching section of `.claude/docs/securecrt-reference.md`. If something should behave "like SecureCRT", that file is the source of truth; update it when the feature teaches us more.
2. **Spec.** Write it to `.claude/docs/specs/NN-<topic>.md` (NN = roadmap phase number for phases, else the next free number; no dates in names). This overrides the brainstorming default `docs/superpowers/specs`. The user reviews the spec.
3. **Plan.** Invoke `superpowers:writing-plans`. Save to `.claude/docs/plans/NN-<topic>.md` (same NN as the spec).
4. **Branch.**
   ```sh
   git switch main && git pull
   git switch -c feat/<topic>     # fix/<topic> for bugs, docs/<topic> for docs only
   ```
   Never commit to `main` directly.
5. **Implement.**
   - Logic goes into `crates/core`, test-first with `superpowers:test-driven-development`.
   - `crates/app` stays thin and split by feature module (own `Message` / `update` / `view`); `app.rs` only routes.
   - Every new UI string in `fl!` with both `en` and `ru`.
   - Secrets never touch files or logs.
6. **Verify.** `just verify` must pass. Then look at the real UI with `.claude/skills/shagshell-feature/headless.sh` (adapted from shagoff-commander's in phase 1: invisible sway + `wtype` + `grim`; never the user's desktop or config): `start`, keys, `shot name`, then Read the png. Connection features are checked against a local `sshd` on `localhost` (password via askpass, SFTP put/get). Add the manual check to `TESTING.md`.
7. **Review.** Run `/code-review` and fix what it confirms.
8. **PR.**
   - Tick the phase / feature in `ROADMAP.md`; if the architecture changed, update `CLAUDE.md` and `specs/00-architecture.md`.
   - Bump `version` in the root `Cargo.toml`: feature → minor, fix → patch.
   - ```sh
     git push -u origin feat/<topic>
     gh pr create --fill        # body links the spec and the plan
     gh pr checks --watch
     ```
   - Merge only after the user says so: `gh pr merge --merge --delete-branch`.
   - Tag the bumped version on `main` and push the tag (`vX.Y.Z`): the release workflow attaches the .deb.

Bug fix: use `superpowers:systematic-debugging`; add a `regression_<what>` test in core and a row in `TESTING.md`; branch `fix/<topic>`.
