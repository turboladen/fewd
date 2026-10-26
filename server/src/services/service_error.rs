use migration::total_minutes::ACCEPTED_TIME_UNITS;
use sea_orm::DbErr;

use crate::dto::PersonServingDto;

/// Describes how a service write that validates its input can fail.
#[derive(Debug)]
pub enum ServiceError {
    /// The input broke a domain rule. The message names the field and the
    /// accepted values, so the caller can retry with corrected input.
    //
    // The HTTP layer answers 400 and the MCP layer returns a tool-level error.
    Validation(ValidationError),
    /// Wraps a SeaORM or SQLite failure, which bubbles through the standard
    /// error pipeline.
    Database(DbErr),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(e) => e.fmt(f),
            Self::Database(e) => e.fmt(f),
        }
    }
}

impl From<DbErr> for ServiceError {
    fn from(e: DbErr) -> Self {
        Self::Database(e)
    }
}

impl From<ValidationError> for ServiceError {
    fn from(e: ValidationError) -> Self {
        Self::Validation(e)
    }
}

impl std::error::Error for ServiceError {}

impl std::error::Error for ValidationError {}

/// Names the domain rule a write broke. Every variant's `Display` message is
/// shown to callers verbatim: it opens with the name of the input field at
/// fault (`rating`, `prep_time`, ...) and says how to fix the value.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// `rating` does not round to a whole star from 1 to 5. Carries the
    /// rating exactly as the caller sent it.
    RatingOutOfRange(f64),
    /// A recipe time field (`prep_time`, `cook_time`, `total_time`) carries
    /// a unit that is not minutes, hours, or days.
    UnrecognizedTimeUnit { field: &'static str, unit: String },
    /// A recipe time field carries a negative value.
    NegativeTime { field: &'static str, value: i32 },
    /// The duration does not fit in whole minutes.
    TimeTooLarge { field: &'static str },
    /// `servings` is below 1. Carries the count as the caller sent it.
    NonPositiveServings(i32),
    /// A meal serving's `servings_count` is not a finite number above 0.
    /// Carries the count as the caller sent it.
    NonPositiveServingsCount(f64),
    /// A stored meal holds a `servings_count` that is not a finite number
    /// above 0, so it cannot be copied into a template.
    StoredServingsCount { meal_id: String, count: f64 },
    /// A meal `date` is not a calendar date in `YYYY-MM-DD` form. Carries
    /// the date as the caller sent it.
    InvalidDate(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RatingOutOfRange(n) => write!(
                f,
                "rating must be a whole number from 1 to 5 (got {n}). Fractional values round to the nearest star."
            ),
            Self::UnrecognizedTimeUnit { field, unit } => write!(
                f,
                "{field} unit '{unit}' is not recognized. Use {ACCEPTED_TIME_UNITS}."
            ),
            Self::NegativeTime { field, value } => {
                write!(f, "{field} must not be negative (got {value}).")
            }
            Self::TimeTooLarge { field } => {
                write!(
                    f,
                    "{field} is too long to store as whole minutes. Use a shorter duration."
                )
            }
            Self::NonPositiveServings(n) => write!(
                f,
                "servings must be >= 1 (got {n}). Recipes need at least one serving so shopping-list scaling works."
            ),
            Self::NonPositiveServingsCount(n) => write!(
                f,
                "servings_count must be > 0 (got {n}). Use 0.5 for a half portion; negative or zero values would corrupt the shopping list."
            ),
            Self::StoredServingsCount { meal_id, count } => write!(
                f,
                "servings_count {count} on meal {meal_id} is not above 0. Edit the meal so every servings_count is above 0, then save it as a template."
            ),
            Self::InvalidDate(date) => write!(
                f,
                "date must be a calendar date in YYYY-MM-DD form (got '{date}')."
            ),
        }
    }
}

/// Check that every recipe serving in a meal or template asks for a finite
/// `servings_count` above 0. Ad-hoc servings carry no count and always pass.
///
/// # Errors
///
/// Returns [`ValidationError::NonPositiveServingsCount`] for the first count
/// that fails.
pub fn check_servings_counts(servings: &[PersonServingDto]) -> Result<(), ValidationError> {
    for serving in servings {
        if let PersonServingDto::Recipe { servings_count, .. } = serving {
            if !servings_count.is_finite() || *servings_count <= 0.0 {
                return Err(ValidationError::NonPositiveServingsCount(*servings_count));
            }
        }
    }
    Ok(())
}

/// Round `rating` to a whole star, rejecting a value that does not round
/// into 1–5. NaN and the infinities are rejected.
pub fn whole_star_rating(rating: f64) -> Result<f64, ValidationError> {
    let rounded = rating.round();
    if !(1.0..=5.0).contains(&rounded) {
        return Err(ValidationError::RatingOutOfRange(rating));
    }
    Ok(rounded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_star_rating_rounds_into_range() {
        assert_eq!(whole_star_rating(4.4), Ok(4.0));
        assert_eq!(whole_star_rating(0.5), Ok(1.0));
        assert_eq!(whole_star_rating(5.4), Ok(5.0));
    }

    #[test]
    fn whole_star_rating_rejects_values_outside_one_to_five() {
        for raw in [0.0, 0.4, 5.5, -1.0, f64::NAN, f64::INFINITY] {
            assert!(
                matches!(
                    whole_star_rating(raw),
                    Err(ValidationError::RatingOutOfRange(_))
                ),
                "{raw} must be rejected"
            );
        }
    }

    #[test]
    fn rating_message_reports_the_value_sent() {
        // The range check runs on the rounded value; reporting that instead
        // would reject 5.6 with "(got 6)", a number the caller never sent.
        let message = whole_star_rating(5.6).unwrap_err().to_string();
        assert!(message.contains("1 to 5"), "{message}");
        assert!(message.contains("got 5.6"), "{message}");
    }

    #[test]
    fn every_validation_message_opens_with_its_field() {
        // The match in `field_of` is exhaustive, so a new variant fails to
        // compile here until it names the field it concerns. Add a sample of
        // it to `samples` at the same time.
        fn field_of(err: &ValidationError) -> &'static str {
            match err {
                ValidationError::RatingOutOfRange(_) => "rating",
                ValidationError::UnrecognizedTimeUnit { field, .. }
                | ValidationError::NegativeTime { field, .. }
                | ValidationError::TimeTooLarge { field } => field,
                ValidationError::NonPositiveServings(_) => "servings",
                ValidationError::NonPositiveServingsCount(_)
                | ValidationError::StoredServingsCount { .. } => "servings_count",
                ValidationError::InvalidDate(_) => "date",
            }
        }
        let samples = [
            ValidationError::RatingOutOfRange(9.0),
            ValidationError::UnrecognizedTimeUnit {
                field: "cook_time",
                unit: "sols".into(),
            },
            ValidationError::NegativeTime {
                field: "prep_time",
                value: -1,
            },
            ValidationError::TimeTooLarge {
                field: "total_time",
            },
            ValidationError::NonPositiveServings(0),
            ValidationError::NonPositiveServingsCount(0.0),
            ValidationError::StoredServingsCount {
                meal_id: "meal-1".into(),
                count: 0.0,
            },
            ValidationError::InvalidDate("06/10/2025".into()),
        ];
        for err in samples {
            let message = err.to_string();
            assert!(message.starts_with(field_of(&err)), "{message}");
        }
    }

    #[test]
    fn database_variant_displays_the_underlying_error() {
        let err = ServiceError::from(DbErr::Custom("disk full".into()));
        assert!(err.to_string().contains("disk full"));
    }
}
