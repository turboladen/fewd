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
4. Confirm the service came up: `ssh <user>@<host> "systemctl status fewd"`
5. Regenerate the migration baseline snapshot, following
   `server/tests/fixtures/schema-snapshots/README.md`

`just deploy` is the entire build-and-ship step. It depends on `build-arm64`,
which runs `bun run build` and then cross-compiles the release binary for
`aarch64-unknown-linux-gnu`; the frontend is embedded in that binary via
`rust-embed`, so one binary push carries frontend changes too. The recipe also
copies `deploy/fewd.service` to both `/opt/fewd/` and `/etc/systemd/system/`
before `daemon-reload` and restart, which is how unit-file edits reach the
running service. Run the recipe as a whole; a hand-rolled subset that skips the
`/etc` copy leaves systemd loading a stale unit (the `fewd-82e` 403 regression).

The recipe prints its success line as soon as `systemctl start` returns, so step
4 is what tells you the server bound its port instead of crash-looping.

A host that has never run fewd needs `just setup-remote <user>@<host>` first: it
creates `/opt/fewd`, stages the unit file there, and runs
`deploy/setup-remote.sh` to add the `fewd` service user and enable the unit.

Target: $ARGUMENTS (default: production)
