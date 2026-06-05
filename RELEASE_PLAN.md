# Release Plan — "Foundations & the Kitchen"

A UI/UX + tech-debt catch-up release, following two MCP-focused releases. Theme: **make the
core cook-a-recipe loop reliable, then pay down the highest-leverage debt under it.**

> Status: planned 2026-06-04. Phases execute in order; Phase 1 (tech debt) starts first by
> request. Each bead is one scoped PR; the bead is closed in a separate post-merge
> `chore(beads): close <id> after PR #<N> merge` commit on `main` (project convention).

## Conventions that apply to this release

- **Scoped PRs.** One bead per PR. Don't bundle bead state changes (claim/close) into the fix PR.
- **Migrations are frozen-in-time.** Never edit a shipped migration; add a new
  `m<DATE>_<NNN>_<desc>.rs`. Use raw `PRAGMA`/`Statement` SQL for introspection, not
  `SchemaManager` helpers (they panic without the `sqlx-sqlite` feature in release builds).
- **Schema-changing beads gate on a release build + smoke test.** Run
  `cargo build --release` and `just smoke-test` before pushing — `cargo test` (dev mode)
  can hide feature-flag mismatches.
- **Deploy carries the frontend.** `just deploy` rebuilds + embeds `dist/` via `rust-embed`;
  there's no separate `dist/` sync.

---

## Phase 1 — Tech debt (start here)

| # | Bead                                                                              | Order rationale                                                                                                                                                                                        | Gate                                                          |
| - | --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------- |
| 1 | **fewd-4rg** — apply SQLite pragmas to every pooled connection                    | Smallest, fully independent, `db.rs`-only, no migration. Clean warm-up; pure correctness (`busy_timeout` is per-connection and ephemeral; non-pragma'd pooled connections fail-fast on `SQLITE_BUSY`). | `cargo test`                                                  |
| 2 | **fewd-lb2** — MCP-write → planner-read round-trip test                           | Establishes the cross-boundary safety net **before** the enum refactor. Lands green on today's canonical behavior; must stay green through #3.                                                         | `cargo test`                                                  |
| 3 | **fewd-2pf** — promote `meal_type` to a Rust enum (DTO + entity + API + MCP + TS) | The big refactor, now protected by #2. Deletes `canonical_meal_type`; includes a migration normalizing legacy non-canonical rows. **Blocked-by fewd-lb2 in beads.**                                    | ⚠️ schema change → `cargo build --release` + `just smoke-test` |
| 4 | **fewd-2fc** — `total_minutes` column for clean time filtering                    | Independent migration; fixes the `unit='hours'` silent-miss in `search_recipes`. Sequenced after #3 so only one migration is in flight at a time.                                                      | ⚠️ schema change → release build + smoke-test                  |

**Why lb2 before 2pf:** fewd-2pf rewrites how `meal_type` crosses every boundary. That is
exactly the kind of refactor that can silently re-break the "lowercase `dinner` is invisible
to the planner" invariant (the original bug, commit `db20f56`). The round-trip test
characterizes the contract as an executable spec; the enum refactor then proves it preserved
it. This is the only **hard** dependency in Phase 1 — fewd-4rg and fewd-2fc are genuinely
independent and left ungated so `bd ready` stays truthful; their position here is a
recommended execution order, not a blocker.

## Phase 2 — Kitchen reliability

| # | Bead                                                                                  | Note                                                                                                                                                                                                                                              |
| - | ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 5 | **fewd-0vp** — display app version in UI                                              | Tiny, but do it first in this phase: it's the 2-second diagnostic for whether fewd-6kq is just deploy lag.                                                                                                                                        |
| 6 | **fewd-6kq** — enhanced-view offline resilience _(scoped: confirm + regression test)_ | Step 1: confirm prod matches `main` (deploy + check the version chip from #5). Then add a regression test that a failed/timed-out enhance call never hides the recipe. **Full offline caching of enhanced text is deferred out of this release.** |
| 7 | **fewd-e4z** — print / offline recipe view                                            | `window.print()` + the first-ever `@media print` block in `index.css`. Direct fix for the 2026-05-25 carry-the-laptop incident. Independent of #6.                                                                                                |
| 8 | **fewd-awo** — cooking-mode check-off + current-step highlight                        | Builds on the shipped `CookingView`. Tap-to-complete steps, ingredient check-off, current-step tint, localStorage progress. Maps to the `fewd-cook-mode-ui-principles` memory. Pure frontend.                                                     |

## Phase 3 — Planner polish

| #  | Bead                                                                | Note                                                                                                                                  |
| -- | ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| 9  | **fewd-lyf** — default new serving to the slot's last-picked recipe | Cheap, low-risk; kills the "re-pick the same recipe per person" friction. Also fixes the `recipes[0]` (alphabetically-first) default. |
| 10 | **fewd-vej** — searchable `<RecipePicker>` combobox                 | Bigger. Compounds with #9 — smart default + typeahead means you rarely open the picker, and can type when you do.                     |

---

## Shipping

After the chosen phases merge + sync with `origin`, tag `main` with `v2026-MM-DD`
(`.N` suffix for same-day hotfixes) and cut a notes-only GitHub release
(`gh release create <tag> --title <tag> --notes-file <f> --latest`). No binaries —
the dietpi box builds from source at deploy time via `just deploy`.
