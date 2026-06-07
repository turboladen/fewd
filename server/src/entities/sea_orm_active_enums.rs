//! Shared SeaORM active enums (mirrors the file `sea-orm-cli` generates for enums
//! used by more than one entity). `MealType` backs the `meal_type` column on both
//! `meals` and `meal_templates`.

use std::fmt;
use std::str::FromStr;

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// The four meal slots the planner renders. Stored as Title-Case TEXT
/// (`"Breakfast"`/`"Lunch"`/`"Dinner"`/`"Snack"`) so the column type is unchanged.
///
/// `DeriveActiveEnum` controls the DB representation; the separate serde derives +
/// `#[serde(rename)]` pin the JSON wire value to the same Title-Case strings — the
/// HTTP routes return `Json<meal::Model>` directly, so the serde form *is* the
/// contract the web planner does strict equality against (`meal_type === 'Dinner'`).
///
/// Input normalization (accepting lowercase `"dinner"` from the MCP/LLM) lives in
/// [`FromStr`], which is case-insensitive — this is what absorbed the old
/// `canonical_meal_type` helper.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum MealType {
    #[sea_orm(string_value = "Breakfast")]
    #[serde(rename = "Breakfast")]
    Breakfast,
    #[sea_orm(string_value = "Lunch")]
    #[serde(rename = "Lunch")]
    Lunch,
    #[sea_orm(string_value = "Dinner")]
    #[serde(rename = "Dinner")]
    Dinner,
    #[sea_orm(string_value = "Snack")]
    #[serde(rename = "Snack")]
    Snack,
}

impl MealType {
    /// The canonical Title-Case string, matching both the DB representation and the
    /// serde wire value.
    pub fn as_str(&self) -> &'static str {
        match self {
            MealType::Breakfast => "Breakfast",
            MealType::Lunch => "Lunch",
            MealType::Dinner => "Dinner",
            MealType::Snack => "Snack",
        }
    }
}

impl FromStr for MealType {
    type Err = ();

    /// Case-insensitive, whitespace-trimming parse. Callers map `Err(())` to whatever
    /// actionable error fits their surface (the MCP layer → `InputError::UnknownMealType`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "breakfast" => Ok(MealType::Breakfast),
            "lunch" => Ok(MealType::Lunch),
            "dinner" => Ok(MealType::Dinner),
            "snack" => Ok(MealType::Snack),
            _ => Err(()),
        }
    }
}

impl fmt::Display for MealType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<MealType> for String {
    fn from(value: MealType) -> Self {
        value.as_str().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_is_case_insensitive_and_trims() {
        for input in ["dinner", "DINNER", "Dinner", "  dInNeR  "] {
            assert_eq!(
                input.parse::<MealType>(),
                Ok(MealType::Dinner),
                "input: {input:?}"
            );
        }
        assert_eq!("breakfast".parse(), Ok(MealType::Breakfast));
        assert_eq!("LUNCH".parse(), Ok(MealType::Lunch));
        assert_eq!(" snack ".parse(), Ok(MealType::Snack));
    }

    #[test]
    fn from_str_rejects_unknown() {
        assert_eq!("brunch".parse::<MealType>(), Err(()));
        assert_eq!("".parse::<MealType>(), Err(()));
    }

    #[test]
    fn display_and_into_string_are_title_case() {
        assert_eq!(MealType::Dinner.to_string(), "Dinner");
        assert_eq!(String::from(MealType::Snack), "Snack");
    }

    /// The load-bearing guard: the serde wire value MUST be the exact Title-Case the
    /// web planner compares against (`meal.meal_type === 'Dinner'`). If a future
    /// refactor adds `#[serde(rename_all)]` or drops a rename, this fails.
    #[test]
    fn serde_value_pins_title_case_and_round_trips() {
        assert_eq!(
            serde_json::to_string(&MealType::Dinner).unwrap(),
            "\"Dinner\""
        );
        assert_eq!(
            serde_json::to_string(&MealType::Breakfast).unwrap(),
            "\"Breakfast\""
        );
        let parsed: MealType = serde_json::from_str("\"Lunch\"").unwrap();
        assert_eq!(parsed, MealType::Lunch);
    }

    /// `meal_type` is spelled out as a literal in four independent places: the
    /// DeriveActiveEnum `string_value` (DB), the `#[serde(rename)]` (HTTP wire), the
    /// `as_str` match, and the `FromStr` match. If any drifts from the others, a meal
    /// writes one string to the DB but serializes another to the planner — the exact
    /// invisible-meal bug this enum exists to kill. Assert all four agree, for every
    /// variant (so a newly-added variant can't be half-wired).
    #[test]
    fn all_string_representations_agree_for_every_variant() {
        use sea_orm::{ActiveEnum, Iterable};
        for variant in MealType::iter() {
            let canonical = variant.as_str();
            assert_eq!(
                variant.to_value(),
                canonical,
                "DB string_value != as_str for {variant:?}"
            );
            assert_eq!(
                serde_json::to_string(&variant).unwrap(),
                format!("\"{canonical}\""),
                "serde value != as_str for {variant:?}",
            );
            assert_eq!(
                canonical.parse::<MealType>(),
                Ok(variant),
                "FromStr != as_str for {variant:?}"
            );
        }
    }
}
