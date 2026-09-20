---
name: verify
description: Run the quality gates this repo enforces — cargo fmt, clippy, cargo test, migration drift, dprint, eslint, tsc, vitest, typos, lockfile freshness, and the API/MCP smoke — in one pass. By default it runs only the gates the branch's changes against origin/main select, and none when nothing changed; pass --all for every gate. Use before opening or updating a PR, when asked whether a branch is green, whether CI will pass, or to check/fix formatting and lint across the repo.
---

# Verifying the branch

`.claude/skills/verify/verify.mjs` runs the gates `.github/workflows/ci.yml`
enforces, plus two it doesn't, and reports all of them in one pass rather than
stopping at the first failure. By default it runs only the gates the branch's
changes select, the same selection CI makes, so a green run still means this
branch will pass CI.

Run it from the repo root. A warm full run is about **30 seconds**; a docs-only
change runs `dprint` and `typos` in well under one.

```bash
bun .claude/skills/verify/verify.mjs
```

It prints which files selected which scopes before the gates run:

```
scopes vs origin/main:
  docs      README.md, docs/mcp-testing.md
  rust      server/src/main.rs
```

## Scopes

`scripts/changed-scopes.mjs` lists the files changed since the merge base with
`origin/main`, plus staged, unstaged and untracked files, and maps them to
scopes. `.claude/rules/ci.md` has the full path rules.

| scope      | paths                                                      | gates                                            |
| ---------- | ---------------------------------------------------------- | ------------------------------------------------ |
| `docs`     | markdown, `LICENSE*`, `.beads/`, JSON under `server/`      | `dprint`                                         |
| `frontend` | `src/`, `public/`, `package.json`, `bun.lock`, the configs | `dprint`, `lockfile`, `lint`, `types`, `fe-test` |
| `rust`     | `server/` except JS/TS, `Cargo.toml`, `Cargo.lock`         | `dprint`, the cargo gates, `smoke`, `migration`  |
| `all`      | any path no rule matches, such as `justfile` or `scripts/` | every gate                                       |

`typos` runs for any change. With no changes at all, verify prints
`no changes vs origin/main; use --all to run every gate` and exits 0. If the
change list cannot be computed, for example without an `origin/main` ref,
every gate runs.

## Why not `just ci`

`just ci` is the fast pre-push gate (14s warm), and it is incomplete on
purpose. It runs neither the **migration drift smoke test** nor the
**frozen-lockfile check** — the two gates that pass locally and fail in CI,
because one needs a release build and the other a fresh checkout.

Folding them in is tempting, since warm they cost about a second. Don't: the
migration gate builds `--release`, which runs to minutes against a cold or
stale `target/`, and `.claude/hooks/ci-before-push.sh` lets the push through
ungated when it runs past its 900s timeout. Every push would be a gamble on the
state of `target/`.

`just ci` also fails fast, so it shows one problem at a time. Use it while
working; use this before a PR.

## Gates

| id           | gate                                                                      | tier  | warm  |
| ------------ | ------------------------------------------------------------------------- | ----- | ----- |
| `fmt`        | `cargo fmt --all -- --check`                                              | CI    | 0.1s  |
| `dprint`     | `dprint check`                                                            | CI    | 0.1s  |
| `typos`      | `typos --config .typos.toml`                                              | CI    | 0.0s  |
| `lockfile`   | `bun install --frozen-lockfile`                                           | CI    | 0.1s  |
| `lint`       | `bun run lint` (eslint)                                                   | CI    | 4s    |
| `types`      | `bunx tsc --noEmit`                                                       | extra | 1s    |
| `fe-test`    | `bun run test` (vitest)                                                   | CI    | 3s    |
| `clippy`     | `cargo clippy -p fewd-server --all-targets --all-features -- -D warnings` | CI    | 0.4s  |
| `mig-clippy` | `cargo clippy -p migration --all-targets -- -D warnings`                  | CI    | 0.3s  |
| `rust-test`  | `cargo test -p fewd-server --all-features`                                | CI    | 6–16s |
| `mig-test`   | `cargo test -p migration`                                                 | CI    | 0.3s  |
| `smoke`      | `driver.mjs smoke` from the run-fewd-server skill                         | extra | 3s    |
| `migration`  | `bash scripts/migration-smoke-test.sh`                                    | CI    | 1–12s |

**CI** gates block the merge. **extra** gates do not — they cover the two ways
this repo breaks after a green CI run: a type error (CI never runs `tsc` or the
build, so one merges green and breaks `bun run build` at deploy time) and a
runtime break in the API/MCP surface.

## Flags

```bash
bun .claude/skills/verify/verify.mjs --all          # every gate, whatever changed
bun .claude/skills/verify/verify.mjs --scope rust   # the gates of a scope (repeatable)
bun .claude/skills/verify/verify.mjs --base HEAD~3  # select from changes since another revision
bun .claude/skills/verify/verify.mjs --fast         # skip migration drift (the only release build)
bun .claude/skills/verify/verify.mjs --ci-only      # merge-blocking gates only
bun .claude/skills/verify/verify.mjs --skip smoke   # leave out a gate (repeatable)
bun .claude/skills/verify/verify.mjs --fix          # cargo fmt + dprint fmt + eslint --fix, then verify
                                                    # with --only, just that gate's fixer
bun .claude/skills/verify/verify.mjs --only lint    # one gate
bun .claude/skills/verify/verify.mjs --list         # gate ids, tiers and scopes
```

`--only`, `--all` and `--scope` each choose the gates, so pass at most one.

Output on success is one line per gate plus a total; on failure, the last 25
lines of each failing gate, then a summary naming the failed ids. Exit is 0 or 1.

```
▸ cargo fmt … ok (0.2s)
▸ dprint check … ok (0.1s)
…
▸ migration drift … ok (11.6s)

13 gates in 29.5s
PASS
```

## Gotchas

- **A `git push` or `gh pr create` already triggers a gate.** The
  `PreToolUse` hook at `.claude/hooks/ci-before-push.sh` runs this script in
  the tree the command targets, as `--ci-only --fast --skip lockfile` with the
  scopes of the outgoing commits and the working tree, and blocks the push if
  it fails. That is the _narrower_ set, so a run of this skill is not redundant
  with it. A tree without `scripts/changed-scopes.mjs` gets `just ci` instead.
  Its timeout is **900 seconds**, and a timeout lets the push through ungated,
  which a slow cold `cargo clippy` can cause. Bypass one call with
  `SKIP_CI_HOOK=1 git push`.
- **`cargo test` passing does not mean clippy passes.** Dead code is a warning
  to the test build and an error under clippy's `-D warnings`. Verified:
  an unused function leaves `rust-test` green and fails `clippy` with
  `error: function … is never used`.
- **A missing `dist/` breaks the Rust build with misleading errors.**
  RustEmbed's `#[folder = "../dist"]` is checked at macro expansion, and the
  real message — `#[derive(RustEmbed)] folder '…/dist' does not exist` — is
  followed by three bogus `no associated function or constant named 'get'
  found for struct 'Assets'` errors. Read the first error, not the loudest.
  This harness creates the placeholder before the Rust gates, exactly as CI
  does; `bun run build` replaces it with a real bundle.
- **`bun run test` and `bun test` are different runners.** The npm script runs
  vitest with the jsdom environment. Bare `bun test` uses Bun's built-in runner
  with no jsdom: it reports `325 fail` out of 422 with `ReferenceError:
  document is not defined`, and still exits 0. CI runs the npm script.
- **The migration gate is the only one that builds `--release`.** Warm it is
  about a second; after a server-side code change expect ~35s while the
  release binary rebuilds, and minutes from a cold `target/`.
- **The `migration` gate needs port 3099 free.** It pre-flights and fails in
  0.1s with `Port 3099 is already bound (PID …)` rather than spending the build
  first. Set `SMOKE_TEST_PORT` to move it. The `smoke` gate binds `PORT=0`, so
  it needs no free port of its own.
- **Formatters exit non-zero when they leave something behind.** In `--fix`,
  `left issues it cannot fix` next to eslint means unused variables or type
  errors remain — the gate run below names them.
- **The Rust gates run from the repo root, and clippy and tests run one
  package at a time**, matching CI. A single clippy or test run over both
  packages unifies the migration crate's dev-dependency features into the
  server build and passes code the release build rejects. Never `cd server` to
  _run_ the server: the database path resolves against the working directory,
  so that creates a parallel database.

## Troubleshooting

| Symptom                                               | Fix                                                                                                                                                    |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `error: lockfile had changes, but lockfile is frozen` | `package.json` moved without `bun.lock`. Run `bun install` and commit the refreshed lockfile.                                                          |
| `Port 3099 is already bound`                          | An earlier smoke run is still up. `lsof -nP -iTCP:3099 -sTCP:LISTEN` and kill it, or `SMOKE_TEST_PORT=3199`.                                           |
| `typos: Executable not found in $PATH`                | `cargo install typos-cli` — the binary is `typos`, the crate is `typos-cli`.                                                                           |
| `dprint: Executable not found in $PATH`               | `cargo install dprint`.                                                                                                                                |
| `no gate matches --only <id>`                         | Ids come from `--list`; they are short (`lint`, not `eslint`).                                                                                         |
| `no changes vs origin/main` on a branch with work     | The work is already on `origin/main`, or the ref is stale. Run `git fetch`, or pass `--all`.                                                           |
| A gate you expected did not run                       | Check the `scopes vs origin/main` lines. A path missing from a rule in `scripts/changed-scopes.mjs` runs everything, never less.                       |
| Gate passes here, CI fails                            | Compare against `.github/workflows/ci.yml`. The gate table above mirrors it; if a check was added there, add a gate for it to `GATES` in `verify.mjs`. |
