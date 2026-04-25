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
}
