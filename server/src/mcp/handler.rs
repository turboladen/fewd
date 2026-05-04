use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rmcp::handler::server::common::FromContextPart;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::handler::server::wrapper::Parameters;
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
    CreateMealError, CreateMealInput, CreateRecipeInput, DateRangeParams, EmptyParams,
    GetRecipeParams, SearchRecipesParams,
};
use super::AuthenticatedPerson;

pub const FAMILY_OVERVIEW_URI: &str = "fewd://family/overview";

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
        description = "Return the name of the authenticated family member. Useful for verifying your MCP bearer-token configuration."
    )]
    async fn whoami(
        &self,
        Parameters(_): Parameters<EmptyParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let name = authenticated_name(&context)?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Hello, {name}. You are authenticated with fewd."
        ))]))
    }

    #[tool(
        name = "list_curated_recipes",
        description = "Return a bounded shortlist (≤30 unless the family has more than 30 favorites — favorites are never truncated) of likely-relevant recipes: every is_favorite first, then most-recently-made, then top-rated, deduped. Use this as the default starting point for meal-planning — it keeps tool payloads small. For everything else use `search_recipes` with at least one filter; the full archive is intentionally not exposed (the web UI is for human browsing)."
    )]
    async fn list_curated_recipes(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<CallToolResult, McpError> {
        let recipes = RecipeService::get_curated(&self.db)
            .await
            .map_err(db_error)?;
        let out = recipes
            .iter()
            .map(recipe_to_brief)
            .collect::<Result<Vec<_>, _>>()
            .map_err(internal_error)?;
        tool_json_result(&out)
    }

    #[tool(
        name = "search_recipes",
        description = "Search recipes by one or more filters. Bare calls (no filters / `query='*'`) are rejected — call `list_curated_recipes` for an unfiltered shortlist. Filters: `query` (case-insensitive substring on name); `tags` (case-insensitive exact match, multiple tags = AND); `max_total_time_minutes` (assumes recipe total_time is in minutes — recipes authored in hours won't match, known limitation); `min_rating`; `is_favorite`; `unmade_since_days`; `excludes_for_persons` (named family members whose dislikes exclude matching recipes — substring match on ingredient names, e.g. 'olive oil' is excluded when a person dislikes 'olive'). Returns brief rows — use `get_recipe` with the slug for full details. Unknown person names return an actionable error pointing at `list_people`.",
        input_schema = rmcp::handler::server::common::schema_for_type::<SearchRecipesParams>()
    )]
    async fn search_recipes(
        &self,
        LenientParameters(params): LenientParameters<SearchRecipesParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = match params {
            Ok(v) => v,
            Err(e) => {
                return Ok(tool_user_error(format!(
                    "search_recipes: {e}. Check the tool schema for required fields."
                )))
            }
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
            Err(DislikeResolveError::Internal(err)) => return Err(err),
        };

        let filters = SearchFilters {
            query: params.normalized_query(),
            tags: params.normalized_tags(),
            max_total_time_minutes: params.max_total_time_minutes,
            min_rating: params.min_rating,
            is_favorite: params.is_favorite,
            unmade_since_days: params.unmade_since_days,
            excluded_ingredient_substrings,
        };

        let recipes = RecipeService::search_filtered(&self.db, filters)
            .await
            .map_err(db_error)?;
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

        let lookups = MealLookups::load(&self.db)
            .await
            .map_err(|e| DislikeResolveError::Internal(db_error(e)))?;
        let people = PersonService::get_all(&self.db)
            .await
            .map_err(|e| DislikeResolveError::Internal(db_error(e)))?;
        let people_by_id: HashMap<&str, &person::Model> =
            people.iter().map(|p| (p.id.as_str(), p)).collect();

        flatten_disliked_substrings(names, &lookups, &people_by_id)
    }

    #[tool(
        name = "get_recipe",
        description = "Fetch the full record for one recipe by slug: ingredients (with amounts and units), instructions, nutrition, prep/cook time, and any parent recipe it was adapted from.",
        input_schema = rmcp::handler::server::common::schema_for_type::<GetRecipeParams>()
    )]
    async fn get_recipe(
        &self,
        LenientParameters(params): LenientParameters<GetRecipeParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = match params {
            Ok(v) => v,
            Err(e) => {
                return Ok(tool_user_error(format!(
                    "get_recipe: {e}. Check the tool schema for required fields."
                )))
            }
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
        description = "List all active family members with their dietary goals, dislikes, favorites, and notes. Use their `name` (case-insensitive) as the identifier when creating meals."
    )]
    async fn list_people(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<CallToolResult, McpError> {
        let people = PersonService::get_all(&self.db).await.map_err(db_error)?;
        let out = people
            .iter()
            .map(person_to_prefs)
            .collect::<Result<Vec<_>, _>>()
            .map_err(internal_error)?;
        tool_json_result(&out)
    }

    /// Tool mirror of the `fewd://family/overview` resource. Resources in
    /// MCP are expected to be surfaced by the host for user-side attachment
    /// (paperclip UI) and are not addressable by the LLM on its own, so a
    /// tool is the only way to let Claude read the overview autonomously.
    #[tool(
        name = "get_family_overview",
        description = "Return a human-readable Markdown overview of every active family member — dietary goals, dislikes, favorites, notes — in a single block. Use this to ground meal-planning replies without calling list_people and stitching fields together. Equivalent to the fewd://family/overview resource."
    )]
    async fn get_family_overview(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<CallToolResult, McpError> {
        let people = PersonService::get_all(&self.db).await.map_err(db_error)?;
        let markdown = render_family_overview(&people).map_err(internal_error)?;
        Ok(CallToolResult::success(vec![Content::text(markdown)]))
    }

    #[tool(
        name = "list_meals",
        description = "List all scheduled meals within an inclusive date range. Each meal lists the assigned servings — who's eating which recipe (or ad-hoc items), how many servings, and optional notes.",
        input_schema = rmcp::handler::server::common::schema_for_type::<DateRangeParams>()
    )]
    async fn list_meals(
        &self,
        LenientParameters(params): LenientParameters<DateRangeParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = match params {
            Ok(v) => v,
            Err(e) => {
                return Ok(tool_user_error(format!(
                    "list_meals: {e}. Check the tool schema for required fields."
                )))
            }
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
        description = "Generate a consolidated grocery list for the given date range: ingredients are aggregated across meals and scaled by person-servings, with unit conversion where compatible. Each item shows the per-meal sources so the user can trace back which recipe contributed what.",
        input_schema = rmcp::handler::server::common::schema_for_type::<DateRangeParams>()
    )]
    async fn get_shopping_list(
        &self,
        LenientParameters(params): LenientParameters<DateRangeParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = match params {
            Ok(v) => v,
            Err(e) => {
                return Ok(tool_user_error(format!(
                    "get_shopping_list: {e}. Check the tool schema for required fields."
                )))
            }
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
        description = "Create a new recipe. The slug is auto-generated from the name (with a numeric suffix on collisions). Returns the full created recipe. Before calling this, prefer `search_recipes` to avoid duplicates — the LLM should check whether a similar recipe already exists.",
        input_schema = rmcp::handler::server::common::schema_for_type::<CreateRecipeInput>()
    )]
    async fn create_recipe(
        &self,
        LenientParameters(input): LenientParameters<CreateRecipeInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = match input {
            Ok(v) => v,
            Err(e) => {
                return Ok(tool_user_error(format!(
                    "create_recipe: {e}. Check the tool schema for required fields."
                )))
            }
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
        description = "Schedule a meal on a specific date. Each serving assigns one family member to either an existing recipe (by slug) or an ad-hoc ingredient list. Unknown names or slugs return a clear error so the caller can retry with corrected values. Returns the created meal with slugs/names resolved.",
        input_schema = rmcp::handler::server::common::schema_for_type::<CreateMealInput>()
    )]
    async fn create_meal(
        &self,
        LenientParameters(input): LenientParameters<CreateMealInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = match input {
            Ok(v) => v,
            Err(e) => {
                return Ok(tool_user_error(format!(
                    "create_meal: {e}. Check the tool schema for required fields."
                )))
            }
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
                "fewd MCP: plan dinners and generate shopping lists. \
                 Start with `get_family_overview` (or the fewd://family/overview resource) \
                 to see everyone's diets/dislikes, then `list_curated_recipes` for the \
                 family's likely-relevant shortlist — or `search_recipes` with filters \
                 (tags, max time, min rating, excludes_for_persons, …) when you need \
                 something specific. Use `create_recipe` to add a new one. \
                 Schedule meals with `create_meal` (one call per dinner slot, by date and \
                 family-member name). When the week's planned, `get_shopping_list` over \
                 the date range produces the consolidated grocery list. All date inputs \
                 are YYYY-MM-DD.",
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

fn authenticated_name(context: &RequestContext<RoleServer>) -> Result<String, McpError> {
    let parts = context
        .extensions
        .get::<axum::http::request::Parts>()
        .ok_or_else(|| McpError::internal_error("missing http request parts", None))?;
    let person = parts
        .extensions
        .get::<AuthenticatedPerson>()
        .ok_or_else(|| McpError::internal_error("missing authenticated person", None))?;
    Ok(person.0.name.clone())
}

fn tool_json_result<T: Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(value).map_err(|err| {
        tracing::error!(?err, "MCP tool: failed to serialize result");
        McpError::internal_error(format!("failed to serialize result: {err}"), None)
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn db_error(err: sea_orm::DbErr) -> McpError {
    tracing::error!(?err, "MCP tool: database error");
    McpError::internal_error(format!("database error: {err}"), None)
}

fn internal_error(msg: String) -> McpError {
    tracing::error!(%msg, "MCP tool: internal error");
    McpError::internal_error(msg, None)
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

/// Parameter wrapper that defers deserialize errors to the handler so they
/// can be returned as tool-level errors instead of bubbling up as JSON-RPC
/// `-32602` ("invalid_params") responses. rmcp's stock `Parameters<T>`
/// extractor errors at extraction time, which most MCP clients render as a
/// generic "Tool execution failed" with the message dropped — the same UX
/// class fewd-m95 fixed for handler-side errors. `LenientParameters<T>`
/// always succeeds at extraction and exposes a `Result<T, String>` for the
/// handler to inspect; on `Err` the handler returns
/// `Ok(tool_user_error("…"))` so the actionable serde message reaches the
/// LLM.
///
/// The rmcp `#[tool]` macro only generates input schemas from a literal
/// `Parameters<T>` ident in the function signature, so every method using
/// `LenientParameters<T>` must specify
/// `input_schema = rmcp::handler::server::common::schema_for_type::<T>()`
/// in its `#[tool(...)]` attribute to keep the LLM-facing JSON Schema
/// intact. Tools that take `EmptyParams` don't need this wrapper — there
/// are no fields to omit, so deserialize can't fail.
pub(super) struct LenientParameters<T>(pub Result<T, String>);

impl<S, T> FromContextPart<ToolCallContext<'_, S>> for LenientParameters<T>
where
    T: DeserializeOwned,
{
    fn from_context_part(context: &mut ToolCallContext<S>) -> Result<Self, McpError> {
        let arguments = context.arguments.take().unwrap_or_default();
        let parsed = serde_json::from_value::<T>(serde_json::Value::Object(arguments))
            .map_err(|e| e.to_string());
        Ok(LenientParameters(parsed))
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
    /// error). Surface as `McpError` so it logs server-side.
    Internal(McpError),
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
            DislikeResolveError::Internal(internal_error(format!(
                "person id '{id}' resolved by MealLookups is no longer active; retry the tool call"
            )))
        })?;
        let dislikes: Vec<String> = serde_json::from_str(&person.dislikes).map_err(|err| {
            DislikeResolveError::Internal(internal_error(format!(
                "person '{}' has malformed dislikes JSON: {err}",
                person.name
            )))
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
        let DislikeResolveError::Internal(mcp_err) = err else {
            panic!("expected Internal, got {err:?}");
        };
        let msg = format!("{mcp_err:?}");
        assert!(msg.contains("Broken"), "error must name the person: {msg}");
        assert!(
            msg.contains("malformed dislikes JSON"),
            "error must describe the failure mode: {msg}"
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
        let DislikeResolveError::Internal(mcp_err) = err else {
            panic!("expected Internal, got {err:?}");
        };
        let msg = format!("{mcp_err:?}");
        assert!(
            msg.contains("p1"),
            "error must include the orphan id: {msg}"
        );
        assert!(
            msg.contains("retry"),
            "error must hint that a retry is appropriate: {msg}"
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
            .get_recipe(LenientParameters(Ok(GetRecipeParams {
                slug: "ghost-pasta".into(),
            })))
            .await;
        assert_tool_user_error(result, &["ghost-pasta", "list_curated_recipes"]);
    }

    #[tokio::test]
    async fn list_meals_invalid_date_returns_tool_level_error() {
        let mcp = setup_test_mcp().await;
        let result = mcp
            .list_meals(LenientParameters(Ok(DateRangeParams {
                start_date: "garbage".into(),
                end_date: "2026-01-01".into(),
            })))
            .await;
        assert_tool_user_error(result, &["start_date", "YYYY-MM-DD", "garbage"]);
    }

    #[tokio::test]
    async fn get_shopping_list_invalid_date_returns_tool_level_error() {
        let mcp = setup_test_mcp().await;
        let result = mcp
            .get_shopping_list(LenientParameters(Ok(DateRangeParams {
                start_date: "2026-01-01".into(),
                end_date: "garbage".into(),
            })))
            .await;
        assert_tool_user_error(result, &["end_date", "YYYY-MM-DD", "garbage"]);
    }

    #[tokio::test]
    async fn create_meal_unknown_meal_type_returns_tool_level_error() {
        let mcp = setup_test_mcp().await;
        let result = mcp
            .create_meal(LenientParameters(Ok(CreateMealInput {
                date: "2026-01-01".into(),
                meal_type: "brunch".into(),
                order_index: None,
                servings: vec![],
            })))
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
        let result = mcp.create_recipe(LenientParameters(Ok(input))).await;
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
        let result = mcp.create_recipe(LenientParameters(Ok(input))).await;
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
            .list_meals(LenientParameters(Ok(DateRangeParams {
                start_date: "2026-04-30".into(),
                end_date: "2026-04-26".into(),
            })))
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
        let result = mcp.create_meal(LenientParameters(Ok(input))).await;
        assert_tool_user_error(result, &["Bob", "list_people"]);
    }

    #[tokio::test]
    async fn create_recipe_missing_source_returns_tool_level_error() {
        let mcp = setup_test_mcp().await;
        // Reproduces the actual rmcp deserialize failure surfaced by Claude
        // Desktop testing of PR #27: the LLM omitted the required `source`
        // field. Without LenientParameters this would fail upstream of the
        // handler with JSON-RPC -32602, rendered as the opaque "Tool
        // execution failed" the LLM can't recover from. With
        // LenientParameters, the deserialize error reaches the handler and
        // becomes a tool-level error naming the missing field.
        let bad_json = serde_json::json!({
            "name": "Test Recipe",
            // missing "source"
            "servings": 4,
            "instructions": "Cook.",
            "ingredients": []
        });
        let parsed: Result<CreateRecipeInput, String> =
            serde_json::from_value(bad_json).map_err(|e| e.to_string());
        let result = mcp.create_recipe(LenientParameters(parsed)).await;
        assert_tool_user_error(result, &["create_recipe", "source"]);
    }
}
