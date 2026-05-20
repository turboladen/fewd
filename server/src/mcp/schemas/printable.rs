//! `get_meal_planner_printable` input schema. Renders a fridge-ready HTML
//! card for a short upcoming window of meals — the canonical "what's for
//! dinner this week" layout, deterministic per call.
//!
//! Range is capped tighter than the general [`DateRangeParams`] cap (14 days
//! vs. 366) because this output is for a single 8.5×11 sheet, not a
//! quarterly meal plan. The cap is enforced in [`PrintableInput::validate`]
//! using [`InputError::DateRangeTooWide`] — same error variant the broader
//! cap uses, just with a tighter `max_days`.

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::Deserialize;

use super::common::validate_date_yyyy_mm_dd;
use super::errors::InputError;
use super::meals::canonical_meal_type;

/// Hard cap on the date span for a single printable. The template is sized
/// to fit a US Letter portrait sheet at default print scale; loosening this
/// without also loosening the template's typography / row density will
/// silently overflow onto a second page (the original pain point this tool
/// is meant to eliminate).
pub const MAX_PRINTABLE_DAYS: i64 = 13; // span = end - start; 13 → 14 inclusive days

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PrintableInput {
    /// Inclusive start date in YYYY-MM-DD format.
    pub start_date: String,
    /// Inclusive end date in YYYY-MM-DD format. Capped at 14 inclusive days
    /// of span — wider windows are rejected because the output is sized
    /// for a single fridge-printable sheet.
    pub end_date: String,
    /// Which meal slots to include. Defaults to `["Dinner"]` — fridge
    /// cards almost always show dinners only. Pass multiple
    /// (case-insensitive; same canonical Title Case as `create_meal`) to
    /// widen.
    #[serde(default = "default_include")]
    pub include: Vec<String>,
    /// Show per-person serving notes from `meal.servings[i].notes` as
    /// inline tag pills (e.g. "Cleo: plain gyoza"). Default true.
    #[serde(default = "default_true")]
    pub show_servings: bool,
    /// Show "who's eating what" person-name prefixes on per-serving notes.
    /// Default false — most fridge cards don't need it, and the kid-specific
    /// notes (which DO read better with the name prefix) get it via the
    /// `notes` text itself ("Cleo: plain rice").
    #[serde(default)]
    pub show_assignees: bool,
    /// Optional top-right header badge (e.g. "Freezer Clear",
    /// "Back-friendly week"). Short — fits on one line.
    #[serde(default)]
    pub week_theme: Option<String>,
    /// Top-right list of "use up before X" notes that appear under the
    /// header badge (e.g. "Frozen gyoza → Monday"). Keep under 4 entries
    /// for fit.
    #[serde(default)]
    pub use_up_notes: Vec<String>,
    /// Dark footer block with cross-day reminders. Each item has a bolded
    /// `prefix` ("Wed night:") and a body. Keep under 5 items so the
    /// reminders block stays on one page.
    #[serde(default)]
    pub dont_forget: Vec<DontForgetItem>,
    /// Per-day annotations keyed by date. `tag` renders as a small badge
    /// under the day name ("Steve Cooks"); `blurb` overrides the recipe's
    /// stored description for that day; `prep_notes` render as red
    /// reminder pills in the meal's tag row ("Marinate morning of",
    /// "Defrost the fish at lunch"). Overlays with dates outside
    /// [start, end] are silently dropped.
    #[serde(default)]
    pub day_overlays: Vec<DayOverlay>,
    /// Optional left-footer subtitle ("Back-friendly week · Hot weather
    /// menu"). When omitted, the tool generates a predictability footer
    /// of the form "Generated YYYY-MM-DD · {date range} · {slot list}".
    #[serde(default)]
    pub foot_note: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DontForgetItem {
    /// Bolded prefix (e.g. "Wed night:", "Fri morning:").
    pub prefix: String,
    /// Body of the reminder.
    pub body: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DayOverlay {
    /// Date this overlay applies to (YYYY-MM-DD). Overlays whose date is
    /// outside the printable's range are silently dropped — easier on the
    /// LLM than an error, since stale overlays from a previous range are
    /// a common case.
    pub date: String,
    /// Optional short badge under the day name ("Time Crunch",
    /// "Date Night"). Keep under ~14 chars for fit.
    #[serde(default)]
    pub tag: Option<String>,
    /// Optional override for the recipe's stored description (the
    /// one-line blurb under the meal title). Use when the recipe's
    /// description doesn't fit the week's framing. Truncated visually
    /// to 2 lines.
    #[serde(default)]
    pub blurb: Option<String>,
    /// Time-sensitive prep notes rendered as red reminder pills
    /// ("Marinate morning of", "Defrost the fish at lunch").
    #[serde(default)]
    pub prep_notes: Vec<String>,
}

/// Result of [`PrintableInput::validate`]. Carries already-parsed dates
/// and already-normalized slot names so the renderer doesn't re-parse.
#[derive(Debug)]
pub struct ValidatedRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
    /// Canonical Title-Case slot names ("Breakfast" / "Lunch" / "Dinner"
    /// / "Snack"). Deduped while preserving first-seen order.
    pub include: Vec<String>,
}

impl PrintableInput {
    /// Validate dates parse, span fits the 14-day cap, and every slot in
    /// `include` is a known meal type. Returns the parsed dates +
    /// canonicalized slot list so the caller can render without
    /// re-validating.
    pub fn validate(&self) -> Result<ValidatedRange, InputError> {
        let start = validate_date_yyyy_mm_dd(&self.start_date, "start_date")?;
        let end = validate_date_yyyy_mm_dd(&self.end_date, "end_date")?;
        if start > end {
            return Err(InputError::ReversedDateRange {
                start_date: self.start_date.clone(),
                end_date: self.end_date.clone(),
            });
        }
        let span = (end - start).num_days();
        if span > MAX_PRINTABLE_DAYS {
            return Err(InputError::PrintableSpanTooWide {
                days: span,
                max_days: MAX_PRINTABLE_DAYS,
            });
        }

        let include = if self.include.is_empty() {
            vec!["Dinner".to_string()]
        } else {
            let mut seen = std::collections::HashSet::new();
            let mut out = Vec::with_capacity(self.include.len());
            for slot in &self.include {
                let canonical = canonical_meal_type(slot)
                    .ok_or_else(|| InputError::UnknownMealType(slot.clone()))?;
                if seen.insert(canonical.clone()) {
                    out.push(canonical);
                }
            }
            out
        };

        Ok(ValidatedRange {
            start,
            end,
            include,
        })
    }
}

fn default_include() -> Vec<String> {
    vec!["Dinner".to_string()]
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(start: &str, end: &str) -> PrintableInput {
        PrintableInput {
            start_date: start.to_string(),
            end_date: end.to_string(),
            include: vec![],
            show_servings: true,
            show_assignees: false,
            week_theme: None,
            use_up_notes: vec![],
            dont_forget: vec![],
            day_overlays: vec![],
            foot_note: None,
        }
    }

    #[test]
    fn validate_accepts_seven_day_dinner_week() {
        let v = input("2026-05-11", "2026-05-17").validate().unwrap();
        assert_eq!(v.start, NaiveDate::from_ymd_opt(2026, 5, 11).unwrap());
        assert_eq!(v.end, NaiveDate::from_ymd_opt(2026, 5, 17).unwrap());
        // Empty `include` defaults to Dinner-only.
        assert_eq!(v.include, vec!["Dinner".to_string()]);
    }

    #[test]
    fn validate_accepts_exactly_fourteen_inclusive_days() {
        // 14 inclusive days = span of 13. The bead's "capped at 14 days"
        // means inclusive day count, not span. May 1 → May 14 = 14 days.
        let v = input("2026-05-01", "2026-05-14").validate().unwrap();
        assert_eq!(v.end - v.start, chrono::Duration::days(13));
    }

    #[test]
    fn validate_rejects_span_exceeding_fourteen_days() {
        // May 1 → May 15 = 15 inclusive days = span 14, one over the cap.
        let err = input("2026-05-01", "2026-05-15").validate().unwrap_err();
        let msg = format!("{err}");
        assert!(
            matches!(
                err,
                InputError::PrintableSpanTooWide {
                    days: 14,
                    max_days: 13
                }
            ),
            "expected PrintableSpanTooWide (14/13), got {err:?}"
        );
        // Message must be actionable for the LLM — name the cap and the
        // recovery, and be specific to the printable use case (not the
        // generic "multi-megabyte response" wording).
        assert!(msg.contains("13"), "msg should reference the cap: {msg}");
        assert!(
            msg.to_lowercase().contains("narrow"),
            "msg should hint at narrowing: {msg}"
        );
        assert!(
            msg.to_lowercase().contains("sheet")
                || msg.to_lowercase().contains("page")
                || msg.to_lowercase().contains("printable"),
            "msg should mention the print-fit reason: {msg}"
        );
    }

    #[test]
    fn validate_rejects_reversed_range() {
        let err = input("2026-05-17", "2026-05-11").validate().unwrap_err();
        assert!(matches!(err, InputError::ReversedDateRange { .. }));
    }

    #[test]
    fn validate_rejects_malformed_start_date() {
        let err = input("May 11", "2026-05-17").validate().unwrap_err();
        let msg = format!("{err}");
        assert!(matches!(
            err,
            InputError::InvalidDate {
                field: "start_date",
                ..
            }
        ));
        assert!(msg.contains("YYYY-MM-DD"));
    }

    #[test]
    fn validate_normalizes_meal_slot_strings_to_title_case() {
        let mut p = input("2026-05-11", "2026-05-17");
        p.include = vec!["dinner".into(), "BREAKFAST".into(), "  Lunch  ".into()];
        let v = p.validate().unwrap();
        assert_eq!(
            v.include,
            vec!["Dinner".to_string(), "Breakfast".into(), "Lunch".into()]
        );
    }

    #[test]
    fn validate_dedupes_meal_slots_preserving_first_seen_order() {
        let mut p = input("2026-05-11", "2026-05-17");
        p.include = vec!["Dinner".into(), "dinner".into(), "DINNER".into()];
        let v = p.validate().unwrap();
        assert_eq!(v.include, vec!["Dinner".to_string()]);
    }

    #[test]
    fn validate_rejects_unknown_meal_slot() {
        let mut p = input("2026-05-11", "2026-05-17");
        p.include = vec!["brunch".into()];
        let err = p.validate().unwrap_err();
        let msg = format!("{err}");
        assert!(matches!(err, InputError::UnknownMealType(_)));
        assert!(msg.contains("brunch"));
        assert!(msg.contains("Breakfast"));
    }

    #[test]
    fn validate_empty_include_defaults_to_dinner() {
        let v = input("2026-05-11", "2026-05-17").validate().unwrap();
        assert_eq!(v.include, vec!["Dinner".to_string()]);
    }
}
