//! Argument schemas for MCP prompts.
//!
//! Unlike tool inputs, a prompt's argument struct does double duty: each field
//! becomes a *labeled, user-facing form field* in MCP clients (Claude Desktop
//! renders one input per argument, required ones marked). So the field set and
//! the doc-comments here are a UX surface, not just a parser contract — the
//! split into discrete optional fields is deliberate: it turns the weekly
//! planning prompt into a checklist the human fills in, instead of one
//! free-form box that's easy to under-specify.

use schemars::JsonSchema;
use serde::Deserialize;

use crate::dto::deserialize_optional_string_empty_as_none;

/// Inputs to the `weekly_dinner_plan` prompt. Two required fields anchor the
/// week; the four optional fields are the recurring categories a human tends to
/// forget — surfacing each as its own form field is the point.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WeeklyDinnerPlanArgs {
    /// Monday of the week to plan, in YYYY-MM-DD format. Any day of the week is
    /// accepted and snapped to that week's Monday — the plan always covers
    /// Monday through Sunday.
    pub week_start_date: String,

    /// This week's schedule, in plain prose: per-day activities, who's home or
    /// away, evening commitments, and any easy / fast-food nights. Write it
    /// however is natural — the assistant parses the prose.
    pub family_schedule: String,

    /// On-hand ingredients to prioritize using up this week (e.g. "frozen Dover
    /// sole filets, chile-verde chicken burger patties"). Leave blank if none.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string_empty_as_none"
    )]
    pub ingredients_to_use_up: Option<String>,

    /// Seasonal, weather, or cuisine influence for the week (e.g. "getting hot
    /// out — lean lighter, less oven time"). Leave blank if nothing special.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string_empty_as_none"
    )]
    pub style_or_season: Option<String>,

    /// Preference for new vs. existing recipes, and how to choose among existing
    /// ones (e.g. "mostly new this week; for repeats, favor ones we haven't
    /// planned in a while"). Leave blank for no preference.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string_empty_as_none"
    )]
    pub recipe_preference: Option<String>,

    /// Energy, time, or physical limits that should shape how much effort each
    /// meal takes (e.g. "back issue — keep prep low-effort, less time on my
    /// feet"). Leave blank if no constraints.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string_empty_as_none"
    )]
    pub effort_constraints: Option<String>,
}
