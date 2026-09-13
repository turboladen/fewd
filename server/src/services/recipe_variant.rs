//! Build a variant of a stored recipe by applying ingredient and instruction
//! changes to a copy of it. Nothing here touches the database: the result is a
//! [`CreateRecipeDto`] that the caller persists through `RecipeService::create`.

use serde::de::DeserializeOwned;

use crate::dto::{CreateRecipeDto, IngredientAmountDto, IngredientDto, TimeValueDto};
use crate::entities::recipe;
use crate::services::ingredient_splitter;
use crate::services::recipe_times::drop_unusable_import_times;

/// The `source` every variant is stored with. The web UI labels a recipe with
/// this source as "Adapted from" its parent.
pub const VARIANT_SOURCE: &str = "ai_adapted";

/// Names one ingredient already on the recipe.
///
/// `name` is compared case-insensitively against the whole stored name, with
/// surrounding whitespace ignored. When that finds nothing and `prep` is
/// [`PrepFilter::Any`], a comma'd `name` such as "garlic, minced" is split
/// into name and prep and tried again. Only the primary ingredient on each
/// line is matched, never its `or_alternative`.
///
/// When several ingredients match and all of them are identical in every
/// field, the first one is used, since no filter could tell them apart.
#[derive(Debug, Clone)]
pub struct IngredientMatch {
    pub name: String,
    pub prep: PrepFilter,
}

/// Which prep an [`IngredientMatch`] accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepFilter {
    /// Matches an ingredient with any prep, or none.
    Any,
    /// Matches only an ingredient with no prep.
    NoPrep,
    /// Matches only an ingredient with this prep, compared case-insensitively
    /// with surrounding whitespace ignored.
    Exactly(String),
}

/// One edit to a recipe's ingredient list.
#[derive(Debug, Clone)]
pub enum IngredientChange {
    /// Appends the ingredient to the end of the list.
    Add(IngredientDto),
    /// Swaps the matched ingredient for `replacement` in the same position.
    Replace {
        target: IngredientMatch,
        replacement: IngredientDto,
    },
    /// Drops the matched ingredient.
    Remove { target: IngredientMatch },
}

/// Replaces the single occurrence of `find` in the instructions with `replace`.
/// An empty `replace` deletes the text.
#[derive(Debug, Clone)]
pub struct InstructionEdit {
    pub find: String,
    pub replace: String,
}

/// How a variant's instructions differ from its parent's.
#[derive(Debug, Clone)]
pub enum InstructionChange {
    /// Uses this text as the variant's instructions.
    Replace(String),
    /// Applies these edits in order to the parent's instructions.
    Edits(Vec<InstructionEdit>),
}

/// Everything a variant changes relative to its parent. `None` in an optional
/// field inherits the parent's value.
///
/// The caller validates `name` and the override strings before building: this
/// module copies them into the variant as given.
#[derive(Debug, Clone)]
pub struct VariantSpec {
    pub name: String,
    pub description: Option<String>,
    pub ingredient_changes: Vec<IngredientChange>,
    pub instruction_change: Option<InstructionChange>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
}

/// Why a variant could not be built. Change numbers are 1-based positions in
/// the list the caller supplied.
#[derive(Debug)]
pub enum VariantError {
    UnmatchedIngredient {
        number: usize,
        name: String,
        prep: PrepFilter,
        available: Vec<String>,
    },
    /// Several ingredients match. `separable_by_prep` is false when they all
    /// share the same prep, so no prep filter can pick one of them.
    AmbiguousIngredient {
        number: usize,
        name: String,
        candidates: Vec<String>,
        separable_by_prep: bool,
    },
    EmptyEditFind {
        number: usize,
    },
    EditNotFound {
        number: usize,
        find: String,
    },
    EditAmbiguous {
        number: usize,
        find: String,
        count: usize,
    },
    /// The parent's stored JSON does not parse. This is a server-side fault,
    /// not something the caller can correct.
    MalformedParent(String),
}

// Messages here are shared by every caller of this module, so they describe
// the problem in terms of the recipe and never name a particular API or tool.
impl std::fmt::Display for VariantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnmatchedIngredient {
                number,
                name,
                prep,
                available,
            } => {
                write!(f, "ingredient change #{number}: no ingredient named '{name}'")?;
                match prep {
                    PrepFilter::Any => {}
                    PrepFilter::NoPrep => write!(f, " with no prep")?,
                    PrepFilter::Exactly(prep) => write!(f, " with prep '{prep}'")?,
                }
                if available.is_empty() {
                    write!(f, ". The recipe has no ingredients.")?;
                } else {
                    write!(
                        f,
                        ". The recipe's ingredients are: {}.",
                        available.join(", ")
                    )?;
                }
                write!(
                    f,
                    " An or_alternative is not matched on its own; replace the whole ingredient line that carries it."
                )
            }
            Self::AmbiguousIngredient {
                number,
                name,
                candidates,
                separable_by_prep: true,
            } => write!(
                f,
                "ingredient change #{number}: '{name}' matches {} ingredients ({}). Give the prep of the one you mean, or \"prep\": \"\" for the one with no prep.",
                candidates.len(),
                candidates.join("; ")
            ),
            Self::AmbiguousIngredient {
                number,
                name,
                candidates,
                separable_by_prep: false,
            } => write!(
                f,
                "ingredient change #{number}: '{name}' matches {} ingredients that no change can tell apart ({}). Supply the whole ingredient list instead.",
                candidates.len(),
                candidates.join("; ")
            ),
            Self::EmptyEditFind { number } => write!(
                f,
                "instruction edit #{number}: the text to find is empty. Give the exact text to replace."
            ),
            Self::EditNotFound { number, find } => write!(
                f,
                "instruction edit #{number}: '{find}' does not occur in the instructions. The text to find must match exactly, including whitespace and punctuation; for a broad change, send the full instructions instead."
            ),
            Self::EditAmbiguous {
                number,
                find,
                count,
            } => write!(
                f,
                "instruction edit #{number}: '{find}' occurs {count} times in the instructions. The text to find must match exactly once, so include more of the surrounding text, or send the full instructions instead."
            ),
            Self::MalformedParent(detail) => {
                write!(f, "the parent recipe has malformed stored data: {detail}")
            }
        }
    }
}

/// Build the [`CreateRecipeDto`] for a variant of `parent`.
///
/// The variant links to `parent` and is stored with [`VARIANT_SOURCE`]. It
/// inherits the parent's description, notes, tags, times, servings, portion
/// size, and icon unless `spec` overrides them. Nutrition is inherited only
/// when there are no ingredient changes, since changed ingredients make the
/// parent's figures wrong. `source_url` is never inherited, and favorite,
/// rating, and planning history start fresh because `RecipeService::create`
/// sets them. `RecipeService::create` also derives a total of prep plus cook
/// when the parent stores no total, so the saved variant can carry a total
/// time its parent lacks.
///
/// A parent time that `RecipeService::create` would reject, such as one in
/// an unrecognized unit, is left off the variant and logged as a warning.
///
/// # Errors
///
/// Returns a [`VariantError`] when a change does not apply or when the
/// parent's stored JSON is malformed. Nothing is built unless every change
/// applies.
//
// Dropping rather than rejecting keeps the parent untouched: the only fix a
// rejection could suggest is editing the parent, which is exactly what
// adapting avoids. The web adapt flow drops such times the same way.
pub fn build_variant_dto(
    parent: &recipe::Model,
    spec: VariantSpec,
) -> Result<CreateRecipeDto, VariantError> {
    let prep_time: Option<TimeValueDto> =
        parse_optional_stored(parent.prep_time.as_deref(), "prep_time")?;
    let cook_time: Option<TimeValueDto> =
        parse_optional_stored(parent.cook_time.as_deref(), "cook_time")?;
    let total_time: Option<TimeValueDto> =
        parse_optional_stored(parent.total_time.as_deref(), "total_time")?;

    let ingredients_changed = !spec.ingredient_changes.is_empty();
    let parent_ingredients: Vec<IngredientDto> = parse_stored(&parent.ingredients, "ingredients")?;
    let ingredients = apply_ingredient_changes(parent_ingredients, spec.ingredient_changes)?;

    let instructions = match spec.instruction_change {
        None => parent.instructions.clone(),
        Some(InstructionChange::Replace(text)) => text,
        Some(InstructionChange::Edits(edits)) if edits.is_empty() => parent.instructions.clone(),
        Some(InstructionChange::Edits(edits)) => {
            apply_instruction_edits(&parent.instructions, &edits)?
        }
    };

    let nutrition_per_serving = if ingredients_changed {
        None
    } else {
        parse_optional_stored(
            parent.nutrition_per_serving.as_deref(),
            "nutrition_per_serving",
        )?
    };
    let tags = match spec.tags {
        Some(tags) => tags,
        None => parse_stored(&parent.tags, "tags")?,
    };

    let mut dto = CreateRecipeDto {
        name: spec.name,
        description: spec.description.or_else(|| parent.description.clone()),
        source: VARIANT_SOURCE.to_string(),
        source_url: None,
        parent_recipe_id: Some(parent.id.clone()),
        prep_time,
        cook_time,
        total_time,
        servings: parent.servings,
        portion_size: parse_optional_stored(parent.portion_size.as_deref(), "portion_size")?,
        instructions,
        ingredients,
        nutrition_per_serving,
        tags,
        notes: spec.notes.or_else(|| parent.notes.clone()),
        icon: parent.icon.clone(),
    };
    drop_unusable_import_times(&mut dto);
    Ok(dto)
}

/// Apply `changes` in order, each one seeing the list as the earlier changes
/// left it.
///
/// # Errors
///
/// Returns [`VariantError::UnmatchedIngredient`] or
/// [`VariantError::AmbiguousIngredient`] for the first replace or remove whose
/// target does not name exactly one ingredient.
pub fn apply_ingredient_changes(
    mut ingredients: Vec<IngredientDto>,
    changes: Vec<IngredientChange>,
) -> Result<Vec<IngredientDto>, VariantError> {
    for (position, change) in changes.into_iter().enumerate() {
        let number = position + 1;
        match change {
            IngredientChange::Add(ingredient) => ingredients.push(ingredient),
            IngredientChange::Replace {
                target,
                replacement,
            } => {
                let at = locate_ingredient(&ingredients, number, &target)?;
                ingredients[at] = replacement;
            }
            IngredientChange::Remove { target } => {
                let at = locate_ingredient(&ingredients, number, &target)?;
                ingredients.remove(at);
            }
        }
    }
    Ok(ingredients)
}

/// Apply `edits` in order, each one searching the text as the earlier edits
/// left it. Windows line endings are normalized to `\n` in the instructions
/// and in every edit before matching, and the result keeps `\n` endings.
///
/// # Errors
///
/// Returns [`VariantError::EmptyEditFind`], [`VariantError::EditNotFound`], or
/// [`VariantError::EditAmbiguous`] for the first edit whose `find` is empty or
/// does not occur exactly once.
pub fn apply_instruction_edits(
    instructions: &str,
    edits: &[InstructionEdit],
) -> Result<String, VariantError> {
    let mut text = normalize_line_endings(instructions);
    for (position, edit) in edits.iter().enumerate() {
        let number = position + 1;
        let find = normalize_line_endings(&edit.find);
        if find.is_empty() {
            return Err(VariantError::EmptyEditFind { number });
        }
        match occurrences(&text, &find) {
            0 => {
                return Err(VariantError::EditNotFound {
                    number,
                    find: edit.find.clone(),
                })
            }
            1 => text = text.replacen(&find, &normalize_line_endings(&edit.replace), 1),
            count => {
                return Err(VariantError::EditAmbiguous {
                    number,
                    find: edit.find.clone(),
                    count,
                })
            }
        }
    }
    Ok(text)
}

fn locate_ingredient(
    ingredients: &[IngredientDto],
    number: usize,
    target: &IngredientMatch,
) -> Result<usize, VariantError> {
    let mut hits = matching_positions(ingredients, &target.name, &target.prep);
    // The whole string is tried first so a stored name that itself contains a
    // comma stays matchable; the split only rescues a caller who folded the
    // prep into the name, which cannot be the case when a prep filter was sent.
    if hits.is_empty() && target.prep == PrepFilter::Any {
        let (name, prep) = ingredient_splitter::normalize(target.name.clone(), None);
        let prep = prep.map_or(PrepFilter::Any, PrepFilter::Exactly);
        hits = matching_positions(ingredients, &name, &prep);
    }

    match hits.as_slice() {
        [only] => Ok(*only),
        [] => Err(VariantError::UnmatchedIngredient {
            number,
            name: target.name.clone(),
            prep: target.prep.clone(),
            available: ingredients.iter().map(describe_ingredient).collect(),
        }),
        [first, rest @ ..]
            if rest
                .iter()
                .all(|&i| identical(&ingredients[*first], &ingredients[i])) =>
        {
            Ok(*first)
        }
        several => Err(VariantError::AmbiguousIngredient {
            number,
            name: target.name.clone(),
            candidates: several
                .iter()
                .map(|&i| describe_candidate(&ingredients[i]))
                .collect(),
            separable_by_prep: several.iter().any(|&i| {
                effective_prep(&ingredients[i]) != effective_prep(&ingredients[several[0]])
            }),
        }),
    }
}

// The prep a filter compares against: trimmed and lowercased, with a blank
// prep counting as none, exactly as `matching_positions` treats it.
fn effective_prep(ingredient: &IngredientDto) -> Option<String> {
    ingredient
        .prep
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_lowercase)
}

fn matching_positions(ingredients: &[IngredientDto], name: &str, prep: &PrepFilter) -> Vec<usize> {
    ingredients
        .iter()
        .enumerate()
        .filter(|(_, ingredient)| {
            let stored_prep = ingredient.prep.as_deref().filter(|p| !p.trim().is_empty());
            same_text(&ingredient.name, name)
                && match prep {
                    PrepFilter::Any => true,
                    PrepFilter::NoPrep => stored_prep.is_none(),
                    PrepFilter::Exactly(wanted) => {
                        stored_prep.is_some_and(|stored| same_text(stored, wanted))
                    }
                }
        })
        .map(|(i, _)| i)
        .collect()
}

// `IngredientDto` has no `PartialEq`, and its serialized form covers every
// field, alternatives included.
fn identical(a: &IngredientDto, b: &IngredientDto) -> bool {
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}

fn same_text(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

fn describe_ingredient(ingredient: &IngredientDto) -> String {
    match ingredient.prep.as_deref().filter(|p| !p.trim().is_empty()) {
        Some(prep) => format!("{} ({prep})", ingredient.name),
        None => ingredient.name.clone(),
    }
}

// Candidates in an ambiguity share a name and may share a prep, so they are
// listed with the amount, unit, and notes that tell them apart.
fn describe_candidate(ingredient: &IngredientDto) -> String {
    let amount = match &ingredient.amount {
        IngredientAmountDto::Single { value } => value.to_string(),
        IngredientAmountDto::Range { min, max } => format!("{min}-{max}"),
    };
    let mut text = format!("{}: {amount}", describe_ingredient(ingredient));
    if !ingredient.unit.trim().is_empty() {
        text.push(' ');
        text.push_str(ingredient.unit.trim());
    }
    if let Some(notes) = ingredient.notes.as_deref().filter(|n| !n.trim().is_empty()) {
        text.push_str(&format!(" (notes: {})", notes.trim()));
    }
    text
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}

// Counts overlapping occurrences: "aa" in "aaa" is two, so an edit whose find
// could apply at either position is reported as ambiguous.
fn occurrences(haystack: &str, needle: &str) -> usize {
    haystack
        .char_indices()
        .filter(|(i, _)| haystack[*i..].starts_with(needle))
        .count()
}

fn parse_stored<T: DeserializeOwned>(raw: &str, field: &str) -> Result<T, VariantError> {
    serde_json::from_str(raw)
        .map_err(|e| VariantError::MalformedParent(format!("{field} is not valid JSON: {e}")))
}

fn parse_optional_stored<T: DeserializeOwned>(
    raw: Option<&str>,
    field: &str,
) -> Result<Option<T>, VariantError> {
    match raw {
        Some(text) if !text.trim().is_empty() => parse_stored(text, field).map(Some),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::IngredientAmountDto;
    use serde_json::{json, Value};

    fn stored_ingredients() -> Value {
        json!([
            {"name": "potatoes", "amount": {"type": "single", "value": 2.0}, "unit": "pound", "notes": null},
            {"name": "italian sausage", "prep": "casings removed", "amount": {"type": "single", "value": 1.0}, "unit": "pound", "notes": null},
            {"name": "sage", "prep": "fresh", "amount": {"type": "range", "min": 6.0, "max": 8.0}, "unit": "leaf", "notes": "torn at the end"},
            {"name": "parmesan", "amount": {"type": "single", "value": 0.33}, "unit": "cup", "notes": null,
             "or_alternative": {"name": "pecorino romano", "amount": {"type": "single", "value": 0.25}, "unit": "cup", "notes": null}},
            {"name": "salt", "prep": "for the water", "amount": {"type": "single", "value": 1.0}, "unit": "tablespoon", "notes": null},
            {"name": "salt", "prep": "to taste", "amount": {"type": "single", "value": 0.0}, "unit": "", "notes": null}
        ])
    }

    fn parent() -> recipe::Model {
        let now = chrono::Utc::now();
        recipe::Model {
            id: "parent-id".into(),
            slug: "potato-gnocchi".into(),
            name: "Potato Gnocchi".into(),
            description: Some("Pillowy gnocchi with sausage".into()),
            source: "manual".into(),
            source_url: Some("https://example.com/gnocchi".into()),
            parent_recipe_id: None,
            prep_time: Some(r#"{"value":20,"unit":"minutes"}"#.into()),
            cook_time: Some(r#"{"value":1,"unit":"hour"}"#.into()),
            total_time: None,
            total_minutes: None,
            servings: 4,
            portion_size: Some(r#"{"value":1.5,"unit":"cup"}"#.into()),
            instructions: "Boil the potatoes.\n\nBrown the italian sausage.\n\nToss with sage."
                .into(),
            ingredients: stored_ingredients().to_string(),
            nutrition_per_serving: Some(
                r#"{"calories":600,"protein_grams":25,"carbs_grams":70,"fat_grams":20,"notes":null}"#
                    .into(),
            ),
            tags: r#"["dinner","italian"]"#.into(),
            notes: Some("Family favorite".into()),
            icon: Some("🥔".into()),
            is_favorite: true,
            times_planned: 7,
            last_planned: Some(now),
            rating: Some(5.0),
            created_at: now,
            updated_at: now,
        }
    }

    fn parent_ingredients() -> Vec<IngredientDto> {
        serde_json::from_value(stored_ingredients()).expect("fixture ingredients parse")
    }

    fn spec() -> VariantSpec {
        VariantSpec {
            name: "Spicy Gnocchi".into(),
            description: None,
            ingredient_changes: vec![],
            instruction_change: None,
            tags: None,
            notes: None,
        }
    }

    fn ingredient(name: &str) -> IngredientDto {
        IngredientDto {
            name: name.into(),
            prep: None,
            amount: IngredientAmountDto::Single { value: 1.0 },
            unit: "pound".into(),
            notes: None,
            or_alternative: None,
        }
    }

    // `None` accepts any prep, a blank string only no prep, and anything else
    // exactly that prep, the same mapping the MCP converter applies.
    fn target(name: &str, prep: Option<&str>) -> IngredientMatch {
        IngredientMatch {
            name: name.into(),
            prep: match prep {
                None => PrepFilter::Any,
                Some(p) if p.trim().is_empty() => PrepFilter::NoPrep,
                Some(p) => PrepFilter::Exactly(p.into()),
            },
        }
    }

    fn names(ingredients: &[IngredientDto]) -> Vec<&str> {
        ingredients.iter().map(|i| i.name.as_str()).collect()
    }

    fn edit(find: &str, replace: &str) -> InstructionEdit {
        InstructionEdit {
            find: find.into(),
            replace: replace.into(),
        }
    }

    // ─── Ingredient changes ─────────────────────────────────────────

    #[test]
    fn replace_keeps_the_ingredient_in_place() {
        let result = apply_ingredient_changes(
            parent_ingredients(),
            vec![IngredientChange::Replace {
                target: target("italian sausage", None),
                replacement: ingredient("hot italian sausage"),
            }],
        )
        .expect("replace applies");
        assert_eq!(result[1].name, "hot italian sausage");
        assert_eq!(result.len(), parent_ingredients().len());
    }

    #[test]
    fn remove_drops_only_the_target() {
        let result = apply_ingredient_changes(
            parent_ingredients(),
            vec![IngredientChange::Remove {
                target: target("sage", None),
            }],
        )
        .expect("remove applies");
        assert_eq!(
            names(&result),
            ["potatoes", "italian sausage", "parmesan", "salt", "salt"]
        );
    }

    #[test]
    fn add_appends_to_the_end() {
        let result = apply_ingredient_changes(
            parent_ingredients(),
            vec![IngredientChange::Add(ingredient("red pepper flakes"))],
        )
        .expect("add applies");
        assert_eq!(
            result.last().map(|i| i.name.as_str()),
            Some("red pepper flakes")
        );
    }

    #[test]
    fn a_later_change_sees_a_name_an_earlier_replace_introduced() {
        let result = apply_ingredient_changes(
            parent_ingredients(),
            vec![
                IngredientChange::Replace {
                    target: target("italian sausage", None),
                    replacement: ingredient("chorizo"),
                },
                IngredientChange::Replace {
                    target: target("chorizo", None),
                    replacement: ingredient("hot italian sausage"),
                },
            ],
        )
        .expect("both replaces apply");
        assert_eq!(result[1].name, "hot italian sausage");

        // The name the first replace retired is gone for the second change.
        let err = apply_ingredient_changes(
            parent_ingredients(),
            vec![
                IngredientChange::Remove {
                    target: target("italian sausage", None),
                },
                IngredientChange::Remove {
                    target: target("italian sausage", None),
                },
            ],
        )
        .expect_err("the second remove has nothing to match");
        assert!(matches!(
            err,
            VariantError::UnmatchedIngredient { number: 2, .. }
        ));
    }

    #[test]
    fn matching_ignores_case_and_surrounding_whitespace() {
        let result = apply_ingredient_changes(
            parent_ingredients(),
            vec![IngredientChange::Remove {
                target: target("  Italian SAUSAGE ", Some(" Casings Removed ")),
            }],
        )
        .expect("case and whitespace do not matter");
        assert!(!names(&result).contains(&"italian sausage"));
    }

    #[test]
    fn a_stored_name_containing_a_comma_matches_as_a_whole() {
        let ingredients = vec![ingredient("sausage, hot italian"), ingredient("sausage")];
        let result = apply_ingredient_changes(
            ingredients,
            vec![IngredientChange::Remove {
                target: target("sausage, hot italian", None),
            }],
        )
        .expect("the whole stored name matches before any split");
        assert_eq!(names(&result), ["sausage"]);
    }

    #[test]
    fn a_comma_target_falls_back_to_split_name_and_prep() {
        let mut garlic = ingredient("garlic");
        garlic.prep = Some("minced".into());
        let result = apply_ingredient_changes(
            vec![garlic, ingredient("olive oil")],
            vec![IngredientChange::Remove {
                target: target("garlic, minced", None),
            }],
        )
        .expect("the split name and prep match");
        assert_eq!(names(&result), ["olive oil"]);
    }

    #[test]
    fn prep_picks_one_of_several_same_named_ingredients() {
        let result = apply_ingredient_changes(
            parent_ingredients(),
            vec![IngredientChange::Remove {
                target: target("salt", Some("to taste")),
            }],
        )
        .expect("prep disambiguates");
        let salts: Vec<_> = result.iter().filter(|i| i.name == "salt").collect();
        assert_eq!(salts.len(), 1);
        assert_eq!(salts[0].prep.as_deref(), Some("for the water"));
    }

    #[test]
    fn same_named_ingredients_without_a_prep_filter_are_ambiguous() {
        let err = apply_ingredient_changes(
            parent_ingredients(),
            vec![IngredientChange::Remove {
                target: target("salt", None),
            }],
        )
        .expect_err("two salts match");
        let VariantError::AmbiguousIngredient { candidates, .. } = &err else {
            panic!("expected AmbiguousIngredient, got {err:?}");
        };
        assert_eq!(
            candidates,
            &["salt (for the water): 1 tablespoon", "salt (to taste): 0"]
        );
        let message = err.to_string();
        assert!(message.contains("Give the prep"), "{message}");
        assert!(message.contains(r#""prep": """#), "{message}");
    }

    fn butters() -> Vec<IngredientDto> {
        let mut softened = ingredient("butter");
        softened.prep = Some("softened".into());
        vec![ingredient("butter"), softened]
    }

    #[test]
    fn a_blank_prep_targets_the_ingredient_with_no_prep() {
        let result = apply_ingredient_changes(
            butters(),
            vec![IngredientChange::Remove {
                target: target("butter", Some("")),
            }],
        )
        .expect("only the plain butter has no prep");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].prep.as_deref(), Some("softened"));
    }

    #[test]
    fn a_prep_targets_the_same_named_ingredient_that_has_it() {
        let result = apply_ingredient_changes(
            butters(),
            vec![IngredientChange::Remove {
                target: target("butter", Some("softened")),
            }],
        )
        .expect("only one butter is softened");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].prep, None);
    }

    #[test]
    fn a_blank_prep_with_no_unprepped_match_is_unmatched() {
        let err = apply_ingredient_changes(
            parent_ingredients(),
            vec![IngredientChange::Remove {
                target: target("salt", Some("")),
            }],
        )
        .expect_err("both salts carry a prep");
        assert!(err.to_string().contains("'salt' with no prep"), "{err}");
    }

    #[test]
    fn same_prep_candidates_report_a_distinct_message_without_prep_advice() {
        let mut tablespoons = ingredient("olive oil");
        tablespoons.amount = IngredientAmountDto::Single { value: 2.0 };
        tablespoons.unit = "tablespoon".into();
        let mut cup = ingredient("olive oil");
        cup.amount = IngredientAmountDto::Single { value: 0.25 };
        cup.unit = "cup".into();
        cup.notes = Some("for greasing".into());

        let err = apply_ingredient_changes(
            vec![tablespoons, cup],
            vec![IngredientChange::Remove {
                target: target("olive oil", None),
            }],
        )
        .expect_err("two olive oils with no prep cannot be told apart");
        assert!(
            matches!(
                err,
                VariantError::AmbiguousIngredient {
                    separable_by_prep: false,
                    ..
                }
            ),
            "{err:?}"
        );
        let message = err.to_string();
        assert!(message.contains("olive oil: 2 tablespoon"), "{message}");
        assert!(
            message.contains("olive oil: 0.25 cup (notes: for greasing)"),
            "{message}"
        );
        assert!(!message.contains("prep"), "{message}");
    }

    #[test]
    fn a_blank_stored_prep_is_described_as_no_prep() {
        let mut blank = ingredient("butter");
        blank.prep = Some("  ".into());
        let err = apply_ingredient_changes(
            vec![blank],
            vec![IngredientChange::Remove {
                target: target("margarine", None),
            }],
        )
        .expect_err("no margarine");
        let message = err.to_string();
        assert!(message.contains("are: butter."), "{message}");
        assert!(!message.contains("butter ("), "{message}");
    }

    #[test]
    fn identical_duplicates_resolve_to_the_first() {
        let mut result = apply_ingredient_changes(
            vec![ingredient("egg"), ingredient("flour"), ingredient("egg")],
            vec![IngredientChange::Replace {
                target: target("egg", None),
                replacement: ingredient("flax egg"),
            }],
        )
        .expect("indistinguishable duplicates are not ambiguous");
        assert_eq!(names(&result), ["flax egg", "flour", "egg"]);

        // Duplicates that differ in any field stay ambiguous.
        let mut heavier = ingredient("egg");
        heavier.amount = IngredientAmountDto::Single { value: 2.0 };
        result = vec![ingredient("egg"), heavier];
        let err = apply_ingredient_changes(
            result,
            vec![IngredientChange::Remove {
                target: target("egg", None),
            }],
        )
        .expect_err("different amounts can still be told apart");
        assert!(matches!(err, VariantError::AmbiguousIngredient { .. }));
    }

    #[test]
    fn an_unmatched_target_lists_the_available_ingredients() {
        let err = apply_ingredient_changes(
            parent_ingredients(),
            vec![
                IngredientChange::Add(ingredient("basil")),
                IngredientChange::Replace {
                    target: target("sage", Some("dried")),
                    replacement: ingredient("thyme"),
                },
            ],
        )
        .expect_err("no dried sage");
        let message = err.to_string();
        assert!(message.contains("#2"), "{message}");
        assert!(message.contains("'sage' with prep 'dried'"), "{message}");
        assert!(message.contains("sage (fresh)"), "{message}");
        assert!(message.contains("basil"), "{message}");
        assert!(message.contains("or_alternative"), "{message}");
    }

    #[test]
    fn an_alternative_only_name_is_not_matched() {
        for change in [
            IngredientChange::Remove {
                target: target("pecorino romano", None),
            },
            IngredientChange::Replace {
                target: target("pecorino romano", None),
                replacement: ingredient("grana padano"),
            },
        ] {
            let err = apply_ingredient_changes(parent_ingredients(), vec![change])
                .expect_err("alternatives are not matched");
            assert!(
                matches!(err, VariantError::UnmatchedIngredient { .. }),
                "{err:?}"
            );
        }
    }

    #[test]
    fn untouched_ingredients_round_trip_exactly() {
        // Ranges, fractional amounts, notes, prep, and an or_alternative chain
        // must all survive a change to a different ingredient.
        let result = apply_ingredient_changes(
            parent_ingredients(),
            vec![IngredientChange::Add(ingredient("basil"))],
        )
        .expect("add applies");
        let untouched = serde_json::to_value(&result[..result.len() - 1]).expect("serializes");
        assert_eq!(untouched, stored_ingredients());
    }

    // ─── Instruction edits ──────────────────────────────────────────

    #[test]
    fn edits_apply_in_sequence() {
        let text = apply_instruction_edits(
            "Brown the sausage.",
            &[
                edit("sausage", "hot sausage"),
                edit("hot sausage.", "hot sausage well."),
            ],
        )
        .expect("both edits apply");
        assert_eq!(text, "Brown the hot sausage well.");
    }

    #[test]
    fn crlf_line_endings_match_either_way() {
        let stored = "Boil.\r\n\r\nBrown the sausage.\r\nServe.";
        let with_crlf_find =
            apply_instruction_edits(stored, &[edit("sausage.\r\nServe", "chorizo.\r\nServe")])
                .expect("a CRLF find matches CRLF text");
        let with_lf_find =
            apply_instruction_edits(stored, &[edit("sausage.\nServe", "chorizo.\nServe")])
                .expect("an LF find matches CRLF text");
        assert_eq!(with_crlf_find, "Boil.\n\nBrown the chorizo.\nServe.");
        assert_eq!(with_lf_find, with_crlf_find);
    }

    #[test]
    fn an_empty_replace_deletes_the_text() {
        let text = apply_instruction_edits("Toss with sage. Serve.", &[edit(" Serve.", "")])
            .expect("deletion applies");
        assert_eq!(text, "Toss with sage.");
    }

    #[test]
    fn a_missing_find_is_reported_with_exact_match_guidance() {
        let err = apply_instruction_edits("Brown the sausage.", &[edit("brown the sausage", "x")])
            .expect_err("matching is case-sensitive");
        assert!(matches!(err, VariantError::EditNotFound { number: 1, .. }));
        let message = err.to_string();
        assert!(message.contains("exactly"), "{message}");
        assert!(message.contains("whitespace"), "{message}");
        assert!(message.contains("full instructions"), "{message}");
    }

    #[test]
    fn a_repeated_find_is_ambiguous() {
        let err = apply_instruction_edits("Stir. Stir again.", &[edit("Stir", "Whisk")])
            .expect_err("Stir occurs twice");
        assert!(matches!(err, VariantError::EditAmbiguous { count: 2, .. }));
        assert!(err.to_string().contains("full instructions"), "{err}");

        // Overlapping occurrences count too.
        let err = apply_instruction_edits("aaa", &[edit("aa", "b")]).expect_err("overlaps");
        assert!(matches!(err, VariantError::EditAmbiguous { count: 2, .. }));
    }

    #[test]
    fn an_empty_find_is_rejected() {
        let err = apply_instruction_edits("Stir.", &[edit("Stir", "Whisk"), edit("", "x")])
            .expect_err("an empty find would match everywhere");
        assert!(matches!(err, VariantError::EmptyEditFind { number: 2 }));
    }

    // ─── Building the variant ───────────────────────────────────────

    #[test]
    fn a_variant_inherits_the_parent_and_links_to_it() {
        let parent = parent();
        let dto = build_variant_dto(&parent, spec()).expect("builds");

        assert_eq!(dto.name, "Spicy Gnocchi");
        assert_eq!(dto.parent_recipe_id.as_deref(), Some("parent-id"));
        assert_eq!(dto.source, VARIANT_SOURCE);
        assert_eq!(dto.source_url, None);
        assert_eq!(dto.description, parent.description);
        assert_eq!(dto.notes, parent.notes);
        assert_eq!(dto.icon, parent.icon);
        assert_eq!(dto.servings, 4);
        assert_eq!(dto.tags, ["dinner", "italian"]);
        assert_eq!(dto.instructions, parent.instructions);
        assert_eq!(
            serde_json::to_value(&dto.ingredients).unwrap(),
            stored_ingredients()
        );
        assert_eq!(
            serde_json::to_value(&dto.portion_size).unwrap(),
            json!({"value": 1.5, "unit": "cup"})
        );
        assert_eq!(
            serde_json::to_value(&dto.nutrition_per_serving).unwrap()["calories"],
            600
        );
    }

    #[test]
    fn overrides_replace_the_inherited_values() {
        let dto = build_variant_dto(
            &parent(),
            VariantSpec {
                description: Some("Gnocchi with a kick".into()),
                tags: Some(vec!["dinner".into(), "spicy".into()]),
                notes: Some("Tuesday version".into()),
                instruction_change: Some(InstructionChange::Replace("Just cook it.".into())),
                ..spec()
            },
        )
        .expect("builds");
        assert_eq!(dto.description.as_deref(), Some("Gnocchi with a kick"));
        assert_eq!(dto.tags, ["dinner", "spicy"]);
        assert_eq!(dto.notes.as_deref(), Some("Tuesday version"));
        assert_eq!(dto.instructions, "Just cook it.");
    }

    #[test]
    fn nutrition_is_cleared_only_when_ingredients_change() {
        let kept = build_variant_dto(
            &parent(),
            VariantSpec {
                instruction_change: Some(InstructionChange::Edits(vec![edit(
                    "with sage",
                    "with basil",
                )])),
                ..spec()
            },
        )
        .expect("builds");
        assert!(kept.nutrition_per_serving.is_some());

        let cleared = build_variant_dto(
            &parent(),
            VariantSpec {
                ingredient_changes: vec![IngredientChange::Add(ingredient("basil"))],
                ..spec()
            },
        )
        .expect("builds");
        assert!(cleared.nutrition_per_serving.is_none());
    }

    #[test]
    fn instructions_untouched_by_edits_are_stored_as_is() {
        let parent = recipe::Model {
            instructions: "Boil.\r\nServe.".into(),
            ..parent()
        };
        for change in [None, Some(InstructionChange::Edits(vec![]))] {
            let dto = build_variant_dto(
                &parent,
                VariantSpec {
                    instruction_change: change,
                    ..spec()
                },
            )
            .expect("builds");
            assert_eq!(dto.instructions, "Boil.\r\nServe.");
        }
    }

    #[test]
    fn readable_parent_times_pass_through_as_stored() {
        let parent = recipe::Model {
            total_time: Some(r#"{"value":90,"unit":"Mins"}"#.into()),
            ..parent()
        };
        let dto = build_variant_dto(&parent, spec()).expect("builds");
        assert_eq!(
            serde_json::to_value((&dto.prep_time, &dto.cook_time, &dto.total_time)).unwrap(),
            json!([
                {"value": 20, "unit": "minutes"},
                {"value": 1, "unit": "hour"},
                {"value": 90, "unit": "Mins"}
            ])
        );
    }

    #[test]
    fn a_parent_time_fewd_cannot_read_is_not_inherited() {
        let parent = recipe::Model {
            cook_time: Some(r#"{"value":3,"unit":"fortnights"}"#.into()),
            total_time: Some(r#"{"value":-5,"unit":"minutes"}"#.into()),
            ..parent()
        };
        let dto = build_variant_dto(&parent, spec())
            .expect("an unreadable parent time does not block the variant");
        assert!(dto.cook_time.is_none(), "unrecognized unit is dropped");
        assert!(dto.total_time.is_none(), "negative value is dropped");
        assert_eq!(
            serde_json::to_value(&dto.prep_time).unwrap(),
            json!({"value": 20, "unit": "minutes"}),
            "a readable time is still inherited"
        );
    }

    #[test]
    fn malformed_stored_json_is_a_parent_fault() {
        let cases = [
            recipe::Model {
                ingredients: "not json".into(),
                ..parent()
            },
            recipe::Model {
                tags: "not json".into(),
                ..parent()
            },
            recipe::Model {
                cook_time: Some("not json".into()),
                ..parent()
            },
            recipe::Model {
                nutrition_per_serving: Some("{".into()),
                ..parent()
            },
        ];
        for parent in cases {
            let err = build_variant_dto(&parent, spec()).expect_err("malformed");
            assert!(matches!(err, VariantError::MalformedParent(_)), "{err:?}");
        }
    }

    #[test]
    fn a_failing_change_fails_the_whole_build() {
        let err = build_variant_dto(
            &parent(),
            VariantSpec {
                ingredient_changes: vec![IngredientChange::Remove {
                    target: target("ghost pepper", None),
                }],
                ..spec()
            },
        )
        .expect_err("unmatched");
        assert!(matches!(err, VariantError::UnmatchedIngredient { .. }));
    }

    #[test]
    fn messages_name_no_particular_tool() {
        let errors = [
            VariantError::UnmatchedIngredient {
                number: 1,
                name: "x".into(),
                prep: PrepFilter::Exactly("y".into()),
                available: vec!["z".into()],
            },
            VariantError::AmbiguousIngredient {
                number: 1,
                name: "x".into(),
                candidates: vec!["x (a)".into(), "x (b)".into()],
                separable_by_prep: true,
            },
            VariantError::AmbiguousIngredient {
                number: 1,
                name: "x".into(),
                candidates: vec!["x: 1".into(), "x: 2".into()],
                separable_by_prep: false,
            },
            VariantError::EmptyEditFind { number: 1 },
            VariantError::EditNotFound {
                number: 1,
                find: "x".into(),
            },
            VariantError::EditAmbiguous {
                number: 1,
                find: "x".into(),
                count: 2,
            },
            VariantError::MalformedParent("tags is not valid JSON".into()),
        ];
        for err in errors {
            let message = err.to_string();
            for tool in [
                "get_recipe",
                "update_recipe",
                "create_recipe",
                "search_recipes",
                "adapt_recipe",
            ] {
                assert!(!message.contains(tool), "{message} names {tool}");
            }
        }
    }
}
