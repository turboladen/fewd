---
name: verify
description: Run every quality gate this repo enforces — cargo fmt, clippy, cargo test, migration drift, dprint, eslint, tsc, vitest, typos, lockfile freshness, and the API/MCP smoke — in one pass. Use before opening or updating a PR, when asked whether a branch is green, whether CI will pass, or to check/fix formatting and lint across the repo.
---

# Verifying the branch

`.claude/skills/verify/verify.mjs` runs every gate `.github/workflows/ci.yml`
enforces, plus two it doesn't, and reports all of them in one pass rather than
stopping at the first failure. A green run means this branch will pass CI.

Run it from the repo root. A warm full run is about **30 seconds**.

```bash
bun .claude/skills/verify/verify.mjs
```

## Why not `just ci` or `scripts/ci-check.sh`

Both exist and both are incomplete. Neither runs the **migration drift smoke
test** or the **frozen-lockfile check** — and those are the two gates that pass
locally and fail in CI, because they only break on a release build or a fresh
checkout. Use them for a quick pass; use this before a PR.

`just ci` also fails fast, so it shows you one problem at a time.

## Gates

| id          | gate                                                       | tier  | warm  |
| ----------- | ---------------------------------------------------------- | ----- | ----- |
| `fmt`       | `cargo fmt --all -- --check`                               | CI    | 0.1s  |
| `dprint`    | `dprint check`                                             | CI    | 0.1s  |
| `typos`     | `typos --config .typos.toml`                               | CI    | 0.0s  |
| `lint`      | `bun run lint` (eslint)                                    | CI    | 4s    |
| `types`     | `bunx tsc --noEmit`                                        | extra | 1s    |
| `lockfile`  | `bun install --frozen-lockfile`                            | CI    | 0.1s  |
| `fe-test`   | `bun run test` (vitest)                                    | CI    | 3s    |
| `clippy`    | `cargo clippy --all-targets --all-features -- -D warnings` | CI    | 0.4s  |
| `rust-test` | `cargo test --all-features`                                | CI    | 6–16s |
| `smoke`     | `driver.mjs smoke` from the run-fewd-server skill          | extra | 3s    |
| `migration` | `bash scripts/migration-smoke-test.sh`                     | CI    | 1–12s |

**CI** gates block the merge. **extra** gates do not — they cover the two ways
this repo breaks after a green CI run: a type error (CI never runs `tsc` or the
build, so one merges green and breaks `bun run build` at deploy time) and a
runtime break in the API/MCP surface.

## Flags

```bash
bun .claude/skills/verify/verify.mjs --fast      # skip migration drift (the only release build)
bun .claude/skills/verify/verify.mjs --ci-only   # merge-blocking gates only
bun .claude/skills/verify/verify.mjs --fix       # cargo fmt + dprint fmt + eslint --fix, then verify
bun .claude/skills/verify/verify.mjs --only lint # one gate
bun .claude/skills/verify/verify.mjs --list      # gate ids and tiers
```

Output on success is one line per gate plus a total; on failure, the last 25
lines of each failing gate, then a summary naming the failed ids. Exit is 0 or 1.

```
▸ cargo fmt … ok (0.2s)
▸ dprint check … ok (0.1s)
…
▸ migration drift … ok (11.6s)

11 gates in 29.5s
PASS
```

## Gotchas

- **A `git push` or `gh pr create` already triggers `just ci`.** The
  `PreToolUse` hook at `.claude/hooks/ci-before-push.sh` blocks the push if it
  fails, with a **300-second** timeout. It runs the _narrower_ set, so this
  skill is not redundant with it — and a slow cold `cargo clippy` can push that
  hook past its timeout. Bypass one call with `SKIP_CI_HOOK=1 git push`.
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
- **It needs port 3099 free.** It pre-flights and fails in 0.1s with
  `Port 3099 is already bound (PID …)` rather than spending the build first.
  `just db-reset` uses the same port. Set `SMOKE_TEST_PORT` to move it.
- **Formatters exit non-zero when they leave something behind.** In `--fix`,
  `left issues it cannot fix` next to eslint means unused variables or type
  errors remain — the gate run below names them.
- **The Rust gates run from `server/`**, matching CI. That is safe for
  `fmt`/`clippy`/`test` (verified: `cargo test` does not write
  `server/data/fewd.db`), but never `cd server` to _run_ the server — that
  creates a parallel database.

## Troubleshooting

| Symptom                                               | Fix                                                                                                                                        |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `error: lockfile had changes, but lockfile is frozen` | `package.json` moved without `bun.lock`. Run `bun install` and commit the refreshed lockfile.                                              |
| `Port 3099 is already bound`                          | A `just db-reset` or an earlier smoke run is still up. `lsof -nP -iTCP:3099 -sTCP:LISTEN` and kill it, or `SMOKE_TEST_PORT=3199`.          |
| `typos: Executable not found in $PATH`                | `cargo install typos-cli` — the binary is `typos`, the crate is `typos-cli`.                                                               |
| `dprint: Executable not found in $PATH`               | `cargo install dprint`.                                                                                                                    |
| `no gate matches --only <id>`                         | Ids come from `--list`; they are short (`lint`, not `eslint`).                                                                             |
| Gate passes here, CI fails                            | Compare against `.github/workflows/ci.yml`. The gate table above mirrors it; if a step was added there, add it to `GATES` in `verify.mjs`. |
