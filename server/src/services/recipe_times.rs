use migration::total_minutes::canonical_time_unit;

use crate::dto::TimeValueDto;
use crate::services::service_error::ValidationError;

/// A recipe duration whose unit is recognized, with the unit in its
/// canonical plural form (`minutes`, `hours`, or `days`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTime {
    pub value: i32,
    pub unit: &'static str,
    pub minutes: i32,
}

impl CheckedTime {
    pub fn to_dto(&self) -> TimeValueDto {
        TimeValueDto {
            value: self.value,
            unit: self.unit.to_string(),
        }
    }
}

/// Validate one recipe time field and canonicalize its unit. `field` names
/// the field in the returned error, e.g. `"cook_time"`.
///
/// # Errors
///
/// Returns [`ValidationError`] when the unit is not minutes, hours, or days,
/// when the value is negative, or when the duration overflows whole minutes.
pub fn check_time(
    field: &'static str,
    time: &TimeValueDto,
) -> Result<CheckedTime, ValidationError> {
    let Some((unit, minutes_per_unit)) = canonical_time_unit(&time.unit) else {
        return Err(ValidationError::UnrecognizedTimeUnit {
            field,
            unit: time.unit.clone(),
        });
    };
    if time.value < 0 {
        return Err(ValidationError::NegativeTime {
            field,
            value: time.value,
        });
    }
    let minutes = time
        .value
        .checked_mul(minutes_per_unit)
        .ok_or(ValidationError::TimeTooLarge { field })?;
    Ok(CheckedTime {
        value: time.value,
        unit,
        minutes,
    })
}

/// The three time columns as a recipe row stores them (JSON text).
#[derive(Debug, Default, Clone, Copy)]
pub struct StoredTimes<'a> {
    pub prep_time: Option<&'a str>,
    pub cook_time: Option<&'a str>,
    pub total_time: Option<&'a str>,
}

/// The time fields a write request carries. `None` means "not sent".
#[derive(Debug, Default, Clone)]
pub struct SentTimes {
    pub prep_time: Option<TimeValueDto>,
    pub cook_time: Option<TimeValueDto>,
    pub total_time: Option<TimeValueDto>,
}

/// The time columns a write must set. `None` leaves a column exactly as
/// stored; a `Some` total also sets `total_minutes`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TimeWrites {
    pub prep_time: Option<CheckedTime>,
    pub cook_time: Option<CheckedTime>,
    pub total_time: Option<CheckedTime>,
}

/// Decide which time columns a write changes, validating only those.
///
/// A sent field equal to the stored one is not a change: it is neither
/// validated nor rewritten. Equal means the same number of minutes, or,
/// when the stored value cannot be parsed, the same raw value and unit.
/// Pass [`StoredTimes::default`] for a create, where every sent field is a
/// change.
///
/// # Errors
///
/// Returns [`ValidationError`] for a changed field that [`check_time`]
/// rejects.
//
// The web form resends all three fields on every save, including a legacy
// unit its select cannot represent, so validating an untouched field would
// reject a name-only edit over a value the user never saw.
pub fn resolve_times(
    stored: StoredTimes<'_>,
    sent: &SentTimes,
) -> Result<TimeWrites, ValidationError> {
    Ok(TimeWrites {
        prep_time: changed_time("prep_time", stored.prep_time, sent.prep_time.as_ref())?,
        cook_time: changed_time("cook_time", stored.cook_time, sent.cook_time.as_ref())?,
        total_time: changed_time("total_time", stored.total_time, sent.total_time.as_ref())?,
    })
}

/// A stored time column, parsed as far as it will go.
#[derive(Debug)]
enum Stored {
    Missing,
    Parsed(CheckedTime),
    /// Malformed JSON (`None`), or well-formed JSON `check_time` rejects.
    Unparseable(Option<TimeValueDto>),
}

fn parse_stored(field: &'static str, raw: Option<&str>) -> Stored {
    let Some(raw) = raw else {
        return Stored::Missing;
    };
    let Ok(time) = serde_json::from_str::<TimeValueDto>(raw) else {
        return Stored::Unparseable(None);
    };
    match check_time(field, &time) {
        Ok(checked) => Stored::Parsed(checked),
        Err(_) => Stored::Unparseable(Some(time)),
    }
}

fn changed_time(
    field: &'static str,
    stored: Option<&str>,
    sent: Option<&TimeValueDto>,
) -> Result<Option<CheckedTime>, ValidationError> {
    let Some(sent) = sent else {
        return Ok(None);
    };
    let unchanged = match parse_stored(field, stored) {
        Stored::Missing => false,
        Stored::Parsed(old) => check_time(field, sent).is_ok_and(|new| new.minutes == old.minutes),
        Stored::Unparseable(old) => {
            old.is_some_and(|old| old.value == sent.value && old.unit == sent.unit)
        }
    };
    if unchanged {
        return Ok(None);
    }
    check_time(field, sent).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time(value: i32, unit: &str) -> TimeValueDto {
        TimeValueDto {
            value,
            unit: unit.into(),
        }
    }

    fn json(value: i32, unit: &str) -> String {
        serde_json::to_string(&time(value, unit)).unwrap()
    }

    fn checked(value: i32, unit: &'static str, minutes: i32) -> CheckedTime {
        CheckedTime {
            value,
            unit,
            minutes,
        }
    }

    #[test]
    fn check_time_canonicalizes_recognized_units() {
        assert_eq!(
            check_time("prep_time", &time(2, " HR ")),
            Ok(checked(2, "hours", 120))
        );
        assert_eq!(
            check_time("prep_time", &time(1, "d")),
            Ok(checked(1, "days", 1440))
        );
    }

    #[test]
    fn check_time_rejects_unusable_values_naming_the_field() {
        assert_eq!(
            check_time("cook_time", &time(5, "fortnights")),
            Err(ValidationError::UnrecognizedTimeUnit {
                field: "cook_time",
                unit: "fortnights".into()
            })
        );
        assert_eq!(
            check_time("cook_time", &time(-5, "minutes")),
            Err(ValidationError::NegativeTime {
                field: "cook_time",
                value: -5
            })
        );
        assert_eq!(
            check_time("total_time", &time(i32::MAX, "hours")),
            Err(ValidationError::TimeTooLarge {
                field: "total_time"
            })
        );
    }

    #[test]
    fn unrecognized_unit_message_names_the_accepted_units() {
        let message = check_time("total_time", &time(3, "sols"))
            .unwrap_err()
            .to_string();
        for fragment in ["total_time", "'sols'", "minutes", "hours", "days"] {
            assert!(message.contains(fragment), "{fragment:?} in {message}");
        }
    }

    #[test]
    fn create_writes_every_sent_field_canonicalized() {
        let writes = resolve_times(
            StoredTimes::default(),
            &SentTimes {
                prep_time: Some(time(10, "min")),
                cook_time: None,
                total_time: Some(time(1, "hr")),
            },
        )
        .unwrap();
        assert_eq!(writes.prep_time, Some(checked(10, "minutes", 10)));
        assert_eq!(writes.cook_time, None);
        assert_eq!(writes.total_time, Some(checked(1, "hours", 60)));
    }

    #[test]
    fn sent_field_with_the_stored_minutes_is_not_rewritten() {
        let stored_total = json(1, "hours");
        let writes = resolve_times(
            StoredTimes {
                total_time: Some(&stored_total),
                ..Default::default()
            },
            &SentTimes {
                total_time: Some(time(60, "minutes")),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(writes, TimeWrites::default());
    }

    #[test]
    fn legacy_unit_resent_unchanged_is_neither_validated_nor_rewritten() {
        let stored_total = json(3, "fortnights");
        let writes = resolve_times(
            StoredTimes {
                total_time: Some(&stored_total),
                ..Default::default()
            },
            &SentTimes {
                total_time: Some(time(3, "fortnights")),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(writes, TimeWrites::default());
    }

    #[test]
    fn changed_field_with_an_unrecognized_unit_is_rejected() {
        let stored_cook = json(20, "minutes");
        let err = resolve_times(
            StoredTimes {
                cook_time: Some(&stored_cook),
                ..Default::default()
            },
            &SentTimes {
                cook_time: Some(time(4, "fortnights")),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ValidationError::UnrecognizedTimeUnit {
                field: "cook_time",
                ..
            }
        ));
    }

    #[test]
    fn valid_value_replacing_a_legacy_unit_is_written() {
        let stored_total = json(3, "fortnights");
        let writes = resolve_times(
            StoredTimes {
                total_time: Some(&stored_total),
                ..Default::default()
            },
            &SentTimes {
                total_time: Some(time(45, "minutes")),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(writes.total_time, Some(checked(45, "minutes", 45)));
    }
}
