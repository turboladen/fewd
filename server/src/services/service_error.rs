use sea_orm::DbErr;

/// Failure modes returned by service writes that validate their input.
#[derive(Debug)]
pub enum ServiceError {
    /// The input broke a domain rule. The message names the field and the
    /// accepted values, so the caller can retry with corrected input; the
    /// HTTP layer answers 400 and the MCP layer returns a tool-level error.
    Validation(ValidationError),
    /// SeaORM / SQLite failure. Bubbles through the standard error pipeline.
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

/// A domain rule a write broke. The `Display` text is shown to callers
/// verbatim, so it names the field and says how to fix the value.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// Carries the rating exactly as the caller sent it.
    RatingOutOfRange(f64),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RatingOutOfRange(n) => write!(
                f,
                "rating must be a whole number from 1 to 5 (got {n}). Fractional values round to the nearest star."
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
    fn database_variant_displays_the_underlying_error() {
        let err = ServiceError::from(DbErr::Custom("disk full".into()));
        assert!(err.to_string().contains("disk full"));
    }
}
