use crate::dto::{CreateRecipeDto, IngredientAmountDto, IngredientDto, TimeValueDto};
use crate::services::ingredient_amount::{is_known_unit, try_parse_amount_dto};
use crate::services::ingredient_splitter::{first_top_level_or, split_name_and_prep};
use crate::services::paren_notes::peel_size_paren;

pub struct RecipeParser;

impl RecipeParser {
    pub fn parse_markdown(markdown: &str) -> Result<CreateRecipeDto, String> {
        let lines: Vec<&str> = markdown.lines().collect();
        if lines.is_empty() {
            return Err("Empty markdown".to_string());
        }

        let name = parse_name(&lines)?;
        let description = parse_description(&lines);
        let prep_time = parse_time_field(&lines, "prep time");
        let cook_time = parse_time_field(&lines, "cook time");
        let total_time = parse_time_field(&lines, "total time");
        let servings = parse_servings(&lines).unwrap_or(4);
        let ingredients = parse_ingredients(&lines);
        let instructions = parse_section(&lines, "instructions");
        let tags = parse_tags(&lines);
        let notes = parse_section(&lines, "notes");

        if name.is_empty() {
            return Err("Recipe name is required".to_string());
        }

        Ok(CreateRecipeDto {
            name,
            description: if description.is_empty() {
                None
            } else {
                Some(description)
            },
            source: "markdown_import".to_string(),
            source_url: None,
            parent_recipe_id: None,
            prep_time,
            cook_time,
            total_time,
            servings,
            portion_size: None,
            instructions: instructions.unwrap_or_default(),
            ingredients,
            nutrition_per_serving: None,
            tags,
            notes,
            icon: None,
        })
    }
}

fn parse_name(lines: &[&str]) -> Result<String, String> {
    for line in lines {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("# ") {
            let name = name.trim();
            // Skip if it's a subsection (## or more)
            if !name.starts_with('#') {
                return Ok(name.to_string());
            }
        }
    }
    Err("No recipe name found (expected # Recipe Name)".to_string())
}

/// Extract description: text between the H1 and the first ## or metadata line
fn parse_description(lines: &[&str]) -> String {
    let mut started = false;
    let mut desc_lines = Vec::new();

    for line in lines {
        let trimmed = line.trim();

        if !started {
            if trimmed.starts_with("# ") && !trimmed.starts_with("## ") {
                started = true;
            }
            continue;
        }

        // Stop at next section or metadata
        if trimmed.starts_with("## ") {
            break;
        }
        if is_metadata_line(trimmed) {
            continue;
        }
        if !trimmed.is_empty() {
            desc_lines.push(trimmed);
        }
    }

    desc_lines.join("\n")
}

fn is_metadata_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.starts_with("prep time:")
        || lower.starts_with("cook time:")
        || lower.starts_with("total time:")
        || lower.starts_with("servings:")
}

fn parse_time_field(lines: &[&str], field_name: &str) -> Option<TimeValueDto> {
    let prefix = format!("{}:", field_name);
    for line in lines {
        let lower = line.trim().to_lowercase();
        if lower.starts_with(&prefix) {
            let value_str = line.trim()[prefix.len()..].trim();
            return parse_time_value(value_str);
        }
    }
    None
}

fn parse_time_value(s: &str) -> Option<TimeValueDto> {
    // Strip parenthetical like "(plus 4-8 hours marinating)"
    let s = if let Some(open) = s.find('(') {
        s[..open].trim()
    } else {
        s.trim()
    };

    let s = s.to_lowercase();

    // Handle ranges like "35-40 minutes" → take first number
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let first_num = parts[0].split('-').next()?;
    let value: i32 = first_num.parse().ok()?;

    let unit = if parts.len() > 1 {
        match parts[1] {
            "hour" | "hours" | "hr" | "hrs" => "hours".to_string(),
            "day" | "days" => "days".to_string(),
            _ => "minutes".to_string(),
        }
    } else {
        "minutes".to_string()
    };

    Some(TimeValueDto { value, unit })
}

fn parse_servings(lines: &[&str]) -> Option<i32> {
    for line in lines {
        let lower = line.trim().to_lowercase();
        if lower.starts_with("servings:") {
            let value_str = line.trim()["servings:".len()..].trim();
            // Handle "12-15 chicken pieces" → take first number
            let first_token = value_str.split_whitespace().next()?;
            // Handle range "12-15" → take first number
            let first_num = first_token.split('-').next()?;
            return first_num.parse().ok();
        }
    }
    None
}

fn parse_ingredients(lines: &[&str]) -> Vec<IngredientDto> {
    let section_lines = get_section_lines(lines, "ingredients");
    section_lines
        .iter()
        .filter(|line| !line.starts_with("###"))
        .filter_map(|line| parse_ingredient_line(line))
        .collect()
}

fn parse_ingredient_line(line: &str) -> Option<IngredientDto> {
    let line = line.trim();

    // Strip bullet prefix: "- " or "* "
    let line = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))?
        .trim();

    if line.is_empty() {
        return None;
    }

    // Extract parenthetical notes: "orange juice (fresh is best)" → name + notes
    let (line, notes) = extract_notes(line);

    // Peel mid-string size-info parens like "(28 oz each)" out of the line
    // before tokenization so the noun trailing the parenthetical doesn't get
    // sliced off by `splitn(3, ' ')`. See fewd-i47.
    //
    // When both extract_notes (trailing parens) and peel_size_paren
    // (mid-string size parens) produce notes, merge with "; " — same
    // pattern m20260429_000015 uses for the backfill — so a line like
    // `2 cans (28 oz each) tomatoes (drained)` keeps both descriptors.
    let (line, peeled_notes) = peel_size_paren(&line);
    let notes = match (notes, peeled_notes) {
        (Some(trailing), Some(peeled)) => Some(format!("{trailing}; {peeled}")),
        (Some(n), None) | (None, Some(n)) => Some(n),
        (None, None) => None,
    };

    // fewd-4nb: detect an `<primary> or <alt>` alternative at paren depth 0.
    // The split runs AFTER extract_notes/peel_size_paren so any parenthesized
    // "(or 1 cup half-and-half)" already lives in `notes` and we never see it
    // here. Guard: only split when the right half starts with a parseable
    // amount, so "salt or pepper to taste" stays a single ingredient.
    let (line, alt_line) = match first_top_level_or(&line) {
        Some(idx) => {
            let right = line[idx + 4..].trim();
            let first_word = right.split_whitespace().next();
            if first_word.and_then(try_parse_amount).is_some() {
                (line[..idx].trim().to_string(), Some(right.to_string()))
            } else {
                (line, None)
            }
        }
        None => (line, None),
    };
    let or_alternative = alt_line
        .and_then(|alt| parse_ingredient_line(&format!("* {alt}")))
        .map(Box::new);

    let parts: Vec<&str> = line.splitn(3, ' ').collect();

    let mut primary = match parts.len() {
        // Just a name like "salt"
        1 => build_ingredient(
            parts[0],
            IngredientAmountDto::Single { value: 1.0 },
            "to taste".to_string(),
            notes,
        ),
        2 => {
            if let Some(amount) = try_parse_amount(parts[0]) {
                // Amount + name like "2 eggs"
                build_ingredient(parts[1], amount, "whole".to_string(), notes)
            } else {
                // Two-word name like "black pepper"
                build_ingredient(
                    &line,
                    IngredientAmountDto::Single { value: 1.0 },
                    "to taste".to_string(),
                    notes,
                )
            }
        }
        // 3 parts: dispatch on whether parts[1] is a known unit.
        //
        //   - "2 cups flour" → parts[1]="cups" IS a unit → name="flour",
        //     unit="cups". Standard case.
        //   - "6 cloves garlic, minced" → parts[1]="cloves" IS a unit →
        //     name="garlic, minced" → splitter peels prep.
        //   - "1 zucchini, sliced into half-moons" → parts[1]="zucchini,"
        //     is NOT a unit (compound name with prep clause). Treat the
        //     whole `parts[1] + parts[2]` as the raw name (preserving the
        //     comma so the splitter peels prep), set unit="whole".
        //   - "4 medium red onions" → parts[1]="medium" is NOT a unit
        //     (size modifier). Same compound-name treatment.
        //
        // The is_known_unit dispatch was added for fewd-4i3; before that
        // the parser unconditionally treated parts[1] as the unit, which
        // produced `name="sliced into half-moons", unit="zucchini,"` and
        // similar misparses in prod data.
        _ => {
            if let Some(amount) = try_parse_amount(parts[0]) {
                let token = parts[1];
                let token_stripped = token.strip_suffix(',').unwrap_or(token);
                if is_known_unit(token_stripped) {
                    build_ingredient(parts[2], amount, token_stripped.to_string(), notes)
                } else {
                    // Preserve the original token (with any trailing comma)
                    // so the splitter can peel prep.
                    let raw_name = format!("{} {}", token, parts[2]);
                    build_ingredient(&raw_name, amount, "whole".to_string(), notes)
                }
            } else {
                // Entire line is a name (no parseable amount).
                build_ingredient(
                    &line,
                    IngredientAmountDto::Single { value: 1.0 },
                    "to taste".to_string(),
                    notes,
                )
            }
        }
    };
    primary.or_alternative = or_alternative;
    Some(primary)
}

/// Apply the name/prep splitter to a raw name segment before constructing
/// the DTO, so comma'd prep clauses like "garlic, minced" land in the
/// dedicated `prep` field rather than being baked into `name`.
fn build_ingredient(
    raw_name: &str,
    amount: IngredientAmountDto,
    unit: String,
    notes: Option<String>,
) -> IngredientDto {
    let (name, prep) = split_name_and_prep(raw_name);
    IngredientDto {
        name,
        prep,
        amount,
        unit,
        notes,
        or_alternative: None,
    }
}

/// Extract parenthetical notes from end of string.
/// "orange juice (fresh is best)" → ("orange juice", Some("fresh is best"))
///
/// Only treats `(...)` as notes when the closing `)` is at the very end
/// (after trimming). A mid-string parenthetical like
/// `"pear (or Fuji apple), grated"` keeps its suffix intact so the splitter
/// downstream can find the top-level comma — without this constraint,
/// `extract_notes` would silently drop the `, grated`.
fn extract_notes(s: &str) -> (String, Option<String>) {
    let trimmed = s.trim_end();
    if !trimmed.ends_with(')') {
        return (s.to_string(), None);
    }
    let close = trimmed.len() - 1;
    let Some(open) = trimmed[..close].rfind('(') else {
        return (s.to_string(), None);
    };
    let notes = trimmed[open + 1..close].trim().to_string();
    let name = trimmed[..open].trim().to_string();
    if notes.is_empty() || name.is_empty() {
        return (s.to_string(), None);
    }
    (name, Some(notes))
}

/// Try to parse an amount string, returning None if it's not a recognized
/// number / fraction / range. Delegates to the shared
/// [`crate::services::ingredient_amount::try_parse_amount_dto`] so the
/// runtime parser and the backfill migration agree on what's parseable.
fn try_parse_amount(s: &str) -> Option<IngredientAmountDto> {
    try_parse_amount_dto(s)
}

fn parse_tags(lines: &[&str]) -> Vec<String> {
    let section_lines = get_section_lines(lines, "tags");
    if section_lines.is_empty() {
        return Vec::new();
    }

    // Tags can be comma-separated on one or more lines
    section_lines
        .iter()
        .flat_map(|line| {
            line.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
        })
        .collect()
}

fn parse_section(lines: &[&str], section_name: &str) -> Option<String> {
    let section_lines = get_section_lines(lines, section_name);
    if section_lines.is_empty() {
        return None;
    }
    Some(section_lines.join("\n"))
}

/// Get all lines within a ## section, stopping at the next ## (but not ###) or end of file.
/// Subsection headers (###) are included as content lines.
fn get_section_lines<'a>(lines: &'a [&str], section_name: &str) -> Vec<&'a str> {
    let target = format!("## {}", section_name);
    let mut in_section = false;
    let mut result = Vec::new();

    for line in lines {
        let trimmed = line.trim();

        if in_section {
            // Stop at another ## section, but NOT ### subsections
            if trimmed.starts_with("## ") && !trimmed.starts_with("### ") {
                break;
            }
            if !trimmed.is_empty() {
                result.push(trimmed);
            }
        } else if trimmed.to_lowercase().starts_with(&target.to_lowercase()) {
            in_section = true;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(md: &str) -> CreateRecipeDto {
        RecipeParser::parse_markdown(md).unwrap()
    }

    #[test]
    fn test_basic_recipe() {
        let md = "\
# Chicken Tacos
A simple weeknight dinner

Prep time: 10 minutes
Cook time: 20 minutes
Servings: 4

## Ingredients
- 1 lb chicken
- 8 tortillas
- 1 cup salsa

## Instructions
1. Cook the chicken
2. Warm tortillas
3. Assemble tacos

## Tags
dinner, quick, mexican";

        let recipe = parse(md);
        assert_eq!(recipe.name, "Chicken Tacos");
        assert_eq!(
            recipe.description,
            Some("A simple weeknight dinner".to_string())
        );
        assert_eq!(recipe.servings, 4);
        assert_eq!(recipe.source, "markdown_import");
        assert_eq!(recipe.parent_recipe_id, None);

        let prep = recipe.prep_time.unwrap();
        assert_eq!(prep.value, 10);
        assert_eq!(prep.unit, "minutes");

        let cook = recipe.cook_time.unwrap();
        assert_eq!(cook.value, 20);
        assert_eq!(cook.unit, "minutes");

        assert_eq!(recipe.ingredients.len(), 3);
        assert_eq!(recipe.tags, vec!["dinner", "quick", "mexican"]);
    }

    #[test]
    fn test_empty_markdown_fails() {
        assert!(RecipeParser::parse_markdown("").is_err());
    }

    #[test]
    fn test_no_name_fails() {
        let md = "Just some text\nwithout a heading";
        assert!(RecipeParser::parse_markdown(md).is_err());
    }

    #[test]
    fn test_minimal_recipe() {
        let md = "# Toast\n\n## Instructions\nPut bread in toaster";
        let recipe = parse(md);
        assert_eq!(recipe.name, "Toast");
        assert_eq!(recipe.servings, 4); // default
        assert!(recipe.prep_time.is_none());
        assert!(recipe.cook_time.is_none());
        assert_eq!(recipe.ingredients.len(), 0);
    }

    #[test]
    fn test_time_parsing_hours() {
        let md = "# Stew\nCook time: 3 hours\n\n## Instructions\nSimmer";
        let recipe = parse(md);
        let cook = recipe.cook_time.unwrap();
        assert_eq!(cook.value, 3);
        assert_eq!(cook.unit, "hours");
    }

    #[test]
    fn test_time_parsing_range() {
        let md = "# Bread\nTotal time: 35-40 minutes\n\n## Instructions\nBake";
        let recipe = parse(md);
        let total = recipe.total_time.unwrap();
        assert_eq!(total.value, 35); // takes first number
        assert_eq!(total.unit, "minutes");
    }

    #[test]
    fn test_time_parsing_parenthetical() {
        let md =
            "# Marinade\nPrep time: 15 minutes (plus 4 hours marinating)\n\n## Instructions\nMix";
        let recipe = parse(md);
        let prep = recipe.prep_time.unwrap();
        assert_eq!(prep.value, 15); // ignores parenthetical
    }

    #[test]
    fn test_servings_range() {
        let md = "# Wings\nServings: 12-15 pieces\n\n## Instructions\nFry";
        let recipe = parse(md);
        assert_eq!(recipe.servings, 12); // takes first number
    }

    #[test]
    fn test_ingredient_amount_unit_name() {
        let md = "# Test\n\n## Ingredients\n- 2 cups flour\n\n## Instructions\nMix";
        let recipe = parse(md);
        assert_eq!(recipe.ingredients.len(), 1);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "flour");
        assert_eq!(ing.unit, "cups");
        assert!(
            matches!(ing.amount, IngredientAmountDto::Single { value } if (value - 2.0).abs() < 0.001)
        );
    }

    #[test]
    fn test_ingredient_fraction() {
        let md = "# Test\n\n## Ingredients\n- 1/2 cup sugar\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert!(
            matches!(ing.amount, IngredientAmountDto::Single { value } if (value - 0.5).abs() < 0.001)
        );
    }

    #[test]
    fn test_ingredient_range() {
        let md = "# Test\n\n## Ingredients\n- 1-2 eggs\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert!(
            matches!(ing.amount, IngredientAmountDto::Range { min, max } if (min - 1.0).abs() < 0.001 && (max - 2.0).abs() < 0.001)
        );
    }

    #[test]
    fn test_ingredient_name_only() {
        let md = "# Test\n\n## Ingredients\n- salt\n\n## Instructions\nSeason";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "salt");
        assert_eq!(ing.unit, "to taste");
    }

    #[test]
    fn test_ingredient_with_prep_clause() {
        // "6 cloves garlic, minced" — splitn(3, ' ') leaves "garlic, minced"
        // in parts[2]. The splitter pulls "minced" into the prep field.
        let md = "# Test\n\n## Ingredients\n- 6 cloves garlic, minced\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "garlic");
        assert_eq!(ing.prep, Some("minced".to_string()));
        assert_eq!(ing.unit, "cloves");
    }

    #[test]
    fn test_ingredient_with_notes() {
        let md = "# Test\n\n## Ingredients\n- 1 cup orange juice (fresh is best)\n\n## Instructions\nPour";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.notes, Some("fresh is best".to_string()));
    }

    #[test]
    fn test_ingredient_two_word_name() {
        let md = "# Test\n\n## Ingredients\n- black pepper\n\n## Instructions\nSeason";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "black pepper");
        assert_eq!(ing.unit, "to taste");
    }

    #[test]
    fn test_tags_comma_separated() {
        let md = "# Test\n\n## Instructions\nDo it\n\n## Tags\ndinner, quick, easy";
        let recipe = parse(md);
        assert_eq!(recipe.tags, vec!["dinner", "quick", "easy"]);
    }

    #[test]
    fn test_notes_section() {
        let md = "# Test\n\n## Instructions\nDo it\n\n## Notes\nServe with rice";
        let recipe = parse(md);
        assert_eq!(recipe.notes, Some("Serve with rice".to_string()));
    }

    #[test]
    fn test_section_case_insensitive() {
        let md = "# Test\n\n## INGREDIENTS\n- 1 cup flour\n\n## INSTRUCTIONS\nMix";
        let recipe = parse(md);
        assert_eq!(recipe.ingredients.len(), 1);
        assert!(recipe.instructions.contains("Mix"));
    }

    #[test]
    fn test_extract_notes_helper() {
        let (name, notes) = extract_notes("orange juice (fresh is best)");
        assert_eq!(name, "orange juice");
        assert_eq!(notes, Some("fresh is best".to_string()));

        let (name, notes) = extract_notes("plain text");
        assert_eq!(name, "plain text");
        assert_eq!(notes, None);
    }

    #[test]
    fn test_extract_notes_preserves_mid_string_parens_with_suffix() {
        // The `)` is NOT at the end — there's a `, grated` suffix after it.
        // `extract_notes` must leave the string alone so the suffix flows
        // through to the splitter, which can then peel `grated` off as prep.
        let (name, notes) = extract_notes("pear (or Fuji apple), grated");
        assert_eq!(name, "pear (or Fuji apple), grated");
        assert_eq!(notes, None);
    }

    #[test]
    fn test_compound_non_unit_with_prep() {
        // The fewd-4i3 hero case: parts[1]="zucchini," is not a unit, so
        // the parser merges it into the name and lets the splitter peel
        // the prep clause.
        let md = "# Test\n\n## Ingredients\n- 1 zucchini, sliced into half-moons\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "zucchini");
        assert_eq!(ing.prep, Some("sliced into half-moons".to_string()));
        assert_eq!(ing.unit, "whole");
    }

    #[test]
    fn test_compound_non_unit_no_prep() {
        // "4 medium red onions" — parts[1]="medium" is not a unit.
        let md = "# Test\n\n## Ingredients\n- 4 medium red onions\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "medium red onions");
        assert_eq!(ing.prep, None);
        assert_eq!(ing.unit, "whole");
    }

    #[test]
    fn test_bay_leaves() {
        // "2 bay leaves" — parts[1]="bay" not a unit. Compound name.
        let md = "# Test\n\n## Ingredients\n- 2 bay leaves\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "bay leaves");
        assert_eq!(ing.unit, "whole");
    }

    #[test]
    fn test_em_dash_range_with_compound_name() {
        // "12–15 fresh sage leaves" — em-dash range, parts[1]="fresh" not a unit.
        let md = "# Test\n\n## Ingredients\n- 12–15 fresh sage leaves\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "fresh sage leaves");
        assert!(matches!(
            ing.amount,
            IngredientAmountDto::Range { min, max } if (min - 12.0).abs() < 0.001 && (max - 15.0).abs() < 0.001
        ));
        assert_eq!(ing.unit, "whole");
    }

    #[test]
    fn test_em_dash_range_with_unit() {
        // "3–4 lbs bone-in beef short ribs" — parts[1]="lbs" IS a unit.
        let md =
            "# Test\n\n## Ingredients\n- 3–4 lbs bone-in beef short ribs\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "bone-in beef short ribs");
        assert_eq!(ing.unit, "lbs");
        assert!(matches!(
            ing.amount,
            IngredientAmountDto::Range { min, max } if (min - 3.0).abs() < 0.001 && (max - 4.0).abs() < 0.001
        ));
    }

    #[test]
    fn test_unicode_fraction() {
        let md = "# Test\n\n## Ingredients\n- ¼ teaspoon salt\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "salt");
        assert_eq!(ing.unit, "teaspoon");
        assert!(
            matches!(ing.amount, IngredientAmountDto::Single { value } if (value - 0.25).abs() < 0.001)
        );
    }

    #[test]
    fn test_mixed_unicode_fraction() {
        let md = "# Test\n\n## Ingredients\n- 1½ cups milk\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "milk");
        assert_eq!(ing.unit, "cups");
        assert!(
            matches!(ing.amount, IngredientAmountDto::Single { value } if (value - 1.5).abs() < 0.001)
        );
    }

    #[test]
    fn test_label_fraction_falls_through() {
        // "80/20 ground beef" — `80/20` is a label, not a fraction. The
        // tightened ASCII fraction bounds reject it, so the entire line
        // becomes the name (no parseable amount).
        let md = "# Test\n\n## Ingredients\n- 80/20 ground beef\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "80/20 ground beef");
        assert_eq!(ing.unit, "to taste");
        assert!(
            matches!(ing.amount, IngredientAmountDto::Single { value } if (value - 1.0).abs() < 0.001)
        );
    }

    #[test]
    fn test_ingredient_with_paren_alternative_and_prep() {
        // End-to-end: a markdown line where the parenthetical is NOT trailing
        // notes but a varietal alternative, followed by a prep clause. The
        // parser should preserve the parens in `name` and put `grated` in
        // `prep`.
        //
        // Pre-fewd-xez this dropped `, grated` and produced name="pear",
        // notes=Some("or Fuji apple"). Pre-fewd-4i3 the 3-part branch put
        // "Asian" in `unit`, leaving name="pear (or Fuji apple)" — wrong
        // because "Asian" is a varietal modifier, not a unit. After
        // fewd-4i3's is_known_unit dispatch, the full varietal stays in
        // `name`: "Asian pear (or Fuji apple)".
        let md = "# Test\n\n## Ingredients\n- 1 Asian pear (or Fuji apple), grated\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "Asian pear (or Fuji apple)");
        assert_eq!(ing.prep, Some("grated".to_string()));
        assert_eq!(ing.unit, "whole");
        assert_eq!(ing.notes, None);
    }

    #[test]
    fn test_ingredient_merges_size_parens_with_trailing_notes() {
        // Regression for the precedence bug flagged in fewd-i47 review: when
        // both extract_notes (trailing `(drained)`) and peel_size_paren
        // (mid-string `(28 oz each)`) fire, both notes must survive. Pre-fix
        // `peeled_notes.or(notes)` silently dropped the trailing notes.
        let md = "# Test\n\n## Ingredients\n- 2 cans (28 oz each) crushed tomatoes (drained)\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "crushed tomatoes");
        assert_eq!(ing.unit, "cans");
        assert_eq!(ing.notes.as_deref(), Some("drained; 28 oz each"));
    }

    #[test]
    fn test_ingredient_with_mid_string_size_parens() {
        // The fewd-i47 hero case: parens carry per-can size info, the noun
        // ("crushed San Marzano tomatoes") trails after `)` with no comma.
        // Pre-fix: extract_notes left the line untouched, splitn(3, ' ')
        // sliced off "cans" as the unit, and the entire `(28 oz each)
        // crushed San Marzano tomatoes` substring landed in `name` —
        // dropping the noun in the rendered UI.
        //
        // Post-fix: peel_size_paren rewrites the line to `2 cans crushed
        // San Marzano tomatoes` with notes="28 oz each" before
        // tokenization, so the standard known-unit dispatch produces the
        // expected DTO.
        let md = "# Test\n\n## Ingredients\n- 2 cans (28 oz each) crushed San Marzano tomatoes\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "crushed San Marzano tomatoes");
        assert_eq!(ing.unit, "cans");
        assert_eq!(ing.notes.as_deref(), Some("28 oz each"));
        assert_eq!(ing.prep, None);
        assert!(
            matches!(ing.amount, IngredientAmountDto::Single { value } if (value - 2.0).abs() < 0.001)
        );
    }

    /// Live-data calibration: parse the original markdown lines that
    /// produced the misbucketed prod rows surfaced by the dietpi audit on
    /// 2026-04-27. Asserts the post-fewd-4i3 parser produces clean
    /// IngredientDto shapes — closing the loop with the m14 backfill,
    /// which produces the same shapes from the post-m13 corrupted state.
    #[test]
    fn live_data_calibration() {
        fn line(md_line: &str) -> IngredientDto {
            let md = format!(
                "# Test\n\n## Ingredients\n- {}\n\n## Instructions\nMix",
                md_line
            );
            parse(&md).ingredients.into_iter().next().unwrap()
        }

        // Compound non-unit at parts[1] (Pattern A's source).
        let zucchini = line("1 zucchini, sliced into half-moons");
        assert_eq!(zucchini.name, "zucchini");
        assert_eq!(zucchini.prep.as_deref(), Some("sliced into half-moons"));
        assert_eq!(zucchini.unit, "whole");

        let scallions = line("1 scallions, thinly sliced");
        assert_eq!(scallions.name, "scallions");
        assert_eq!(scallions.prep.as_deref(), Some("thinly sliced"));

        // Em-dash range: parts[1] is a unit.
        let ribs = line("3–4 lbs bone-in beef short ribs");
        assert_eq!(ribs.name, "bone-in beef short ribs");
        assert_eq!(ribs.unit, "lbs");
        assert!(matches!(
            ribs.amount,
            IngredientAmountDto::Range { min, max } if (min - 3.0).abs() < 0.001 && (max - 4.0).abs() < 0.001
        ));

        // Em-dash range: parts[1] is NOT a unit (compound name path).
        let sage = line("12–15 fresh sage leaves");
        assert_eq!(sage.name, "fresh sage leaves");
        assert_eq!(sage.unit, "whole");

        // Unicode vulgar fraction.
        let salt = line("¼ teaspoon salt");
        assert_eq!(salt.name, "salt");
        assert_eq!(salt.unit, "teaspoon");

        // Mixed Unicode fraction.
        let milk = line("1½ cups milk");
        assert_eq!(milk.name, "milk");
        assert_eq!(milk.unit, "cups");

        // Compound name w/o prep (size modifier at parts[1]).
        let onions = line("4 medium red onions");
        assert_eq!(onions.name, "medium red onions");
        assert_eq!(onions.unit, "whole");

        // 80/20 falls through to "no parseable amount" (whole line is name).
        let beef = line("80/20 ground beef");
        assert_eq!(beef.name, "80/20 ground beef");
        assert_eq!(beef.unit, "to taste");

        // fewd-i47: mid-string size parens, noun trails. Pre-fix this row
        // dropped the noun in the UI; post-fix it parses cleanly.
        let tomatoes = line("2 cans (28 oz each) crushed San Marzano tomatoes");
        assert_eq!(tomatoes.name, "crushed San Marzano tomatoes");
        assert_eq!(tomatoes.unit, "cans");
        assert_eq!(tomatoes.notes.as_deref(), Some("28 oz each"));

        // fewd-4nb: top-level " or " splits cleanly. The `(8-inch)` is
        // preserved in the primary's name because "inch" is not in the
        // known-unit registry — that's the existing fewd-prh territory,
        // out of scope here. What matters: the alt becomes its own DTO
        // with the right amount/unit/name.
        let tortillas = line("8 medium flour tortillas (8-inch) or 10 corn tortillas");
        assert_eq!(tortillas.name, "medium flour tortillas (8-inch)");
        assert_eq!(tortillas.unit, "whole");
        assert!(matches!(
            tortillas.amount,
            IngredientAmountDto::Single { value } if (value - 8.0).abs() < 0.001
        ));
        let alt = tortillas
            .or_alternative
            .as_ref()
            .expect("alternative present");
        assert_eq!(alt.name, "corn tortillas");
        assert_eq!(alt.unit, "whole");
        assert!(matches!(
            alt.amount,
            IngredientAmountDto::Single { value } if (value - 10.0).abs() < 0.001
        ));
        assert!(alt.or_alternative.is_none());

        // fewd-4nb: bare "or" without a parseable amount on the right side
        // does NOT split — `pepper` is not an amount.
        let salt_pepper = line("salt or pepper to taste");
        assert_eq!(salt_pepper.name, "salt or pepper to taste");
        assert!(salt_pepper.or_alternative.is_none());

        // fewd-4nb: parenthesized alternative with a unit token gets peeled
        // into `notes` by extract_notes (closing `)` at end of line), so the
        // " or " splitter never sees the inner alternative — preserves the
        // existing pre-fewd-4nb behavior for this shape.
        let milk_alt = line("1 cup milk (or 1 cup half-and-half)");
        assert_eq!(milk_alt.name, "milk");
        assert_eq!(milk_alt.unit, "cup");
        assert!(milk_alt.or_alternative.is_none());
        assert_eq!(milk_alt.notes.as_deref(), Some("or 1 cup half-and-half"));

        // fewd-4nb: depth-aware — inner "or" inside parens does not split.
        // The downstream comma-prep splitter still owns this case.
        let pear = line("pear (or Fuji apple), grated");
        assert_eq!(pear.name, "pear (or Fuji apple)");
        assert_eq!(pear.prep.as_deref(), Some("grated"));
        assert!(pear.or_alternative.is_none());

        // fewd-4nb: chained `X or Y or Z` builds a depth-2 linked chain via
        // recursion on the right half — the result is `milk → cream → water`,
        // NOT `milk → [cream, water]`. Pinned here so a future change that
        // flips `first_top_level_or` to find the rightmost ` or ` (also a
        // valid interpretation) doesn't silently reverse the order.
        let chained = line("1 cup milk or 2 cups cream or 3 cups water");
        assert_eq!(chained.name, "milk");
        let alt1 = chained.or_alternative.as_ref().expect("first alt");
        assert_eq!(alt1.name, "cream");
        let alt2 = alt1.or_alternative.as_ref().expect("second alt");
        assert_eq!(alt2.name, "water");
        assert!(alt2.or_alternative.is_none());
    }

    #[test]
    fn test_try_parse_amount() {
        assert!(
            matches!(try_parse_amount("2"), Some(IngredientAmountDto::Single { value }) if (value - 2.0).abs() < 0.001)
        );
        assert!(
            matches!(try_parse_amount("1/4"), Some(IngredientAmountDto::Single { value }) if (value - 0.25).abs() < 0.001)
        );
        assert!(
            matches!(try_parse_amount("2-3"), Some(IngredientAmountDto::Range { min, max }) if (min - 2.0).abs() < 0.001 && (max - 3.0).abs() < 0.001)
        );
        assert!(try_parse_amount("flour").is_none());
        // En-dash and em-dash ranges (real recipe inputs use Unicode dashes).
        assert!(
            matches!(try_parse_amount("12–15"), Some(IngredientAmountDto::Range { min, max }) if (min - 12.0).abs() < 0.001 && (max - 15.0).abs() < 0.001)
        );
        assert!(
            matches!(try_parse_amount("3—4"), Some(IngredientAmountDto::Range { min, max }) if (min - 3.0).abs() < 0.001 && (max - 4.0).abs() < 0.001)
        );
        // Standalone Unicode vulgar fractions.
        assert!(
            matches!(try_parse_amount("¼"), Some(IngredientAmountDto::Single { value }) if (value - 0.25).abs() < 0.001)
        );
        // Mixed Unicode fraction.
        assert!(
            matches!(try_parse_amount("1½"), Some(IngredientAmountDto::Single { value }) if (value - 1.5).abs() < 0.001)
        );
        // Label-shaped strings rejected (`80/20 ground beef` should not
        // produce 4.0 — the label falls through to "no parseable amount").
        assert!(try_parse_amount("80/20").is_none());
    }
}
