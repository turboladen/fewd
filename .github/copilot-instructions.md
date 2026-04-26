# Copilot review instructions

Context Copilot review needs that isn't visible from a diff alone — deployment posture, where to find established conventions, and the design debt already filed elsewhere so the reviewer doesn't re-surface it.

## Deployment posture

fewd is a **LAN-only, single-household, self-hosted** web app. Intended deployment is one Raspberry Pi (or similar) on a home network, used by 2–5 family members. No multi-tenancy. No public-internet exposure. No proxy in front. The threat model is "household member stumbles into a tool," not "hostile actor on the open web."

Findings framed against a public-SaaS threat model — "rate limiting needed," "session memory could grow if attacked," "what if many concurrent users do X" — usually don't apply. Worth flagging when genuinely uncertain, but include the assumption being checked so the project owner can weigh it.

## Where to look before flagging a non-default choice

Several decisions in this repo look unusual on a cold read but have explicit rationale. Before flagging an "X seems wrong" finding:

1. **The commit message of the change you're reviewing.** Non-default choices typically have a paragraph explaining why. The MCP session keep-alive (`server/src/mcp/mod.rs`, set to 7 days) is the canonical example — looks long, is deliberate, full history is in the commit body.
2. **`CLAUDE.md`** — the Cross-Boundary Conventions section lists invariants the type system doesn't enforce (Title-Case `meal_type`, slot-encoded `order_index`, DB path resolution), and the MCP Server section summarizes the tool-design principles.
3. **The PR conversation** — design questions sometimes resolve in chat before the code lands.

## Categories of finding that are usually already tracked

Several classes of structural concern are well-known in this codebase and tracked as bead work items with deliberate priority. Re-surfacing them in PR review is usually noise. If a finding falls into one of these categories, assume it's tracked unless the PR text or commit message specifically rebuts that:

- **Free-form-string fields that should be enums.** Several DTO/entity fields (`IngredientDto.unit`, `Meal.meal_type`, etc.) are typed as `String` and rely on convention to stay consistent. The plan is to promote them to enums; flagging the absence of a type isn't useful.
- **`IngredientDto` shape concerns**, especially around purchasable form vs preparation form (e.g. "garlic cloves, thinly sliced" as one string). Split into `name + prep` is queued.
- **Missing cross-boundary tests** between MCP writes and HTTP reads (or any "data round-trips through two surfaces with different conventions") — known coverage gap, filed.
- **Shopping-list ergonomics** — rounding, pantry-staple separation, grocery-section classification, MCP/UI exposure of variants. All queued behind the MCP server work.

Flag a finding in one of these categories only if the change in this PR _makes the eventual fix harder_ — e.g. introducing a new free-form-string field where an enum would slot cleanly into the planned migration. Pure observation that "this could be a richer type" is already on the work list.

## MCP module specifics

`server/src/mcp/` follows seven design principles summarized in `CLAUDE.md`'s "MCP Server" section. Two come up in review most often:

- **Errors on unknown references should be actionable.** When a tool input names a slug / person / tag that doesn't exist, the error should point the LLM at the discovery tool ("Call `list_people` to see valid names"), not just state the failure. If a new write path lacks this, it's a real finding worth flagging.
- **Respect the domain model's expressiveness at the MCP boundary.** Fractional `servings_count: f64` (a kid eats 0.5 portions), tagged `IngredientAmountDto::Range`, structured ingredient amounts — these all carry real information. Don't suggest flattening them to "simpler" shapes (integer servings, free-text amounts) for LLM-friendliness; the shopping-list math depends on the structure surviving end-to-end.

## Task tracking

This project uses [bd (beads)](https://github.com/turboladen/beads) for all task tracking, not GitHub Issues. If a PR review surfaces a follow-up worth tracking, suggest "this could be filed as a bead" rather than "open a GitHub Issue." Priorities are P0–P4 (numeric, not "high/medium/low").

The serialized form at `.beads/issues.jsonl` is a flat-file export of a workflow tracker. A few conventions that look editable from a diff but aren't:

- **Bug descriptions are historical.** They capture what was observed at filing time and aren't retroactively rewritten to past tense once a fix lands — the audit trail is what makes closed bugs useful months later. Don't suggest editing a bug's description or workaround block inside the same PR that fixes it.
- **Closure is a separate post-merge commit on main**, by convention `chore(beads): close fewd-XXX after PR #YY merge` (see `cc57e53`, `379f601`). Setting `"status":"closed"` inside the fix PR would have `bd list --status=open` falsely report the fix is shipped while it sits in review.
- **Timestamps are real moments**, not invented. Don't suggest synthetic `closed_at` / `started_at` values; reusing an unrelated `updated_at` field fabricates history.
