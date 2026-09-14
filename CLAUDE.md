# CLAUDE.md

Family meal planner and cocktail manager: a Rust/Axum/SeaORM/SQLite server in `server/` and a React/TypeScript/Vite app in `src/`, shipped as one binary with the frontend embedded. `REQUIREMENTS.md` holds the specification.

Topic rules live in `.claude/rules/`. A rule without a `paths:` list loads every session; the rest load when you work on matching files (server, frontend, migrations, MCP, CI, deploy).

## Essentials

- Use `bun`/`bunx`, never `npm`/`npx`.
- Run the app with `just dev` (server on :3000, Vite on :5173, API proxied). It runs the server from the workspace root, so the dev database is `data/fewd.db`.
- Before opening a PR, run `bun .claude/skills/verify/verify.mjs` (the /verify skill): the CI gates your changes select, in one pass. `--all` runs every gate.
- A Claude Code hook gates any Bash command containing `git push` or `gh pr create` and blocks it on failure. It checks the tree the command targets, so `cd .claude/worktrees/X && git push` checks X. The paths the outgoing commits and the working tree change select its gates through `scripts/changed-scopes.mjs`, the classifier verify.mjs and CI use too, and it runs nothing when there are none. `gh stack submit` and `gh stack sync` push without it, so run `just ci` or /verify before those.
- Track all work in beads (below). Project rules belong in `.claude/rules/`, not inside the beads block, which `bd` may regenerate.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd dolt push
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->
