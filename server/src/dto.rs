use serde::{Deserialize, Serialize};

// ─── Shared helper ──────────────────────────────────────────────

/// Deserialize an f64 that may be null (AI sometimes returns null for "to taste" amounts)
pub fn deserialize_f64_or_null<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<f64>::deserialize(deserializer).map(|opt| opt.unwrap_or(0.0))
}

// ─── Recipe DTOs ────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TimeValueDto {
    pub value: i32,
    pub unit: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortionSizeDto {
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum IngredientAmountDto {
    #[serde(rename = "single")]
    Single {
        #[serde(deserialize_with = "deserialize_f64_or_null")]
        value: f64,
    },
    #[serde(rename = "range")]
    Range {
        #[serde(deserialize_with = "deserialize_f64_or_null")]
        min: f64,
        #[serde(deserialize_with = "deserialize_f64_or_null")]
        max: f64,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IngredientDto {
    /// Purchasable identity ("garlic", "lemon"). Distinct varietals like
    /// "boneless skinless chicken breast" vs "whole chicken" are kept as
    /// separate names so the shopping aggregator treats them as separate
    /// purchases.
    pub name: String,
    /// Optional preparation form ("minced", "thinly sliced", "cut into wedges
    /// for serving"). Belongs to the recipe context, not the shopping list —
    /// the shopping aggregator ignores `prep` and groups by `name`.
    #[serde(default)]
    pub prep: Option<String>,
    pub amount: IngredientAmountDto,
    #[serde(default)]
    pub unit: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NutritionDto {
    pub calories: Option<i32>,
    pub protein_grams: Option<i32>,
    pub carbs_grams: Option<i32>,
    pub fat_grams: Option<i32>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateRecipeDto {
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub source_url: Option<String>,
    pub parent_recipe_id: Option<String>,
    pub prep_time: Option<TimeValueDto>,
    pub cook_time: Option<TimeValueDto>,
    pub total_time: Option<TimeValueDto>,
    pub servings: i32,
    pub portion_size: Option<PortionSizeDto>,
    pub instructions: String,
    pub ingredients: Vec<IngredientDto>,
    pub nutrition_per_serving: Option<NutritionDto>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct UpdateRecipeDto {
    pub name: Option<String>,
    pub description: Option<String>,
    pub prep_time: Option<TimeValueDto>,
    pub cook_time: Option<TimeValueDto>,
    pub total_time: Option<TimeValueDto>,
    pub servings: Option<i32>,
    pub portion_size: Option<PortionSizeDto>,
    pub instructions: Option<String>,
    pub ingredients: Option<Vec<IngredientDto>>,
    pub nutrition_per_serving: Option<NutritionDto>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub is_favorite: Option<bool>,
    pub rating: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ImportRecipeDto {
    pub markdown: String,
}

/// Single-recipe GET response: the stored row plus the parent's display
/// name and slug (resolved server-side) so the frontend doesn't have to
/// do a second round-trip to render "adapted from X".
#[derive(Debug, Serialize)]
pub struct RecipeResponse {
    #[serde(flatten)]
    pub recipe: crate::entities::recipe::Model,
    pub parent_name: Option<String>,
    pub parent_slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AdaptRecipeDto {
    pub recipe_id: String,
    pub person_options: Vec<crate::services::recipe_adapter::PersonAdaptOptions>,
    pub user_instructions: String,
}

#[derive(Debug, Deserialize)]
pub struct ImportRecipeFromUrlDto {
    pub url: String,
}

// ─── Person DTOs ────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct CreatePersonDto {
    pub name: String,
    pub birthdate: String,
    pub dietary_goals: Option<String>,
    pub dislikes: Vec<String>,
    pub favorites: Vec<String>,
    pub notes: Option<String>,
    pub drink_preferences: Option<Vec<String>>,
    pub drink_dislikes: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdatePersonDto {
    pub name: Option<String>,
    pub birthdate: Option<String>,
    pub dietary_goals: Option<String>,
    pub dislikes: Option<Vec<String>>,
    pub favorites: Option<Vec<String>>,
    pub notes: Option<String>,
    pub is_active: Option<bool>,
    pub drink_preferences: Option<Vec<String>>,
    pub drink_dislikes: Option<Vec<String>>,
}

// ─── Meal DTOs ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "food_type")]
pub enum PersonServingDto {
    #[serde(rename = "recipe")]
    Recipe {
        person_id: String,
        recipe_id: String,
        servings_count: f64,
        notes: Option<String>,
    },
    #[serde(rename = "adhoc")]
    Adhoc {
        person_id: String,
        adhoc_items: Vec<IngredientDto>,
        notes: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateMealDto {
    pub date: String,
    pub meal_type: String,
    pub order_index: i32,
    pub servings: Vec<PersonServingDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateMealDto {
    pub date: Option<String>,
    pub meal_type: Option<String>,
    pub order_index: Option<i32>,
    pub servings: Option<Vec<PersonServingDto>>,
}

// ─── Meal Template DTOs ─────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateMealTemplateDto {
    pub name: String,
    pub meal_type: String,
    pub servings: Vec<PersonServingDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateMealTemplateDto {
    pub name: Option<String>,
    pub meal_type: Option<String>,
    pub servings: Option<Vec<PersonServingDto>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateFromMealDto {
    pub meal_id: String,
    pub name: String,
}

// ─── Shopping DTOs ──────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum SourceType {
    #[serde(rename = "recipe")]
    Recipe,
    #[serde(rename = "adhoc")]
    Adhoc,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IngredientSourceDto {
    pub amount: IngredientAmountDto,
    pub unit: String,
    pub source_type: SourceType,
    pub source_name: Option<String>,
    pub meal_id: String,
    pub meal_date: String,
    pub meal_type: String,
    pub recipe_servings: Option<i32>,
    pub person_servings: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AggregatedIngredientDto {
    pub ingredient_name: String,
    pub total_amount: Option<IngredientAmountDto>,
    pub total_unit: Option<String>,
    pub items: Vec<IngredientSourceDto>,
}

// ─── Suggestion DTOs ────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct GetSuggestionsDto {
    pub person_ids: Vec<String>,
    pub reference_date: String,
}

#[derive(Debug, Deserialize)]
pub struct AiSuggestMealsDto {
    pub person_options: Vec<crate::services::recipe_adapter::PersonAdaptOptions>,
    pub meal_type: String,
    pub character: crate::services::ai_suggestion_service::MealCharacter,
    pub feedback: Option<String>,
    pub previous_suggestion_names: Option<Vec<String>>,
}

// ─── Settings DTOs ──────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelOption {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_requests: u64,
}

#[derive(Debug, Deserialize)]
pub struct SetSettingBody {
    pub value: String,
}

// ─── Bar Item DTOs ─────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateBarItemDto {
    pub name: String,
    pub category: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BulkBarItemsDto {
    pub items: Vec<CreateBarItemDto>,
}

// ─── Drink Recipe DTOs ─────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateDrinkRecipeDto {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_drink_source")]
    pub source: String,
    pub source_url: Option<String>,
    #[serde(default = "default_servings_one")]
    pub servings: i32,
    pub instructions: String,
    pub ingredients: Vec<IngredientDto>,
    pub technique: Option<String>,
    pub glassware: Option<String>,
    pub garnish: Option<String>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub is_non_alcoholic: Option<bool>,
}

fn default_drink_source() -> String {
    "manual".to_string()
}

fn default_servings_one() -> i32 {
    1
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateDrinkRecipeDto {
    pub name: Option<String>,
    pub description: Option<String>,
    pub servings: Option<i32>,
    pub instructions: Option<String>,
    pub ingredients: Option<Vec<IngredientDto>>,
    pub technique: Option<String>,
    pub glassware: Option<String>,
    pub garnish: Option<String>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub is_favorite: Option<bool>,
    pub is_non_alcoholic: Option<bool>,
    pub rating: Option<f64>,
}

// ─── Cocktail Suggestion DTOs ──────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum DrinkMood {
    /// A named cocktail style/family (e.g. "Sours", "Ancestrals", "Highballs")
    #[serde(rename = "style")]
    Style { label: String },
    /// Freeform user description
    #[serde(rename = "custom")]
    Custom { text: String },
}

#[derive(Debug, Deserialize)]
pub struct AiSuggestCocktailsDto {
    pub person_ids: Vec<String>,
    pub bar_item_ids: Vec<String>,
    pub mood: DrinkMood,
    pub include_non_alcoholic: bool,
    pub feedback: Option<String>,
    pub previous_suggestion_names: Option<Vec<String>>,
}
