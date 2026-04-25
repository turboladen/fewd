use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    AnnotateAble, CallToolResult, Content, Implementation, ListResourcesResult,
    PaginatedRequestParams, RawResource, ReadResourceRequestParams, ReadResourceResult, Resource,
    ResourceContents, ResourcesCapability, ServerCapabilities, ServerInfo, ToolsCapability,
};
use rmcp::service::RequestContext;
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler};
use sea_orm::DatabaseConnection;
use serde::Serialize;

use crate::services::meal_service::MealService;
use crate::services::person_service::PersonService;
use crate::services::recipe_service::RecipeService;
use crate::services::shopping_service::ShoppingService;

use super::lookups::MealLookups;
use super::schemas::{
    create_meal_input_to_dto, create_recipe_input_to_dto, meal_to_brief, person_to_prefs,
    recipe_to_brief, recipe_to_full, render_family_overview, shopping_item_from_dto,
    CreateMealInput, CreateRecipeInput, DateRangeParams, EmptyParams, GetRecipeParams,
    SearchParams,
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
        name = "list_recipes",
        description = "List every recipe in fewd (brief shape: slug, name, tags, total time, rating, etc.). Ingredients and instructions are omitted — use `get_recipe` with the slug to fetch the full record."
    )]
    async fn list_recipes(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<CallToolResult, McpError> {
        let recipes = RecipeService::get_all(&self.db).await.map_err(db_error)?;
        let out = recipes
            .iter()
            .map(recipe_to_brief)
            .collect::<Result<Vec<_>, _>>()
            .map_err(internal_error)?;
        tool_json_result(&out)
    }

    #[tool(
        name = "search_recipes",
        description = "Search recipes by case-insensitive substring on the recipe name. Returns brief rows — use `get_recipe` with the slug for full details."
    )]
    async fn search_recipes(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let recipes = RecipeService::search(&self.db, params.query)
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
        name = "get_recipe",
        description = "Fetch the full record for one recipe by slug: ingredients (with amounts and units), instructions, nutrition, prep/cook time, and any parent recipe it was adapted from."
    )]
    async fn get_recipe(
        &self,
        Parameters(params): Parameters<GetRecipeParams>,
    ) -> Result<CallToolResult, McpError> {
        let normalized = params.slug.trim().to_lowercase();
        let recipe = RecipeService::get_by_slug(&self.db, normalized.clone())
            .await
            .map_err(db_error)?
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!(
                        "no recipe with slug '{}'. Call search_recipes or list_recipes to find valid slugs.",
                        params.slug
                    ),
                    None,
                )
            })?;

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
        description = "List all scheduled meals within an inclusive date range. Each meal lists the assigned servings — who's eating which recipe (or ad-hoc items), how many servings, and optional notes."
    )]
    async fn list_meals(
        &self,
        Parameters(params): Parameters<DateRangeParams>,
    ) -> Result<CallToolResult, McpError> {
        params
            .validate()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

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
        description = "Generate a consolidated grocery list for the given date range: ingredients are aggregated across meals and scaled by person-servings, with unit conversion where compatible. Each item shows the per-meal sources so the user can trace back which recipe contributed what."
    )]
    async fn get_shopping_list(
        &self,
        Parameters(params): Parameters<DateRangeParams>,
    ) -> Result<CallToolResult, McpError> {
        params
            .validate()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let list = ShoppingService::get_shopping_list(&self.db, params.start_date, params.end_date)
            .await
            .map_err(db_error)?;
        let out: Vec<_> = list.into_iter().map(shopping_item_from_dto).collect();
        tool_json_result(&out)
    }

    #[tool(
        name = "create_recipe",
        description = "Create a new recipe. The slug is auto-generated from the name (with a numeric suffix on collisions). Returns the full created recipe. Before calling this, prefer `search_recipes` to avoid duplicates — the LLM should check whether a similar recipe already exists."
    )]
    async fn create_recipe(
        &self,
        Parameters(input): Parameters<CreateRecipeInput>,
    ) -> Result<CallToolResult, McpError> {
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
                    .map_err(db_error)?
                    .ok_or_else(|| {
                        McpError::invalid_params(
                            format!(
                                "parent_recipe_slug '{slug}' does not exist. Omit it or use a valid slug from search_recipes."
                            ),
                            None,
                        )
                    })?;
                Some((parent.id, parent.slug))
            }
        };
        let (parent_recipe_id, parent_slug_canonical) = match parent_resolution {
            Some((id, slug)) => (Some(id), Some(slug)),
            None => (None, None),
        };

        let dto = create_recipe_input_to_dto(input, parent_recipe_id)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        let created = RecipeService::create(&self.db, dto)
            .await
            .map_err(db_error)?;
        let full = recipe_to_full(&created, parent_slug_canonical).map_err(internal_error)?;
        tool_json_result(&full)
    }

    #[tool(
        name = "create_meal",
        description = "Schedule a meal on a specific date. Each serving assigns one family member to either an existing recipe (by slug) or an ad-hoc ingredient list. Unknown names or slugs return a clear error so the caller can retry with corrected values. Returns the created meal with slugs/names resolved."
    )]
    async fn create_meal(
        &self,
        Parameters(input): Parameters<CreateMealInput>,
    ) -> Result<CallToolResult, McpError> {
        let lookups = MealLookups::load(&self.db).await.map_err(db_error)?;
        let dto = create_meal_input_to_dto(input, &lookups)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

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
                 to see everyone's diets/dislikes, then `list_recipes` or `search_recipes` \
                 to find existing recipes — or `create_recipe` to add a new one. \
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
