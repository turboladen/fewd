//! Rules that recipe, drink, meal, and template writes apply to names, tags,
//! and ingredient lists before storing them.

use crate::dto::{IngredientDto, PersonServingDto};
use crate::services::service_error::ValidationError;

/// Trim each tag and drop the ones left empty.
//
// `RecipeService::search_filtered` compares `LOWER(je.value) = ?` against a
// needle the caller has already trimmed, so a stored " dinner" answers no tag
// search while still rendering in the UI. Every recipe and drink write path
// (the HTTP routes, the imports, and the MCP tools) reaches the tags column
// through its service's `create` and `update`, so normalizing there keeps
// that state unreachable. Casing is kept for display, because every reader
// compares tags case-insensitively.
pub fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    tags.into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Trim `name`, rejecting it when nothing but whitespace remains.
///
/// # Errors
///
/// Returns [`ValidationError::EmptyField`] naming `field` for a blank name.
pub fn required_name(field: &'static str, name: String) -> Result<String, ValidationError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::EmptyField { field });
    }
    Ok(trimmed.to_string())
}

/// Reject `text` when it holds nothing but whitespace. Unlike
/// [`required_name`], it does not trim, so the caller stores the text as
/// written.
///
/// # Errors
///
/// Returns [`ValidationError::EmptyField`] naming `field` for blank text.
//
// Instructions are Markdown, where leading indentation and trailing newlines
// can carry meaning, so the check must not trim what gets stored.
pub fn required_text(field: &'static str, text: &str) -> Result<(), ValidationError> {
    if text.trim().is_empty() {
        return Err(ValidationError::EmptyField { field });
    }
    Ok(())
}

/// Trim the name of every ingredient in `ingredients` and of every
/// alternative they carry. `list` is the field that holds the list, such as
/// `ingredients`.
///
/// # Errors
///
/// Returns [`ValidationError::EmptyIngredientName`] for the first blank
/// name, locating it below `list` with positions counted from 1. The list
/// may already be partly trimmed when it fails.
pub fn trim_ingredient_names(
    list: &str,
    ingredients: &mut [IngredientDto],
) -> Result<(), ValidationError> {
    for (index, ingredient) in ingredients.iter_mut().enumerate() {
        trim_ingredient_name(ingredient).map_err(|path| ValidationError::EmptyIngredientName {
            field: format!("{list}[{}].{path}", index + 1),
        })?;
    }
    Ok(())
}

// A blank name stops the walk. The error is the dotted path, below this
// ingredient, of the blank field.
fn trim_ingredient_name(ingredient: &mut IngredientDto) -> Result<(), String> {
    ingredient.name = ingredient.name.trim().to_string();
    if ingredient.name.is_empty() {
        return Err("name".to_string());
    }
    if let Some(alternative) = ingredient.or_alternative.as_deref_mut() {
        trim_ingredient_name(alternative).map_err(|path| format!("or_alternative.{path}"))?;
    }
    Ok(())
}

/// Apply [`trim_ingredient_names`] to the items of every ad-hoc serving in
/// `servings`. Recipe servings carry no names and are left alone.
///
/// # Errors
///
/// Returns [`ValidationError::EmptyIngredientName`] for the first blank
/// name, located as `servings[N].adhoc_items[M]...` with positions counted
/// from 1.
pub fn trim_adhoc_item_names(servings: &mut [PersonServingDto]) -> Result<(), ValidationError> {
    for (index, serving) in servings.iter_mut().enumerate() {
        if let PersonServingDto::Adhoc { adhoc_items, .. } = serving {
            trim_ingredient_names(&format!("servings[{}].adhoc_items", index + 1), adhoc_items)?;
        }
    }
    Ok(())
}

/// Drops every ingredient of an imported or AI-generated recipe or drink
/// whose name is blank, and clears every alternative whose name is blank,
/// logging a warning that names `recipe` for each one. Generated recipes
/// and drinks reach it through `sanitize_generated_recipe` and
/// `sanitize_generated_drink`.
//
// The tokens are already spent, and a row with no name cannot be shopped
// for or shown, so losing the row beats losing the recipe. Direct create
// and update calls still reject the same rows.
pub fn drop_unusable_import_ingredients(recipe: &str, ingredients: &mut Vec<IngredientDto>) {
    let mut number = 0;
    ingredients.retain(|ingredient| {
        number += 1;
        let named = !ingredient.name.trim().is_empty();
        if !named {
            tracing::warn!(
                %recipe,
                ingredient = number,
                "dropping an ingredient with no name from a generated recipe"
            );
        }
        named
    });
    for ingredient in ingredients.iter_mut() {
        clear_blank_alternatives(recipe, ingredient);
    }
}

fn clear_blank_alternatives(recipe: &str, ingredient: &mut IngredientDto) {
    let blank = ingredient
        .or_alternative
        .as_ref()
        .is_some_and(|alternative| alternative.name.trim().is_empty());
    if blank {
        tracing::warn!(
            %recipe,
            ingredient = %ingredient.name,
            "dropping an alternative with no name from a generated recipe"
        );
        ingredient.or_alternative = None;
    } else if let Some(alternative) = ingredient.or_alternative.as_deref_mut() {
        clear_blank_alternatives(recipe, alternative);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::IngredientAmountDto;

    fn named(name: &str) -> IngredientDto {
        IngredientDto {
            name: name.to_string(),
            prep: None,
            amount: IngredientAmountDto::Single { value: 1.0 },
            unit: String::new(),
            notes: None,
            or_alternative: None,
        }
    }

    fn with_alternative(name: &str, alternative: IngredientDto) -> IngredientDto {
        IngredientDto {
            or_alternative: Some(Box::new(alternative)),
            ..named(name)
        }
    }

    #[test]
    fn required_name_trims_and_rejects_blanks() {
        assert_eq!(required_name("name", " Tacos\t".into()), Ok("Tacos".into()));
        assert_eq!(
            required_name("name", " \n".into()),
            Err(ValidationError::EmptyField { field: "name" })
        );
    }

    #[test]
    fn required_text_rejects_blanks_and_accepts_padded_text() {
        for blank in ["", "   ", "\t\n"] {
            assert_eq!(
                required_text("instructions", blank),
                Err(ValidationError::EmptyField {
                    field: "instructions"
                }),
                "{blank:?}"
            );
        }
        assert_eq!(required_text("instructions", "  Stir.\n"), Ok(()));
    }

    #[test]
    fn trim_ingredient_names_locates_a_blank_nested_alternative() {
        let mut list = vec![
            named(" garlic "),
            with_alternative("lime", with_alternative("lemon", named(" "))),
        ];
        let err = trim_ingredient_names("ingredients", &mut list).unwrap_err();
        assert_eq!(
            err,
            ValidationError::EmptyIngredientName {
                field: "ingredients[2].or_alternative.or_alternative.name".into()
            }
        );
    }

    #[test]
    fn drop_unusable_import_ingredients_keeps_named_rows_in_order() {
        let mut list = vec![
            named(" "),
            with_alternative("lime", with_alternative("lemon", named(""))),
            named("\t"),
            with_alternative("rum", named("cachaça")),
        ];
        drop_unusable_import_ingredients("Daiquiri", &mut list);

        let names: Vec<&str> = list.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["lime", "rum"]);
        let lemon = list[0]
            .or_alternative
            .as_deref()
            .expect("named alternative");
        assert_eq!(lemon.name, "lemon");
        assert!(lemon.or_alternative.is_none());
        assert_eq!(list[1].or_alternative.as_deref().unwrap().name, "cachaça");
    }
}
