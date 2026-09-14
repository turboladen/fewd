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

The Rust jobs run from the repo root. `cargo fmt --all` checks both packages at once, but clippy and `cargo test` run once per package: `-p fewd-server --all-features`, then `-p migration`. Don't merge them into one `--workspace` run, which is also what a bare `cargo test` or `cargo clippy` from the root does. Selecting both packages unifies the migration crate's dev-dependency features (`sqlx-sqlite` and `runtime-tokio-rustls` on sea-orm-migration) into the server build, so clippy and tests pass code that fails in the release build. Don't swap the `-p` flags for `cd server` either: from `server/`, bare cargo selects only `fewd-server`, so the migration crate's tests never run.

## Runner and toolchain notes

All jobs run on `ubuntu-latest`; CI has no host-arch dependency because the dietpi deploy cross-compiles aarch64. These are easy to break:

- **Rust caching is `Swatinem/rust-cache@v2`** with `workspaces: ". -> target"`. The cargo workspace manifest, `Cargo.lock`, and `target/` all live at the **repo root** (members `server`, `server/migration`). Do NOT "correct" it to `server -> server/target`: that caches an empty directory, a silent no-op.
- **`bun-version` is pinned** (`oven-sh/setup-bun@v2`, currently `1.3.14`); bumping bun means editing that pin.
- **`bun install --frozen-lockfile`** fails CI on a stale `bun.lock`; after a dependency change, run `bun install` and commit the refreshed lockfile.
- **Typos is installed via `taiki-e/install-action@v2` (`tool: typos`)**. The id is `typos`, not the crate name `typos-cli`.
- Workflow-level `permissions: contents: read` and per-job `timeout-minutes` are set; the `auto-format` job keeps job-level `contents: write` (job-level permissions replace workflow-level ones) so its push still works.
- **Job names are not required status checks today** (main has no branch protection); keep them stable anyway so enabling protection later needs no rename. `All Checks Passed` is the one to require: it runs even when a dependency fails, because a job skipped by a failed `needs` counts as passing.

## Running the gates locally

`bun .claude/skills/verify/verify.mjs` (the /verify skill) runs every CI gate plus `tsc` and the API/MCP smoke test in one pass; use it before a PR. `just ci` is the fail-fast subset: it omits the migration drift smoke test and `bun install --frozen-lockfile`, the two gates that pass locally and fail in CI. Keep it that way, because the migration gate builds `--release`, which is fast warm but takes minutes against a cold `target/`.

The Claude Code `PreToolUse` hook `.claude/hooks/ci-before-push.sh`, registered in `.claude/settings.json` with a 900s timeout, gates any Bash command containing `git push` or `gh pr create`. It is not a git hook. It validates the tree the command targets: the Bash tool's working directory, moved by any `cd <path>` that runs before the push, so `cd .claude/worktrees/X && git push` checks X. It collects the paths the outgoing commits change, diffed from their merge-base with the upstream, or with `origin/main` for a first push and for `gh pr create`. Uncommitted and untracked paths count too, because the gate runs on the working tree and a `git commit` earlier in the same command pushes them. Then it picks a gate:

- No changed paths, or a push that only deletes remote refs, runs nothing.
- Changes only to markdown, `LICENSE`, `.git-blame-ignore-revs` or `.beads/` run `dprint check` and `typos`, because dprint formats markdown and typos scans every file.
- Anything else runs `just ci`. That includes a `.gitignore` change, which alters what eslint sees, and a push whose refspec names any branch other than the current one, since the diff cannot see that branch's commits.

A failing gate blocks the command, and so does a missing `just`, `dprint` or `typos`. `SKIP_CI_HOOK=1` bypasses the hook for one command only when it prefixes every `git push` or `gh pr create` in that command.

Limits to keep in mind:

- The hook fails open on its own errors and on timeout: the command runs ungated. Its stderr note reaches the user only in verbose or transcript mode, and the agent never sees it.
- A `cd` the hook cannot resolve after expanding a leading `~` blocks the command, because validating any other tree could pass a branch nobody tested. That covers a bare `cd`, `cd -`, flags such as `-P`, variables, command substitution and missing directories. Run the `cd` as its own Bash call, then push. A `cd` inside a pipeline is ignored.
- Two harmless cases also block, because the hook reads them as unresolvable `cd` commands: a heredoc body line whose first word is `cd`, and a `cd` with a redirection such as `cd X 2>/dev/null && git push`.
- Only the tree of the first push is validated. A `cd` after it is ignored, so `cd A && git push && cd B && git push` checks A alone; push from separate trees in separate calls.
- Matching is substring-based and the command split ignores quoting and subshell boundaries. `git -C path push`, aliases, `gh stack submit` and `gh stack sync` are not gated, so run `just ci` or /verify yourself before those. A commit message that quotes `git push` gates its command too, a quoted `&& SKIP_CI_HOOK=1 git push` counts as a bypass, and `(cd X) && git push` validates X.
