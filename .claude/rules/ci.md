---
paths:
  - ".github/**"
  - "justfile"
  - ".claude/hooks/**"
  - ".claude/settings.json"
  - ".claude/skills/verify/**"
---

# CI and local gates

## Workflows

`.github/workflows/ci.yml` runs the verification jobs on every `pull_request`, whatever its base branch: Rust (`cargo fmt --check`, clippy, `cargo test`, and the migration drift smoke test), TypeScript (`dprint check`, `bun run lint`, `bun run test`), and typos. The push-triggered formatter lives in `auto-format.yml` (every branch except `main`), kept separate so the two event types don't produce duplicate or skipped jobs.

Checks run against the base as it stood when the run started, so a push to the base branch or a retarget refreshes nothing until the PR's next push. Don't add `edited` to the trigger types to cover retargets: title and body edits would then re-run CI and cancel in-progress runs through the `concurrency` group.

The Rust jobs run `cd server && cargo test --all-features` and `cd server && cargo clippy --all-targets --all-features -- -D warnings`. From a member directory, `cargo test` tests only the `fewd-server` package, so the migration crate's own tests do not run in CI.

## Runner and toolchain notes

All jobs run on `ubuntu-latest`; CI has no host-arch dependency because the dietpi deploy cross-compiles aarch64. These are easy to break:

- **Rust caching is `Swatinem/rust-cache@v2`** with `workspaces: ". -> target"`. The cargo workspace manifest, `Cargo.lock`, and `target/` all live at the **repo root** (members `server`, `server/migration`), even though later steps `cd server`. Do NOT "correct" it to `server -> server/target`: that caches an empty directory, a silent no-op.
- **`bun-version` is pinned** (`oven-sh/setup-bun@v2`, currently `1.3.14`); bumping bun means editing that pin.
- **`bun install --frozen-lockfile`** fails CI on a stale `bun.lock`; after a dependency change, run `bun install` and commit the refreshed lockfile.
- **Typos is installed via `taiki-e/install-action@v2` (`tool: typos`)**. The id is `typos`, not the crate name `typos-cli`.
- Workflow-level `permissions: contents: read` and per-job `timeout-minutes` are set; the `auto-format` job keeps job-level `contents: write` (job-level permissions replace workflow-level ones) so its push still works.
- **Job names are not required status checks today** (main has no branch protection); keep them stable anyway so enabling protection later needs no rename. `All Checks Passed` is the one to require: it runs even when a dependency fails, because a job skipped by a failed `needs` counts as passing.

## Running the gates locally

`bun .claude/skills/verify/verify.mjs` (the /verify skill) runs every CI gate plus `tsc` and the API/MCP smoke test in one pass; use it before a PR. `just ci` is the fail-fast subset: it omits the migration drift smoke test and `bun install --frozen-lockfile`, the two gates that pass locally and fail in CI. Keep it that way, because the migration gate builds `--release`, which is fast warm but takes minutes against a cold `target/`.

`just ci` is what the Claude Code `PreToolUse` hook (`.claude/hooks/ci-before-push.sh`, registered in `.claude/settings.json`) runs before any command containing `git push` or `gh pr create`, with a 300s timeout. It is not a git hook, and it does not match `gh stack submit` or `gh stack sync`, which push branches too, so run `just ci` or /verify yourself before those. It runs in the session's current working directory, so push from the worktree whose branch you are pushing; `SKIP_CI_HOOK=1` bypasses it for one command.
