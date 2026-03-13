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

1. Run CI checks: `./scripts/ci-check.sh`
2. Build frontend: `bun run build`
3. Build server for target architecture: `bun run build:server:arm64` (or appropriate target)
4. Confirm with user before deploying
5. Transfer binary and restart service

Target: $ARGUMENTS (default: production)
