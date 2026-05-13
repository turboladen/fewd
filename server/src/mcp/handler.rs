use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rmcp::handler::server::common::FromContextPart;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{
    AnnotateAble, CallToolResult, Content, Implementation, ListResourcesResult,
    PaginatedRequestParams, RawResource, ReadResourceRequestParams, ReadResourceResult, Resource,
    ResourceContents, ResourcesCapability, ServerCapabilities, ServerInfo, ToolsCapability,
};
use rmcp::service::RequestContext;
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler};
use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::entities::person;
use crate::services::meal_service::MealService;
use crate::services::person_service::PersonService;
use crate::services::recipe_service::{RecipeService, SearchFilters};
use crate::services::shopping_service::ShoppingService;

use super::lookups::MealLookups;
use super::schemas::{
    create_meal_input_to_dto, create_recipe_input_to_dto, meal_to_brief, person_to_prefs,
    recipe_to_brief, recipe_to_full, render_family_overview, shopping_item_from_dto,
    update_person_input_to_dto, CreateMealError, CreateMealInput, CreateRecipeInput,
    DateRangeParams, EmptyParams, GetRecipeParams, SearchRecipesParams, UpdatePersonInput,
};
use super::AuthenticatedPerson;

pub const FAMILY_OVERVIEW_URI: &str = "fewd://family/overview";

/// Per-call upper bound on the row count any list-returning tool will
/// surface. Defense-in-depth against an LLM-driven wide query
/// (hallucinated wildcard, imported public-recipe corpus) producing a
/// multi-megabyte response. Date-range-bounded tools still need this
/// because the date cap (`MAX_DATE_RANGE_DAYS`) doesn't bound rows
/// directly — at four meal slots × four people, 366 days can land
/// around 5–6k servings.
pub const MAX_LIST_RESULTS: usize = 500;

#[derive(Clone)]
pub struct FewdMcp {
    db: Arc<DatabaseConnection>,
}

#[tool_router]
impl FewdMcp {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db: Arc::new(db) }
    }

    /// Smoke-test tool. Keeps the auth → tool pipeline verifiable without
    /// requiring any data in the DB.
    #[tool(
        name = "whoami",
        description = "Verify your MCP bearer-token configuration is wired correctly — call this when a tool returns auth errors or when first connecting. Returns the authenticated family member's name.",
        input_schema = rmcp::handler::server::common::schema_for_type::<EmptyParams>()
    )]
    async fn whoami(
        &self,
        params: LenientParameters<EmptyParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(e) = params.into_tool_input("whoami") {
            return Ok(e);
        }
        let name = authenticated_name(&context)?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Hello, {name}. You are authenticated with fewd."
        ))]))
    }

    #[tool(
        name = "list_curated_recipes",
        description = "Use as the default starting point for meal-planning when the user hasn't named a specific dish or ingredient — returns the family's likely-relevant shortlist (every favorite first, then most-recently-made, then top-rated, deduped, ≤30 unless favorites exceed that). For targeted lookups by ingredient, tag, time, rating, or person preference, call `search_recipes` instead. The full archive is intentionally not exposed — the web UI is for human browsing.",
        input_schema = rmcp::handler::server::common::schema_for_type::<EmptyParams>()
    )]
    async fn list_curated_recipes(
        &self,
        params: LenientParameters<EmptyParams>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(e) = params.into_tool_input("list_curated_recipes") {
            return Ok(e);
        }
        let recipes = RecipeService::get_curated(&self.db)
            .await
            .map_err(db_error)?;
        if let Some(err) = enforce_list_cap(
            "list_curated_recipes",
            recipes.len(),
            "Most likely cause: an unusually large favorites set (favorites are never truncated by the curated-shortlist policy). Mark fewer recipes as favorites or call search_recipes with a narrower filter for targeted lookups.",
        ) {
            return Ok(err);
        }
        let out = recipes
            .iter()
            .map(recipe_to_brief)
            .collect::<Result<Vec<_>, _>>()
            .map_err(internal_error)?;
        tool_json_result(&out)
    }

    #[tool(
        name = "search_recipes",
        description = "Find specific recipes when the user names an ingredient, tag, time constraint, rating, or person preference — call BEFORE `create_meal` (to find a slug to schedule) or `create_recipe` (to avoid creating a near-duplicate). Bare calls (no filters / `query='*'`) are rejected — call `list_curated_recipes` for an unfiltered shortlist. Filters: `query` (case-insensitive substring on name); `tags` (case-insensitive exact match, multiple tags = AND); `max_total_time_minutes` (assumes recipe total_time is in minutes — recipes authored in hours won't match, known limitation); `min_rating`; `is_favorite`; `unmade_since_days`; `excludes_for_persons` (named family members whose dislikes exclude matching recipes — substring match on ingredient names, e.g. 'olive oil' is excluded when a person dislikes 'olive'); `includes_ingredient_substrings` (recipes must contain ALL listed substrings in some ingredient name — case-insensitive, multiple values AND together, possibly across different ingredients; use for 'what can I make with spam?' / 'recipes that use leftover rice' / combine with `tags=[\"dinner\"]` for 'dinner recipes with spam'). Returns brief rows — use `get_recipe` with the slug for full details. Unknown person names return an actionable error pointing at `list_people`.",
        input_schema = rmcp::handler::server::common::schema_for_type::<SearchRecipesParams>()
    )]
    async fn search_recipes(
        &self,
        params: LenientParameters<SearchRecipesParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = match params.into_tool_input("search_recipes") {
            Ok(v) => v,
            Err(e) => return Ok(e),
        };
        if let Err(msg) = params.validate_has_filter() {
            return Ok(tool_user_error(msg));
        }

        let excluded_ingredient_substrings = match self
            .resolve_dislikes_for_persons(params.excludes_for_persons.as_deref())
            .await
        {
            Ok(v) => v,
            Err(DislikeResolveError::UnknownPerson(msg)) => return Ok(tool_user_error(msg)),
            Err(DislikeResolveError::Internal(detail)) => return Err(internal_error(detail)),
        };

        let filters = SearchFilters {
            query: params.normalized_query(),
            tags: params.normalized_tags(),
            max_total_time_minutes: params.max_total_time_minutes,
            min_rating: params.min_rating,
            is_favorite: params.is_favorite,
            unmade_since_days: params.unmade_since_days,
            excluded_ingredient_substrings,
            included_ingredient_substrings: params.normalized_included_substrings(),
        };

        let recipes = RecipeService::search_filtered(&self.db, filters)
            .await
            .map_err(db_error)?;
        if let Some(err) = enforce_list_cap(
            "search_recipes",
            recipes.len(),
            "Add another filter (tags, max_total_time_minutes, min_rating, is_favorite, unmade_since_days) or tighten the existing query to narrow the result set.",
        ) {
            return Ok(err);
        }
        let out = recipes
            .iter()
            .map(recipe_to_brief)
            .collect::<Result<Vec<_>, _>>()
            .map_err(internal_error)?;
        tool_json_result(&out)
    }

    /// Resolve `excludes_for_persons` (a list of family-member names) into the
    /// flat, deduped, lowercased set of disliked ingredient substrings the
    /// service-layer filter expects. Returns a typed error so the caller can
    /// distinguish LLM-recoverable problems (unknown name → retry with
    /// corrected list) from internal failures (TOCTOU, malformed JSON →
    /// JSON-RPC protocol error so it logs server-side).
    async fn resolve_dislikes_for_persons(
        &self,
        names: Option<&[String]>,
    ) -> Result<Vec<String>, DislikeResolveError> {
        let Some(names) = names else {
            return Ok(Vec::new());
        };
        if names.is_empty() {
            return Ok(Vec::new());
        }

        let lookups = MealLookups::load(&self.db).await.map_err(|err| {
            tracing::error!(?err, "MCP tool: MealLookups::load failed");
            DislikeResolveError::Internal(format!("MealLookups::load failed: {err}"))
        })?;
        let people = PersonService::get_all(&self.db).await.map_err(|err| {
            tracing::error!(?err, "MCP tool: PersonService::get_all failed");
            DislikeResolveError::Internal(format!("PersonService::get_all failed: {err}"))
        })?;
        let people_by_id: HashMap<&str, &person::Model> =
            people.iter().map(|p| (p.id.as_str(), p)).collect();

        flatten_disliked_substrings(names, &lookups, &people_by_id)
    }

    #[tool(
        name = "get_recipe",
        description = "Read the full recipe — call AFTER `search_recipes` or `list_curated_recipes` returned a slug worth inspecting (when the user wants ingredients, instructions, nutrition, or prep time). Returns ingredients (with amounts and units), instructions, nutrition, prep/cook time, and any parent recipe it was adapted from.",
        input_schema = rmcp::handler::server::common::schema_for_type::<GetRecipeParams>()
    )]
    async fn get_recipe(
        &self,
        params: LenientParameters<GetRecipeParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = match params.into_tool_input("get_recipe") {
            Ok(v) => v,
            Err(e) => return Ok(e),
        };
        let normalized = params.slug.trim().to_lowercase();
        let recipe = RecipeService::get_by_slug(&self.db, normalized.clone())
            .await
            .map_err(db_error)?;
        let Some(recipe) = recipe else {
            return Ok(tool_user_error(format!(
                "no recipe with slug '{}'. Call list_curated_recipes for the shortlist or search_recipes with a filter to find valid slugs.",
                params.slug
            )));
        };

        let parent_slug = match recipe.parent_recipe_id.as_deref() {
            None => None,
            Some(parent_id) => {
                let parent = RecipeService::get_by_id(&self.db, parent_id.to_string())
                    .await
                    .map_err(db_error)?;
                if parent.is_none() {
                    tracing::warn!(
                        recipe_slug = %recipe.slug,
                        parent_recipe_id = parent_id,
                        "recipe references a parent that no longer exists; omitting parent_recipe_slug"
                    );
                }
                parent.map(|p| p.slug)
            }
        };

        let out = recipe_to_full(&recipe, parent_slug).map_err(internal_error)?;
        tool_json_result(&out)
    }

    #[tool(
        name = "list_people",
        description = "Look up active family members' canonical names — call BEFORE `create_meal` (to assign servings), `search_recipes`'s `excludes_for_persons`, `update_person` (to write preference learnings back), or any other tool that takes a person name, so the names match exactly. Returns each member's dietary goals, dislikes, favorites, notes, and drink preferences/dislikes; the `name` field (case-insensitive) is the identifier.",
        input_schema = rmcp::handler::server::common::schema_for_type::<EmptyParams>()
    )]
    async fn list_people(
        &self,
        params: LenientParameters<EmptyParams>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(e) = params.into_tool_input("list_people") {
            return Ok(e);
        }
        let people = PersonService::get_all(&self.db).await.map_err(db_error)?;
        if let Some(err) = enforce_list_cap(
            "list_people",
            people.len(),
            "list_people has no per-call filter — the only path to a smaller result set is reducing the active-person count via the web UI.",
        ) {
            return Ok(err);
        }
        let out = people
            .iter()
            .map(person_to_prefs)
            .collect::<Result<Vec<_>, _>>()
            .map_err(internal_error)?;
        tool_json_result(&out)
    }

    #[tool(
        name = "update_person",
        description = "Record what you've learned about a family member's preferences this session so future conversations inherit it — call AFTER `list_people` or `get_family_overview` to grab the canonical `name`. Updates any subset of `notes`, `dislikes`, `favorites`, `drink_preferences`, `drink_dislikes`; omitted (or null) fields are left unchanged. There is no way to clear a field back to null via this tool — use the web UI for that. Identification is case-insensitive on `name`. Authorization: any authenticated family member may update any other member (single-household trust model). Changes are visible to subsequent `list_people` / `get_family_overview` calls immediately.",
        input_schema = rmcp::handler::server::common::schema_for_type::<UpdatePersonInput>()
    )]
    async fn update_person(
        &self,
        input: LenientParameters<UpdatePersonInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = match input.into_tool_input("update_person") {
            Ok(v) => v,
            Err(e) => return Ok(e),
        };
        // `find_active_by_name` is the same case-insensitive resolver
        // `list_people` and `create_meal` use, and it intentionally
        // collapses "no match" and "ambiguous match" both to `Ok(None)`
        // (the ambiguous case is logged separately via `tracing::warn!`
        // for the operator). Surface a single actionable error pointing
        // at `list_people` — matching the existing pattern in
        // `create_meal`'s person-resolution path so the LLM's recovery
        // route is identical across tools.
        let original_name = input.name.clone();
        let person = match PersonService::find_active_by_name(&self.db, &input.name)
            .await
            .map_err(db_error)?
        {
            Some(p) => p,
            None => {
                return Ok(tool_user_error(format!(
                    "update_person: no active family member named '{original_name}'. Call list_people for valid names."
                )));
            }
        };
        let dto = update_person_input_to_dto(input);
        let updated = PersonService::update(&self.db, person.id, dto)
            .await
            .map_err(db_error)?;
        let out = person_to_prefs(&updated).map_err(internal_error)?;
        tool_json_result(&out)
    }

    /// Tool mirror of the `fewd://family/overview` resource. Resources in
    /// MCP are expected to be surfaced by the host for user-side attachment
    /// (paperclip UI) and are not addressable by the LLM on its own, so a
    /// tool is the only way to let Claude read the overview autonomously.
    #[tool(
        name = "get_family_overview",
        description = "Ground a meal-planning conversation by reading every family member's diet, dislikes, favorites, notes, and drink preferences in one block — call this FIRST in any planning session before `list_curated_recipes` / `search_recipes`. Returns a human-readable Markdown summary (use `list_people` instead when you need structured fields per member; use `update_person` to persist preference learnings back). Equivalent to the `fewd://family/overview` resource.",
        input_schema = rmcp::handler::server::common::schema_for_type::<EmptyParams>()
    )]
    async fn get_family_overview(
        &self,
        params: LenientParameters<EmptyParams>,
    ) -> Result<CallToolResult, McpError> {
        if let Err(e) = params.into_tool_input("get_family_overview") {
            return Ok(e);
        }
        let people = PersonService::get_all(&self.db).await.map_err(db_error)?;
        // Symmetric with `list_people` so the cap can't be bypassed by
        // routing through this tool. The `read_resource` path for
        // `fewd://family/overview` is intentionally not capped: it's
        // user-initiated attachment (paperclip UI) and not addressable
        // by the LLM autonomously, so the bypass vector doesn't apply.
        if let Some(err) = enforce_list_cap(
            "get_family_overview",
            people.len(),
            "get_family_overview has no per-call filter — the only path to a smaller result set is reducing the active-person count via the web UI.",
        ) {
            return Ok(err);
        }
        let markdown = render_family_overview(&people).map_err(internal_error)?;
        Ok(CallToolResult::success(vec![Content::text(markdown)]))
    }

    #[tool(
        name = "list_meals",
        description = "Check what's already scheduled BEFORE `create_meal` — surfaces existing meals so you don't double-book a dinner slot, and lets you spot week-over-week patterns ('we had pasta twice last week, switch it up'). Returns all meals in an inclusive date range with each serving's person, recipe (or ad-hoc items), serving count, and per-serving notes.",
        input_schema = rmcp::handler::server::common::schema_for_type::<DateRangeParams>()
    )]
    async fn list_meals(
        &self,
        params: LenientParameters<DateRangeParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = match params.into_tool_input("list_meals") {
            Ok(v) => v,
            Err(e) => return Ok(e),
        };
        if let Err(e) = params.validate() {
            return Ok(tool_user_error(e.to_string()));
        }

        let meals = MealService::get_all_for_date_range(
            &self.db,
            params.start_date.clone(),
            params.end_date.clone(),
        )
        .await
        .map_err(db_error)?;
        if let Some(err) = enforce_list_cap(
            "list_meals",
            meals.len(),
            "Narrow start_date / end_date to a smaller window — even within the date-range cap, a busy household can produce thousands of meal rows.",
        ) {
            return Ok(err);
        }

        let lookups = MealLookups::load(&self.db).await.map_err(db_error)?;
        let out = meals
            .iter()
            .map(|m| meal_to_brief(m, &lookups))
            .collect::<Result<Vec<_>, _>>()
            .map_err(internal_error)?;
        tool_json_result(&out)
    }

    #[tool(
        name = "get_shopping_list",
        description = "Produce the week's grocery list AFTER scheduling meals with `create_meal` — call when the user says 'what do I need to buy?' or after a planning session ends. Aggregates ingredients across all scheduled meals in the date range, scaled by person-servings, with unit conversion where compatible. Each item carries per-meal sources so 'why is there extra flour?' is traceable back to the contributing recipes.",
        input_schema = rmcp::handler::server::common::schema_for_type::<DateRangeParams>()
    )]
    async fn get_shopping_list(
        &self,
        params: LenientParameters<DateRangeParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = match params.into_tool_input("get_shopping_list") {
            Ok(v) => v,
            Err(e) => return Ok(e),
        };
        if let Err(e) = params.validate() {
            return Ok(tool_user_error(e.to_string()));
        }

        let list = ShoppingService::get_shopping_list(&self.db, params.start_date, params.end_date)
            .await
            .map_err(db_error)?;
        let out: Vec<_> = list.into_iter().map(shopping_item_from_dto).collect();
        tool_json_result(&out)
    }

    #[tool(
        name = "create_recipe",
        description = "Add a new recipe when the user describes one not already in the catalog — call `search_recipes` FIRST to check for duplicates (the LLM should resolve 'this is the same as carbonara, edit that one' rather than create a near-twin). The slug is auto-generated from the name (with a numeric suffix on collisions). Returns the full created recipe.",
        input_schema = rmcp::handler::server::common::schema_for_type::<CreateRecipeInput>()
    )]
    async fn create_recipe(
        &self,
        input: LenientParameters<CreateRecipeInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = match input.into_tool_input("create_recipe") {
            Ok(v) => v,
            Err(e) => return Ok(e),
        };
        // Resolve the parent (if any) once, capturing both the id we need
        // for storage AND the canonical slug we want to echo back. Echoing
        // the original input would round-trip whatever case/whitespace the
        // LLM happened to send (e.g. "  Carbonara  ") — a string that
        // wouldn't round-trip cleanly through get_recipe.
        let parent_resolution: Option<(String, String)> = match input.parent_recipe_slug.as_deref()
        {
            None => None,
            Some(slug) => {
                let normalized = slug.trim().to_lowercase();
                let parent = RecipeService::get_by_slug(&self.db, normalized)
                    .await
                    .map_err(db_error)?;
                let Some(parent) = parent else {
                    return Ok(tool_user_error(format!(
                        "parent_recipe_slug '{slug}' does not exist. Omit it or use a valid slug from search_recipes."
                    )));
                };
                Some((parent.id, parent.slug))
            }
        };
        let (parent_recipe_id, parent_slug_canonical) = match parent_resolution {
            Some((id, slug)) => (Some(id), Some(slug)),
            None => (None, None),
        };

        let dto = match create_recipe_input_to_dto(input, parent_recipe_id) {
            Ok(dto) => dto,
            Err(e) => return Ok(tool_user_error(e.to_string())),
        };
        let created = RecipeService::create(&self.db, dto)
            .await
            .map_err(db_error)?;
        let full = recipe_to_full(&created, parent_slug_canonical).map_err(internal_error)?;
        tool_json_result(&full)
    }

    #[tool(
        name = "create_meal",
        description = "Schedule a planned meal — call AFTER `list_meals` (to confirm the slot is empty) and `search_recipes` / `get_recipe` (to find the slug). Each serving assigns one family member to either an existing recipe (by slug) or an ad-hoc ingredient list. Unknown names or slugs return a clear error so the caller can retry with corrected values. Returns the created meal with slugs/names resolved.",
        input_schema = rmcp::handler::server::common::schema_for_type::<CreateMealInput>()
    )]
    async fn create_meal(
        &self,
        input: LenientParameters<CreateMealInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = match input.into_tool_input("create_meal") {
            Ok(v) => v,
            Err(e) => return Ok(e),
        };
        let lookups = MealLookups::load(&self.db).await.map_err(db_error)?;
        // Match each variant so a future `CreateMealError` variant that
        // is NOT LLM-recoverable (e.g. an internal-failure variant) fails
        // to compile until it's categorized rather than being silently
        // routed through `tool_user_error`.
        let dto = match create_meal_input_to_dto(input, &lookups) {
            Ok(dto) => dto,
            Err(CreateMealError::Input(e)) => return Ok(tool_user_error(e.to_string())),
            Err(CreateMealError::Resolve(e)) => return Ok(tool_user_error(e.to_string())),
        };

        let created = MealService::create(&self.db, dto).await.map_err(db_error)?;
        let brief = meal_to_brief(&created, &lookups).map_err(internal_error)?;
        tool_json_result(&brief)
    }
}

#[tool_handler]
impl ServerHandler for FewdMcp {
    fn get_info(&self) -> ServerInfo {
        let mut capabilities = ServerCapabilities::default();
        capabilities.tools = Some(ToolsCapability::default());
        capabilities.resources = Some(ResourcesCapability::default());
        ServerInfo::new(capabilities)
            .with_server_info(Implementation::new("fewd-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "fewd MCP: plan dinners and generate shopping lists. Canonical workflow: \
                 (1) DISCOVER family context — `get_family_overview` (or the \
                 `fewd://family/overview` resource) for diets/dislikes/favorites; \
                 `list_people` for structured fields. \
                 (2) PLAN recipes — `list_curated_recipes` for the family shortlist; \
                 `search_recipes` with filters (tags, max time, ingredient, \
                 excludes_for_persons, …) for targeted lookups; `get_recipe` for full \
                 details on a slug; `create_recipe` only when nothing matches. \
                 (3) CHECK existing schedule — `list_meals` over the target date range \
                 to see what's already booked and avoid duplicates. \
                 (4) SCHEDULE meals — `create_meal` per date+slot, assigning family \
                 members by name to recipes by slug (or ad-hoc items). \
                 (5) SHOP — `get_shopping_list` over the date range produces the \
                 consolidated grocery list. \
                 (6) CAPTURE — when you learn something durable about a family member \
                 mid-conversation (a standing note, a new like/dislike, a drink \
                 preference), call `update_person` so it's available next session \
                 instead of dying with this conversation. \
                 All date inputs are YYYY-MM-DD.",
            )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let mut raw = RawResource::new(FAMILY_OVERVIEW_URI, "family-overview");
        raw.description = Some(
            "Markdown summary of every active family member: dietary goals, dislikes, favorites, \
             and notes. Auto-load at conversation start for context."
                .into(),
        );
        raw.mime_type = Some("text/markdown".into());
        let resources: Vec<Resource> = vec![raw.no_annotation()];
        Ok(ListResourcesResult::with_all_items(resources))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        if request.uri != FAMILY_OVERVIEW_URI {
            return Err(McpError::invalid_params(
                format!("unknown resource uri: {}", request.uri),
                None,
            ));
        }
        // Intentionally NOT capped via `enforce_list_cap` — see the
        // matching note in `get_family_overview`. MCP resources are
        // application-controlled (host-mediated user attachment) per
        // the spec's resource semantics, so the LLM-bypass concern
        // that drove the cap on `list_people` / `get_family_overview`
        // doesn't apply here. A user explicitly attaching the family
        // overview wants the whole thing.
        let people = PersonService::get_all(&self.db).await.map_err(db_error)?;
        let markdown = render_family_overview(&people).map_err(internal_error)?;
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            markdown,
            &request.uri,
        )
        .with_mime_type("text/markdown")]))
    }
}

// ─── Helpers ────────────────────────────────────────────────────

/// Canonical extraction site for the authenticated identity. Any tool
/// that wants to read who's making the call MUST go through this helper
/// (don't reach into `context.extensions` directly) — it's the only way
/// to make a future "self-only" or role-based authorization check
/// findable by `git grep authenticated_person` instead of being
/// scattered across tool bodies.
///
/// Returns a tracing-logged `McpError` when the bearer middleware
/// failed to plumb the context. Both failure modes are operator bugs
/// in the middleware setup, not client-correctable input. The wire
/// strings (`"missing http request parts"` / `"missing authenticated
/// person"`) are fixed non-sensitive constants that intentionally
/// surface the failure mode in-band as operator-debuggable signal —
/// the redaction contract from fewd-2y6.4 is specifically about
/// *variable* detail (DbErr Display, formatted internal state); these
/// constants don't fit that concern.
///
/// Today only `whoami` consumes this. The structural problem the
/// existence of this helper addresses (vs. "no per-user state at all"):
/// when self-only checks need to land, they go through this exact site,
/// so adding one new check is one new call site rather than a sweep of
/// every tool. Integration coverage at `server/tests/mcp_auth_plumbing_test.rs`
/// pins the end-to-end plumbing.
pub(super) fn authenticated_person(
    context: &RequestContext<RoleServer>,
) -> Result<&AuthenticatedPerson, McpError> {
    let parts = context
        .extensions
        .get::<axum::http::request::Parts>()
        .ok_or_else(|| {
            tracing::error!("MCP auth: missing http request parts in tool context");
            McpError::internal_error("missing http request parts", None)
        })?;
    parts
        .extensions
        .get::<AuthenticatedPerson>()
        .ok_or_else(|| {
            tracing::error!("MCP auth: missing AuthenticatedPerson extension");
            McpError::internal_error("missing authenticated person", None)
        })
}

/// Thin wrapper around [`authenticated_person`] for the common case
/// where the caller only needs the family member's display name (e.g.
/// `whoami`). Single extraction site means there's one place to change
/// if the auth surface ever exposes additional fields.
fn authenticated_name(context: &RequestContext<RoleServer>) -> Result<String, McpError> {
    authenticated_person(context).map(|p| p.0.name.clone())
}

fn tool_json_result<T: Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(value).map_err(|err| {
        tracing::error!(?err, "MCP tool: failed to serialize result");
        // Wire message stays opaque; serde Display can echo struct field
        // names back to the LLM client. Details land in tracing only.
        McpError::internal_error("failed to serialize result", None)
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

// `db_error` and `internal_error` deliberately return a fixed wire message.
// SeaORM's `DbErr` Display embeds SQLite/SQLx detail (column names,
// constraint names, occasionally parameter values), and `internal_error`
// callers pass formatted internal state. Logging the verbose detail via
// `tracing` keeps it on the operator side; the JSON-RPC client sees only
// the opaque label. Use `tool_user_error` for messages that *should*
// reach the LLM (input validation, unknown references).
//
// Some call sites (e.g. `resolve_dislikes_for_persons`) also emit a
// structured `tracing::error!(?err, …)` before formatting the
// diagnostic into a String — that's a feature, not a duplicate-bug:
// the call site captures full Debug fidelity and the helper logs the
// flattened diagnostic as a uniform backstop.
fn db_error(err: sea_orm::DbErr) -> McpError {
    tracing::error!(?err, "MCP tool: database error");
    McpError::internal_error("database error", None)
}

fn internal_error(detail: String) -> McpError {
    tracing::error!(%detail, "MCP tool: internal error");
    McpError::internal_error("internal server error", None)
}

/// Build a tool-level error result so the actionable message reaches the
/// LLM. Use this for input-validation failures and unknown-reference errors
/// — anything the LLM can recover from by retrying with corrected input.
/// JSON-RPC protocol errors (`Err(McpError)`) are typically displayed by
/// MCP clients as a generic "Tool execution failed" with the message
/// dropped, so they're reserved for transport / internal failures.
fn tool_user_error(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![Content::text(message.into())])
}

/// Reject a list-returning tool call when the pre-conversion row count
/// exceeds [`MAX_LIST_RESULTS`]. Returns `Some(CallToolResult)` to
/// surface to the client when over cap; returns `None` when under cap
/// so the caller can proceed. The `narrow_hint` should name concrete
/// filters (or discovery tools) the LLM can use to recover.
///
/// The cap fires *after* the service layer materializes its `Vec` —
/// acceptable at household scale (the rejection-path allocation is
/// negligible against the cap's 500-row ceiling). If a future entity
/// grows past ~10k rows in the wild, push the limit down into the
/// service-layer SQL (`.limit(MAX_LIST_RESULTS + 1)`) so we never
/// allocate the rejected rows in the first place.
fn enforce_list_cap(tool: &str, count: usize, narrow_hint: &str) -> Option<CallToolResult> {
    if count > MAX_LIST_RESULTS {
        Some(tool_user_error(format!(
            "{tool} returned {count} rows, exceeding the {MAX_LIST_RESULTS}-row per-call cap. {narrow_hint}"
        )))
    } else {
        None
    }
}

/// Parameter wrapper that defers deserialize errors to the handler so they
/// can be returned as tool-level errors instead of bubbling up as JSON-RPC
/// `-32602` ("invalid_params") responses. rmcp's stock `Parameters<T>`
/// extractor errors at extraction time, which most MCP clients render as a
/// generic "Tool execution failed" with the actionable message dropped.
/// `LenientParameters<T>` always succeeds at extraction; on deserialize
/// failure the handler matches on the inner result and returns
/// `Ok(tool_user_error(…))` via [`Self::into_tool_input`].
///
/// **You MUST also specify** `input_schema =
/// rmcp::handler::server::common::schema_for_type::<T>()` on the `#[tool]`
/// attribute. The rmcp `#[tool]` macro only auto-generates input schemas
/// from a literal `Parameters<T>` ident in the function signature
/// (rmcp-macros `find_parameters_type_in_sig`), so without the override
/// the LLM sees an empty schema and can't discover the tool's inputs.
pub(super) struct LenientParameters<T>(Result<T, String>);

impl<T: DeserializeOwned> LenientParameters<T> {
    /// Core extraction logic, separated from [`FromContextPart`] so it can
    /// be exercised in tests without spinning up a full `ToolCallContext`
    /// (which is `#[non_exhaustive]` and requires a service reference).
    /// Mirrors rmcp's stock `Parameters<T>` deserialize step exactly,
    /// except errors are captured into the inner `Result`.
    fn extract(arguments: Option<rmcp::model::JsonObject>) -> Self {
        let arguments = arguments.unwrap_or_default();
        let parsed = serde_json::from_value::<T>(serde_json::Value::Object(arguments))
            .map_err(|e| e.to_string());
        LenientParameters(parsed)
    }
}

impl<T> LenientParameters<T> {
    /// Unwrap the inner `Result` into either the deserialized value or a
    /// tool-level error formatted with the originating tool's name. Pairs
    /// with the early-return pattern in handlers:
    /// `let input = match input.into_tool_input("name") { Ok(v) => v, Err(e) => return Ok(e) };`
    pub(super) fn into_tool_input(self, tool_name: &'static str) -> Result<T, CallToolResult> {
        self.0.map_err(|e| {
            tool_user_error(format!("{tool_name}: {e}. Check the tool's input schema."))
        })
    }

    /// Construct a `LenientParameters` directly from a value, bypassing the
    /// rmcp deserialize layer. Tests use this to drive handler logic
    /// without round-tripping through `from_context_part`.
    #[cfg(test)]
    pub(super) fn for_test(value: T) -> Self {
        LenientParameters(Ok(value))
    }
}

impl<S, T> FromContextPart<ToolCallContext<'_, S>> for LenientParameters<T>
where
    T: DeserializeOwned,
{
    fn from_context_part(context: &mut ToolCallContext<S>) -> Result<Self, McpError> {
        Ok(Self::extract(context.arguments.take()))
    }
}

/// Failure modes for `flatten_disliked_substrings` / the MCP-side resolver.
/// Split so the caller can route LLM-recoverable problems to a tool-level
/// error and internal failures to a JSON-RPC protocol error.
#[derive(Debug)]
pub(super) enum DislikeResolveError {
    /// LLM-recoverable: the named person doesn't exist (or is ambiguous).
    /// Carries the user-facing message; gets surfaced via `tool_user_error`.
    UnknownPerson(String),
    /// Server-side problem (TOCTOU race, malformed `dislikes` JSON, DB
    /// error). Carries the operator-facing diagnostic string; the call
    /// site wraps it via `internal_error()` so the wire stays opaque
    /// while the detail still reaches tracing. Keep the variant a plain
    /// `String` — the wrap into `McpError` happens at the protocol-layer
    /// call site only, never inside this helper.
    Internal(String),
}

/// Pure helper extracted from `FewdMcp::resolve_dislikes_for_persons` so the
/// resolution + dedup logic is testable without spinning up a DB. Takes the
/// already-loaded lookups + people-by-id map; the wrapper handles I/O.
///
/// Returns `UnknownPerson` for names not in `lookups` (the LLM will see an
/// actionable retry hint pointing at `list_people`) and `Internal` for the
/// rare TOCTOU window where a person resolved through `MealLookups` got
/// deactivated before the second `get_all` ran, or for malformed JSON.
fn flatten_disliked_substrings(
    names: &[String],
    lookups: &MealLookups,
    people_by_id: &HashMap<&str, &person::Model>,
) -> Result<Vec<String>, DislikeResolveError> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for raw in names {
        let id = lookups.person_id_for_name(raw).ok_or_else(|| {
            DislikeResolveError::UnknownPerson(format!(
                "excludes_for_persons: no active family member named '{raw}'. Call list_people for valid names."
            ))
        })?;
        let person = people_by_id.get(id).ok_or_else(|| {
            DislikeResolveError::Internal(format!(
                "person id '{id}' resolved by MealLookups is no longer active; retry the tool call"
            ))
        })?;
        let dislikes: Vec<String> = serde_json::from_str(&person.dislikes).map_err(|err| {
            DislikeResolveError::Internal(format!(
                "person '{}' has malformed dislikes JSON: {err}",
                person.name
            ))
        })?;
        for d in dislikes {
            let normalized = d.trim().to_lowercase();
            if !normalized.is_empty() && seen.insert(normalized.clone()) {
                out.push(normalized);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};

    fn mk_person(id: &str, name: &str, dislikes_json: &str) -> person::Model {
        person::Model {
            id: id.to_string(),
            name: name.to_string(),
            birthdate: NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
            dietary_goals: None,
            dislikes: dislikes_json.to_string(),
            favorites: "[]".to_string(),
            notes: None,
            drink_preferences: None,
            drink_dislikes: None,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            mcp_token_hash: None,
            mcp_token_fingerprint: None,
        }
    }

    fn lookups_for(people: &[person::Model]) -> MealLookups {
        MealLookups::from_people_and_recipes(
            people
                .iter()
                .map(|p| (p.id.clone(), p.name.clone()))
                .collect(),
            vec![],
        )
    }

    fn index(people: &[person::Model]) -> HashMap<&str, &person::Model> {
        people.iter().map(|p| (p.id.as_str(), p)).collect()
    }

    #[test]
    fn flatten_unknown_name_returns_user_retryable_error_with_actionable_hint() {
        let people = vec![mk_person("p1", "Alice", "[\"olives\"]")];
        let lookups = lookups_for(&people);
        let by_id = index(&people);

        let err = flatten_disliked_substrings(&["Bob".to_string()], &lookups, &by_id).unwrap_err();
        let DislikeResolveError::UnknownPerson(msg) = err else {
            panic!("expected UnknownPerson, got {err:?}");
        };
        assert!(
            msg.contains("excludes_for_persons"),
            "error must name the offending field: {msg}"
        );
        assert!(
            msg.contains("Bob"),
            "error must echo the unknown name: {msg}"
        );
        assert!(
            msg.contains("list_people"),
            "error must point at the discovery tool: {msg}"
        );
    }

    #[test]
    fn flatten_dedups_across_people_case_insensitively() {
        // Alice dislikes "Olives" (Title-case) and "beets".
        // Bob dislikes "OLIVES  " (uppercase + trailing whitespace) and "carrots".
        // Expected output: ["olives", "beets", "carrots"] (deduped, order
        // preserves first appearance per person).
        let people = vec![
            mk_person("p1", "Alice", "[\"Olives\",\"beets\"]"),
            mk_person("p2", "Bob", "[\"OLIVES  \",\"carrots\"]"),
        ];
        let lookups = lookups_for(&people);
        let by_id = index(&people);

        let out = flatten_disliked_substrings(
            &["Alice".to_string(), "Bob".to_string()],
            &lookups,
            &by_id,
        )
        .unwrap();

        assert_eq!(out, vec!["olives", "beets", "carrots"]);
    }

    #[test]
    fn flatten_drops_empty_and_whitespace_only_dislike_entries() {
        let people = vec![mk_person(
            "p1",
            "Alice",
            "[\"olives\",\"\",\"   \",\"beets\"]",
        )];
        let lookups = lookups_for(&people);
        let by_id = index(&people);

        let out = flatten_disliked_substrings(&["Alice".to_string()], &lookups, &by_id).unwrap();
        assert_eq!(out, vec!["olives", "beets"]);
    }

    #[test]
    fn flatten_malformed_dislikes_json_returns_internal_error_naming_person() {
        let people = vec![mk_person("p1", "Broken", "not-json")];
        let lookups = lookups_for(&people);
        let by_id = index(&people);

        let err =
            flatten_disliked_substrings(&["Broken".to_string()], &lookups, &by_id).unwrap_err();
        let DislikeResolveError::Internal(detail) = err else {
            panic!("expected Internal, got {err:?}");
        };
        // Detail is the operator-facing diagnostic that gets logged;
        // the wire-side McpError stays opaque (see
        // `internal_error_wire_message_omits_caller_supplied_detail`).
        assert!(
            detail.contains("Broken"),
            "diagnostic must name the person: {detail}"
        );
        assert!(
            detail.contains("malformed dislikes JSON"),
            "diagnostic must describe the failure mode: {detail}"
        );
    }

    #[test]
    fn flatten_lookup_id_missing_from_index_returns_internal_error() {
        // Simulates the TOCTOU window where MealLookups resolved a name
        // but the second PersonService::get_all returned a snapshot without
        // that id (e.g. the person was deactivated between the two reads).
        let people = vec![mk_person("p1", "Alice", "[\"olives\"]")];
        let lookups = lookups_for(&people);
        // Empty index — Alice's id is not present.
        let by_id: HashMap<&str, &person::Model> = HashMap::new();

        let err =
            flatten_disliked_substrings(&["Alice".to_string()], &lookups, &by_id).unwrap_err();
        let DislikeResolveError::Internal(detail) = err else {
            panic!("expected Internal, got {err:?}");
        };
        assert!(
            detail.contains("p1"),
            "diagnostic must include the orphan id: {detail}"
        );
        assert!(
            detail.contains("retry"),
            "diagnostic must hint that a retry is appropriate: {detail}"
        );
    }

    #[test]
    fn tool_user_error_returns_call_tool_result_with_is_error_and_text_content() {
        // Regression guard for the "Tool execution failed with no detail"
        // problem: rejection messages MUST surface as CallToolResult { is_error: true,
        // content: [Content::text(...)] } so MCP clients display the actionable
        // text to the LLM. JSON-RPC protocol errors get displayed as a generic
        // "Tool execution failed" with the message dropped.
        let result = tool_user_error("retry with at least one filter");
        assert_eq!(result.is_error, Some(true));
        assert_eq!(result.content.len(), 1);
        let serialized = serde_json::to_string(&result).expect("CallToolResult serializes");
        assert!(
            serialized.contains("retry with at least one filter"),
            "message must round-trip through serialization: {serialized}"
        );
        assert!(
            serialized.contains("\"isError\":true"),
            "isError flag must serialize for the wire: {serialized}"
        );
    }

    // ─── LLM-facing error message contracts ─────────────────────────
    //
    // `tool_user_error(e.to_string())` makes the Display strings of
    // `InputError`, `ResolveError`, and `CreateMealError` the user-facing
    // messages reaching the LLM. These tests pin the contract: every
    // variant must echo the offending value AND point at a discovery
    // tool (or describe the expected format) so the LLM can retry.
    // Without this guard a future variant could ship with a vague
    // Display impl and silently degrade the LLM-recovery path.

    #[test]
    fn input_error_displays_actionable_messages_for_each_variant() {
        use super::super::schemas::errors::InputError;

        let cases = [
            (
                InputError::NonPositiveServings(0).to_string(),
                vec!["servings must be >= 1", "0"],
            ),
            (
                InputError::NonPositiveServingsCount(-0.5).to_string(),
                vec!["servings_count must be > 0", "-0.5"],
            ),
            (
                InputError::UnknownMealType("brunch".into()).to_string(),
                vec!["Breakfast", "Lunch", "Dinner", "Snack", "brunch"],
            ),
            (
                InputError::EmptyName("name").to_string(),
                vec!["name", "empty"],
            ),
            (
                InputError::InvalidDate {
                    field: "start_date",
                    value: "garbage".into(),
                }
                .to_string(),
                vec!["start_date", "YYYY-MM-DD", "garbage"],
            ),
            (
                InputError::ReversedDateRange {
                    start_date: "2026-04-30".into(),
                    end_date: "2026-04-26".into(),
                }
                .to_string(),
                vec!["start_date", "end_date", "2026-04-30", "2026-04-26"],
            ),
            (
                InputError::DateRangeTooWide {
                    days: 1500,
                    max_days: 366,
                }
                .to_string(),
                vec!["1500", "366", "Narrow"],
            ),
        ];

        for (msg, expected_fragments) in &cases {
            for frag in expected_fragments {
                assert!(
                    msg.contains(frag),
                    "InputError message {msg:?} must mention {frag:?} so the LLM can fix the call"
                );
            }
        }
    }

    #[test]
    fn resolve_error_displays_point_at_discovery_tool() {
        use super::super::schemas::errors::ResolveError;

        let person_msg = ResolveError::UnknownPerson("Bob".into()).to_string();
        assert!(
            person_msg.contains("Bob"),
            "must echo unknown name: {person_msg}"
        );
        assert!(
            person_msg.contains("list_people"),
            "must point at discovery tool: {person_msg}"
        );

        let recipe_msg = ResolveError::UnknownRecipe("ghost-pasta".into()).to_string();
        assert!(
            recipe_msg.contains("ghost-pasta"),
            "must echo unknown slug: {recipe_msg}"
        );
        assert!(
            recipe_msg.contains("list_curated_recipes") || recipe_msg.contains("search_recipes"),
            "must point at a recipe-discovery tool: {recipe_msg}"
        );
    }

    #[test]
    fn create_meal_error_forwards_inner_message() {
        use super::super::schemas::errors::{CreateMealError, InputError, ResolveError};

        let from_input: CreateMealError = InputError::UnknownMealType("brunch".into()).into();
        assert_eq!(
            from_input.to_string(),
            InputError::UnknownMealType("brunch".into()).to_string(),
            "CreateMealError::Input must forward InputError's Display verbatim"
        );

        let from_resolve: CreateMealError = ResolveError::UnknownPerson("Bob".into()).into();
        assert_eq!(
            from_resolve.to_string(),
            ResolveError::UnknownPerson("Bob".into()).to_string(),
            "CreateMealError::Resolve must forward ResolveError's Display verbatim"
        );
    }

    #[test]
    fn tool_user_error_preserves_input_error_display_message() {
        use super::super::schemas::errors::InputError;

        // End-to-end: an InputError::Display string survives the wrap into
        // CallToolResult and shows up on the wire as the LLM-visible content.
        let err = InputError::InvalidDate {
            field: "start_date",
            value: "garbage".into(),
        };
        let result = tool_user_error(err.to_string());
        let serialized = serde_json::to_string(&result).expect("CallToolResult serializes");
        assert!(serialized.contains("\"isError\":true"));
        assert!(
            serialized.contains("YYYY-MM-DD"),
            "format hint must reach the wire: {serialized}"
        );
        assert!(
            serialized.contains("garbage"),
            "offending value must reach the wire: {serialized}"
        );
    }

    // ─── Call-site wiring contracts ─────────────────────────────────
    //
    // The Display contracts above guarantee the *messages* are good, but
    // they don't catch a revert of the call-site wiring (where a tool
    // returns `Err(McpError::invalid_params(...))` instead of
    // `Ok(tool_user_error(...))`). These tests invoke each tool against
    // an empty in-memory DB and assert the result is a tool-level error,
    // not a JSON-RPC protocol error. A revert silently regresses the
    // Claude Desktop UX and these tests are the only thing that fails.

    async fn setup_test_mcp() -> FewdMcp {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite connects");
        <migration::Migrator as migration::MigratorTrait>::up(&db, None)
            .await
            .expect("migrations run on empty DB");
        FewdMcp::new(db)
    }

    fn assert_tool_user_error(
        result: Result<CallToolResult, McpError>,
        expected_fragments: &[&str],
    ) {
        let call_result = result.expect("tool call must return Ok(CallToolResult), not Err(McpError) — JSON-RPC protocol errors get displayed as a generic 'Tool execution failed' by most MCP clients");
        assert_eq!(
            call_result.is_error,
            Some(true),
            "must be a tool-level error (is_error: true), not a successful result"
        );
        let serialized = serde_json::to_string(&call_result).expect("serializes");
        for frag in expected_fragments {
            assert!(
                serialized.contains(frag),
                "tool-level error must contain {frag:?}: {serialized}"
            );
        }
    }

    #[tokio::test]
    async fn get_recipe_unknown_slug_returns_tool_level_error_not_protocol_error() {
        use super::super::schemas::GetRecipeParams;

        let mcp = setup_test_mcp().await;
        let result = mcp
            .get_recipe(LenientParameters::for_test(GetRecipeParams {
                slug: "ghost-pasta".into(),
            }))
            .await;
        assert_tool_user_error(result, &["ghost-pasta", "list_curated_recipes"]);
    }

    #[tokio::test]
    async fn list_meals_invalid_date_returns_tool_level_error() {
        let mcp = setup_test_mcp().await;
        let result = mcp
            .list_meals(LenientParameters::for_test(DateRangeParams {
                start_date: "garbage".into(),
                end_date: "2026-01-01".into(),
            }))
            .await;
        assert_tool_user_error(result, &["start_date", "YYYY-MM-DD", "garbage"]);
    }

    #[tokio::test]
    async fn get_shopping_list_invalid_date_returns_tool_level_error() {
        let mcp = setup_test_mcp().await;
        let result = mcp
            .get_shopping_list(LenientParameters::for_test(DateRangeParams {
                start_date: "2026-01-01".into(),
                end_date: "garbage".into(),
            }))
            .await;
        assert_tool_user_error(result, &["end_date", "YYYY-MM-DD", "garbage"]);
    }

    #[tokio::test]
    async fn create_meal_unknown_meal_type_returns_tool_level_error() {
        let mcp = setup_test_mcp().await;
        let result = mcp
            .create_meal(LenientParameters::for_test(CreateMealInput {
                date: "2026-01-01".into(),
                meal_type: "brunch".into(),
                order_index: None,
                servings: vec![],
            }))
            .await;
        assert_tool_user_error(result, &["brunch", "Breakfast", "Lunch", "Dinner", "Snack"]);
    }

    #[tokio::test]
    async fn create_recipe_unknown_parent_slug_returns_tool_level_error() {
        let mcp = setup_test_mcp().await;
        // Empty DB → parent_recipe_slug lookup returns None. Other fields
        // pass validation so the tool reaches the parent-resolution branch.
        let input: CreateRecipeInput = serde_json::from_str(
            r#"{
                "name": "Test Recipe",
                "source": "manual",
                "parent_recipe_slug": "ghost-recipe",
                "servings": 4,
                "instructions": "Cook.",
                "ingredients": []
            }"#,
        )
        .expect("CreateRecipeInput JSON shape");
        let result = mcp.create_recipe(LenientParameters::for_test(input)).await;
        assert_tool_user_error(result, &["ghost-recipe", "search_recipes"]);
    }

    #[tokio::test]
    async fn create_recipe_invalid_input_returns_tool_level_error() {
        let mcp = setup_test_mcp().await;
        // No parent slug → input validation runs and rejects servings=0
        // before any DB query.
        let input: CreateRecipeInput = serde_json::from_str(
            r#"{
                "name": "Test Recipe",
                "source": "manual",
                "servings": 0,
                "instructions": "Cook.",
                "ingredients": []
            }"#,
        )
        .expect("CreateRecipeInput JSON shape");
        let result = mcp.create_recipe(LenientParameters::for_test(input)).await;
        assert_tool_user_error(result, &["servings must be >= 1", "0"]);
    }

    #[tokio::test]
    async fn list_meals_reversed_date_range_returns_tool_level_error() {
        let mcp = setup_test_mcp().await;
        // Both dates parse cleanly but start > end. The service-layer
        // SQL filter is `date >= start AND date <= end`, which silently
        // returns [] for reversed input — indistinguishable from "no
        // meals scheduled". Tool-level error is the only signal the LLM
        // gets that the range is backwards.
        let result = mcp
            .list_meals(LenientParameters::for_test(DateRangeParams {
                start_date: "2026-04-30".into(),
                end_date: "2026-04-26".into(),
            }))
            .await;
        assert_tool_user_error(
            result,
            &["start_date", "end_date", "2026-04-30", "2026-04-26"],
        );
    }

    #[tokio::test]
    async fn create_meal_unknown_person_returns_tool_level_error() {
        let mcp = setup_test_mcp().await;
        // Date + meal_type + servings_count all valid → reaches
        // serving_input_to_dto → resolve_person("Bob") → empty DB has no
        // people → CreateMealError::Resolve(UnknownPerson). Exercises the
        // Resolve arm of the exhaustive match in create_meal.
        // ServingInput lives in a private submodule, so build the input
        // via JSON to avoid widening the schemas public surface.
        let input: CreateMealInput = serde_json::from_str(
            r#"{
                "date": "2026-01-01",
                "meal_type": "Dinner",
                "servings": [{
                    "kind": "recipe",
                    "person_name": "Bob",
                    "recipe_slug": "doesnt-matter",
                    "servings_count": 1.0
                }]
            }"#,
        )
        .expect("CreateMealInput JSON shape");
        let result = mcp.create_meal(LenientParameters::for_test(input)).await;
        assert_tool_user_error(result, &["Bob", "list_people"]);
    }

    // ─── update_person ──────────────────────────────────────────────
    //
    // Tools that mutate state need a happy-path test that re-reads the
    // row (so we catch a regression where the service call is dropped
    // and the tool reports success without writing). Negative paths
    // assert the actionable-error contract so the LLM can recover.

    use crate::dto::CreatePersonDto;

    async fn seed_person(mcp: &FewdMcp, name: &str) -> person::Model {
        PersonService::create(
            &mcp.db,
            CreatePersonDto {
                name: name.into(),
                birthdate: "1990-01-01".into(),
                dietary_goals: None,
                dislikes: vec!["olives".into()],
                favorites: vec!["pasta".into()],
                notes: Some("original note".into()),
                drink_preferences: Some(vec!["whiskey".into()]),
                drink_dislikes: Some(vec!["gin".into()]),
            },
        )
        .await
        .expect("seed person")
    }

    #[tokio::test]
    async fn update_person_happy_path_writes_notes_and_returns_canonical_state() {
        let mcp = setup_test_mcp().await;
        seed_person(&mcp, "Cleo").await;

        let result = mcp
            .update_person(LenientParameters::for_test(UpdatePersonInput {
                name: "Cleo".into(),
                notes: Some("pull from oven before glazing".into()),
                dislikes: None,
                favorites: None,
                drink_preferences: None,
                drink_dislikes: None,
            }))
            .await
            .expect("update_person returns Ok");
        assert_ne!(
            result.is_error,
            Some(true),
            "happy path must not be a tool-level error: {result:?}"
        );
        let body = serde_json::to_string(&result).expect("serializes");
        // The returned payload is `PersonWithPrefs`, so the new notes
        // appear, the unchanged JSON-array fields round-trip, and the
        // drink fields stay populated from the seed.
        assert!(body.contains("pull from oven before glazing"));
        assert!(body.contains("olives"));
        assert!(body.contains("pasta"));
        assert!(body.contains("whiskey"));
        assert!(body.contains("gin"));

        // Re-read via the service to catch any "echoes-but-didn't-write"
        // regression — the tool's response could theoretically come from
        // a successfully-built `PersonWithPrefs` even if the SQL update
        // silently failed.
        let reloaded = PersonService::find_active_by_name(&mcp.db, "Cleo")
            .await
            .expect("lookup succeeds")
            .expect("Cleo still exists");
        assert_eq!(
            reloaded.notes.as_deref(),
            Some("pull from oven before glazing")
        );
    }

    #[tokio::test]
    async fn update_person_partial_update_leaves_other_fields_untouched() {
        let mcp = setup_test_mcp().await;
        seed_person(&mcp, "Cleo").await;

        let result = mcp
            .update_person(LenientParameters::for_test(UpdatePersonInput {
                name: "Cleo".into(),
                notes: None,
                dislikes: None,
                favorites: None,
                drink_preferences: Some(vec!["mezcal".into(), "amaro".into()]),
                drink_dislikes: None,
            }))
            .await
            .expect("update_person returns Ok");
        assert_ne!(result.is_error, Some(true));

        let reloaded = PersonService::find_active_by_name(&mcp.db, "Cleo")
            .await
            .expect("lookup succeeds")
            .expect("Cleo still exists");
        // Touched field updated.
        assert_eq!(
            reloaded.drink_preferences.as_deref(),
            Some("[\"mezcal\",\"amaro\"]")
        );
        // Untouched fields preserved byte-for-byte from the seed.
        assert_eq!(reloaded.notes.as_deref(), Some("original note"));
        assert_eq!(reloaded.dislikes, "[\"olives\"]");
        assert_eq!(reloaded.favorites, "[\"pasta\"]");
        assert_eq!(reloaded.drink_dislikes.as_deref(), Some("[\"gin\"]"));
    }

    #[tokio::test]
    async fn update_person_unknown_name_returns_tool_level_error_not_protocol_error() {
        let mcp = setup_test_mcp().await;
        // Empty DB → find_active_by_name returns None → tool-level error
        // pointing at `list_people` per the cross-tool recovery convention.
        let result = mcp
            .update_person(LenientParameters::for_test(UpdatePersonInput {
                name: "Phantom".into(),
                notes: Some("doesn't matter".into()),
                dislikes: None,
                favorites: None,
                drink_preferences: None,
                drink_dislikes: None,
            }))
            .await;
        assert_tool_user_error(result, &["Phantom", "list_people"]);
    }

    #[tokio::test]
    async fn update_person_is_case_insensitive_and_trims_the_lookup_name() {
        let mcp = setup_test_mcp().await;
        seed_person(&mcp, "Cleo").await;

        // Inherits the trim+lowercase normalization from
        // `PersonService::find_active_by_name`. Verifies the tool does
        // NOT add its own normalization (single source of truth).
        let result = mcp
            .update_person(LenientParameters::for_test(UpdatePersonInput {
                name: "  CLEO  ".into(),
                notes: Some("normalized lookup".into()),
                dislikes: None,
                favorites: None,
                drink_preferences: None,
                drink_dislikes: None,
            }))
            .await
            .expect("update_person returns Ok");
        assert_ne!(result.is_error, Some(true));

        let reloaded = PersonService::find_active_by_name(&mcp.db, "Cleo")
            .await
            .expect("lookup succeeds")
            .expect("Cleo still exists");
        assert_eq!(reloaded.notes.as_deref(), Some("normalized lookup"));
    }

    // ─── LenientParameters extraction layer ─────────────────────────
    //
    // Direct unit tests for `LenientParameters::extract`, the static
    // entry point that `from_context_part` delegates to. Tests the
    // wrapper's deserialize behavior without constructing a full
    // `ToolCallContext` (which is `#[non_exhaustive]` and requires a
    // service reference). A regression that flipped the wrapper from
    // capturing-into-Result to erroring-at-extraction would fail these
    // — the `for_test`-driven integration tests above wouldn't, since
    // they always inject a value and never round-trip through extract.

    fn args_with(pairs: &[(&str, serde_json::Value)]) -> Option<rmcp::model::JsonObject> {
        let mut map = serde_json::Map::new();
        for (k, v) in pairs {
            map.insert((*k).to_string(), v.clone());
        }
        Some(map)
    }

    #[test]
    fn lenient_parameters_extract_missing_required_field_captures_serde_error() {
        // Reproduces the actual rmcp deserialize failure surfaced by live
        // Claude Desktop testing: required `source` omitted on
        // `CreateRecipeInput`. The wrapper must capture this as `Err`
        // (string includes the field name), not error at extraction.
        let args = args_with(&[
            ("name", serde_json::json!("Test Recipe")),
            ("servings", serde_json::json!(4)),
            ("instructions", serde_json::json!("Cook.")),
            ("ingredients", serde_json::json!([])),
        ]);
        let LenientParameters(parsed) = LenientParameters::<CreateRecipeInput>::extract(args);
        let err = parsed.expect_err("missing required field must surface as Err");
        assert!(
            err.contains("source"),
            "error must name the missing field: {err}"
        );
    }

    #[test]
    fn lenient_parameters_extract_well_formed_input_yields_ok() {
        let args = args_with(&[
            ("name", serde_json::json!("Test Recipe")),
            ("source", serde_json::json!("manual")),
            ("servings", serde_json::json!(4)),
            ("instructions", serde_json::json!("Cook.")),
            ("ingredients", serde_json::json!([])),
        ]);
        let LenientParameters(parsed) = LenientParameters::<CreateRecipeInput>::extract(args);
        parsed.expect("well-formed input must yield Ok");
    }

    #[test]
    fn lenient_parameters_extract_none_arguments_treated_as_empty_object() {
        // rmcp delivers `arguments: None` when the LLM sent no params at
        // all. The wrapper must treat that as "empty object", which then
        // fails deserialize for any struct with required fields. (For
        // EmptyParams this would succeed — separately tested via the
        // EmptyParams tools' integration paths.)
        let LenientParameters(parsed) = LenientParameters::<CreateRecipeInput>::extract(None);
        let err = parsed.expect_err("None args → empty object → missing required fields");
        assert!(
            err.contains("name") || err.contains("source") || err.contains("servings"),
            "error must name a missing required field: {err}"
        );
    }

    // ─── Wrapper-routed integration test ────────────────────────────
    //
    // End-to-end: the same JsonObject shape Claude Desktop sent
    // (missing `source`) goes through `extract` → handler → wire. Pins
    // the message-on-the-wire contract that LLM clients depend on.

    #[tokio::test]
    async fn create_recipe_missing_source_returns_tool_level_error() {
        let mcp = setup_test_mcp().await;
        let args = args_with(&[
            ("name", serde_json::json!("Test Recipe")),
            ("servings", serde_json::json!(4)),
            ("instructions", serde_json::json!("Cook.")),
            ("ingredients", serde_json::json!([])),
        ]);
        let params = LenientParameters::<CreateRecipeInput>::extract(args);
        let result = mcp.create_recipe(params).await;
        assert_tool_user_error(result, &["create_recipe", "source"]);
    }

    // ─── Error-helper redaction (fewd-2y6.4) ────────────────────────
    //
    // Pins the contract that protocol-level error helpers must NOT
    // echo internal detail (DbErr Display, formatted internal state,
    // serde error text) onto the wire. Verbose detail belongs in
    // tracing only.

    #[test]
    fn db_error_wire_message_omits_dberr_display_text() {
        let err = sea_orm::DbErr::Custom("schema:column 'people.dislikes'".to_string());
        let mcp_err = db_error(err);
        let wire = mcp_err.message.as_ref();
        assert!(
            !wire.contains("people.dislikes"),
            "wire message must not echo DbErr text: {wire}"
        );
        assert!(
            !wire.contains("schema:"),
            "wire message must not echo schema marker: {wire}"
        );
        assert_eq!(wire, "database error");
        // Pin the second JSON-RPC error channel: `data` must stay None
        // so a future contributor can't reintroduce a leak via that field.
        assert!(mcp_err.data.is_none(), "data must not carry detail");
    }

    #[test]
    fn internal_error_wire_message_omits_caller_supplied_detail() {
        let mcp_err = internal_error("constraint violation on people.email_unique".to_string());
        let wire = mcp_err.message.as_ref();
        assert!(
            !wire.contains("people.email_unique"),
            "wire message must not echo internal detail: {wire}"
        );
        assert!(
            !wire.contains("constraint"),
            "wire message must not echo internal detail: {wire}"
        );
        assert_eq!(wire, "internal server error");
        assert!(mcp_err.data.is_none(), "data must not carry detail");
    }

    // ─── List-result cap (fewd-2y6.5) ───────────────────────────────

    #[test]
    fn enforce_list_cap_passes_through_under_cap() {
        assert!(enforce_list_cap("list_people", 0, "hint").is_none());
        assert!(enforce_list_cap("list_people", MAX_LIST_RESULTS, "hint").is_none());
    }

    #[test]
    fn enforce_list_cap_rejects_over_cap_with_actionable_message() {
        let result = enforce_list_cap(
            "search_recipes",
            MAX_LIST_RESULTS + 1,
            "Add another filter or tighten the existing query.",
        )
        .expect("over-cap must produce a result");
        assert_eq!(
            result.is_error,
            Some(true),
            "must be a tool-level error (is_error: true)"
        );
        let serialized = serde_json::to_string(&result).expect("serializes");
        assert!(
            serialized.contains("search_recipes"),
            "must name the tool: {serialized}"
        );
        assert!(
            serialized.contains(&format!("{}", MAX_LIST_RESULTS + 1)),
            "must echo the actual row count: {serialized}"
        );
        assert!(
            serialized.contains(&format!("{MAX_LIST_RESULTS}")),
            "must echo the cap: {serialized}"
        );
        assert!(
            serialized.contains("Add another filter"),
            "must surface the narrowing hint: {serialized}"
        );
    }

    #[tokio::test]
    async fn list_meals_too_wide_date_range_returns_tool_level_error() {
        // The bead's worst-case scenario: an LLM-hallucinated range that
        // would otherwise fan out into a multi-megabyte response. The
        // input cap fires before the service-layer query runs.
        let mcp = setup_test_mcp().await;
        let result = mcp
            .list_meals(LenientParameters::for_test(DateRangeParams {
                start_date: "0001-01-01".into(),
                end_date: "9999-12-31".into(),
            }))
            .await;
        assert_tool_user_error(result, &["366", "Narrow"]);
    }

    #[test]
    fn tool_json_result_serialize_failure_redacts_serde_text() {
        // serde_json's stock failure modes (NaN, non-string keys, etc.)
        // get coerced to `null`/string in recent versions, so use a
        // custom Serialize that returns Err with identifiable text.
        struct AlwaysFails;
        impl Serialize for AlwaysFails {
            fn serialize<S: serde::Serializer>(&self, _s: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("schema-detail: column 'foo'"))
            }
        }

        let result = tool_json_result(&AlwaysFails);
        let mcp_err = result.expect_err("AlwaysFails must fail to serialize");
        let wire = mcp_err.message.as_ref();
        assert!(
            !wire.contains("schema-detail") && !wire.contains("column"),
            "wire message must not echo serde detail: {wire}"
        );
        assert_eq!(wire, "failed to serialize result");
        assert!(mcp_err.data.is_none(), "data must not carry detail");
    }

    // ─── Tool-description discoverability (fewd-tqx) ────────────────
    //
    // MCP clients like Claude Desktop use embedding-similarity tool_search
    // to load only the top-K matches against the user's query — descriptions
    // act as a retrieval index, not just docs. Lead with task intent
    // ("Check what's scheduled BEFORE…", "Produce the grocery list AFTER…")
    // so the high-signal tokens land where the embedding sees them. Live
    // 2026-05-09 evidence: `list_meals` and `get_shopping_list` failed to
    // rank into Claude Desktop's top-5 because their descriptions led with
    // "Returns…" / "Generate a consolidated…" instead of intent verbs.

    #[test]
    fn every_tool_description_leads_with_intent_verb() {
        // Allowlist is intentionally small: each entry is one tool's
        // first word. Adding a tool means picking a verb from this set
        // (or extending it with intent — never with mechanics like
        // "Return", "Generate", "List", "Fetch").
        const INTENT_VERB_ALLOWLIST: &[&str] = &[
            "Verify",   // whoami
            "Use",      // list_curated_recipes
            "Find",     // search_recipes
            "Read",     // get_recipe
            "Look",     // list_people
            "Record",   // update_person
            "Ground",   // get_family_overview
            "Check",    // list_meals
            "Produce",  // get_shopping_list
            "Add",      // create_recipe
            "Schedule", // create_meal
        ];

        let router = FewdMcp::tool_router();
        let tools = router.list_all();
        assert!(
            !tools.is_empty(),
            "tool_router must expose at least one tool"
        );
        for tool in &tools {
            let description = tool.description.as_deref().unwrap_or("");
            let first_word = description.split_whitespace().next().unwrap_or("");
            assert!(
                INTENT_VERB_ALLOWLIST.contains(&first_word),
                "{}: description must lead with a task-intent verb (one of {:?}), got {first_word:?}. \
                 See fewd-tqx — descriptions act as a retrieval index for MCP tool_search; \
                 leading with mechanics ('Returns…', 'Generates…') sinks the tool's rank in \
                 embedding-similarity selection. Full description: {description:?}",
                tool.name, INTENT_VERB_ALLOWLIST,
            );
        }
    }
}
