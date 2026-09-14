use migration::total_minutes::ACCEPTED_TIME_UNITS;
use sea_orm::DbErr;

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
        }
    }
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
