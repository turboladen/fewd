//! Shared input schemas and bidirectional value types used by more than one
//! submodule. Small conversion helpers (amount/time/nutrition/portion) live
//! here too since they're used both on read and write paths.

use chrono::{DateTime, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::dto::{IngredientAmountDto, IngredientDto, NutritionDto, PortionSizeDto, TimeValueDto};

use super::errors::InputError;

// ─── Input schemas shared across tools ───────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EmptyParams {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchParams {
    /// Case-insensitive substring to match against recipe names.
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetRecipeParams {
    /// URL-safe slug that uniquely identifies the recipe (e.g. "carbonara" or
    /// "roasted-chicken-2"). Use `list_recipes` or `search_recipes` to find
    /// slugs.
    pub slug: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DateRangeParams {
    /// Inclusive start date in YYYY-MM-DD format.
    pub start_date: String,
    /// Inclusive end date in YYYY-MM-DD format.
    pub end_date: String,
}

impl DateRangeParams {
    /// Validate both dates parse as YYYY-MM-DD before the service layer
    /// sees them. The service layer would reject malformed dates as a
    /// `DbErr::Custom` which gets wrapped as an MCP `internal_error` —
    /// turning a user-input mistake into something the LLM can't cleanly
    /// retry. Catching it here lets us surface `invalid_params` instead.
    pub fn validate(&self) -> Result<(), InputError> {
        validate_date_yyyy_mm_dd(&self.start_date, "start_date")?;
        validate_date_yyyy_mm_dd(&self.end_date, "end_date")?;
        Ok(())
    }
}

/// Confirm a date string parses as YYYY-MM-DD. Used by tool handlers to
/// surface bad date formats as `invalid_params` rather than letting them
/// reach the service layer (which converts them to opaque DB errors).
pub(super) fn validate_date_yyyy_mm_dd(value: &str, field: &'static str) -> Result<(), InputError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| InputError::InvalidDate {
            field,
            value: value.to_string(),
        })
}

// ─── Bidirectional value types (input + output) ──────────────────

/// Ingredient shape used across both tool inputs and outputs. Mirrors
/// [`IngredientDto`] but adds [`JsonSchema`] so MCP clients can introspect
/// the structure, and accepts both directions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IngredientOut {
    /// Purchasable identity (e.g. "garlic"). Distinct varietals like
    /// "boneless skinless chicken breast" vs "whole chicken" stay as separate
    /// names — the shopping list aggregates by this field.
    pub name: String,
    /// Optional preparation form (e.g. "minced", "thinly sliced", "cut into
    /// wedges for serving"). The shopping aggregator ignores this — prep is
    /// for the recipe step, not the grocery list.
    #[serde(default)]
    pub prep: Option<String>,
    /// Quantity. Either an exact amount or a min/max range.
    pub amount: IngredientAmountOut,
    /// Unit of measure (e.g. "cup", "gram", "each"). May be empty for
    /// unit-less items.
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IngredientAmountOut {
    Single { value: f64 },
    Range { min: f64, max: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NutritionOut {
    pub calories: Option<i32>,
    pub protein_grams: Option<i32>,
    pub carbs_grams: Option<i32>,
    pub fat_grams: Option<i32>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimeOut {
    pub value: i32,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PortionSizeOut {
    pub value: f64,
    pub unit: String,
}

// ─── JSON parsing helpers for fields stored as TEXT ──────────────

pub(super) fn parse_json<'a, T: Deserialize<'a>>(raw: &'a str, context: &str) -> Result<T, String> {
    serde_json::from_str(raw).map_err(|e| format!("malformed {context} JSON: {e}"))
}

pub(super) fn parse_optional_json<T: for<'a> Deserialize<'a>>(
    raw: Option<&str>,
    context: &str,
) -> Result<Option<T>, String> {
    match raw {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => parse_json(s, context).map(Some),
    }
}

pub(super) fn format_date(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d").to_string()
}

// ─── Value-type conversions (DTO ↔ MCP) ──────────────────────────

pub(super) fn ingredient_out(ing: &IngredientDto) -> IngredientOut {
    IngredientOut {
        name: ing.name.clone(),
        prep: ing.prep.clone(),
        amount: amount_out(ing.amount.clone()),
        unit: ing.unit.clone(),
        notes: ing.notes.clone(),
    }
}

pub(super) fn amount_out(a: IngredientAmountDto) -> IngredientAmountOut {
    match a {
        IngredientAmountDto::Single { value } => IngredientAmountOut::Single { value },
        IngredientAmountDto::Range { min, max } => IngredientAmountOut::Range { min, max },
    }
}

pub(super) fn time_out(t: TimeValueDto) -> TimeOut {
    TimeOut {
        value: t.value,
        unit: t.unit,
    }
}

pub(super) fn portion_out(p: PortionSizeDto) -> PortionSizeOut {
    PortionSizeOut {
        value: p.value,
        unit: p.unit,
    }
}

pub(super) fn nutrition_out(n: NutritionDto) -> NutritionOut {
    NutritionOut {
        calories: n.calories,
        protein_grams: n.protein_grams,
        carbs_grams: n.carbs_grams,
        fat_grams: n.fat_grams,
        notes: n.notes,
    }
}

pub(super) fn ingredient_in(ing: IngredientOut) -> IngredientDto {
    // Defensive: if a caller hands us `name = "garlic, minced", prep = None`,
    // normalize it through the splitter so the comma'd prep ends up in the
    // dedicated field. Already-split inputs (`prep` populated, or no comma in
    // `name`) pass through unchanged because the splitter is idempotent.
    let (name, prep) = if ing.prep.is_none() && ing.name.contains(',') {
        crate::services::ingredient_splitter::split_name_and_prep(&ing.name)
    } else {
        (ing.name, ing.prep)
    };
    IngredientDto {
        name,
        prep,
        amount: amount_in(ing.amount),
        unit: ing.unit,
        notes: ing.notes,
    }
}

pub(super) fn amount_in(a: IngredientAmountOut) -> IngredientAmountDto {
    match a {
        IngredientAmountOut::Single { value } => IngredientAmountDto::Single { value },
        IngredientAmountOut::Range { min, max } => IngredientAmountDto::Range { min, max },
    }
}

pub(super) fn time_in(t: TimeOut) -> TimeValueDto {
    TimeValueDto {
        value: t.value,
        unit: t.unit,
    }
}

pub(super) fn portion_in(p: PortionSizeOut) -> PortionSizeDto {
    PortionSizeDto {
        value: p.value,
        unit: p.unit,
    }
}

pub(super) fn nutrition_in(n: NutritionOut) -> NutritionDto {
    NutritionDto {
        calories: n.calories,
        protein_grams: n.protein_grams,
        carbs_grams: n.carbs_grams,
        fat_grams: n.fat_grams,
        notes: n.notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_out_preserves_single_and_range() {
        match amount_out(IngredientAmountDto::Single { value: 2.5 }) {
            IngredientAmountOut::Single { value } => assert_eq!(value, 2.5),
            _ => panic!("expected Single"),
        }
        match amount_out(IngredientAmountDto::Range { min: 1.0, max: 2.0 }) {
            IngredientAmountOut::Range { min, max } => {
                assert_eq!(min, 1.0);
                assert_eq!(max, 2.0);
            }
            _ => panic!("expected Range"),
        }
    }

    #[test]
    fn parse_optional_json_treats_empty_as_none() {
        let v: Option<Vec<String>> = parse_optional_json(None, "tags").unwrap();
        assert!(v.is_none());
        let v: Option<Vec<String>> = parse_optional_json(Some(""), "tags").unwrap();
        assert!(v.is_none());
        let v: Option<Vec<String>> = parse_optional_json(Some("   "), "tags").unwrap();
        assert!(v.is_none());
    }

    #[test]
    fn parse_optional_json_parses_valid_content() {
        let v: Option<Vec<String>> = parse_optional_json(Some("[\"a\",\"b\"]"), "tags").unwrap();
        assert_eq!(v.unwrap(), vec!["a", "b"]);
    }

    #[test]
    fn parse_optional_json_errors_on_invalid_content() {
        let r: Result<Option<Vec<String>>, _> = parse_optional_json(Some("not json"), "tags");
        assert!(r.is_err());
    }

    #[test]
    fn date_range_params_accepts_valid_dates() {
        let p = DateRangeParams {
            start_date: "2026-04-20".into(),
            end_date: "2026-04-26".into(),
        };
        assert!(p.validate().is_ok());
    }

    #[test]
    fn date_range_params_rejects_malformed_start() {
        let p = DateRangeParams {
            start_date: "April 20".into(),
            end_date: "2026-04-26".into(),
        };
        let err = p.validate().unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("start_date"));
        assert!(msg.contains("YYYY-MM-DD"));
    }

    #[test]
    fn date_range_params_rejects_malformed_end() {
        let p = DateRangeParams {
            start_date: "2026-04-20".into(),
            end_date: "tomorrow".into(),
        };
        let err = p.validate().unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("end_date"));
    }
}
