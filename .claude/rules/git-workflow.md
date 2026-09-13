# Git, PRs, and bead workflow

## Branch and commit naming

Branches: `fewd-<id>/<short-slug>` (e.g. `fewd-abc/mcp-host-allowlist`). The branch name is the only place a bead ID goes.

Commit messages, PR titles, and PR descriptions never contain bead IDs, because a reader of `main` or GitHub cannot look them up. This repo squash-merges with every commit's message in the merge body, so the rule covers each commit on a branch, not just the PR title. Use a conventional-commits prefix with a domain scope — `fix(mcp): ...`, `feat(recipes): ...`, `ci: ...`, `docs: ...` — and describe follow-up work in words ("tracked separately"). PR numbers and commit SHAs are fine to cite. Older commits on `main` carry bead scopes; don't copy that style.

## Dependent PRs

When one PR builds on another, make them a real GitHub stack with the `gh stack` extension (`gh stack init`, `add`, `submit`, `sync`, and `merge`), so each PR shows only its own layer. Don't merge sibling branches into each other or note the dependency in a PR description.

## Bead closure: post-merge, not inside the fix PR

Close a bead AFTER its fix PR merges, on `main`, with `bd close <id>` followed by `bd dolt push`. Because `.beads/issues.jsonl` is untracked here, that produces no commit: there is nothing to stage and nothing to restore. Do NOT flip `status: closed` while the PR is in review; it makes `bd ready` / `bd list` report the fix as shipped when it is still under review.

The Dolt DB under `.beads/` is the source of truth: `bd dolt push` syncs it to `refs/dolt/data`, and a fresh `bd init` bootstraps from there. `.beads/issues.jsonl` is a local export mirror that `export.auto` rewrites on every bead mutation. Tracking it would leave a permanently dirty working tree, which makes `git pull --rebase` refuse, and would put full-snapshot diff noise that reviewers misread into unrelated PRs. The ignore rule lives in the **top-level** `.gitignore`, not `.beads/.gitignore`, because the latter is bd-managed and is overwritten on upgrade. A `.beads/issues.jsonl` diff in a PR means something re-added the file to the index.

## Worktrees

Parallel work runs in git worktrees under `.claude/worktrees/`. A fresh worktree has no `dist/` or `node_modules/`; symlink both from the main checkout before building. Run `bd` commands against the main checkout's `.beads/`.
