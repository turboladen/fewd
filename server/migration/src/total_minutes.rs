//! Converts a recipe `total_time` (value + free-form unit) into whole minutes.
//!
//! Shared by the runtime recipe create/update path (`recipe_service`) and the
//! `m20260607_000019` backfill migration so the value the `search_recipes`
//! time filter compares is normalized identically whether a recipe was authored
//! before or after the `total_minutes` column landed. Living in the migration
//! crate keeps that single source of truth (the server depends on `migration`).

/// Normalize a `(value, unit)` duration into whole minutes. Unit matching is
/// case-insensitive and trims surrounding whitespace.
///
/// Returns `None` for an unrecognized unit so callers can distinguish "not
/// time-filterable" from "0 minutes" — a recipe whose unit we can't interpret
/// is left out of time filters rather than silently compared on the wrong scale
/// (the bug this replaces: `json_extract($.value)` assumed every unit was minutes).
pub fn total_time_to_minutes(value: i32, unit: &str) -> Option<i32> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "minute" | "minutes" | "min" | "mins" | "m" => Some(value),
        "hour" | "hours" | "hr" | "hrs" | "h" => value.checked_mul(60),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minute_units_pass_value_through() {
        for unit in [
            "minutes",
            "minute",
            "min",
            "mins",
            "m",
            "MIN",
            "  Minutes  ",
        ] {
            assert_eq!(total_time_to_minutes(30, unit), Some(30), "unit: {unit:?}");
        }
    }

    #[test]
    fn hour_units_multiply_by_sixty() {
        for unit in ["hours", "hour", "hr", "hrs", "h", "HR", " Hours "] {
            assert_eq!(total_time_to_minutes(2, unit), Some(120), "unit: {unit:?}");
        }
    }

    #[test]
    fn unrecognized_unit_is_none() {
        assert_eq!(total_time_to_minutes(5, "fortnights"), None);
        assert_eq!(total_time_to_minutes(5, ""), None);
    }

    #[test]
    fn overflow_saturates_to_none_not_panic() {
        assert_eq!(total_time_to_minutes(i32::MAX, "hours"), None);
    }
}
