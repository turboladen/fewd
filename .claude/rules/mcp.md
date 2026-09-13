---
paths:
  - "server/src/mcp/**"
  - "server/tests/mcp_*"
  - "docs/mcp-testing.md"
---

# MCP server

The MCP server lives at `server/src/mcp/`, mounted at `/mcp` on the Axum router, over Streamable HTTP; Claude Desktop connects via `bunx mcp-remote` (see README).

- `mcp/mod.rs`: router factory and bearer-auth middleware (`Authorization: Bearer <mcp-token>`, a per-person 256-bit opaque token, argon2id-hashed; mint with `POST /api/people/{id}/mcp-token`).
- `mcp/handler.rs`: the `FewdMcp` struct, one method per `#[tool]`, and the `ServerHandler` impl.
- `mcp/lookups.rs`: shared name and id resolution (`MealLookups`).
- `mcp/prompts/`: MCP prompts.
- `mcp/schemas/`: LLM-facing input and output types, split by domain. Their `///` field docs ship to the model as schema descriptions.

Every tool input type, and every type nested inside one, carries `#[serde(deny_unknown_fields)]`; `every_tool_input_schema_denies_unknown_fields` enforces it. Keep `#[serde(flatten)]` out of tool inputs.

## Design principles

Read `bd memories fewd-mcp-design-principles` before extending the tool surface. In short:

- Error on unknown references with actionable messages that name the discovery tool.
- Cross-reference related tools in descriptions, and lead each description with task intent (`every_tool_description_leads_with_intent_verb`).
- Dual-expose important context as both a tool and a resource when clients vary in capability.
- Prefer discoverable tool names.
- Do the right thing by default server-side.
- Iterate on output format from live session feedback.
- Respect the domain model's expressiveness at the boundary rather than flattening it.

## Integration tests via `tower::oneshot`

- rmcp's `StreamableHttpService` enforces a Host allowlist; tests must set `Host: localhost`.
- `initialize` is handled by `ServerHandler::get_info(&self)`, which never reads `RequestContext::extensions`. Tests asserting that auth context reaches tools must drive the full sequence: `initialize`, capture the `mcp-session-id` response header, `notifications/initialized` (202), then `tools/call`. See `server/tests/mcp_auth_plumbing_test.rs`.
- `RequestContext<RoleServer>` can't be constructed in unit tests; exercise tools over the HTTP transport or through wrappers like `mcp::handler::authenticated_person`.

To run or poke the server by hand, use the `run-fewd-server` skill.
