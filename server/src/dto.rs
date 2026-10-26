use serde::{Deserialize, Serialize};

/// Re-exported so callers can use `dto::MealType`; the canonical definition lives with
/// the entities (the lower layer that owns the column).
pub use crate::entities::sea_orm_active_enums::MealType;

use crate::services::unit_converter::ShoppingUnitClass;

// ─── Shared helper ──────────────────────────────────────────────

/// Deserialize an f64 that may be null (AI sometimes returns null for "to taste" amounts)
pub fn deserialize_f64_or_null<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<f64>::deserialize(deserializer).map(|opt| opt.unwrap_or(0.0))
}

/// Coerce `Some("")` and `Some("   ")` into `None` so empty-string optional
/// strings are treated as absent at every boundary. LLM clients sometimes emit
/// `""` instead of `null` for unset optional fields; without this helper the
/// downstream "is this prep set?" predicate disagrees with the renderer's
/// "should I display a prep clause?" predicate, fragmenting shopping
/// aggregation.
pub fn deserialize_optional_string_empty_as_none<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(|opt| opt.filter(|s| !s.trim().is_empty()))
}

/// Deserialize an ingredient's prep, trimming it and coercing a blank value
/// to `None`.
//
// Trimming keeps " minced" and "minced" one prep for every client, the same
// rule the web form applies through `normalizeIngredientPrep`.
pub fn deserialize_prep<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
        .map(|opt| opt.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
}

/// Deserializes an update field that may be absent, null, or a value. With
/// `#[serde(default)]`, an absent field stays `None`, null becomes
/// `Some(None)`, and a value becomes `Some(Some(v))`. Pair it with
/// `skip_serializing_if = "Option::is_none"` so serializing round-trips the
/// three states.
pub fn nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
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
    ///
    /// Deserializing trims it and normalizes a blank value to `None` (LLMs
    /// occasionally emit `""` instead of `null` for unset optional fields),
    /// and `None` skips serialization to keep wire-format shape consistent with the
    /// migration's struct.
    #[serde(
        default,
        deserialize_with = "deserialize_prep",
        skip_serializing_if = "Option::is_none"
    )]
    pub prep: Option<String>,
    pub amount: IngredientAmountDto,
    #[serde(default)]
    pub unit: String,
    pub notes: Option<String>,
    /// Optional alternative ingredient parsed from `<primary> or <alt>` lines
    /// (e.g. "8 flour tortillas or 10 corn tortillas"). Recursive so the
    /// alternative carries its own amount/unit/prep/notes — chained `or`
    /// collapses naturally via the recursive `Box`. Boxed to keep
    /// `IngredientDto`'s size finite.
    ///
    /// The shopping aggregator currently counts only the primary; UX for
    /// "primary vs alternative" selection is deferred — see the fewd-4nb
    /// follow-up bead. Existing rows deserialize this as `None` automatically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub or_alternative: Option<Box<IngredientDto>>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub prep_time: Option<Option<TimeValueDto>>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub cook_time: Option<Option<TimeValueDto>>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub total_time: Option<Option<TimeValueDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servings: Option<i32>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub portion_size: Option<Option<PortionSizeDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingredients: Option<Vec<IngredientDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nutrition_per_serving: Option<NutritionDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub notes: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub icon: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_favorite: Option<bool>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub rating: Option<Option<f64>>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birthdate: Option<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub dietary_goals: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dislikes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorites: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub notes: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drink_preferences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    pub meal_type: MealType,
    pub order_index: i32,
    pub servings: Vec<PersonServingDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateMealDto {
    pub date: Option<String>,
    pub meal_type: Option<MealType>,
    pub order_index: Option<i32>,
    pub servings: Option<Vec<PersonServingDto>>,
}

// ─── Meal Template DTOs ─────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateMealTemplateDto {
    pub name: String,
    pub meal_type: MealType,
    pub servings: Vec<PersonServingDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateMealTemplateDto {
    pub name: Option<String>,
    pub meal_type: Option<MealType>,
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

/// A shopping-list line with its total rounded up to a buyable amount.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ShoppingListItemDto {
    /// The unrounded line: name, exact total, unit and per-meal sources.
    pub aggregate: AggregatedIngredientDto,
    /// The rounded total, or `None` when the sources could not be summed into
    /// one total and unit.
    pub shopping: Option<ShoppingAmountDto>,
}

/// A shopping-list total rounded up for buying. Its unit is the line's
/// `aggregate.total_unit`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ShoppingAmountDto {
    /// The rounded total. A range stays a range, with each end rounded.
    pub amount: IngredientAmountDto,
    /// The rounding rule the unit selected.
    pub class: ShoppingUnitClass,
    /// True when rounding changed the total.
    pub rounded: bool,
}

/// Shopping list partitioned by pantry-staple classification. The shape
/// anticipates two render targets — `items_to_buy` as the primary list,
/// `pantry_staples_to_verify` as a "double-check you have these" section
/// — but neither the MCP tool nor the UI consumes this type yet; today
/// only `ShoppingService::get_shopping_list_split` produces it.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ShoppingListSplitDto {
    pub items_to_buy: Vec<AggregatedIngredientDto>,
    pub pantry_staples_to_verify: Vec<AggregatedIngredientDto>,
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

/// Build provenance of the running binary. `git_sha`/`built_at` are embedded
/// at compile time (see `build.rs`); since the frontend is embedded via
/// rust-embed, this describes the whole deployed artifact, not just the server.
#[derive(Debug, Serialize)]
pub struct VersionInfo {
    pub version: String,
    pub git_sha: String,
    pub built_at: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servings: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingredients: Option<Vec<IngredientDto>>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub technique: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub glassware: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub garnish: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub notes: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub icon: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_favorite: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_non_alcoholic: Option<bool>,
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub rating: Option<Option<f64>>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn update(body: serde_json::Value) -> UpdateRecipeDto {
        serde_json::from_value(body).expect("update body deserializes")
    }

    #[test]
    fn nullable_field_tells_absent_from_null_from_value() {
        assert_eq!(update(serde_json::json!({})).notes, None);
        assert_eq!(
            update(serde_json::json!({ "notes": null })).notes,
            Some(None)
        );
        assert_eq!(
            update(serde_json::json!({ "notes": "salt" })).notes,
            Some(Some("salt".to_string()))
        );
        assert_eq!(
            update(serde_json::json!({ "rating": null })).rating,
            Some(None)
        );
        assert!(matches!(
            update(serde_json::json!({ "total_time": null })).total_time,
            Some(None)
        ));
    }

    #[test]
    fn update_serializes_only_the_fields_it_carries_with_a_clear_as_null() {
        let dto = UpdateRecipeDto {
            name: Some("Soup".into()),
            notes: Some(None),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            serde_json::json!({ "name": "Soup", "notes": null })
        );
    }

    #[test]
    fn prep_is_trimmed_and_a_blank_prep_is_absent() {
        let prep = |value: serde_json::Value| {
            serde_json::from_value::<IngredientDto>(serde_json::json!({
                "name": "onion",
                "prep": value,
                "amount": { "type": "single", "value": 1.0 },
                "notes": null,
            }))
            .expect("ingredient deserializes")
            .prep
        };
        assert_eq!(prep(" thinly sliced ".into()), Some("thinly sliced".into()));
        assert_eq!(prep("  ".into()), None);
        assert_eq!(prep(serde_json::Value::Null), None);
    }
}
