//! Recipe-related MCP input/output types and conversion helpers.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::dto::{
    CreateRecipeDto, IngredientDto, NutritionDto, PortionSizeDto, TimeValueDto, UpdateRecipeDto,
};
use crate::entities::recipe;
use crate::services::recipe_variant::{
    IngredientChange, IngredientMatch, InstructionChange, InstructionEdit, PrepFilter, VariantSpec,
};
use crate::services::service_error::whole_star_rating;

use super::common::{
    blank_to_none, format_date, ingredient_in, ingredient_out, nutrition_in, nutrition_out,
    parse_json, parse_optional_json, portion_in, portion_out, time_in, time_out, IngredientOut,
    NutritionOut, PortionSizeOut, TimeOut,
};
use super::errors::InputError;
use super::McpToolInput;

/// Trimmed recipe shape for list/search. Omits ingredients/instructions to
/// keep tool payloads small — use `get_recipe` for the full record.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RecipeBrief {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub icon: Option<String>,
    pub servings: i32,
    pub total_time: Option<TimeOut>,
    pub times_planned: i32,
    pub last_planned: Option<String>,
    pub rating: Option<f64>,
    pub is_favorite: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RecipeFull {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub source_url: Option<String>,
    /// Slug of the recipe this was adapted from, if any.
    pub parent_recipe_slug: Option<String>,
    pub prep_time: Option<TimeOut>,
    pub cook_time: Option<TimeOut>,
    pub total_time: Option<TimeOut>,
    pub servings: i32,
    pub portion_size: Option<PortionSizeOut>,
    pub instructions: String,
    pub ingredients: Vec<IngredientOut>,
    pub nutrition_per_serving: Option<NutritionOut>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub is_favorite: bool,
    pub times_planned: i32,
    pub last_planned: Option<String>,
    pub rating: Option<f64>,
}

/// Input for `search_recipes`. Every filter is optional, but at least one
/// must be provided — call [`SearchRecipesParams::validate_has_filter`]
/// before building the service-layer query. Bare / wildcard calls are
/// rejected with a pointer at `list_curated_recipes`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchRecipesParams {
    /// Case-insensitive substring on the recipe name. Empty string and `*`
    /// are treated as no-query (and don't count as a filter on their own).
    #[serde(default)]
    pub query: Option<String>,
    /// Tag membership (case-insensitive exact match). Multiple tags compose
    /// as AND — recipe must have every listed tag.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Maximum recipe total time in minutes. The recipe's `total_time` is
    /// normalized to minutes regardless of its authored unit (minutes, hours,
    /// or days), so an hour-authored recipe matches correctly. Recipes with no
    /// total time, or whose stored total uses an unrecognized unit, are
    /// excluded.
    #[serde(default)]
    pub max_total_time_minutes: Option<i32>,
    /// Minimum star rating. Recipes with no rating are excluded.
    #[serde(default)]
    pub min_rating: Option<f64>,
    /// If true, only is_favorite recipes; if false, only non-favorites.
    #[serde(default)]
    pub is_favorite: Option<bool>,
    /// Recipes not planned in at least N days (or never planned).
    #[serde(default)]
    pub unplanned_since_days: Option<i32>,
    /// Exclude recipes that contain ingredients any of these family members
    /// dislikes. Each named person's `dislikes` are matched as
    /// case-insensitive substrings against ingredient names — e.g. "olive
    /// oil" is excluded when a person dislikes "olive". Plan around this
    /// when the substring is genuinely shared between an avoided and
    /// acceptable ingredient. Unknown names return an actionable error
    /// pointing at `list_people`.
    #[serde(default)]
    pub excludes_for_persons: Option<Vec<String>>,
    /// Restrict results to recipes that contain ALL of these substrings in
    /// some ingredient name (case-insensitive substring match). Multiple
    /// values AND together — `["spam","cheese"]` means "recipes with both
    /// spam AND cheese," possibly in different ingredients. Composes with
    /// every other filter. Mirror of `excludes_for_persons` (which removes
    /// ingredients); use this when the user names an ingredient they want
    /// to USE. "olive" matches "olive oil" and "pitted olives" alike —
    /// this is lower-stakes than the exclude side (extra hits are easier
    /// to mentally filter than missing hits).
    #[serde(default)]
    pub includes_ingredient_substrings: Option<Vec<String>>,
}

impl McpToolInput for SearchRecipesParams {}

impl SearchRecipesParams {
    /// Reject the all-empty / wildcard-only case. The full archive is
    /// intentionally not exposed via this tool — for an unfiltered shortlist
    /// the LLM should call `list_curated_recipes`.
    ///
    /// Validation is based on the *normalized* form of each filter so the
    /// outcome is consistent with what the service actually receives. E.g.,
    /// `tags: Some(vec![""])` is non-empty as a Vec but `normalized_tags()`
    /// drops the empty entry, leaving an effectively-empty filter set; this
    /// validator rejects the bare-empty-string case here so the LLM gets the
    /// "needs a filter" hint instead of `RecipeService::search_filtered`'s
    /// internal "caller must validate" error.
    pub fn validate_has_filter(&self) -> Result<(), &'static str> {
        let q_provides_filter = self
            .query
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "*")
            .is_some();
        let tags_provides_filter = self
            .tags
            .as_ref()
            .is_some_and(|v| v.iter().any(|t| !t.trim().is_empty()));
        let excludes_provides_filter = self
            .excludes_for_persons
            .as_ref()
            .is_some_and(|v| v.iter().any(|n| !n.trim().is_empty()));
        let includes_provides_filter = self
            .includes_ingredient_substrings
            .as_ref()
            .is_some_and(|v| v.iter().any(|s| !s.trim().is_empty()));

        if q_provides_filter
            || tags_provides_filter
            || excludes_provides_filter
            || includes_provides_filter
            || self.max_total_time_minutes.is_some()
            || self.min_rating.is_some()
            || self.is_favorite.is_some()
            || self.unplanned_since_days.is_some()
        {
            Ok(())
        } else {
            Err("search_recipes requires at least one filter \
                 (query, tags, max_total_time_minutes, min_rating, is_favorite, \
                 unplanned_since_days, excludes_for_persons, or \
                 includes_ingredient_substrings). \
                 For an unfiltered shortlist call list_curated_recipes.")
        }
    }

    /// Trim the query to a meaningful filter substring or `None`. Strips
    /// whitespace and treats `*` / empty as no-query.
    pub fn normalized_query(&self) -> Option<String> {
        self.query
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "*")
            .map(str::to_string)
    }

    /// Lowercased, whitespace-trimmed tags with empties dropped.
    pub fn normalized_tags(&self) -> Vec<String> {
        self.tags
            .as_ref()
            .map(|v| {
                v.iter()
                    .map(|t| t.trim().to_lowercase())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Lowercased, whitespace-trimmed include substrings — empties dropped,
    /// duplicates removed. The service-layer SQL only lowercases the
    /// ingredient (haystack), so the caller must pre-lowercase substrings.
    pub fn normalized_included_substrings(&self) -> Vec<String> {
        let Some(v) = self.includes_ingredient_substrings.as_ref() else {
            return Vec::new();
        };
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut out: Vec<String> = Vec::new();
        for s in v {
            let normalized = s.trim().to_lowercase();
            if !normalized.is_empty() && seen.insert(normalized.clone()) {
                out.push(normalized);
            }
        }
        out
    }
}

/// Input for `create_recipe`. Mirrors [`CreateRecipeDto`] but replaces
/// `parent_recipe_id` with a slug reference the LLM can actually produce.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRecipeInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Where the recipe came from — e.g. "manual", "claude-chat", "allrecipes.com".
    pub source: String,
    #[serde(default)]
    pub source_url: Option<String>,
    /// Slug of the recipe this was adapted from, if any.
    #[serde(default)]
    pub parent_recipe_slug: Option<String>,
    /// Hands-on time. The `unit` must be minutes, hours, or days (singular,
    /// plural, or the `min` / `hr` / `d` abbreviations); any other unit
    /// rejects the call.
    #[serde(default)]
    pub prep_time: Option<TimeOut>,
    /// Time on the heat. Same `unit` vocabulary as `prep_time`.
    #[serde(default)]
    pub cook_time: Option<TimeOut>,
    /// Omit to store `prep_time` + `cook_time`. Send it when the real total
    /// differs, such as when it includes resting time or overlapping steps.
    #[serde(default)]
    pub total_time: Option<TimeOut>,
    /// Servings the recipe is authored for (e.g. 4). Per-person scaling
    /// happens later at meal-assignment time.
    pub servings: i32,
    #[serde(default)]
    pub portion_size: Option<PortionSizeOut>,
    /// Full preparation instructions. Markdown is fine.
    pub instructions: String,
    pub ingredients: Vec<IngredientOut>,
    #[serde(default)]
    pub nutrition_per_serving: Option<NutritionOut>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Optional emoji / icon character to display next to the recipe.
    #[serde(default)]
    pub icon: Option<String>,
}

// The recipe write inputs share these redirects. Each names where the field
// can be set, never the tool that rejected it, since tools share types.
const RATING_REDIRECT: (&str, &str) = (
    "rating",
    "Call rate_recipe to set a rating, or unrate_recipe to clear it.",
);
const IS_FAVORITE_REDIRECT: (&str, &str) = ("is_favorite", "Call favorite_recipe to set it.");
const DERIVED_FROM_MEALS: &str =
    "The server derives it from meals scheduled with create_meal; omit it.";
const TIMES_PLANNED_REDIRECT: (&str, &str) = ("times_planned", DERIVED_FROM_MEALS);
const LAST_PLANNED_REDIRECT: (&str, &str) = ("last_planned", DERIVED_FROM_MEALS);

impl McpToolInput for CreateRecipeInput {
    const FIELD_REDIRECTS: &'static [(&'static str, &'static str)] = &[
        RATING_REDIRECT,
        IS_FAVORITE_REDIRECT,
        TIMES_PLANNED_REDIRECT,
        LAST_PLANNED_REDIRECT,
        (
            "slug",
            "The server generates the slug from `name`; omit it.",
        ),
    ];
}

pub fn recipe_to_brief(recipe: &recipe::Model) -> Result<RecipeBrief, String> {
    let tags: Vec<String> = parse_json(&recipe.tags, "recipe tags")?;
    let total_time: Option<TimeValueDto> =
        parse_optional_json(recipe.total_time.as_deref(), "recipe total_time")?;
    Ok(RecipeBrief {
        slug: recipe.slug.clone(),
        name: recipe.name.clone(),
        description: recipe.description.clone(),
        tags,
        icon: recipe.icon.clone(),
        servings: recipe.servings,
        total_time: total_time.map(time_out),
        times_planned: recipe.times_planned,
        last_planned: recipe.last_planned.map(format_date),
        rating: recipe.rating,
        is_favorite: recipe.is_favorite,
    })
}

pub fn recipe_to_full(
    recipe: &recipe::Model,
    parent_slug: Option<String>,
) -> Result<RecipeFull, String> {
    let tags: Vec<String> = parse_json(&recipe.tags, "recipe tags")?;
    let ingredients: Vec<IngredientDto> = parse_json(&recipe.ingredients, "recipe ingredients")?;
    let prep_time: Option<TimeValueDto> =
        parse_optional_json(recipe.prep_time.as_deref(), "recipe prep_time")?;
    let cook_time: Option<TimeValueDto> =
        parse_optional_json(recipe.cook_time.as_deref(), "recipe cook_time")?;
    let total_time: Option<TimeValueDto> =
        parse_optional_json(recipe.total_time.as_deref(), "recipe total_time")?;
    let portion_size: Option<PortionSizeDto> =
        parse_optional_json(recipe.portion_size.as_deref(), "recipe portion_size")?;
    let nutrition: Option<NutritionDto> = parse_optional_json(
        recipe.nutrition_per_serving.as_deref(),
        "recipe nutrition_per_serving",
    )?;

    Ok(RecipeFull {
        slug: recipe.slug.clone(),
        name: recipe.name.clone(),
        description: recipe.description.clone(),
        source: recipe.source.clone(),
        source_url: recipe.source_url.clone(),
        parent_recipe_slug: parent_slug,
        prep_time: prep_time.map(time_out),
        cook_time: cook_time.map(time_out),
        total_time: total_time.map(time_out),
        servings: recipe.servings,
        portion_size: portion_size.map(portion_out),
        instructions: recipe.instructions.clone(),
        ingredients: ingredients.iter().map(ingredient_out).collect(),
        nutrition_per_serving: nutrition.map(nutrition_out),
        tags,
        notes: recipe.notes.clone(),
        icon: recipe.icon.clone(),
        is_favorite: recipe.is_favorite,
        times_planned: recipe.times_planned,
        last_planned: recipe.last_planned.map(format_date),
        rating: recipe.rating,
    })
}

/// Input for `import_recipe_url`. The `url` field is typed as [`url::Url`] so
/// malformed URLs fail at deserialization (LenientParameters routes that to a
/// tool_user_error). The handler additionally rejects non-http(s) schemes for a
/// clearer error than the downstream SSRF guard would produce.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportRecipeUrlInput {
    /// Public http(s) URL of the recipe page. The server fetches it, extracts
    /// schema.org/Recipe data (JSON-LD first, html2text fallback), parses the
    /// result with Claude into the same shape as `create_recipe`, and persists.
    pub url: url::Url,
}

impl McpToolInput for ImportRecipeUrlInput {}

/// Input for the `favorite_recipe` MCP tool. `slug` identifies the row and
/// is never written; `is_favorite` is the only column this tool touches.
///
/// `is_favorite` is set absolutely rather than toggled, so the caller never
/// has to know the current state and repeating a call leaves the recipe in
/// the same state.
//
// Every `///` on the input structs below ships to the LLM verbatim as
// that tool's input-schema `description`, so keep rustdoc links and
// internal identifiers out of them.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FavoriteRecipeInput {
    /// Slug of the recipe to favorite or unfavorite (case-insensitive).
    /// Call `search_recipes` or `get_recipe` first to find it.
    pub slug: String,
    /// `true` favorites the recipe, `false` unfavorites it. This is an
    /// absolute set, not a toggle: you never need to read the current
    /// state first, and sending the same value twice leaves the recipe in
    /// the same state.
    pub is_favorite: bool,
}

impl McpToolInput for FavoriteRecipeInput {}

/// Input for the `rate_recipe` MCP tool. `slug` identifies the row and is
/// never written; `rating` is the only column this tool touches.
///
/// Ratings are whole stars. A fractional value rounds to the nearest whole
/// number and the rounded value is what gets stored, so the returned row is
/// the authority on what was recorded. A value that does not round into
/// 1–5 is rejected rather than clamped.
//
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RateRecipeInput {
    /// Slug of the recipe to rate (case-insensitive). Call
    /// `search_recipes` or `get_recipe` first to find it.
    pub slug: String,
    /// Star rating, a whole number from 1 to 5. A fractional value rounds
    /// to the nearest whole star. There is no rating that means "no
    /// rating" — call `unrate_recipe` to remove one.
    #[schemars(range(min = 1, max = 5))]
    pub rating: f64,
}

impl McpToolInput for RateRecipeInput {}

/// Input for the `unrate_recipe` MCP tool. Removing a rating is not the
/// same as rating 1 star: `search_recipes`'s `min_rating` filter excludes
/// unrated recipes entirely, so a cleared recipe drops out of every
/// rating-filtered search rather than ranking at the bottom.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnrateRecipeInput {
    /// Slug of the recipe whose rating to remove (case-insensitive). Call
    /// `search_recipes` or `get_recipe` first to find it.
    pub slug: String,
}

impl McpToolInput for UnrateRecipeInput {}

/// Input for the `update_recipe` MCP tool. `slug` identifies the row to
/// update and is never written — renaming leaves the slug alone, so the
/// same slug keeps addressing the recipe afterwards.
///
/// Every other field is optional with PATCH semantics: an omitted field —
/// or an explicit JSON `null` — leaves the column unchanged.
///
/// **Clear semantics differ by field shape**:
///
/// - The string fields (`name`, `description`, `instructions`, `notes`,
///   `icon`) cannot be blanked. An empty or whitespace-only value means
///   "leave unchanged"; a blank `name` is rejected outright.
/// - The list fields (`ingredients`, `tags`) and the structured blocks
///   (`nutrition_per_serving`, the three time fields, `portion_size`)
///   REPLACE the stored value whole — they are never merged. A partial
///   `ingredients` array therefore deletes every ingredient it omits, and
///   a `nutrition_per_serving` carrying only `calories` nulls the other
///   four fields. Passing `[]` clears a list.
///
/// `is_favorite`, `rating`, `source`, `source_url`, the parent recipe, and
/// the slug are not writable here. Use `favorite_recipe` to set
/// `is_favorite`, and `rate_recipe` to set `rating` or `unrate_recipe` to
/// clear it.
//
// The blank-string coercion runs through `blank_to_none`, which carries
// the invariant it protects. A blank `name` instead mirrors
// `create_recipe_input_to_dto`'s rejection, so create and update agree on
// what a valid recipe name is.
//
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateRecipeInput {
    /// Slug of the recipe to update (case-insensitive). Call
    /// `search_recipes` or `get_recipe` first to find it. This is a lookup
    /// key, not a write — it is never changed, including by a rename.
    pub slug: String,
    /// New display name. Renaming does NOT change the slug.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Hands-on time. The `unit` must be minutes, hours, or days (singular,
    /// plural, or the `min` / `hr` / `d` abbreviations) and is stored in its
    /// plural form. Any other unit rejects the whole call, writing nothing.
    /// A value equal to the stored duration is ignored.
    #[serde(default)]
    pub prep_time: Option<TimeOut>,
    /// Time on the heat. Same `unit` vocabulary as `prep_time`.
    #[serde(default)]
    pub cook_time: Option<TimeOut>,
    /// Sets a new total time; omit it otherwise. Changing `prep_time` or
    /// `cook_time` shifts the stored total by the same amount, so resting or
    /// marinating time survives without this field. A phase the recipe had
    /// no value for is assumed to fit inside the stored total and does not
    /// move it. A total adjusted this way never drops below the longer of
    /// the two phases.
    /// A value equal to the stored total counts as unchanged and does not
    /// stop that adjustment.
    /// When a stored time has a unit outside minutes, hours, or days, the
    /// total is left as stored instead, so send this field too.
    /// This is the duration `search_recipes`' `max_total_time_minutes`
    /// filter compares, and it takes the same `unit` vocabulary as
    /// `prep_time`.
    #[serde(default)]
    pub total_time: Option<TimeOut>,
    /// Servings the recipe is authored for. Must be at least 1. Changing
    /// this does NOT rescale `ingredients`: the shopping list divides the
    /// stored amounts by this number, so a genuine resize has to send a
    /// rescaled `ingredients` array in the same call.
    #[serde(default)]
    pub servings: Option<i32>,
    /// How big one serving is, as a `{value, unit}` pair — e.g.
    /// `{"value": 1.5, "unit": "cup"}`. Replaces the stored pair whole.
    #[serde(default)]
    pub portion_size: Option<PortionSizeOut>,
    /// Replaces the full instruction text rather than appending to it.
    /// A blank value is ignored, so there is no way to erase the steps.
    #[serde(default)]
    pub instructions: Option<String>,
    /// Replaces the whole ingredient list — this is not a merge. Omitted
    /// ingredients are deleted, so read the current list with `get_recipe`
    /// and send it back complete. `[]` clears the list.
    #[serde(default)]
    pub ingredients: Option<Vec<IngredientOut>>,
    /// Replaces the whole nutrition block. Every field inside it is
    /// independently optional, so sending `{"calories": 500}` alone nulls
    /// protein, carbs, fat, and notes.
    #[serde(default)]
    pub nutrition_per_serving: Option<NutritionOut>,
    /// Replaces the whole tag list. `[]` clears it.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Emoji / icon character displayed next to the recipe.
    #[serde(default)]
    pub icon: Option<String>,
}

impl McpToolInput for UpdateRecipeInput {
    const FIELD_REDIRECTS: &'static [(&'static str, &'static str)] = &[
        RATING_REDIRECT,
        IS_FAVORITE_REDIRECT,
        TIMES_PLANNED_REDIRECT,
        LAST_PLANNED_REDIRECT,
    ];
}

pub fn create_recipe_input_to_dto(
    input: CreateRecipeInput,
    parent_recipe_id: Option<String>,
) -> Result<CreateRecipeDto, InputError> {
    if input.name.trim().is_empty() {
        return Err(InputError::EmptyName("name"));
    }
    if input.servings < 1 {
        return Err(InputError::NonPositiveServings(input.servings));
    }

    Ok(CreateRecipeDto {
        name: input.name,
        description: input.description,
        source: input.source,
        source_url: input.source_url,
        parent_recipe_id,
        prep_time: input.prep_time.map(time_in),
        cook_time: input.cook_time.map(time_in),
        total_time: input.total_time.map(time_in),
        servings: input.servings,
        portion_size: input.portion_size.map(portion_in),
        instructions: input.instructions,
        ingredients: input.ingredients.into_iter().map(ingredient_in).collect(),
        nutrition_per_serving: input.nutrition_per_serving.map(nutrition_in),
        tags: input.tags,
        notes: input.notes,
        icon: input.icon,
    })
}

/// Input for the `adapt_recipe` MCP tool, which saves a changed copy of an
/// existing recipe as a new recipe linked to it. The parent recipe is never
/// modified.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdaptRecipeInput {
    /// Slug of the recipe to adapt (case-insensitive). Call `search_recipes`
    /// or `get_recipe` first to find it.
    pub parent_recipe_slug: String,
    /// Display name for the new variant, such as "Gnocchi with Hot Italian
    /// Sausage". The variant's slug is generated from it.
    pub name: String,
    /// Replaces the parent's description on the variant. Omit it, or send a
    /// blank value, to keep the parent's.
    #[serde(default)]
    pub description: Option<String>,
    /// Ingredient edits, applied in order. Each one sees the list as the
    /// earlier edits left it.
    #[serde(default)]
    pub ingredient_changes: Option<Vec<IngredientChangeInput>>,
    /// Find/replace edits to the parent's instructions, applied in order to
    /// the text the earlier edits left. Cannot be combined with
    /// `instructions`.
    #[serde(default)]
    pub instruction_edits: Option<Vec<InstructionEditInput>>,
    /// Full replacement for the parent's instructions. Markdown is fine.
    /// Cannot be combined with `instruction_edits`.
    #[serde(default)]
    pub instructions: Option<String>,
    /// Replaces the parent's tag list on the variant. Omit it, or send only
    /// blank tags, to keep the parent's tags; send `[]` to clear them.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Why this variant exists, such as "Made spicier for Tuesday". Omit it,
    /// or send a blank value, to keep the parent's notes.
    #[serde(default)]
    pub notes: Option<String>,
}

/// One edit to the parent's ingredient list, chosen by `op`.
//
// Every variant is a struct variant: schemars gives a newtype variant of an
// internally tagged enum no `additionalProperties: false`, so its unknown
// fields would go unreported in the published schema.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum IngredientChangeInput {
    /// Appends `ingredient` to the end of the list.
    Add { ingredient: IngredientOut },
    /// Swaps the ingredient matching `name` for `with`, keeping its position.
    Replace {
        /// Whole name of the ingredient to replace, as `get_recipe` shows it
        /// (case-insensitive).
        name: String,
        /// Prep of the ingredient to replace, needed only when several
        /// ingredients share `name`. Omit it to match any prep, or send ""
        /// to match only the one with no prep.
        #[serde(default)]
        prep: Option<String>,
        /// The ingredient that takes its place.
        #[serde(rename = "with")]
        replacement: IngredientOut,
    },
    /// Removes the ingredient matching `name`.
    Remove {
        /// Whole name of the ingredient to remove, as `get_recipe` shows it
        /// (case-insensitive).
        name: String,
        /// Prep of the ingredient to remove, needed only when several
        /// ingredients share `name`. Omit it to match any prep, or send ""
        /// to match only the one with no prep.
        #[serde(default)]
        prep: Option<String>,
    },
}

/// One find/replace edit to the parent's instructions.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstructionEditInput {
    /// Exact text to replace. It must occur exactly once in the instructions,
    /// matching whitespace and punctuation.
    pub find: String,
    /// Text to put in its place. An empty string deletes the found text.
    pub replace: String,
}

const PARENT_SLUG_REDIRECT: &str =
    "Name the recipe to adapt with parent_recipe_slug; the variant's slug is generated from name.";
const INSTRUCTION_EDITS_REDIRECT: &str =
    "Send find/replace pairs in instruction_edits, or the full text in instructions.";
const INHERITED_FROM_PARENT: &str =
    "The variant inherits it from the parent recipe; change it afterwards with update_recipe on the variant's slug.";
const VARIANT_SOURCE_REDIRECT: &str =
    "A variant's source is always ai_adapted, and parent_recipe_slug records where it came from; omit it.";

impl McpToolInput for AdaptRecipeInput {
    const FIELD_REDIRECTS: &'static [(&'static str, &'static str)] = &[
        RATING_REDIRECT,
        IS_FAVORITE_REDIRECT,
        TIMES_PLANNED_REDIRECT,
        LAST_PLANNED_REDIRECT,
        ("slug", PARENT_SLUG_REDIRECT),
        ("parent_slug", PARENT_SLUG_REDIRECT),
        ("recipe_slug", PARENT_SLUG_REDIRECT),
        (
            "ingredients",
            "Describe ingredient edits as add, replace, or remove ops in ingredient_changes. To supply a whole ingredient list, call create_recipe with parent_recipe_slug instead.",
        ),
        ("instruction_changes", INSTRUCTION_EDITS_REDIRECT),
        ("edits", INSTRUCTION_EDITS_REDIRECT),
        ("instruction_ops", INSTRUCTION_EDITS_REDIRECT),
        ("servings", INHERITED_FROM_PARENT),
        ("prep_time", INHERITED_FROM_PARENT),
        ("cook_time", INHERITED_FROM_PARENT),
        ("total_time", INHERITED_FROM_PARENT),
        ("portion_size", INHERITED_FROM_PARENT),
        ("icon", INHERITED_FROM_PARENT),
        ("nutrition_per_serving", INHERITED_FROM_PARENT),
        ("source", VARIANT_SOURCE_REDIRECT),
        ("source_url", VARIANT_SOURCE_REDIRECT),
    ];
}

/// Translate `AdaptRecipeInput` into the service-layer `VariantSpec`,
/// rejecting input no variant can be built from. The caller resolves
/// `parent_recipe_slug` itself, so nothing here reads it.
///
/// A blank `description` or `notes`, or a `tags` list of only blank entries,
/// means "keep the parent's", while `tags: []` clears them. The call must
/// carry at least one ingredient or instruction change, and cannot send both
/// `instructions` and `instruction_edits`.
pub fn adapt_recipe_input_to_spec(input: AdaptRecipeInput) -> Result<VariantSpec, InputError> {
    // An explicit `null` list means the same as an omitted one.
    let ingredient_changes = input.ingredient_changes.unwrap_or_default();
    let instruction_edits = input.instruction_edits.unwrap_or_default();
    if input.name.trim().is_empty() {
        return Err(InputError::EmptyName("name"));
    }
    if input.instructions.is_some() && !instruction_edits.is_empty() {
        return Err(InputError::ConflictingInstructionChanges);
    }
    if input
        .instructions
        .as_deref()
        .is_some_and(|text| text.trim().is_empty())
    {
        return Err(InputError::EmptyName("instructions"));
    }
    if ingredient_changes.is_empty() && input.instructions.is_none() && instruction_edits.is_empty()
    {
        return Err(InputError::NoAdaptationChanges);
    }

    let ingredient_changes = ingredient_changes
        .into_iter()
        .enumerate()
        .map(|(position, change)| ingredient_change_in(position + 1, change))
        .collect::<Result<Vec<_>, _>>()?;
    // Create drops blank tags, so a list of only blanks would silently clear
    // the parent's tags. Only an explicit `[]` clears them.
    let tags = input
        .tags
        .filter(|tags| tags.is_empty() || tags.iter().any(|tag| !tag.trim().is_empty()));
    // A `find` is matched character for character, so a whitespace-only one is
    // a real edit. An empty one is left to `build_variant_dto`, which rejects
    // it by edit number.
    let instruction_change = match input.instructions {
        Some(text) => Some(InstructionChange::Replace(text)),
        None if instruction_edits.is_empty() => None,
        None => Some(InstructionChange::Edits(
            instruction_edits
                .into_iter()
                .map(|edit| InstructionEdit {
                    find: edit.find,
                    replace: edit.replace,
                })
                .collect(),
        )),
    };

    Ok(VariantSpec {
        name: input.name,
        description: blank_to_none(input.description),
        ingredient_changes,
        instruction_change,
        tags,
        notes: blank_to_none(input.notes),
    })
}

// Added and replacement ingredients run through `ingredient_in`, so a comma'd
// name is split into name and prep exactly as on create. Match targets pass
// through untouched: the service tries the whole string before any split.
//
// An omitted `prep` and an explicit blank one mean different things, which is
// why `prep` is not routed through `blank_to_none`: omitted accepts any prep,
// while "" picks out the ingredient that has none.
fn ingredient_change_in(
    number: usize,
    change: IngredientChangeInput,
) -> Result<IngredientChange, InputError> {
    let target = |name: String, prep: Option<String>| {
        if name.trim().is_empty() {
            return Err(InputError::EmptyIngredientChangeName(number));
        }
        let prep = match prep {
            None => PrepFilter::Any,
            Some(prep) if prep.trim().is_empty() => PrepFilter::NoPrep,
            Some(prep) => PrepFilter::Exactly(prep),
        };
        Ok(IngredientMatch { name, prep })
    };
    Ok(match change {
        IngredientChangeInput::Add { ingredient } => {
            IngredientChange::Add(ingredient_in(ingredient))
        }
        IngredientChangeInput::Replace {
            name,
            prep,
            replacement,
        } => IngredientChange::Replace {
            target: target(name, prep)?,
            replacement: ingredient_in(replacement),
        },
        IngredientChangeInput::Remove { name, prep } => IngredientChange::Remove {
            target: target(name, prep)?,
        },
    })
}

/// Translate `UpdateRecipeInput` into the `UpdateRecipeDto` the service
/// layer accepts. The caller resolves the row from `slug` before calling,
/// so nothing here writes it.
///
/// See [`UpdateRecipeInput`]'s docstring for the clear semantics this
/// enforces.
pub fn update_recipe_input_to_dto(input: UpdateRecipeInput) -> Result<UpdateRecipeDto, InputError> {
    // `name` and `servings` get exactly the checks
    // `create_recipe_input_to_dto` applies, so a value create would reject
    // can't reach the row through update instead.
    if input.name.as_deref().is_some_and(|n| n.trim().is_empty()) {
        return Err(InputError::EmptyName("name"));
    }
    if let Some(servings) = input.servings {
        if servings < 1 {
            return Err(InputError::NonPositiveServings(servings));
        }
    }

    // The remaining string fields have no create-side rule to mirror, so
    // they take the `update_person` treatment instead: empty means "no
    // change", never "clear". `blank_to_none` carries the rationale.
    //
    // Written as an explicit literal rather than `..Default::default()`:
    // a field added to UpdateRecipeDto later must be categorized here
    // instead of silently defaulting to "leave unchanged".
    Ok(UpdateRecipeDto {
        name: input.name,
        description: blank_to_none(input.description),
        prep_time: input.prep_time.map(time_in),
        cook_time: input.cook_time.map(time_in),
        total_time: input.total_time.map(time_in),
        servings: input.servings,
        portion_size: input.portion_size.map(portion_in),
        instructions: blank_to_none(input.instructions),
        // `ingredient_in` runs the comma'd-name splitter, exactly as on
        // create — skipping it would let "garlic, minced" land as one
        // ingredient name and fragment shopping aggregation.
        ingredients: input
            .ingredients
            .map(|v| v.into_iter().map(ingredient_in).collect()),
        nutrition_per_serving: input.nutrition_per_serving.map(nutrition_in),
        tags: input.tags,
        notes: blank_to_none(input.notes),
        icon: blank_to_none(input.icon),
        // Owned by fewd-3w3 (favorite_recipe) and fewd-no0 (rate_recipe).
        is_favorite: None,
        rating: None,
    })
}

/// Translate `FavoriteRecipeInput` into a one-column `UpdateRecipeDto`.
/// The caller resolves the row from `slug` before calling, so nothing here
/// writes it.
//
// The field literal is explicit for the reason given in
// `update_recipe_input_to_dto`.
//
// `RecipeService::update` sets `is_favorite` directly, so this needs no
// service-side helper — it is an ordinary partial update that happens to
// carry exactly one field.
pub fn favorite_recipe_input_to_dto(input: FavoriteRecipeInput) -> UpdateRecipeDto {
    UpdateRecipeDto {
        name: None,
        description: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        servings: None,
        portion_size: None,
        instructions: None,
        ingredients: None,
        nutrition_per_serving: None,
        tags: None,
        notes: None,
        icon: None,
        is_favorite: Some(input.is_favorite),
        rating: None,
    }
}

/// Translate `RateRecipeInput` into a one-column `UpdateRecipeDto`,
/// rejecting a rating that does not round into 1–5.
///
/// The caller resolves the row from `slug` before calling, so nothing here
/// writes it.
//
// Calls the service's own `whole_star_rating`, so this tool accepts exactly
// the ratings the web UI does. The service would reject the value too, but
// only this `InputError` can point a caller who sent 0 at `unrate_recipe`.
//
// The field literal is explicit for the reason given in
// `update_recipe_input_to_dto`.
pub fn rate_recipe_input_to_dto(input: RateRecipeInput) -> Result<UpdateRecipeDto, InputError> {
    let rounded =
        whole_star_rating(input.rating).map_err(|_| InputError::RatingOutOfRange(input.rating))?;

    Ok(UpdateRecipeDto {
        name: None,
        description: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        servings: None,
        portion_size: None,
        instructions: None,
        ingredients: None,
        nutrition_per_serving: None,
        tags: None,
        notes: None,
        icon: None,
        is_favorite: None,
        rating: Some(rounded),
    })
}

#[cfg(test)]
mod tests {
    use super::super::common::IngredientAmountOut;
    use super::*;

    // ─── adapt_recipe_input_to_spec ─────────────────────────────────

    // Build an input from the minimal valid call with `overrides` merged over
    // it; a `null` override removes that key.
    fn adapt_spec(overrides: serde_json::Value) -> Result<VariantSpec, InputError> {
        let mut args = serde_json::json!({
            "parent_recipe_slug": "potato-gnocchi",
            "name": "Spicy Gnocchi",
            "ingredient_changes": [{"op": "remove", "name": "sage"}],
        });
        let object = args.as_object_mut().expect("the base is an object");
        for (key, value) in overrides.as_object().expect("overrides are an object") {
            if value.is_null() {
                object.remove(key);
            } else {
                object.insert(key.clone(), value.clone());
            }
        }
        let input: AdaptRecipeInput =
            serde_json::from_value(args).expect("AdaptRecipeInput JSON shape");
        adapt_recipe_input_to_spec(input)
    }

    #[test]
    fn adapt_rejects_a_blank_name() {
        let err = adapt_spec(serde_json::json!({"name": "  "})).unwrap_err();
        assert!(matches!(err, InputError::EmptyName("name")), "{err:?}");
    }

    #[test]
    fn adapt_rejects_both_instruction_forms() {
        let err = adapt_spec(serde_json::json!({
            "instructions": "Just cook it.",
            "instruction_edits": [{"find": "sage", "replace": "basil"}],
        }))
        .unwrap_err();
        assert!(
            matches!(err, InputError::ConflictingInstructionChanges),
            "{err:?}"
        );
        let message = err.to_string();
        assert!(message.contains("not both"), "{message}");
    }

    #[test]
    fn adapt_rejects_blank_instructions() {
        let err = adapt_spec(serde_json::json!({"instructions": " \n "})).unwrap_err();
        assert!(
            matches!(err, InputError::EmptyName("instructions")),
            "{err:?}"
        );
    }

    #[test]
    fn adapt_rejects_a_blank_change_target_by_number() {
        let valid = serde_json::json!({"op": "remove", "name": "sage"});
        for blank in [
            serde_json::json!({"op": "remove", "name": " "}),
            serde_json::json!({"op": "replace", "name": "", "with":
                {"name": "basil", "amount": {"kind": "single", "value": 1.0}}}),
        ] {
            let overrides = serde_json::json!({"ingredient_changes": [valid.clone(), blank]});
            let err = adapt_spec(overrides.clone()).unwrap_err();
            assert!(
                matches!(err, InputError::EmptyIngredientChangeName(2)),
                "{overrides}: {err:?}"
            );
            let message = err.to_string();
            assert!(message.starts_with("ingredient change #2:"), "{message}");
        }
    }

    #[test]
    fn adapt_passes_a_whitespace_only_find_through_as_exact_text() {
        let spec = adapt_spec(serde_json::json!({
            "ingredient_changes": null,
            "instruction_edits": [{"find": "  ", "replace": " "}],
        }))
        .expect("a whitespace-only find is an edit");
        let Some(InstructionChange::Edits(edits)) = spec.instruction_change else {
            panic!("expected edits, got {:?}", spec.instruction_change);
        };
        assert_eq!(edits[0].find, "  ");
    }

    #[test]
    fn adapt_treats_null_change_lists_as_omitted() {
        let input: AdaptRecipeInput = serde_json::from_value(serde_json::json!({
            "parent_recipe_slug": "potato-gnocchi",
            "name": "Spicy Gnocchi",
            "ingredient_changes": null,
            "instruction_edits": null,
            "instructions": "Just cook it.",
        }))
        .expect("null change lists deserialize");
        let spec = adapt_recipe_input_to_spec(input).expect("instructions alone are a change");
        assert!(spec.ingredient_changes.is_empty());
        assert!(
            matches!(spec.instruction_change, Some(InstructionChange::Replace(_))),
            "{:?}",
            spec.instruction_change
        );
    }

    #[test]
    fn adapt_requires_an_ingredient_or_instruction_change() {
        for overrides in [
            serde_json::json!({"ingredient_changes": null}),
            serde_json::json!({"ingredient_changes": [], "instruction_edits": []}),
            serde_json::json!({"ingredient_changes": null, "tags": ["spicy"]}),
            serde_json::json!({"ingredient_changes": null, "notes": "Tuesday"}),
            serde_json::json!({"ingredient_changes": null, "description": "Hotter"}),
        ] {
            let err = adapt_spec(overrides.clone()).unwrap_err();
            assert!(
                matches!(err, InputError::NoAdaptationChanges),
                "{overrides}: {err:?}"
            );
            let message = err.to_string();
            assert!(message.contains("update_recipe"), "{message}");
            assert!(message.contains("create_recipe"), "{message}");
        }
    }

    #[test]
    fn adapt_instruction_change_alone_is_enough() {
        let spec = adapt_spec(serde_json::json!({
            "ingredient_changes": null,
            "instruction_edits": [{"find": "sage", "replace": "basil"}],
        }))
        .expect("an instruction edit is a change");
        let Some(InstructionChange::Edits(edits)) = spec.instruction_change else {
            panic!("expected edits, got {:?}", spec.instruction_change);
        };
        assert_eq!(edits[0].find, "sage");
        assert_eq!(edits[0].replace, "basil");
    }

    #[test]
    fn adapt_blank_description_and_notes_inherit_the_parent() {
        let spec = adapt_spec(serde_json::json!({"description": "  ", "notes": ""}))
            .expect("blank overrides are not an error");
        assert!(spec.description.is_none());
        assert!(spec.notes.is_none());
    }

    #[test]
    fn adapt_all_blank_tags_inherit_while_an_empty_list_clears() {
        let blanks = adapt_spec(serde_json::json!({"tags": ["", "   "]})).expect("valid input");
        assert!(
            blanks.tags.is_none(),
            "only blanks must inherit: {:?}",
            blanks.tags
        );

        let cleared = adapt_spec(serde_json::json!({"tags": []})).expect("valid input");
        assert_eq!(cleared.tags, Some(vec![]), "an explicit [] clears the tags");

        let mixed = adapt_spec(serde_json::json!({"tags": ["", "spicy"]})).expect("valid input");
        assert_eq!(mixed.tags, Some(vec!["".to_string(), "spicy".to_string()]));
    }

    #[test]
    fn adapt_omitted_blank_and_given_prep_map_to_distinct_filters() {
        let spec = adapt_spec(serde_json::json!({"ingredient_changes": [
            {"op": "remove", "name": "butter"},
            {"op": "remove", "name": "butter", "prep": ""},
            {"op": "remove", "name": "butter", "prep": null},
            {"op": "remove", "name": "butter", "prep": "softened"},
        ]}))
        .expect("valid changes");
        let filters: Vec<&PrepFilter> = spec
            .ingredient_changes
            .iter()
            .map(|change| match change {
                IngredientChange::Remove { target } => &target.prep,
                other => panic!("expected remove, got {other:?}"),
            })
            .collect();
        assert_eq!(
            filters,
            [
                &PrepFilter::Any,
                &PrepFilter::NoPrep,
                &PrepFilter::Any,
                &PrepFilter::Exactly("softened".into()),
            ]
        );
    }

    #[test]
    fn adapt_splits_comma_names_in_added_and_replacement_ingredients() {
        let spec = adapt_spec(serde_json::json!({"ingredient_changes": [
            {"op": "add", "ingredient":
                {"name": "garlic, minced", "amount": {"kind": "single", "value": 2.0}, "unit": "clove"}},
            {"op": "replace", "name": "sage", "prep": "fresh", "with":
                {"name": "basil, torn", "amount": {"kind": "range", "min": 6.0, "max": 8.0}, "unit": "leaf"}},
        ]}))
        .expect("valid changes");

        let IngredientChange::Add(added) = &spec.ingredient_changes[0] else {
            panic!("expected add, got {:?}", spec.ingredient_changes[0]);
        };
        assert_eq!(added.name, "garlic");
        assert_eq!(added.prep.as_deref(), Some("minced"));

        let IngredientChange::Replace {
            target,
            replacement,
        } = &spec.ingredient_changes[1]
        else {
            panic!("expected replace, got {:?}", spec.ingredient_changes[1]);
        };
        assert_eq!(target.name, "sage");
        assert_eq!(target.prep, PrepFilter::Exactly("fresh".into()));
        assert_eq!(replacement.name, "basil");
        assert_eq!(replacement.prep.as_deref(), Some("torn"));
    }

    fn mk_input(name: &str, servings: i32) -> CreateRecipeInput {
        CreateRecipeInput {
            name: name.into(),
            description: None,
            source: "manual".into(),
            source_url: None,
            parent_recipe_slug: None,
            prep_time: None,
            cook_time: None,
            total_time: None,
            servings,
            portion_size: None,
            instructions: String::new(),
            ingredients: vec![],
            nutrition_per_serving: None,
            tags: vec![],
            notes: None,
            icon: None,
        }
    }

    fn mk_update(slug: &str) -> UpdateRecipeInput {
        UpdateRecipeInput {
            slug: slug.into(),
            ..Default::default()
        }
    }

    #[test]
    fn update_rejects_zero_servings() {
        let err = update_recipe_input_to_dto(UpdateRecipeInput {
            servings: Some(0),
            ..mk_update("x")
        })
        .unwrap_err();
        assert!(format!("{err}").contains("servings"));
    }

    #[test]
    fn update_rejects_negative_servings() {
        let err = update_recipe_input_to_dto(UpdateRecipeInput {
            servings: Some(-1),
            ..mk_update("x")
        })
        .unwrap_err();
        assert!(format!("{err}").contains("servings"));
    }

    #[test]
    fn update_rejects_whitespace_name() {
        let err = update_recipe_input_to_dto(UpdateRecipeInput {
            name: Some("   ".into()),
            ..mk_update("x")
        })
        .unwrap_err();
        assert!(format!("{err}").contains("name"));
    }

    #[test]
    fn update_blank_strings_mean_no_change() {
        // The invariant: no recipe scalar has a clear-to-empty path. A
        // caller sending "" must not persist an empty string as a
        // back-door clear.
        for blank in ["", "   ", "\t\n"] {
            let dto = update_recipe_input_to_dto(UpdateRecipeInput {
                description: Some(blank.into()),
                instructions: Some(blank.into()),
                notes: Some(blank.into()),
                icon: Some(blank.into()),
                ..mk_update("x")
            })
            .expect("blank strings are not an error");
            assert!(dto.description.is_none(), "description, blank {blank:?}");
            assert!(dto.instructions.is_none(), "instructions, blank {blank:?}");
            assert!(dto.notes.is_none(), "notes, blank {blank:?}");
            assert!(dto.icon.is_none(), "icon, blank {blank:?}");
        }
    }

    #[test]
    fn update_empty_lists_survive_as_writes() {
        // `[]` is a legitimate write that clears the list, distinct from
        // omitting the field. Coalescing it to None would silently drop
        // the caller's intent.
        let dto = update_recipe_input_to_dto(UpdateRecipeInput {
            tags: Some(vec![]),
            ingredients: Some(vec![]),
            ..mk_update("x")
        })
        .expect("empty lists are valid");
        assert!(dto.tags.is_some_and(|v| v.is_empty()));
        assert!(dto.ingredients.is_some_and(|v| v.is_empty()));
    }

    #[test]
    fn update_never_writes_favorite_or_rating() {
        let dto = update_recipe_input_to_dto(UpdateRecipeInput {
            name: Some("Renamed".into()),
            servings: Some(6),
            ..mk_update("x")
        })
        .expect("valid input");
        assert!(dto.is_favorite.is_none());
        assert!(dto.rating.is_none());
    }

    // Name the fields a converter actually writes. `UpdateRecipeDto`
    // serializes every field, so reading the keys back rather than
    // listing them means a field added later is covered here without
    // anyone remembering to extend an assertion list.
    fn written_fields(dto: &UpdateRecipeDto) -> Vec<String> {
        serde_json::to_value(dto)
            .expect("UpdateRecipeDto serializes")
            .as_object()
            .expect("a DTO serializes to an object")
            .iter()
            .filter(|(_, value)| !value.is_null())
            .map(|(key, _)| key.clone())
            .collect()
    }

    #[test]
    fn favorite_input_writes_only_is_favorite() {
        // Pins the explicit-literal converter: every column except
        // `is_favorite` must stay "leave unchanged". Tidying the literal
        // back to `..Default::default()` still passes this, but a future
        // UpdateRecipeDto field whose Default is a real value would not.
        for value in [true, false] {
            let dto = favorite_recipe_input_to_dto(FavoriteRecipeInput {
                slug: "x".into(),
                is_favorite: value,
            });
            assert_eq!(dto.is_favorite, Some(value));
            assert_eq!(written_fields(&dto), ["is_favorite"], "is_favorite {value}");
        }
    }

    fn mk_rate(rating: f64) -> RateRecipeInput {
        RateRecipeInput {
            slug: "x".into(),
            rating,
        }
    }

    #[test]
    fn rate_accepts_whole_stars_one_through_five() {
        for stars in [1.0, 2.0, 3.0, 4.0, 5.0] {
            let dto = rate_recipe_input_to_dto(mk_rate(stars))
                .unwrap_or_else(|e| panic!("{stars} stars must be accepted: {e}"));
            assert_eq!(dto.rating, Some(stars));
        }
    }

    #[test]
    fn rate_rounds_at_the_accept_boundaries() {
        // `f64::round` is half-away-from-zero, which puts the accept window
        // on the raw input at 0.5 <= x < 5.5 — not the 1.0..=5.0 the field
        // documentation suggests. Both edges are pinned because both are
        // reachable from an ordinary utterance ("four and a half stars").
        for (raw, stored) in [
            (0.5, 1.0),
            (0.6, 1.0),
            (1.5, 2.0),
            (2.5, 3.0),
            (4.4, 4.0),
            (4.5, 5.0),
            (4.6, 5.0),
            (5.4, 5.0),
            (5.49, 5.0),
        ] {
            let dto = rate_recipe_input_to_dto(mk_rate(raw))
                .unwrap_or_else(|e| panic!("{raw} must be accepted: {e}"));
            assert_eq!(dto.rating, Some(stored), "{raw} must store as {stored}");
        }
    }

    #[test]
    fn rate_rejects_outside_the_accept_window() {
        for raw in [
            0.0,
            0.49,
            5.5,
            5.6,
            6.0,
            -1.0,
            -0.5,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            let Err(err) = rate_recipe_input_to_dto(mk_rate(raw)) else {
                panic!("{raw} must be rejected");
            };
            let err = err.to_string();
            assert!(err.contains("1 to 5"), "{raw}: {err}");
            assert!(
                err.contains("unrate_recipe"),
                "{raw} must point at the clear path: {err}"
            );
        }
    }

    #[test]
    fn rate_error_reports_the_callers_raw_value() {
        // The check runs on the rounded value, so reporting that instead
        // would tell the caller they sent 6 when they sent 5.6.
        let err = rate_recipe_input_to_dto(mk_rate(5.6))
            .expect_err("5.6 rounds to 6 and must be rejected")
            .to_string();
        assert!(err.contains("5.6"), "must quote the value as sent: {err}");
    }

    #[test]
    fn rate_input_writes_only_rating() {
        let dto = rate_recipe_input_to_dto(mk_rate(4.0)).expect("valid rating");
        assert_eq!(dto.rating, Some(4.0));
        assert_eq!(written_fields(&dto), ["rating"]);
    }

    #[test]
    fn update_normalizes_comma_ingredient_name() {
        // Proves the converter routes ingredients through `ingredient_in`
        // rather than mapping them straight across; a comma'd name that
        // skipped the splitter would fragment shopping aggregation.
        let dto = update_recipe_input_to_dto(UpdateRecipeInput {
            ingredients: Some(vec![IngredientOut {
                name: "garlic, minced".into(),
                prep: None,
                amount: IngredientAmountOut::Single { value: 2.0 },
                unit: "clove".into(),
                notes: None,
                or_alternative: None,
            }]),
            ..mk_update("x")
        })
        .expect("valid input");
        let ingredients = dto.ingredients.expect("ingredients written");
        assert_eq!(ingredients[0].name, "garlic");
        assert_eq!(ingredients[0].prep.as_deref(), Some("minced"));
    }

    #[test]
    fn rejects_zero_servings() {
        let err = create_recipe_input_to_dto(mk_input("X", 0), None).unwrap_err();
        assert!(format!("{err}").contains("servings"));
    }

    #[test]
    fn rejects_negative_servings() {
        let err = create_recipe_input_to_dto(mk_input("X", -1), None).unwrap_err();
        assert!(format!("{err}").contains("servings"));
    }

    #[test]
    fn rejects_whitespace_name() {
        let err = create_recipe_input_to_dto(mk_input("   ", 4), None).unwrap_err();
        assert!(format!("{err}").contains("name"));
    }

    #[test]
    fn accepts_minimal_valid_input() {
        let dto = create_recipe_input_to_dto(mk_input("Tacos", 4), None).unwrap();
        assert_eq!(dto.name, "Tacos");
        assert_eq!(dto.servings, 4);
    }

    #[test]
    fn search_params_validate_rejects_all_empty() {
        let p = SearchRecipesParams::default();
        let err = p.validate_has_filter().unwrap_err();
        assert!(err.contains("list_curated_recipes"));
    }

    #[test]
    fn search_params_validate_rejects_wildcard_only_query() {
        let p = SearchRecipesParams {
            query: Some("*".into()),
            ..Default::default()
        };
        assert!(p.validate_has_filter().is_err());
    }

    #[test]
    fn search_params_validate_rejects_whitespace_only_query_and_empty_lists() {
        let p = SearchRecipesParams {
            query: Some("   ".into()),
            tags: Some(vec![]),
            excludes_for_persons: Some(vec![]),
            ..Default::default()
        };
        assert!(p.validate_has_filter().is_err());
    }

    #[test]
    fn search_params_validate_rejects_only_empty_string_tags() {
        // Regression: previously `tags: Some(vec![""])` passed
        // validate_has_filter (Vec is non-empty) but normalized_tags() drops
        // the empty entry — leaving the service with no actual filter and
        // emitting a misleading "caller must validate" error. The validator
        // should now reject based on the normalized form.
        let p = SearchRecipesParams {
            tags: Some(vec!["".into(), "   ".into()]),
            ..Default::default()
        };
        assert!(p.validate_has_filter().is_err());
    }

    #[test]
    fn search_params_validate_rejects_only_empty_string_excludes_for_persons() {
        let p = SearchRecipesParams {
            excludes_for_persons: Some(vec!["".into(), "  ".into()]),
            ..Default::default()
        };
        assert!(p.validate_has_filter().is_err());
    }

    #[test]
    fn search_params_validate_rejects_only_empty_string_includes_ingredient_substrings() {
        let p = SearchRecipesParams {
            includes_ingredient_substrings: Some(vec!["".into(), "  ".into()]),
            ..Default::default()
        };
        assert!(p.validate_has_filter().is_err());
    }

    #[test]
    fn search_params_validate_accepts_only_includes_ingredient_substrings() {
        // Bead-required: a bare call with only this filter must pass —
        // it's the whole reason the filter exists.
        let p = SearchRecipesParams {
            includes_ingredient_substrings: Some(vec!["spam".into()]),
            ..Default::default()
        };
        assert!(p.validate_has_filter().is_ok());
    }

    #[test]
    fn search_params_validate_error_lists_includes_ingredient_substrings() {
        // Regression: the actionable error must enumerate every filter
        // the tool accepts, otherwise the LLM can't recover by adding
        // the new filter from the error message alone.
        let err = SearchRecipesParams::default()
            .validate_has_filter()
            .unwrap_err();
        assert!(
            err.contains("includes_ingredient_substrings"),
            "error must list the new filter: {err}"
        );
    }

    #[test]
    fn search_params_normalized_included_substrings_lowercases_trims_and_dedupes() {
        let p = SearchRecipesParams {
            includes_ingredient_substrings: Some(vec![
                "Spam".into(),
                "  CHEESE  ".into(),
                "spam".into(), // duplicate after normalization
                "".into(),
                "   ".into(),
            ]),
            ..Default::default()
        };
        assert_eq!(p.normalized_included_substrings(), vec!["spam", "cheese"]);
    }

    #[test]
    fn search_params_normalized_included_substrings_none_yields_empty() {
        let p = SearchRecipesParams::default();
        assert!(p.normalized_included_substrings().is_empty());
    }

    #[test]
    fn search_params_validate_accepts_tags_with_one_real_entry_among_empties() {
        // `["", "dinner"]` should pass — the empty string is dropped by
        // normalize, but "dinner" survives and is a real filter.
        let p = SearchRecipesParams {
            tags: Some(vec!["".into(), "dinner".into()]),
            ..Default::default()
        };
        assert!(p.validate_has_filter().is_ok());
    }

    #[test]
    fn search_params_validate_accepts_non_query_filter() {
        // is_favorite=true alone is enough to count as a filter.
        let p = SearchRecipesParams {
            is_favorite: Some(true),
            ..Default::default()
        };
        assert!(p.validate_has_filter().is_ok());
    }

    #[test]
    fn search_params_validate_accepts_query() {
        let p = SearchRecipesParams {
            query: Some("chicken".into()),
            ..Default::default()
        };
        assert!(p.validate_has_filter().is_ok());
    }

    #[test]
    fn search_params_normalized_query_strips_wildcard_and_whitespace() {
        let cases = [
            (None, None),
            (Some(""), None),
            (Some("   "), None),
            (Some("*"), None),
            (Some("  *  "), None),
            (Some("chicken"), Some("chicken".to_string())),
            (Some("  chicken  "), Some("chicken".to_string())),
        ];
        for (input, expected) in cases {
            let p = SearchRecipesParams {
                query: input.map(str::to_string),
                ..Default::default()
            };
            assert_eq!(p.normalized_query(), expected, "input was {input:?}");
        }
    }

    #[test]
    fn search_params_normalized_tags_lowercases_and_drops_empties() {
        let p = SearchRecipesParams {
            tags: Some(vec![
                "Dinner".into(),
                "  EASY  ".into(),
                "".into(),
                "   ".into(),
            ]),
            ..Default::default()
        };
        assert_eq!(p.normalized_tags(), vec!["dinner", "easy"]);
    }
}
