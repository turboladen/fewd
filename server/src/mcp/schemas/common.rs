//! Shared input schemas and bidirectional value types used by more than one
//! submodule. Small conversion helpers (amount/time/nutrition/portion) live
//! here too since they're used both on read and write paths.

use chrono::{DateTime, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::dto::{
    deserialize_optional_string_empty_as_none, IngredientAmountDto, IngredientDto, NutritionDto,
    PortionSizeDto, TimeValueDto,
};

use super::errors::InputError;

// ─── Input schemas shared across tools ───────────────────────────

/// Upper bound on `(end_date - start_date)` for any tool that accepts a
/// [`DateRangeParams`]. With four meal slots × four people, a 366-day
/// span is already ~1.5× the per-call result cap (`MAX_LIST_RESULTS`),
/// so anything wider trips the result cap on `list_meals` anyway —
/// rejecting earlier in `validate()` keeps the failure mode legible
/// ("narrow the date range") instead of producing a misleading
/// "{N} rows exceed cap" message.
pub const MAX_DATE_RANGE_DAYS: i64 = 366;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EmptyParams {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetRecipeParams {
    /// URL-safe slug that uniquely identifies the recipe (e.g. "carbonara" or
    /// "roasted-chicken-2"). Use `list_curated_recipes` for the shortlist or
    /// `search_recipes` (with at least one filter) to find slugs.
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
    /// Validate both dates parse as YYYY-MM-DD AND that `start_date` is
    /// on or before `end_date`. The service layer's SQL filter is
    /// `date >= start AND date <= end`, so a reversed range matches no
    /// rows and silently returns `[]` — indistinguishable from "nothing
    /// scheduled". Surfacing both checks here turns the silent empty
    /// into a tool-level error the LLM can recover from.
    pub fn validate(&self) -> Result<(), InputError> {
        let start = validate_date_yyyy_mm_dd(&self.start_date, "start_date")?;
        let end = validate_date_yyyy_mm_dd(&self.end_date, "end_date")?;
        if start > end {
            return Err(InputError::ReversedDateRange {
                start_date: self.start_date.clone(),
                end_date: self.end_date.clone(),
            });
        }
        let span = (end - start).num_days();
        if span > MAX_DATE_RANGE_DAYS {
            return Err(InputError::DateRangeTooWide {
                days: span,
                max_days: MAX_DATE_RANGE_DAYS,
            });
        }
        Ok(())
    }
}

/// Parse a strict, canonical `YYYY-MM-DD` date: exactly a 4-digit year,
/// 2-digit month, and 2-digit day separated by `-`.
///
/// chrono's `%Y-%m-%d` parse is too lenient on its own — it accepts non-padded
/// (`2026-5-3`), short-year (`5-1-1`), and even sign-prefixed (`-0001-01-01`,
/// which chrono parses *and* re-formats symmetrically) forms that every date
/// field documents as invalid. So we gate on the exact 4-2-2 ASCII-digit shape
/// first, then parse to reject impossible calendar dates (`2026-02-30`).
/// Returns `None` (rather than a concrete error) so callers can attach their
/// own error type — `InputError` here, a serde `D::Error` in the printable
/// overlay deserializer.
pub(super) fn parse_canonical_date(value: &str) -> Option<NaiveDate> {
    let bytes = value.as_bytes();
    let canonical_shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..].iter().all(u8::is_ascii_digit);
    if !canonical_shape {
        return None;
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

/// Confirm a date string is a canonical YYYY-MM-DD date. Returns the parsed
/// `NaiveDate` so callers can do further range checks without re-parsing.
/// Used by tool handlers to surface bad date formats as a tool-level
/// error rather than letting them reach the service layer (which
/// converts them to opaque DB errors).
pub(super) fn validate_date_yyyy_mm_dd(
    value: &str,
    field: &'static str,
) -> Result<NaiveDate, InputError> {
    parse_canonical_date(value).ok_or_else(|| InputError::InvalidDate {
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
    ///
    /// Do NOT bundle preparation form here ("garlic, minced"). Put the prep
    /// clause in the dedicated `prep` field instead. A comma'd name fragments
    /// shopping aggregation across recipes.
    pub name: String,
    /// Optional preparation form (e.g. "minced", "thinly sliced", "cut into
    /// wedges for serving"). The shopping aggregator ignores this — prep is
    /// for the recipe step, not the grocery list.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string_empty_as_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub prep: Option<String>,
    /// Quantity. Either an exact amount or a min/max range.
    pub amount: IngredientAmountOut,
    /// Unit of measure (e.g. "cup", "gram", "each"). May be empty for
    /// unit-less items.
    #[serde(default)]
    pub unit: String,
    /// Drops to absent (rather than `null`) on the wire when unset, matching
    /// the `prep` and `or_alternative` annotations on this same struct so MCP
    /// tool output stays uniform across optional fields. Note: this is more
    /// conservative than `IngredientDto.notes`, which still serializes `null`
    /// over the HTTP API — bringing those into sync would change the public
    /// JSON shape and is out of scope for fewd-4nb.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Optional alternative ingredient parsed from `<primary> or <alt>` lines
    /// (e.g. "8 flour tortillas or 10 corn tortillas"). Recursive so the
    /// alternative carries its own amount/unit/prep/notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub or_alternative: Option<Box<IngredientOut>>,
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
        or_alternative: ing
            .or_alternative
            .as_deref()
            .map(|alt| Box::new(ingredient_out(alt))),
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
    // Defensive: if a caller hands us `name = "garlic, minced", prep = None`
    // (or `prep = Some(""))`, normalize it through the splitter so the comma'd
    // prep ends up in the dedicated field. Idempotent on already-split inputs.
    let (name, prep) = crate::services::ingredient_splitter::normalize(ing.name, ing.prep);
    IngredientDto {
        name,
        prep,
        amount: amount_in(ing.amount),
        unit: ing.unit,
        notes: ing.notes,
        or_alternative: ing.or_alternative.map(|alt| Box::new(ingredient_in(*alt))),
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

    fn out(name: &str, prep: Option<&str>) -> IngredientOut {
        IngredientOut {
            name: name.to_string(),
            prep: prep.map(str::to_string),
            amount: IngredientAmountOut::Single { value: 1.0 },
            unit: "clove".to_string(),
            notes: None,
            or_alternative: None,
        }
    }

    #[test]
    fn ingredient_in_splits_unsplit_name_at_boundary() {
        let dto = ingredient_in(out("garlic, minced", None));
        assert_eq!(dto.name, "garlic");
        assert_eq!(dto.prep.as_deref(), Some("minced"));
    }

    #[test]
    fn ingredient_in_splits_when_prep_is_empty_string() {
        // LLM emitting "" instead of null for an unset optional. Without
        // normalization the comma'd name would slip through and fragment
        // shopping aggregation.
        let dto = ingredient_in(out("garlic, minced", Some("")));
        assert_eq!(dto.name, "garlic");
        assert_eq!(dto.prep.as_deref(), Some("minced"));
    }

    #[test]
    fn ingredient_in_passes_through_already_split() {
        let dto = ingredient_in(out("garlic", Some("minced")));
        assert_eq!(dto.name, "garlic");
        assert_eq!(dto.prep.as_deref(), Some("minced"));
    }

    #[test]
    fn ingredient_in_preserves_caller_prep_even_with_comma_in_name() {
        // If the caller is explicit about both name AND prep, we trust them
        // — even when the name has a comma. The defensive split only fires
        // when prep is genuinely absent.
        let dto = ingredient_in(out("garlic, minced", Some("smashed")));
        assert_eq!(dto.name, "garlic, minced");
        assert_eq!(dto.prep.as_deref(), Some("smashed"));
    }

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

    // ─── Strict canonical YYYY-MM-DD enforcement (fewd-4uf) ──────
    //
    // chrono's `%Y-%m-%d` parse is lenient: it accepts non-padded
    // (`2026-5-3`) and short-year (`5-1-1`) forms. Every date field is
    // documented as strict YYYY-MM-DD, so reject anything that isn't the
    // canonical zero-padded 4-2-2 shape — while still rejecting
    // calendar-impossible dates that happen to match that shape.

    #[test]
    fn validate_date_accepts_canonical_form() {
        let d = validate_date_yyyy_mm_dd("2026-05-03", "date").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2026, 5, 3).unwrap());
    }

    #[test]
    fn validate_date_rejects_non_padded_month_and_day() {
        assert!(validate_date_yyyy_mm_dd("2026-5-3", "date").is_err());
    }

    #[test]
    fn validate_date_rejects_short_year() {
        assert!(validate_date_yyyy_mm_dd("5-1-1", "date").is_err());
    }

    #[test]
    fn validate_date_rejects_signed_year() {
        // chrono parses AND re-formats a leading sign symmetrically, so a
        // round-trip check alone would accept this non-canonical 11-char form.
        // The 4-2-2 ASCII-digit shape gate is what rejects it.
        assert!(validate_date_yyyy_mm_dd("-0001-01-01", "date").is_err());
        assert!(validate_date_yyyy_mm_dd("+2026-05-03", "date").is_err());
    }

    #[test]
    fn validate_date_rejects_five_digit_year() {
        // chrono parses years beyond 9999, but the canonical contract is a
        // 4-digit year; the len-10 shape gate rejects the extra width
        // (Copilot review on PR #50).
        assert!(validate_date_yyyy_mm_dd("10000-01-01", "date").is_err());
    }

    #[test]
    fn validate_date_rejects_missing_separators() {
        assert!(validate_date_yyyy_mm_dd("20260101", "date").is_err());
    }

    #[test]
    fn validate_date_rejects_wrong_separators() {
        assert!(validate_date_yyyy_mm_dd("2026/05/03", "date").is_err());
    }

    #[test]
    fn validate_date_rejects_canonical_shape_with_impossible_day() {
        // Matches the 4-2-2 shape but isn't a real date — a byte-shape-only
        // check would wrongly accept this; parsing must still run.
        assert!(validate_date_yyyy_mm_dd("2026-02-30", "date").is_err());
    }

    #[test]
    fn validate_date_rejects_canonical_shape_with_impossible_month() {
        assert!(validate_date_yyyy_mm_dd("2026-13-01", "date").is_err());
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

    // ─── Date-range span cap (fewd-2y6.5) ────────────────────────
    //
    // Boundary check: the cap is inclusive, so exactly MAX_DATE_RANGE_DAYS
    // is allowed and one more is rejected. Fence-post errors here would
    // reject exactly-one-year queries which are a common case.

    #[test]
    fn date_range_params_accepts_exactly_max_span() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let end = start
            .checked_add_signed(chrono::Duration::days(MAX_DATE_RANGE_DAYS))
            .unwrap();
        let p = DateRangeParams {
            start_date: start.format("%Y-%m-%d").to_string(),
            end_date: end.format("%Y-%m-%d").to_string(),
        };
        assert!(
            p.validate().is_ok(),
            "exact MAX_DATE_RANGE_DAYS span must be allowed (boundary)"
        );
    }

    #[test]
    fn date_range_params_rejects_one_day_over_max_span() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let end = start
            .checked_add_signed(chrono::Duration::days(MAX_DATE_RANGE_DAYS + 1))
            .unwrap();
        let p = DateRangeParams {
            start_date: start.format("%Y-%m-%d").to_string(),
            end_date: end.format("%Y-%m-%d").to_string(),
        };
        let err = p.validate().unwrap_err();
        let msg = format!("{err}");
        assert!(
            matches!(err, InputError::DateRangeTooWide { days, max_days }
                if days == MAX_DATE_RANGE_DAYS + 1 && max_days == MAX_DATE_RANGE_DAYS),
            "expected DateRangeTooWide with exact span / max, got {err:?}"
        );
        // The Display message must be actionable for the LLM — name the
        // overflow, the cap, and the recovery.
        assert!(msg.contains(&format!("{}", MAX_DATE_RANGE_DAYS + 1)));
        assert!(msg.contains(&format!("{MAX_DATE_RANGE_DAYS}")));
        assert!(
            msg.to_lowercase().contains("narrow"),
            "message should hint at narrowing: {msg}"
        );
    }

    #[test]
    fn date_range_params_rejects_extreme_span() {
        // Pin the originally-described scenario from the bead: an
        // LLM-hallucinated wide range that would otherwise fan out into
        // a multi-megabyte response.
        let p = DateRangeParams {
            start_date: "0001-01-01".into(),
            end_date: "9999-12-31".into(),
        };
        assert!(matches!(
            p.validate().unwrap_err(),
            InputError::DateRangeTooWide { .. }
        ));
    }
}
