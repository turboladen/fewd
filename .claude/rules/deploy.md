---
paths:
  - "deploy/**"
  - "justfile"
  - ".claude/skills/deploy/**"
---

# Deploy and releases

## Deploying to the dietpi box

`just deploy <user>@<host>` is the whole deploy. It depends on `build-arm64` (runs `bun run build`, then `cargo build --release --target aarch64-...`), cross-compiles locally, and copies the binary over ssh. A single binary push carries **frontend changes too**: the frontend is embedded via `rust-embed` (`server/src/main.rs`: `#[folder = "../dist"]`). There is no separate `dist/` sync; if `dist/` is stale, the binary is stale.

The recipe copies `deploy/fewd.service` to **both** `/opt/fewd/` and `/etc/systemd/system/`, then `daemon-reload` and start, so unit-file edits (`RUST_LOG`, `Restart=always` with its start-limit cap, `MCP_ALLOWED_HOSTS`) propagate. Don't hand-roll a partial deploy: omitting the `/etc` copy leaves systemd running the stale unit, and a stale `MCP_ALLOWED_HOSTS` answers every LAN MCP request with 403. The recipe runs `systemctl reset-failed` before it starts the unit, so a deploy recovers a unit that tripped the cap. Outside the recipe, a tripped unit refuses manual starts and restarts until `sudo systemctl reset-failed fewd` runs; `deploy/fewd.service` explains the cap.

## Releases

There is no tag-triggered build workflow. A release is an annotated tag on `main` (`vYYYY-MM-DD`, with a `.N` suffix for same-day hotfixes: `v2026-06-01`, `v2026-06-01.1`) plus a hand-written release via `gh release create <tag> --title <tag> --notes-file <f> --latest`. No binaries are attached. Tag after the work is merged and synced with origin.
