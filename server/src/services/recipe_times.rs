use migration::total_minutes::canonical_time_unit;

use crate::dto::{CreateRecipeDto, TimeValueDto};
use crate::services::service_error::ValidationError;

/// Holds a recipe duration whose unit is recognized, with the unit in its
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

/// Holds the three time columns as a recipe row stores them, as JSON text.
#[derive(Debug, Default, Clone, Copy)]
pub struct StoredTimes<'a> {
    pub prep_time: Option<&'a str>,
    pub cook_time: Option<&'a str>,
    pub total_time: Option<&'a str>,
}

/// Holds the time fields a write request carries, where `None` means the
/// field was not sent.
#[derive(Debug, Default, Clone)]
pub struct SentTimes {
    pub prep_time: Option<TimeValueDto>,
    pub cook_time: Option<TimeValueDto>,
    pub total_time: Option<TimeValueDto>,
}

/// Lists the time columns a write must set. `None` leaves a column exactly
/// as stored; a `Some` total also sets `total_minutes`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TimeWrites {
    pub prep_time: Option<CheckedTime>,
    pub cook_time: Option<CheckedTime>,
    pub total_time: Option<CheckedTime>,
}

/// Decide which time columns a write sets, validating only changed fields.
/// Pass [`StoredTimes::default`] for a create.
///
/// A sent field equal to the stored one (same minutes, or the same raw
/// value and unit when the stored value cannot be parsed) is not a change.
/// A changed total is written as sent. Otherwise, when prep or cook
/// changes, the total keeps its old remainder, total − (prep + cook), but
/// never drops below the longer phase; a phase first recorded by this write
/// is assumed to be inside the stored total. With no stored total it
/// becomes prep + cook, a missing phase counting as 0. An unparsable stored
/// value the derivation needs leaves the total as stored.
///
/// # Errors
///
/// Returns [`ValidationError`] for a changed field that [`check_time`]
/// rejects, or when a derived total overflows whole minutes.
//
// The web form resends all three fields on every save, including a legacy
// unit its select cannot represent, so validating an untouched field would
// reject a name-only edit over a value the user never saw. The same resend
// is why an unchanged total still lets a prep/cook change re-derive it.
pub fn resolve_times(
    stored: StoredTimes<'_>,
    sent: &SentTimes,
) -> Result<TimeWrites, ValidationError> {
    if sent.prep_time.is_none() && sent.cook_time.is_none() && sent.total_time.is_none() {
        return Ok(TimeWrites::default());
    }
    let old = StoredParts {
        prep: parse_stored("prep_time", stored.prep_time),
        cook: parse_stored("cook_time", stored.cook_time),
        total: parse_stored("total_time", stored.total_time),
    };
    let prep_time = changed_time("prep_time", &old.prep, sent.prep_time.as_ref())?;
    let cook_time = changed_time("cook_time", &old.cook, sent.cook_time.as_ref())?;
    let mut total_time = changed_time("total_time", &old.total, sent.total_time.as_ref())?;
    if total_time.is_none() && (prep_time.is_some() || cook_time.is_some()) {
        total_time = derive_total(&old, prep_time.as_ref(), cook_time.as_ref())?;
    }
    Ok(TimeWrites {
        prep_time,
        cook_time,
        total_time,
    })
}

/// Records how far a stored time column parses.
#[derive(Debug)]
enum Stored {
    Missing,
    Parsed(CheckedTime),
    /// Holds `None` for malformed JSON, or the decoded value when
    /// `check_time` rejects it.
    Unparsable(Option<TimeValueDto>),
}

struct StoredParts {
    prep: Stored,
    cook: Stored,
    total: Stored,
}

fn parse_stored(field: &'static str, raw: Option<&str>) -> Stored {
    let Some(raw) = raw else {
        return Stored::Missing;
    };
    let Ok(time) = serde_json::from_str::<TimeValueDto>(raw) else {
        return Stored::Unparsable(None);
    };
    match check_time(field, &time) {
        Ok(checked) => Stored::Parsed(checked),
        Err(_) => Stored::Unparsable(Some(time)),
    }
}

fn changed_time(
    field: &'static str,
    stored: &Stored,
    sent: Option<&TimeValueDto>,
) -> Result<Option<CheckedTime>, ValidationError> {
    let Some(sent) = sent else {
        return Ok(None);
    };
    if matches!(stored, Stored::Unparsable(Some(old)) if old.value == sent.value && old.unit == sent.unit)
    {
        return Ok(None);
    }
    let new = check_time(field, sent)?;
    if matches!(stored, Stored::Parsed(old) if old.minutes == new.minutes) {
        return Ok(None);
    }
    Ok(Some(new))
}

/// Carries one phase's share of a derived total.
#[derive(Debug, Clone, Copy)]
struct Part {
    minutes: i64,
    unit: Option<&'static str>,
}

/// Returns the phase as the write leaves it: the changed value if there is
/// one, else the stored value. Returns `None` when that value cannot be
/// parsed.
fn part(change: Option<&CheckedTime>, stored: &Stored) -> Option<Part> {
    let checked = match (change, stored) {
        (Some(new), _) => new,
        (None, Stored::Parsed(old)) => old,
        (None, Stored::Missing) => {
            return Some(Part {
                minutes: 0,
                unit: None,
            })
        }
        (None, Stored::Unparsable(_)) => return None,
    };
    Some(Part {
        minutes: checked.minutes.into(),
        unit: Some(checked.unit),
    })
}

fn derive_total(
    old: &StoredParts,
    prep: Option<&CheckedTime>,
    cook: Option<&CheckedTime>,
) -> Result<Option<CheckedTime>, ValidationError> {
    let (Some(new_prep), Some(new_cook)) = (part(prep, &old.prep), part(cook, &old.cook)) else {
        return Ok(None);
    };
    let sum = new_prep.minutes + new_cook.minutes;
    let (minutes, old_total_unit) = match &old.total {
        Stored::Missing => (sum, None),
        Stored::Unparsable(_) => return Ok(None),
        Stored::Parsed(old_total) => {
            let (Some(old_prep), Some(old_cook)) = (part(None, &old.prep), part(None, &old.cook))
            else {
                return Ok(None);
            };
            // The remainder is negative when prep and cook overlap. A phase
            // with no stored value adds nothing on top of the old total: a
            // recipe that only recorded its total already spent that time
            // somewhere. The floor stops a shrinking phase from pulling the
            // total below the longest single phase, which no recipe can beat.
            let remainder = i64::from(old_total.minutes) - old_prep.minutes - old_cook.minutes;
            let recorded = |new: Part, stored: &Stored| match stored {
                Stored::Missing => 0,
                _ => new.minutes,
            };
            let moved = recorded(new_prep, &old.prep) + recorded(new_cook, &old.cook);
            let floor = new_prep.minutes.max(new_cook.minutes);
            ((moved + remainder).max(floor), Some(old_total.unit))
        }
    };
    let minutes = i32::try_from(minutes).map_err(|_| ValidationError::TimeTooLarge {
        field: "total_time",
    })?;
    let shared_unit = match (new_prep.unit, new_cook.unit) {
        (Some(a), Some(b)) if a != b => None,
        (a, b) => a.or(b),
    };
    Ok(Some(express_minutes(
        minutes,
        [old_total_unit, shared_unit],
    )))
}

/// Express `minutes` in the first preferred unit that divides it evenly,
/// falling back to minutes.
fn express_minutes(minutes: i32, preferred: [Option<&'static str>; 2]) -> CheckedTime {
    preferred
        .into_iter()
        .flatten()
        .find_map(|unit| {
            let (unit, minutes_per_unit) = canonical_time_unit(unit)?;
            (minutes % minutes_per_unit == 0).then_some(CheckedTime {
                value: minutes / minutes_per_unit,
                unit,
                minutes,
            })
        })
        .unwrap_or(CheckedTime {
            value: minutes,
            unit: "minutes",
            minutes,
        })
}

/// Drops each time field of an AI-generated recipe that [`check_time`]
/// rejects, logging a warning for each one. Call it on every recipe the
/// model produces, whether it is saved at once (import) or handed back as a
/// draft the client saves later (adapt, suggest).
//
// Tokens are already spent by then, and a unit the model invented is not
// something the person can fix (the web form's unit select cannot even
// show it), so losing one time field is the better failure than losing the
// recipe. Direct create and update calls still reject the same values.
pub fn drop_unusable_import_times(dto: &mut CreateRecipeDto) {
    let recipe = &dto.name;
    for (field, slot) in [
        ("prep_time", &mut dto.prep_time),
        ("cook_time", &mut dto.cook_time),
        ("total_time", &mut dto.total_time),
    ] {
        let Some(time) = slot.as_ref() else {
            continue;
        };
        if let Err(error) = check_time(field, time) {
            tracing::warn!(%recipe, %error, "dropping unusable time from imported recipe");
            *slot = None;
        }
    }
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

    fn imported(
        prep: Option<(i32, &str)>,
        cook: Option<(i32, &str)>,
        total: Option<(i32, &str)>,
    ) -> CreateRecipeDto {
        let field = |t: Option<(i32, &str)>| t.map(|(v, u)| time(v, u));
        CreateRecipeDto {
            name: "Imported".into(),
            description: None,
            source: "url_import".into(),
            source_url: None,
            parent_recipe_id: None,
            prep_time: field(prep),
            cook_time: field(cook),
            total_time: field(total),
            servings: 4,
            portion_size: None,
            instructions: String::new(),
            ingredients: vec![],
            nutrition_per_serving: None,
            tags: vec![],
            notes: None,
            icon: None,
        }
    }

    #[test]
    fn drop_unusable_import_times_clears_only_the_unusable_fields() {
        let mut dto = imported(
            Some((20, "fortnights")),
            Some((30, "mins")),
            Some((-5, "minutes")),
        );
        drop_unusable_import_times(&mut dto);
        assert!(dto.prep_time.is_none(), "unrecognized unit is dropped");
        assert_eq!(
            dto.cook_time.as_ref().map(|t| (t.value, t.unit.as_str())),
            Some((30, "mins")),
            "a usable field is left for create to canonicalize"
        );
        assert!(dto.total_time.is_none(), "negative value is dropped");
    }

    #[test]
    fn drop_unusable_import_times_leaves_a_clean_import_alone() {
        let mut dto = imported(Some((1, "hour")), None, Some((75, "minutes")));
        drop_unusable_import_times(&mut dto);
        assert!(dto.prep_time.is_some() && dto.total_time.is_some());
    }

    #[test]
    fn resolve_times_with_no_time_fields_writes_nothing_even_over_bad_stored_json() {
        let stored = Row(Some("not json".into()), None, Some(json(3, "fortnights")));
        assert_eq!(
            resolve_times(stored.stored(), &SentTimes::default()).unwrap(),
            TimeWrites::default()
        );
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

    // Holds the stored JSON for (prep, cook, total); each is `None` when the
    // column is NULL.
    struct Row(Option<String>, Option<String>, Option<String>);

    impl Row {
        fn stored(&self) -> StoredTimes<'_> {
            StoredTimes {
                prep_time: self.0.as_deref(),
                cook_time: self.1.as_deref(),
                total_time: self.2.as_deref(),
            }
        }
    }

    fn row(
        prep: Option<(i32, &str)>,
        cook: Option<(i32, &str)>,
        total: Option<(i32, &str)>,
    ) -> Row {
        let col = |t: Option<(i32, &str)>| t.map(|(v, u)| json(v, u));
        Row(col(prep), col(cook), col(total))
    }

    fn sent(
        prep: Option<(i32, &str)>,
        cook: Option<(i32, &str)>,
        total: Option<(i32, &str)>,
    ) -> SentTimes {
        let field = |t: Option<(i32, &str)>| t.map(|(v, u)| time(v, u));
        SentTimes {
            prep_time: field(prep),
            cook_time: field(cook),
            total_time: field(total),
        }
    }

    fn total_after(row: &Row, request: &SentTimes) -> Option<CheckedTime> {
        resolve_times(row.stored(), request).unwrap().total_time
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
            &sent(Some((10, "min")), None, Some((1, "hr"))),
        )
        .unwrap();
        assert_eq!(writes.prep_time, Some(checked(10, "minutes", 10)));
        assert_eq!(writes.cook_time, None);
        assert_eq!(writes.total_time, Some(checked(1, "hours", 60)));
    }

    #[test]
    fn sent_field_with_the_stored_minutes_is_not_rewritten() {
        let stored = row(None, None, Some((1, "hours")));
        let writes =
            resolve_times(stored.stored(), &sent(None, None, Some((60, "minutes")))).unwrap();
        assert_eq!(writes, TimeWrites::default());
    }

    #[test]
    fn legacy_unit_resent_unchanged_is_neither_validated_nor_rewritten() {
        let stored = row(None, None, Some((3, "fortnights")));
        let writes =
            resolve_times(stored.stored(), &sent(None, None, Some((3, "fortnights")))).unwrap();
        assert_eq!(writes, TimeWrites::default());
    }

    #[test]
    fn changed_field_with_an_unrecognized_unit_is_rejected() {
        let stored = row(None, Some((20, "minutes")), None);
        let err =
            resolve_times(stored.stored(), &sent(None, Some((4, "fortnights")), None)).unwrap_err();
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
        let stored = row(None, None, Some((3, "fortnights")));
        let writes =
            resolve_times(stored.stored(), &sent(None, None, Some((45, "minutes")))).unwrap();
        assert_eq!(writes.total_time, Some(checked(45, "minutes", 45)));
    }

    #[test]
    fn cook_change_moves_the_total_by_the_same_amount() {
        // 10 + 20 = 30; raising cook to 90 has to reach search as 100.
        let stored = row(
            Some((10, "minutes")),
            Some((20, "minutes")),
            Some((30, "minutes")),
        );
        assert_eq!(
            total_after(&stored, &sent(None, Some((90, "minutes")), None)),
            Some(checked(100, "minutes", 100))
        );
    }

    #[test]
    fn stale_total_resent_with_a_cook_change_is_re_derived() {
        // The web form's shape: every field resent, the total still the old one.
        let stored = row(
            Some((10, "minutes")),
            Some((20, "minutes")),
            Some((30, "minutes")),
        );
        let writes = resolve_times(
            stored.stored(),
            &sent(
                Some((10, "minutes")),
                Some((90, "minutes")),
                Some((30, "minutes")),
            ),
        )
        .unwrap();
        assert_eq!(writes.prep_time, None, "unchanged prep is not rewritten");
        assert_eq!(writes.total_time, Some(checked(100, "minutes", 100)));
    }

    #[test]
    fn authored_total_is_kept_even_below_prep_plus_cook() {
        let stored = row(
            Some((15, "minutes")),
            Some((30, "minutes")),
            Some((45, "minutes")),
        );
        assert_eq!(
            total_after(
                &stored,
                &sent(None, Some((40, "minutes")), Some((35, "minutes")))
            ),
            Some(checked(35, "minutes", 35))
        );
    }

    #[test]
    fn remainder_keeps_marinating_time_in_the_old_total_unit() {
        // 9 hours = 15 min prep + 45 min cook + 8 hours marinating.
        let stored = row(
            Some((15, "minutes")),
            Some((45, "minutes")),
            Some((9, "hours")),
        );
        assert_eq!(
            total_after(&stored, &sent(None, Some((105, "minutes")), None)),
            Some(checked(10, "hours", 600))
        );
    }

    #[test]
    fn derived_total_falls_back_to_minutes_when_no_unit_divides_it() {
        // 8 hours stored against 15 + 30 leaves a 435-minute remainder;
        // 15 + 45 + 435 = 495, which is not a whole number of hours.
        let stored = row(
            Some((15, "minutes")),
            Some((30, "minutes")),
            Some((8, "hours")),
        );
        assert_eq!(
            total_after(&stored, &sent(None, Some((45, "minutes")), None)),
            Some(checked(495, "minutes", 495))
        );
    }

    #[test]
    fn negative_remainder_from_overlapping_phases_is_kept() {
        // Prep happens while it bakes: 15 + 30 with a 30-minute total.
        let stored = row(
            Some((15, "minutes")),
            Some((30, "minutes")),
            Some((30, "minutes")),
        );
        assert_eq!(
            total_after(&stored, &sent(None, Some((40, "minutes")), None)),
            Some(checked(40, "minutes", 40))
        );
    }

    #[test]
    fn derived_total_never_drops_below_the_longest_phase() {
        // 5 + 30 − 15 = 20, but the 30-minute bake alone takes 30.
        let stored = row(
            Some((15, "minutes")),
            Some((30, "minutes")),
            Some((30, "minutes")),
        );
        assert_eq!(
            total_after(&stored, &sent(Some((5, "minutes")), None, None)),
            Some(checked(30, "minutes", 30))
        );
    }

    #[test]
    fn missing_total_becomes_prep_plus_cook() {
        let stored = row(Some((10, "minutes")), Some((20, "minutes")), None);
        assert_eq!(
            total_after(&stored, &sent(None, Some((30, "minutes")), None)),
            Some(checked(40, "minutes", 40))
        );
    }

    #[test]
    fn missing_prep_counts_as_zero() {
        let stored = row(None, Some((20, "minutes")), None);
        assert_eq!(
            total_after(&stored, &sent(None, Some((25, "minutes")), None)),
            Some(checked(25, "minutes", 25))
        );
    }

    #[test]
    fn create_without_a_total_derives_it_in_the_shared_unit() {
        assert_eq!(
            total_after(
                &Row(None, None, None),
                &sent(Some((1, "hour")), Some((2, "hrs")), None)
            ),
            Some(checked(3, "hours", 180))
        );
        assert_eq!(
            total_after(
                &Row(None, None, None),
                &sent(Some((30, "min")), Some((1, "hour")), None)
            ),
            Some(checked(90, "minutes", 90)),
            "mixed units fall back to minutes"
        );
    }

    #[test]
    fn create_with_no_times_writes_no_total() {
        assert_eq!(
            total_after(&Row(None, None, None), &SentTimes::default()),
            None
        );
    }

    #[test]
    fn unparsable_stored_phase_leaves_the_total_as_stored() {
        let stored = row(
            Some((10, "minutes")),
            Some((2, "fortnights")),
            Some((30, "minutes")),
        );
        assert_eq!(
            total_after(&stored, &sent(Some((15, "minutes")), None, None)),
            None
        );
    }

    #[test]
    fn unparsable_stored_total_is_left_as_stored() {
        let stored = Row(
            Some(json(10, "minutes")),
            Some(json(20, "minutes")),
            Some("not json".into()),
        );
        assert_eq!(
            total_after(&stored, &sent(None, Some((25, "minutes")), None)),
            None
        );
    }

    #[test]
    fn replaced_unparsable_phase_still_blocks_a_remainder() {
        // The old remainder cannot be known without the old cook time.
        let stored = row(
            Some((10, "minutes")),
            Some((2, "fortnights")),
            Some((30, "minutes")),
        );
        assert_eq!(
            total_after(&stored, &sent(None, Some((25, "minutes")), None)),
            None
        );
    }

    #[test]
    fn first_phase_recorded_on_a_total_only_recipe_stays_inside_the_total() {
        // Counting the absent cook time as 0 would push 30 to 50 and hide
        // the recipe from a 45-minute search though its duration never moved.
        let stored = row(None, None, Some((30, "minutes")));
        assert_eq!(
            total_after(&stored, &sent(None, Some((20, "minutes")), None)),
            Some(checked(30, "minutes", 30))
        );
    }

    #[test]
    fn newly_recorded_phase_moves_nothing_while_a_stored_phase_still_does() {
        // prep 10 stored inside a 30-minute total; recording cook 15 adds
        // nothing, and the same write raising prep to 12 adds 2.
        let stored = row(Some((10, "minutes")), None, Some((30, "minutes")));
        assert_eq!(
            total_after(
                &stored,
                &sent(Some((12, "minutes")), Some((15, "minutes")), None)
            ),
            Some(checked(32, "minutes", 32))
        );
    }

    #[test]
    fn newly_recorded_phase_longer_than_the_total_raises_it_to_the_floor() {
        let stored = row(None, None, Some((30, "minutes")));
        assert_eq!(
            total_after(&stored, &sent(None, Some((50, "minutes")), None)),
            Some(checked(50, "minutes", 50))
        );
    }
}
