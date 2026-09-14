---
paths:
  - ".github/**"
  - "justfile"
  - ".claude/hooks/**"
  - ".claude/settings.json"
  - ".claude/skills/verify/**"
  - "scripts/changed-scopes.mjs"
---

# CI and local gates

## Workflows

`.github/workflows/ci.yml` runs the verification jobs on every `pull_request`, whatever its base branch: Rust (`cargo fmt --check`, clippy, `cargo test`, and the migration drift smoke test), TypeScript (`dprint check`, `bun run lint`, `bun run test`), and typos. The changed paths decide which of them run (next section). The push-triggered formatter lives in `auto-format.yml` (every branch except `main`), kept separate so the two event types don't produce duplicate or skipped jobs.

## Path-scoped gates

`scripts/changed-scopes.mjs` maps changed paths to scopes, and verify.mjs, the pre-push hook and ci.yml all select their gates from it. Typos runs for any change.

- `docs`: markdown anywhere, `LICENSE*`, `.git-blame-ignore-revs`, `.beads/`, and JSON under `server/`. It selects dprint.
- `frontend`: `src/`, `public/`, `index.html`, `package.json`, `bun.lock`, and the vite, vitest, eslint and tsconfig files. It selects dprint, the lockfile check, eslint, tsc and vitest.
- `rust`: `server/` except JavaScript and TypeScript files, which dprint, eslint and vitest can see and so match no rule, plus `Cargo.toml` and `Cargo.lock`. It selects dprint, cargo fmt, clippy, cargo test, the API/MCP smoke and the migration drift test. Locally, dprint runs for rust changes so that a `Cargo.toml` change is formatted once dprint covers TOML; in CI, dprint runs only in `Frontend Checks`, which is why JSON under `server/` also takes the docs scope. The drift test catches release-only panics that any Rust change can cause, so it is not limited to migration paths.

A file takes every scope whose rule matches, so a README under `server/` selects docs and rust. A path no rule matches selects every gate. That covers the justfile, `.github/`, `.claude/hooks/`, the skill scripts, `scripts/`, `dprint.jsonc`, `.typos.toml`, `.gitignore` and `deploy/`, and also any non-markdown file under `docs/`, which dprint and eslint still read. A change list that cannot be computed selects every gate too. Before adding a path to a rule, make sure no gate the rule leaves out can see that file; when unsure, leave it unmatched.

`GATE_SCOPES` in the same script maps each verify gate to its scopes, and a gate missing from it runs on any change. ci.yml restates that table in `if:` conditions, so change the two together:

- `Detect Changes` runs the script with `node` against `HEAD^1`, the base tip that a `pull_request` merge commit has as its first parent (`fetch-depth: 2`, `--no-worktree`). It outputs `docs`, `frontend` and `rust` as `true` or `false`. When the diff cannot be computed, the script sets all three true and succeeds; when the script crashes, the job fails and so does `All Checks Passed`.
- `Rust Checks` runs when `rust` is true, including its drift test step.
- `Frontend Checks` runs when `docs` or `frontend` is true. Its bun, lint and test steps need `frontend`, so a docs-only PR runs `dprint check` alone.
- `Typos Check` always runs.

Don't add workflow-level `paths-ignore`: a workflow skipped that way never reports `All Checks Passed`, which leaves that check pending once it is required.

Checks run against the base as it stood when the run started, so a push to the base branch or a retarget refreshes nothing until the PR's next push. Don't add `edited` to the trigger types to cover retargets: title and body edits would then re-run CI and cancel in-progress runs through the `concurrency` group.

The Rust jobs run from the repo root. `cargo fmt --all` checks both packages at once, but clippy and `cargo test` run once per package: `-p fewd-server --all-features`, then `-p migration`. Don't merge them into one `--workspace` run, which is also what a bare `cargo test` or `cargo clippy` from the root does. Selecting both packages unifies the migration crate's dev-dependency features (`sqlx-sqlite` and `runtime-tokio-rustls` on sea-orm-migration) into the server build, so clippy and tests pass code that fails in the release build. Don't swap the `-p` flags for `cd server` either: from `server/`, bare cargo selects only `fewd-server`, so the migration crate's tests never run.

## Runner and toolchain notes

All jobs run on `ubuntu-latest`; CI has no host-arch dependency because the dietpi deploy cross-compiles aarch64. These are easy to break:

- **Rust caching is `Swatinem/rust-cache@v2`** with `workspaces: ". -> target"`. The cargo workspace manifest, `Cargo.lock`, and `target/` all live at the **repo root** (members `server`, `server/migration`). Do NOT "correct" it to `server -> server/target`: that caches an empty directory, a silent no-op.
- **`bun-version` is pinned** (`oven-sh/setup-bun@v2`, currently `1.3.14`); bumping bun means editing that pin.
- **`bun install --frozen-lockfile`** fails CI on a stale `bun.lock`; after a dependency change, run `bun install` and commit the refreshed lockfile.
- **Typos is installed via `taiki-e/install-action@v2` (`tool: typos`)**. The id is `typos`, not the crate name `typos-cli`.
- Workflow-level `permissions: contents: read` and per-job `timeout-minutes` are set; the `auto-format` job keeps job-level `contents: write` (job-level permissions replace workflow-level ones) so its push still works.
- **Job names are not required status checks today** (main has no branch protection); keep them stable anyway so enabling protection later needs no rename. `All Checks Passed` is the one to require: it runs even when a dependency fails, because a job skipped by a failed `needs` counts as passing. Its step holds the rules as data. Every job in `ALWAYS` must succeed, and a job in `GATED` may also be skipped when each of its trigger outputs is exactly `false`. A job in `needs` that is listed in neither fails the check, so add a new job to one of them.

## Running the gates locally

`bun .claude/skills/verify/verify.mjs` (the /verify skill) runs the gates the branch's changes against `origin/main` select, the CI gates plus `tsc` and the API/MCP smoke test, and reports them all in one pass; use it before a PR. `--all` runs every gate regardless of changes. `just ci` is the fail-fast subset of every gate: it omits the migration drift smoke test and `bun install --frozen-lockfile`, the two gates that pass locally and fail in CI. Keep it that way, because the migration gate builds `--release`, which is fast warm but takes minutes against a cold `target/`.

The Claude Code `PreToolUse` hook `.claude/hooks/ci-before-push.sh`, registered in `.claude/settings.json` with a 900s timeout, gates any Bash command containing `git push` or `gh pr create`. It is not a git hook. It validates the tree the command targets: the Bash tool's working directory, moved by any `cd <path>` that runs before the push, so `cd .claude/worktrees/X && git push` checks X. It collects the paths the outgoing commits change, diffed from the upstream when HEAD contains it. A first push, `gh pr create`, and a force push that rewrites pushed commits diff from the merge-base with `origin/main` instead, because a diff from a rewritten upstream cannot see the commits the push drops. Uncommitted and untracked paths count too, because the gate runs on the working tree and a `git commit` earlier in the same command pushes them. Then it picks a gate:

- A push that only deletes remote refs runs nothing.
- The hook runs `scripts/changed-scopes.mjs` inside the target tree. No changed paths runs nothing. Otherwise it runs `verify.mjs --ci-only --fast --skip lockfile` with a `--scope` flag per scope, which with every scope selected is the same set of checks as `just ci`.
- `--all` replaces the scopes for a push whose refspec names another branch, since the diff cannot see that branch's commits, and for a classifier that fails, prints nothing, or prints a line that is not a known scope. The classifier prints `none` for an empty diff and `all` when it cannot diff against the base.
- A tree without `scripts/changed-scopes.mjs`, such as a branch cut before it, runs `just ci`, because its verify.mjs does not accept `--scope`. So does a machine without `bun`. Neither checks for an empty diff first.

A failing gate blocks the command, and so does a missing gate tool such as `just`, `cargo`, `dprint` or `typos`. `SKIP_CI_HOOK=1` bypasses the hook for one command only when it prefixes every `git push` or `gh pr create` in that command.

Limits to keep in mind:

- The hook fails open on its own errors and on timeout: the command runs ungated. Its stderr note reaches the user only in verbose or transcript mode, and the agent never sees it.
- verify.mjs does not stop at the first failure, so a failing run takes longer than `just ci` would. Against a cold `target/` a run can pass the 900s timeout, and the push then goes through ungated.
- A `cd` the hook cannot resolve after expanding a leading `~` blocks the command, because validating any other tree could pass a branch nobody tested. That covers a bare `cd`, `cd -`, flags such as `-P`, variables, command substitution and missing directories. Run the `cd` as its own Bash call, then push. A `cd` inside a pipeline is ignored.
- Two harmless cases also block, because the hook reads them as unresolvable `cd` commands: a heredoc body line whose first word is `cd`, and a `cd` with a redirection such as `cd X 2>/dev/null && git push`.
- Only the tree of the first push is validated. A `cd` after it is ignored, so `cd A && git push && cd B && git push` checks A alone; push from separate trees in separate calls.
- Matching is substring-based and the command split ignores quoting and subshell boundaries. `git -C path push`, aliases, `gh stack submit` and `gh stack sync` are not gated, so run `just ci` or /verify yourself before those. A commit message that quotes `git push` gates its command too, a quoted `&& SKIP_CI_HOOK=1 git push` counts as a bypass, and `(cd X) && git push` validates X.
