//! The `weekly_dinner_plan` prompt body.
//!
//! [`render`] is the single source of the canonical week-planning workflow Steve
//! pastes into Claude Desktop by hand. Keeping it a pure function (no I/O, no
//! request context) makes the exact wording snapshot-testable, so accidental
//! drift in the household-canonical workflow fails CI rather than silently
//! shipping.

use std::fmt::Write as _;

use chrono::{Datelike, Duration, NaiveDate};

use crate::mcp::schemas::WeeklyDinnerPlanArgs;

/// Snap any date to the Monday of its week. Mirrors the frontend's
/// `getMonday()` (`src/utils/dates.ts`): a Sunday belongs to the *prior*
/// week, so it snaps back to that week's Monday rather than forward.
pub(crate) fn monday_of_week(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
}

/// Render the planning prompt for a week starting `monday` (already snapped).
/// `monday` is trusted to be a Monday; `render` only formats and offsets it.
pub fn render(monday: NaiveDate, args: &WeeklyDinnerPlanArgs) -> String {
    let sunday = monday + Duration::days(6);
    let date_range = format!("{monday} to {sunday}");

    let mut out = String::new();

    let _ = writeln!(
        out,
        "Help me plan dinners for the week of Monday {monday} through Sunday \
         {sunday} using the fewd tools.\n"
    );

    out.push_str("This week's context:\n");
    let _ = writeln!(out, "- Family schedule: {}", args.family_schedule.trim());
    push_optional(
        &mut out,
        "Ingredients to use up",
        &args.ingredients_to_use_up,
    );
    push_optional(&mut out, "Style / season", &args.style_or_season);
    push_optional(&mut out, "Recipe preference", &args.recipe_preference);
    push_optional(
        &mut out,
        "Effort / energy constraints",
        &args.effort_constraints,
    );

    let _ = write!(
        out,
        "\nAlways, when planning:\n\
         - Consider each family member's fewd preferences — call \
         get_family_overview (or attach the fewd://family/overview resource) \
         and list_people for diets, dislikes, and favorites.\n\
         - Weigh recipe ratings, and when reusing existing recipes favor ones \
         we haven't planned in a while — use list_curated_recipes and \
         search_recipes (which surface ratings and recency), and get_recipe for \
         full details.\n\
         - Prefer NEW recipes unless I said otherwise — but DON'T store any new \
         recipe until I approve it. Propose it first; only then create_recipe.\n\
         - Check what's already scheduled before adding anything — list_meals \
         over {date_range} to avoid duplicates.\n\
         \n\
         Workflow:\n\
         1. Propose the full week's dinner plan first. Don't schedule anything \
         yet.\n\
         2. Ask me any clarifying questions before deciding.\n\
         3. After I confirm: schedule each day with create_meal (Dinner slot), \
         and create_recipe for any new recipes I approved.\n\
         4. Build the grocery list with get_shopping_list over {date_range}.\n\
         5. Make the fridge printable with get_meal_planner_printable once the \
         week is scheduled.\n\
         \n\
         As you go, tell me if there are fewd tools missing that would help you \
         plan better — that feedback shapes what we build next.\n\
         \n\
         All dates are YYYY-MM-DD."
    );

    out
}

/// Append `- {label}: {value}` only when the optional arg carries non-blank
/// prose. The arg deserializer already maps empty strings to `None`; trimming
/// here keeps the rendered line clean if a value has surrounding whitespace.
fn push_optional(out: &mut String, label: &str, value: &Option<String>) {
    if let Some(v) = value.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let _ = writeln!(out, "- {label}: {v}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_example_args() -> WeeklyDinnerPlanArgs {
        WeeklyDinnerPlanArgs {
            week_start_date: "2026-05-25".to_string(),
            family_schedule: "Monday: Girl Scouts for just Viv. Wednesday: Cleo \
                has aikido 5-6pm and Amanda has book club 6-9pm, so an easy/fast-food \
                night is fine. Every other day is normal."
                .to_string(),
            ingredients_to_use_up: Some(
                "frozen Dover sole filets (2), chile-verde chicken burger patties".to_string(),
            ),
            style_or_season: Some("getting hot out — lean lighter, less oven time".to_string()),
            recipe_preference: Some(
                "mostly new this week; for repeats favor ones we haven't planned in a while"
                    .to_string(),
            ),
            effort_constraints: Some(
                "back issue — keep prep low-effort, less time on my feet".to_string(),
            ),
        }
    }

    fn minimal_args() -> WeeklyDinnerPlanArgs {
        WeeklyDinnerPlanArgs {
            week_start_date: "2026-05-25".to_string(),
            family_schedule: "Normal week, everyone home.".to_string(),
            ingredients_to_use_up: None,
            style_or_season: None,
            recipe_preference: None,
            effort_constraints: None,
        }
    }

    /// Full-args snapshot. Pins the exact canonical workflow text so any
    /// accidental wording drift fails CI. Update deliberately when the
    /// household workflow genuinely changes.
    #[test]
    fn renders_full_example_verbatim() {
        let monday = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap();
        let expected = "Help me plan dinners for the week of Monday 2026-05-25 through Sunday 2026-05-31 using the fewd tools.\n\
\n\
This week's context:\n\
- Family schedule: Monday: Girl Scouts for just Viv. Wednesday: Cleo has aikido 5-6pm and Amanda has book club 6-9pm, so an easy/fast-food night is fine. Every other day is normal.\n\
- Ingredients to use up: frozen Dover sole filets (2), chile-verde chicken burger patties\n\
- Style / season: getting hot out — lean lighter, less oven time\n\
- Recipe preference: mostly new this week; for repeats favor ones we haven't planned in a while\n\
- Effort / energy constraints: back issue — keep prep low-effort, less time on my feet\n\
\n\
Always, when planning:\n\
- Consider each family member's fewd preferences — call get_family_overview (or attach the fewd://family/overview resource) and list_people for diets, dislikes, and favorites.\n\
- Weigh recipe ratings, and when reusing existing recipes favor ones we haven't planned in a while — use list_curated_recipes and search_recipes (which surface ratings and recency), and get_recipe for full details.\n\
- Prefer NEW recipes unless I said otherwise — but DON'T store any new recipe until I approve it. Propose it first; only then create_recipe.\n\
- Check what's already scheduled before adding anything — list_meals over 2026-05-25 to 2026-05-31 to avoid duplicates.\n\
\n\
Workflow:\n\
1. Propose the full week's dinner plan first. Don't schedule anything yet.\n\
2. Ask me any clarifying questions before deciding.\n\
3. After I confirm: schedule each day with create_meal (Dinner slot), and create_recipe for any new recipes I approved.\n\
4. Build the grocery list with get_shopping_list over 2026-05-25 to 2026-05-31.\n\
5. Make the fridge printable with get_meal_planner_printable once the week is scheduled.\n\
\n\
As you go, tell me if there are fewd tools missing that would help you plan better — that feedback shapes what we build next.\n\
\n\
All dates are YYYY-MM-DD.";
        assert_eq!(render(monday, &full_example_args()), expected);
    }

    /// Optional lines are omitted entirely when their arg is absent, but the
    /// required schedule line and the full always-rules / workflow still render.
    #[test]
    fn minimal_args_omit_optional_lines() {
        let monday = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap();
        let rendered = render(monday, &minimal_args());

        assert!(rendered.contains("- Family schedule: Normal week, everyone home.\n"));
        assert!(!rendered.contains("Ingredients to use up"));
        assert!(!rendered.contains("Style / season"));
        assert!(!rendered.contains("Recipe preference"));
        assert!(!rendered.contains("Effort / energy constraints"));
        // Canonical workflow is unconditional.
        assert!(rendered.contains("1. Propose the full week's dinner plan first."));
        assert!(rendered.contains("get_meal_planner_printable"));
        assert!(rendered.contains("tools missing that would help you plan better"));
    }

    /// Whitespace-only optional values are treated as absent.
    #[test]
    fn blank_optional_is_omitted() {
        let monday = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap();
        let mut args = minimal_args();
        args.style_or_season = Some("   ".to_string());
        assert!(!render(monday, &args).contains("Style / season"));
    }

    #[test]
    fn monday_stays_put() {
        let monday = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(); // Monday
        assert_eq!(monday_of_week(monday), monday);
    }

    #[test]
    fn midweek_snaps_back_to_monday() {
        let wednesday = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        assert_eq!(
            monday_of_week(wednesday),
            NaiveDate::from_ymd_opt(2026, 5, 25).unwrap()
        );
    }

    #[test]
    fn sunday_snaps_back_to_prior_monday() {
        // Mirrors getMonday(): Sunday belongs to the week that started the
        // preceding Monday, not the next day.
        let sunday = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        assert_eq!(
            monday_of_week(sunday),
            NaiveDate::from_ymd_opt(2026, 5, 18).unwrap()
        );
    }

    #[test]
    fn rendered_range_runs_monday_through_sunday() {
        let monday = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap();
        let rendered = render(monday, &minimal_args());
        assert!(rendered.contains("week of Monday 2026-05-25 through Sunday 2026-05-31"));
        assert!(rendered.contains("get_shopping_list over 2026-05-25 to 2026-05-31"));
    }
}
