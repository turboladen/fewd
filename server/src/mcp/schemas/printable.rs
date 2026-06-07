//! `get_meal_planner_printable` input schema. Renders a fridge-ready HTML
//! card for a short upcoming window of meals — the canonical "what's for
//! dinner this week" layout, deterministic per call.
//!
//! Range is capped tighter than the general [`DateRangeParams`] cap (14 days
//! vs. 366) because this output is for a single 8.5×11 sheet, not a
//! quarterly meal plan. The cap is enforced in [`PrintableInput::validate`]
//! using [`InputError::PrintableSpanTooWide`] with an actionable message
//! that names the single-sheet-fit reason.
//!
//! [`DateRangeParams`]: super::common::DateRangeParams

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer};

use super::common::validate_date_yyyy_mm_dd;
use super::errors::InputError;
use crate::dto::MealType;

/// Hard cap on the date span for a single printable. The template is sized
/// to fit a US Letter portrait sheet at default print scale; loosening this
/// without also loosening the template's typography / row density will
/// silently overflow onto a second page (the original pain point this tool
/// is meant to eliminate). Constant is a *span* (end − start), so the
/// inclusive day count is `MAX_PRINTABLE_DAYS + 1` (= 14 days).
pub const MAX_PRINTABLE_DAYS: i64 = 13;

/// Caps on overlay collection sizes. Each item adds a fixed vertical chunk
/// to the rendered page; runaway counts overflow the single-sheet budget
/// even when individual strings stay within the line-clamp. These are the
/// only validation strong enough to prevent visible page-2 spillover, so
/// they're enforced as hard `InputError::OverlayListTooLong` rejections
/// rather than as advisory budgets in the docstring.
pub const MAX_USE_UP_NOTES: usize = 6;
pub const MAX_DONT_FORGET_ITEMS: usize = 8;
pub const MAX_PREP_NOTES_PER_DAY: usize = 6;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PrintableInput {
    /// Inclusive start date in YYYY-MM-DD format.
    pub start_date: String,
    /// Inclusive end date in YYYY-MM-DD format. Capped at 14 inclusive
    /// days of span — wider windows are rejected because the output is
    /// sized for a single fridge-printable sheet.
    pub end_date: String,
    /// Which meal slots to include. Defaults to `["Dinner"]` — fridge
    /// cards almost always show dinners only. The headline title and the
    /// `head_title` adapt to non-Dinner slots when widened. Pass multiple
    /// (case-insensitive; same canonical Title Case as `create_meal`) to
    /// include more.
    #[serde(default = "default_include")]
    pub include: Vec<String>,
    /// Show per-person serving notes from `meal.servings[i].notes` as
    /// inline tag pills (e.g. "Cleo: plain gyoza"). Default true.
    #[serde(default = "default_true")]
    pub show_servings: bool,
    /// Show "who's eating what" person-name prefixes on per-serving notes.
    /// Default false — most fridge cards don't need it, and the
    /// kid-specific notes (which DO read better with the name prefix) get
    /// it via the `notes` text itself ("Cleo: plain rice").
    #[serde(default)]
    pub show_assignees: bool,
    /// Optional top-right header badge (e.g. "Freezer Clear",
    /// "Back-friendly week"). Short — fits on one line.
    #[serde(default)]
    pub week_theme: Option<String>,
    /// Top-right list of "use up before X" notes that appear under the
    /// header badge (e.g. "Frozen gyoza → Monday"). Capped at
    /// [`MAX_USE_UP_NOTES`] entries; empty/whitespace-only strings are
    /// rejected as `EmptyOverlayField`.
    #[serde(default)]
    pub use_up_notes: Vec<String>,
    /// Dark footer block with cross-day reminders. Each item has a bolded
    /// `prefix` ("Wed night:") and a body. Capped at
    /// [`MAX_DONT_FORGET_ITEMS`] items so the reminders block stays on one
    /// page.
    #[serde(default)]
    pub dont_forget: Vec<DontForgetItem>,
    /// Per-day annotations keyed by date. `tag` renders as a small badge
    /// under the day name ("Steve Cooks"); `blurb` overrides the recipe's
    /// stored description for that day; `prep_notes` render as red
    /// reminder pills in the meal's tag row ("Marinate morning of",
    /// "Defrost the fish at lunch"). Overlays with dates outside
    /// `[start_date, end_date]` are silently dropped (stale overlays from
    /// a prior call are a common LLM case). Malformed date strings are
    /// rejected with `InputError::InvalidDate` so the LLM can correct.
    #[serde(default)]
    pub day_overlays: Vec<DayOverlay>,
    /// Optional left-footer subtitle ("Back-friendly week · Hot weather
    /// menu"). When omitted, the tool generates a predictability footer
    /// of the form "{date range} · {slot list}".
    #[serde(default)]
    pub foot_note: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DontForgetItem {
    /// Bolded prefix (e.g. "Wed night:", "Fri morning:"). Must not be
    /// empty.
    pub prefix: String,
    /// Body of the reminder. Must not be empty.
    pub body: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DayOverlay {
    /// Date this overlay applies to. Accepts YYYY-MM-DD strings on the
    /// wire; parsed at deserialize time so malformed dates surface as a
    /// JSON-RPC validation error rather than disappearing silently at
    /// render. Out-of-range dates (valid but outside `[start, end]`) are
    /// dropped at render time, since stale overlays from prior calls are
    /// a common LLM case.
    #[serde(deserialize_with = "deserialize_naive_date")]
    pub date: NaiveDate,
    /// Optional short badge under the day name ("Time Crunch",
    /// "Date Night"). Keep under ~14 chars for fit (advisory — long
    /// strings clip via CSS).
    #[serde(default)]
    pub tag: Option<String>,
    /// Optional override for the recipe's stored description (the
    /// one-line blurb under the meal title). Use when the recipe's
    /// description doesn't fit the week's framing. Truncated visually
    /// to 2 lines.
    #[serde(default)]
    pub blurb: Option<String>,
    /// Time-sensitive prep notes rendered as red reminder pills
    /// ("Marinate morning of", "Defrost the fish at lunch"). Capped at
    /// [`MAX_PREP_NOTES_PER_DAY`] per overlay; empty/whitespace-only
    /// strings rejected as `EmptyOverlayField`.
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
    pub include: Vec<MealType>,
}

impl PrintableInput {
    /// Validate dates parse, span fits the 14-day cap, every slot in
    /// `include` is a known meal type, and overlay collection sizes /
    /// string emptiness honor their per-field constraints. Returns the
    /// parsed dates + canonicalized slot list so the caller can render
    /// without re-validating.
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
                days_inclusive: span + 1,
                max_days_inclusive: MAX_PRINTABLE_DAYS + 1,
            });
        }

        let include = if self.include.is_empty() {
            vec![MealType::Dinner]
        } else {
            let mut seen = std::collections::HashSet::new();
            let mut out = Vec::with_capacity(self.include.len());
            for slot in &self.include {
                let canonical: MealType = slot
                    .parse()
                    .map_err(|_| InputError::UnknownMealType(slot.clone()))?;
                if seen.insert(canonical) {
                    out.push(canonical);
                }
            }
            out
        };

        // ── Overlay collection sizes ──────────────────────────────────
        if self.use_up_notes.len() > MAX_USE_UP_NOTES {
            return Err(InputError::OverlayListTooLong {
                field: "use_up_notes",
                count: self.use_up_notes.len(),
                max_count: MAX_USE_UP_NOTES,
            });
        }
        if self.dont_forget.len() > MAX_DONT_FORGET_ITEMS {
            return Err(InputError::OverlayListTooLong {
                field: "dont_forget",
                count: self.dont_forget.len(),
                max_count: MAX_DONT_FORGET_ITEMS,
            });
        }
        for o in &self.day_overlays {
            if o.prep_notes.len() > MAX_PREP_NOTES_PER_DAY {
                return Err(InputError::OverlayListTooLong {
                    field: "day_overlays[i].prep_notes",
                    count: o.prep_notes.len(),
                    max_count: MAX_PREP_NOTES_PER_DAY,
                });
            }
        }

        // ── Empty-string checks on documented-non-empty fields ────────
        for note in &self.use_up_notes {
            if note.trim().is_empty() {
                return Err(InputError::EmptyOverlayField {
                    field: "use_up_notes[i]",
                });
            }
        }
        for item in &self.dont_forget {
            if item.prefix.trim().is_empty() {
                return Err(InputError::EmptyOverlayField {
                    field: "dont_forget[i].prefix",
                });
            }
            if item.body.trim().is_empty() {
                return Err(InputError::EmptyOverlayField {
                    field: "dont_forget[i].body",
                });
            }
        }
        for o in &self.day_overlays {
            for prep in &o.prep_notes {
                if prep.trim().is_empty() {
                    return Err(InputError::EmptyOverlayField {
                        field: "day_overlays[i].prep_notes[j]",
                    });
                }
            }
        }

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

/// Parse YYYY-MM-DD into `NaiveDate` at deserialize time so malformed dates
/// surface as JSON-RPC parameter errors (caught by `LenientParameters` and
/// surfaced as a tool-level error) rather than silently disappearing at
/// render time when the overlay-by-date map is built.
fn deserialize_naive_date<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let s = String::deserialize(deserializer)?;
    // Shares the strict canonical check with `validate_date_yyyy_mm_dd` so
    // the overlay date can't drift to a non-canonical form (`2026-5-3`) the
    // rest of the date surface rejects (fewd-4uf).
    super::common::parse_canonical_date(&s).ok_or_else(|| {
        D::Error::custom(format!(
            "day_overlays[i].date must be YYYY-MM-DD (got '{s}')"
        ))
    })
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
        assert_eq!(v.include, vec![MealType::Dinner]);
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
                    days_inclusive: 15,
                    max_days_inclusive: 14
                }
            ),
            "expected PrintableSpanTooWide (15/14 inclusive), got {err:?}"
        );
        // Message must reference the inclusive day count (matches docs)
        // and the recovery path.
        assert!(
            msg.contains("15") && msg.contains("14"),
            "msg should reference both inclusive day counts: {msg}"
        );
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
            vec![MealType::Dinner, MealType::Breakfast, MealType::Lunch]
        );
    }

    #[test]
    fn validate_dedupes_meal_slots_preserving_first_seen_order() {
        let mut p = input("2026-05-11", "2026-05-17");
        p.include = vec!["Dinner".into(), "dinner".into(), "DINNER".into()];
        let v = p.validate().unwrap();
        assert_eq!(v.include, vec![MealType::Dinner]);
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
        assert_eq!(v.include, vec![MealType::Dinner]);
    }

    // ─── Overlay collection-size caps ──────────────────────────────────

    #[test]
    fn validate_rejects_too_many_use_up_notes() {
        let mut p = input("2026-05-11", "2026-05-17");
        p.use_up_notes = (0..MAX_USE_UP_NOTES + 1)
            .map(|i| format!("note {i}"))
            .collect();
        let err = p.validate().unwrap_err();
        assert!(matches!(
            err,
            InputError::OverlayListTooLong {
                field: "use_up_notes",
                ..
            }
        ));
        let msg = format!("{err}");
        assert!(msg.contains("use_up_notes"));
        assert!(msg.contains(&format!("{}", MAX_USE_UP_NOTES + 1)));
    }

    #[test]
    fn validate_rejects_too_many_dont_forget_items() {
        let mut p = input("2026-05-11", "2026-05-17");
        p.dont_forget = (0..MAX_DONT_FORGET_ITEMS + 1)
            .map(|i| DontForgetItem {
                prefix: format!("Day {i}:"),
                body: format!("body {i}"),
            })
            .collect();
        let err = p.validate().unwrap_err();
        assert!(matches!(
            err,
            InputError::OverlayListTooLong {
                field: "dont_forget",
                ..
            }
        ));
    }

    #[test]
    fn validate_rejects_too_many_prep_notes_in_one_overlay() {
        let mut p = input("2026-05-11", "2026-05-17");
        p.day_overlays = vec![DayOverlay {
            date: NaiveDate::from_ymd_opt(2026, 5, 11).unwrap(),
            tag: None,
            blurb: None,
            prep_notes: (0..MAX_PREP_NOTES_PER_DAY + 1)
                .map(|i| format!("prep {i}"))
                .collect(),
        }];
        let err = p.validate().unwrap_err();
        assert!(matches!(
            err,
            InputError::OverlayListTooLong {
                field: "day_overlays[i].prep_notes",
                ..
            }
        ));
    }

    // ─── Empty-string rejection on documented-non-empty fields ─────────

    #[test]
    fn validate_rejects_empty_use_up_note_string() {
        let mut p = input("2026-05-11", "2026-05-17");
        p.use_up_notes = vec!["valid".into(), "  ".into()];
        let err = p.validate().unwrap_err();
        assert!(matches!(
            err,
            InputError::EmptyOverlayField {
                field: "use_up_notes[i]"
            }
        ));
    }

    #[test]
    fn validate_rejects_empty_dont_forget_prefix() {
        let mut p = input("2026-05-11", "2026-05-17");
        p.dont_forget = vec![DontForgetItem {
            prefix: "".into(),
            body: "valid body".into(),
        }];
        let err = p.validate().unwrap_err();
        assert!(matches!(
            err,
            InputError::EmptyOverlayField {
                field: "dont_forget[i].prefix"
            }
        ));
    }

    #[test]
    fn validate_rejects_empty_dont_forget_body() {
        let mut p = input("2026-05-11", "2026-05-17");
        p.dont_forget = vec![DontForgetItem {
            prefix: "Wed:".into(),
            body: "   ".into(),
        }];
        let err = p.validate().unwrap_err();
        assert!(matches!(
            err,
            InputError::EmptyOverlayField {
                field: "dont_forget[i].body"
            }
        ));
    }

    #[test]
    fn validate_rejects_empty_prep_note() {
        let mut p = input("2026-05-11", "2026-05-17");
        p.day_overlays = vec![DayOverlay {
            date: NaiveDate::from_ymd_opt(2026, 5, 11).unwrap(),
            tag: None,
            blurb: None,
            prep_notes: vec!["valid".into(), "".into()],
        }];
        let err = p.validate().unwrap_err();
        assert!(matches!(
            err,
            InputError::EmptyOverlayField {
                field: "day_overlays[i].prep_notes[j]"
            }
        ));
    }

    // ─── Date deserialization on DayOverlay.date ───────────────────────

    #[test]
    fn day_overlay_deserialize_rejects_malformed_date() {
        // The serde deserializer surfaces malformed dates at parse time
        // (this catches the silent-drop case where a bad date used to
        // disappear at render with no LLM feedback).
        let raw = serde_json::json!({
            "date": "2026-O5-11",  // letter O for zero — easy LLM typo
        });
        let err = serde_json::from_value::<DayOverlay>(raw).unwrap_err();
        assert!(
            err.to_string().contains("YYYY-MM-DD"),
            "deserialize error should mention the format: {err}"
        );
    }

    #[test]
    fn day_overlay_deserialize_rejects_non_canonical_date() {
        // Non-padded but chrono-parseable (`2026-5-3`). Shares the strict
        // canonical check with `validate_date_yyyy_mm_dd` (fewd-4uf) so the
        // overlay date can't drift to a non-canonical form the helper rejects.
        let raw = serde_json::json!({ "date": "2026-5-3" });
        let err = serde_json::from_value::<DayOverlay>(raw).unwrap_err();
        assert!(
            err.to_string().contains("day_overlays"),
            "deserialize error should name the field: {err}"
        );
    }

    #[test]
    fn day_overlay_deserialize_accepts_well_formed_date() {
        let raw = serde_json::json!({ "date": "2026-05-11" });
        let parsed: DayOverlay = serde_json::from_value(raw).unwrap();
        assert_eq!(parsed.date, NaiveDate::from_ymd_opt(2026, 5, 11).unwrap());
    }
}
