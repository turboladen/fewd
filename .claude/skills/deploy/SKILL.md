---
name: deploy
description: Build and deploy the application
disable-model-invocation: true
---

## Current State

- Branch: !`git branch --show-current`
- Status: !`git status --short`
- Last tag: !`git describe --tags --abbrev=0 2>/dev/null || echo 'no tags'`

## Deploy Steps

1. Run every gate: `bun .claude/skills/verify/verify.mjs` (the `/verify` skill)
2. Confirm the target host with the user — a deploy restarts a live service
3. Deploy: `just deploy <user>@<host>`
4. Confirm the new build is serving: `curl -fsS http://<host>:3000/api/version`
   — `git_sha` in the response should match the commit you deployed
5. Wait 30s, then confirm it is not crash-looping:
   `ssh <user>@<host> systemctl show -p NRestarts fewd` should print `NRestarts=0`
6. Regenerate the migration baseline snapshot, following
   `server/tests/fixtures/schema-snapshots/README.md`

`just deploy` is the entire build-and-ship step. It depends on `build-arm64`,
which runs `bun run build` and then cross-compiles the release binary for
`aarch64-unknown-linux-gnu`; the frontend is embedded in that binary via
`rust-embed`, so one binary push carries frontend changes too. The recipe also
copies `deploy/fewd.service` to both `/opt/fewd/` and `/etc/systemd/system/`
before `daemon-reload` and start, which is how unit-file edits reach the
running service. Run the recipe as a whole; a hand-rolled subset that skips the
`/etc` copy leaves systemd loading a stale unit (the `fewd-82e` 403 regression).

The recipe prints its success line as soon as `systemctl start` returns, and the
unit restarts a crashing server until its start-limit cap trips (see
`deploy/fewd.service`), so neither that line nor a momentary `active (running)`
proves anything. Step 4 proves the deployed SHA is answering. Step 5 catches a
server that crashes seconds after binding, because `NRestarts` counts automatic
restarts and the recipe's start sets it to 0. A tripped unit shows `failed` with
`Result: start-limit-hit` and refuses manual starts until
`sudo systemctl reset-failed fewd` runs; re-running `just deploy` also clears it.

A host that has never run fewd needs `just setup-remote <user>@<host>` first: it
creates `/opt/fewd`, stages the unit file there, and runs
`deploy/setup-remote.sh` to add the `fewd` service user and enable the unit.

Target: $ARGUMENTS (default: production)
