//! Recipe-related MCP input/output types and conversion helpers.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::dto::{CreateRecipeDto, IngredientDto, NutritionDto, PortionSizeDto, TimeValueDto};
use crate::entities::recipe;

use super::common::{
    format_date, ingredient_in, ingredient_out, nutrition_in, nutrition_out, parse_json,
    parse_optional_json, portion_in, portion_out, time_in, time_out, IngredientOut, NutritionOut,
    PortionSizeOut, TimeOut,
};
use super::errors::InputError;

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
    pub times_made: i32,
    pub last_made: Option<String>,
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
    pub times_made: i32,
    pub last_made: Option<String>,
    pub rating: Option<f64>,
}

/// Input for `search_recipes`. Every filter is optional, but at least one
/// must be provided — call [`SearchRecipesParams::validate_has_filter`]
/// before building the service-layer query. Bare / wildcard calls are
/// rejected with a pointer at `list_curated_recipes`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SearchRecipesParams {
    /// Case-insensitive substring on the recipe name. Empty string and `*`
    /// are treated as no-query (and don't count as a filter on their own).
    #[serde(default)]
    pub query: Option<String>,
    /// Tag membership (case-insensitive exact match). Multiple tags compose
    /// as AND — recipe must have every listed tag.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Maximum recipe `total_time`, **assumed to be in minutes**. Recipes
    /// authored in a different unit (e.g. hours) will not match — known
    /// limitation pending a normalized `total_minutes` column.
    #[serde(default)]
    pub max_total_time_minutes: Option<i32>,
    /// Minimum star rating. Recipes with no rating are excluded.
    #[serde(default)]
    pub min_rating: Option<f64>,
    /// If true, only is_favorite recipes; if false, only non-favorites.
    #[serde(default)]
    pub is_favorite: Option<bool>,
    /// Recipes not made in at least N days (or never made).
    #[serde(default)]
    pub unmade_since_days: Option<i32>,
    /// Exclude recipes that contain ingredients any of these family members
    /// dislikes. Each named person's `dislikes` are matched as
    /// case-insensitive substrings against ingredient names — e.g. "olive
    /// oil" is excluded when a person dislikes "olive". Plan around this
    /// when the substring is genuinely shared between an avoided and
    /// acceptable ingredient. Unknown names return an actionable error
    /// pointing at `list_people`.
    #[serde(default)]
    pub excludes_for_persons: Option<Vec<String>>,
}

impl SearchRecipesParams {
    /// Reject the all-empty / wildcard-only case. The full archive is
    /// intentionally not exposed via this tool — for an unfiltered shortlist
    /// the LLM should call `list_curated_recipes`.
    pub fn validate_has_filter(&self) -> Result<(), &'static str> {
        let q_provides_filter = self
            .query
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "*")
            .is_some();
        let tags_provides_filter = self.tags.as_ref().is_some_and(|v| !v.is_empty());
        let excludes_provides_filter = self
            .excludes_for_persons
            .as_ref()
            .is_some_and(|v| !v.is_empty());

        if q_provides_filter
            || tags_provides_filter
            || excludes_provides_filter
            || self.max_total_time_minutes.is_some()
            || self.min_rating.is_some()
            || self.is_favorite.is_some()
            || self.unmade_since_days.is_some()
        {
            Ok(())
        } else {
            Err("search_recipes requires at least one filter \
                 (query, tags, max_total_time_minutes, min_rating, is_favorite, \
                 unmade_since_days, or excludes_for_persons). \
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
}

/// Input for `create_recipe`. Mirrors [`CreateRecipeDto`] but replaces
/// `parent_recipe_id` with a slug reference the LLM can actually produce.
#[derive(Debug, Deserialize, JsonSchema)]
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
    #[serde(default)]
    pub prep_time: Option<TimeOut>,
    #[serde(default)]
    pub cook_time: Option<TimeOut>,
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
        times_made: recipe.times_made,
        last_made: recipe.last_made.map(format_date),
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
        times_made: recipe.times_made,
        last_made: recipe.last_made.map(format_date),
        rating: recipe.rating,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

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
