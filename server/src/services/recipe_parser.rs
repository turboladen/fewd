use crate::dto::{CreateRecipeDto, IngredientAmountDto, IngredientDto, TimeValueDto};
use crate::services::ingredient_amount::{
    split_first_word, split_leading_unit, try_parse_amount_dto, unicode_fraction_value,
    RANGE_DASHES,
};
use crate::services::ingredient_splitter::{first_top_level_or, split_name_and_prep};
use crate::services::paren_notes::peel_size_paren;
use migration::total_minutes::canonical_time_unit;

/// The servings a parsed recipe gets when its source states none.
pub const DEFAULT_SERVINGS: i32 = 4;

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
        let servings = parse_servings(&lines).unwrap_or(DEFAULT_SERVINGS);
        let ingredients = parse_ingredients(&lines);
        let instructions = parse_section(&lines, "instructions");
        let tags = parse_tags(&lines);
        let notes = parse_section(&lines, "notes");

        if name.is_empty() {
            return Err("Recipe name is required".to_string());
        }
        // Create rejects blank instructions too, but only this message can
        // name the heading the parser looks for.
        let instructions = instructions
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| {
                "No instructions found: add a '## Instructions' section with the steps".to_string()
            })?;

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
            instructions,
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

    // The value is the leading integer. A range such as "35-40 minutes",
    // "35–40 minutes" or "5 to 10 minutes" keeps its first number.
    let digits_end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let value: i32 = s[..digits_end].parse().ok()?;
    let rest = &s[digits_end..];
    // A decimal or attached fraction ("1.5 hours", "1½ hours") is not a whole
    // count of its unit, so the field is skipped like any other value it
    // cannot read.
    if rest
        .chars()
        .next()
        .is_some_and(|c| !(c.is_whitespace() || c.is_alphabetic() || RANGE_DASHES.contains(&c)))
    {
        return None;
    }

    let tokens = time_tokens(rest);
    let (value, unit) = match skip_range_tail(&tokens) {
        // A value with no unit word counts minutes.
        [] => (value, "minutes"),
        [TimeToken::Word(word), later @ ..] => match canonical_time_unit(word) {
            // An unrecognized word is kept as written. The import route drops a
            // time whose unit it cannot read, rather than storing it as minutes.
            None => (value, *word),
            Some((unit, minutes_per_unit)) => {
                let later = later_part_minutes(later, unit == "hours")?;
                if later.is_empty() {
                    (value, unit)
                } else {
                    let first = value.checked_mul(minutes_per_unit)?;
                    (
                        later.into_iter().try_fold(first, i32::checked_add)?,
                        "minutes",
                    )
                }
            }
        },
        // A separate fraction or number ("1 1/2 hours", "1 ½ hours") is not a
        // whole count either.
        _ => return None,
    };
    Some(TimeValueDto {
        value,
        unit: unit.to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TimeToken<'a> {
    Number(&'a str),
    Word(&'a str),
    /// A range dash, which opens a range's upper bound ("35-40") or joins a
    /// unit to its number ("30-minutes").
    Dash,
    /// A slash, as in a fraction ("1/2") or a rate ("10 minutes/side").
    Slash,
    /// A Unicode vulgar fraction such as `½`.
    Fraction,
    /// A comma or semicolon, which separates two durations.
    Break,
}

/// Splits a time phrase into runs of ASCII digits, runs of letters, and
/// punctuation tokens. Other characters are dropped.
fn time_tokens(s: &str) -> Vec<TimeToken<'_>> {
    let mut tokens = Vec::new();
    let mut chars = s.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let token = if c.is_ascii_digit() || c.is_alphabetic() {
            let is_number = c.is_ascii_digit();
            let same_run = |next: char| {
                if is_number {
                    next.is_ascii_digit()
                } else {
                    next.is_alphabetic()
                }
            };
            let mut end = start + c.len_utf8();
            while let Some(&(index, next)) = chars.peek() {
                if !same_run(next) {
                    break;
                }
                end = index + next.len_utf8();
                chars.next();
            }
            if is_number {
                TimeToken::Number(&s[start..end])
            } else {
                TimeToken::Word(&s[start..end])
            }
        } else if RANGE_DASHES.contains(&c) {
            TimeToken::Dash
        } else if c == '/' {
            TimeToken::Slash
        } else if unicode_fraction_value(c).is_some() {
            TimeToken::Fraction
        } else if c == ',' || c == ';' {
            TimeToken::Break
        } else {
            continue;
        };
        tokens.push(token);
    }
    tokens
}

/// Skips the upper bound of a range ("-40", "to 40", "or 40") at the start of
/// `tokens`, so a range keeps its first number, and skips a dash that joins
/// a unit word to its number ("-minutes").
fn skip_range_tail<'t, 'a>(tokens: &'t [TimeToken<'a>]) -> &'t [TimeToken<'a>] {
    match tokens {
        [TimeToken::Dash | TimeToken::Word("to" | "or"), TimeToken::Number(_), tail @ ..] => tail,
        [TimeToken::Dash, tail @ ..] if matches!(tail.first(), Some(TimeToken::Word(_))) => tail,
        _ => tokens,
    }
}

/// Returns the minutes in each later part of a compound duration such as
/// "1 hr 30 min", "1 hour and 30 minutes" or "1h30", where a bare number
/// after hours counts minutes and a ranged part keeps its first number. Any
/// token other than a number or "and" ends the duration, so "45 min–1 hr"
/// has no later part. Returns `None` when a part is a fraction ("1 hr 1/2",
/// "1 hour ½") or its minutes overflow, and an empty list when `tokens`
/// holds no later part.
fn later_part_minutes(tokens: &[TimeToken], first_in_hours: bool) -> Option<Vec<i32>> {
    let mut parts = Vec::new();
    let mut rest = tokens;
    let mut after_hours = first_in_hours;
    loop {
        if let [TimeToken::Word("and"), tail @ ..] = rest {
            rest = tail;
        }
        if let [TimeToken::Fraction, ..]
        | [TimeToken::Number(_), TimeToken::Slash | TimeToken::Fraction, ..] = rest
        {
            return None;
        }
        let [TimeToken::Number(n), tail @ ..] = rest else {
            break;
        };
        match skip_range_tail(tail) {
            [TimeToken::Word(word), tail @ ..] => {
                let Some((unit, minutes_per_unit)) = canonical_time_unit(word) else {
                    break;
                };
                parts.push(n.parse::<i32>().ok()?.checked_mul(minutes_per_unit)?);
                after_hours = unit == "hours";
                rest = tail;
            }
            [] if after_hours => {
                parts.push(n.parse().ok()?);
                break;
            }
            _ => break,
        }
    }
    Some(parts)
}

fn parse_servings(lines: &[&str]) -> Option<i32> {
    for line in lines {
        let lower = line.trim().to_lowercase();
        if lower.starts_with("servings:") {
            let value_str = line.trim()["servings:".len()..].trim();
            // A range such as "12-15 chicken pieces" or "4–6" keeps its
            // first number.
            let first_token = value_str.split_whitespace().next()?;
            let first_num = first_token.split(RANGE_DASHES).next()?;
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
    // before the line is split into words, so the noun after the
    // parenthetical stays in the name. See fewd-i47.
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

    let (first, after_first) = split_first_word(&line).unwrap_or((&line, ""));
    let amount = try_parse_amount(first).filter(|_| !after_first.is_empty());

    let mut primary = match amount {
        // A line with no amount is a name, such as "salt" or "black pepper".
        None => build_ingredient(
            &line,
            IngredientAmountDto::Single { value: 1.0 },
            "to taste".to_string(),
            notes,
        ),
        // A known unit of one or two words may follow the amount: "2 cups
        // flour", "8 fl oz milk", or "6 cloves garlic, minced", where the
        // splitter then peels "minced". Otherwise the rest is the raw name
        // in whole items, with any comma kept for the splitter: "2 eggs",
        // "4 medium red onions", "1 zucchini, sliced into half-moons".
        Some(amount) => match split_leading_unit(after_first) {
            Some((unit, name)) => build_ingredient(name, amount, unit, notes),
            None => build_ingredient(after_first, amount, "whole".to_string(), notes),
        },
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

    fn time_of(line: &str) -> Option<(i32, String)> {
        let recipe = parse(&format!("# Dish\n{line}\n\n## Instructions\nCook"));
        recipe
            .prep_time
            .or(recipe.cook_time)
            .or(recipe.total_time)
            .map(|time| (time.value, time.unit))
    }

    #[test]
    fn time_units_map_through_the_shared_time_unit_table() {
        for (line, value, unit) in [
            ("Cook time: 2 h", 2, "hours"),
            ("Total time: 1 d", 1, "days"),
            ("Prep time: 10 m", 10, "minutes"),
            ("Prep time: 10 mins", 10, "minutes"),
            ("Cook time: 3 Hours", 3, "hours"),
            ("Prep time: 15min", 15, "minutes"),
            ("Cook time: 2h", 2, "hours"),
            ("Cook time: 1hr", 1, "hours"),
        ] {
            assert_eq!(time_of(line), Some((value, unit.to_string())), "{line}");
        }
    }

    #[test]
    fn time_ranges_keep_the_first_number_and_find_the_unit() {
        for (line, value, unit) in [
            ("Total time: 35–40 minutes", 35, "minutes"),
            ("Total time: 35—40 minutes", 35, "minutes"),
            ("Total time: 35 - 40 minutes", 35, "minutes"),
            ("Total time: 2-3 hours", 2, "hours"),
            ("Prep time: 5 to 10 minutes", 5, "minutes"),
            ("Prep time: 5 or 10 minutes", 5, "minutes"),
            ("Cook time: 1-2h", 1, "hours"),
        ] {
            assert_eq!(time_of(line), Some((value, unit.to_string())), "{line}");
        }
    }

    #[test]
    fn time_without_a_unit_word_counts_minutes() {
        assert_eq!(time_of("Prep time: 15"), Some((15, "minutes".to_string())));
        assert_eq!(
            time_of("Prep time: 10 minutes."),
            Some((10, "minutes".to_string()))
        );
    }

    #[test]
    fn unrecognized_time_unit_is_kept_as_written() {
        assert_eq!(
            time_of("Prep time: 2 fortnights"),
            Some((2, "fortnights".to_string()))
        );
    }

    #[test]
    fn time_that_is_not_a_whole_number_is_skipped() {
        assert_eq!(time_of("Cook time: 1.5 hours"), None);
        assert_eq!(time_of("Cook time: 1½ hours"), None);
        assert_eq!(time_of("Cook time: 1 1/2 hours"), None);
        assert_eq!(time_of("Cook time: 1 ½ hours"), None);
        assert_eq!(time_of("Cook time: about 20 minutes"), None);
    }

    #[test]
    fn compound_time_adds_its_parts_in_minutes() {
        for (line, minutes) in [
            ("Total time: 1 hr 30 min", 90),
            ("Total time: 1 hour and 30 minutes", 90),
            ("Total time: 1h30", 90),
            ("Total time: 1h 30m", 90),
            ("Total time: 1 day 2 hours", 1560),
        ] {
            assert_eq!(
                time_of(line),
                Some((minutes, "minutes".to_string())),
                "{line}"
            );
        }
    }

    #[test]
    fn a_break_between_durations_keeps_the_first() {
        for (line, value, unit) in [
            ("Total time: 45 min–1 hr", 45, "minutes"),
            ("Cook time: 20 minutes - 1 hour", 20, "minutes"),
            ("Cook time: 8 hours, 4 hours on high", 8, "hours"),
            ("Cook time: 10 minutes/side", 10, "minutes"),
            ("Cook time: 20 min/lb", 20, "minutes"),
        ] {
            assert_eq!(time_of(line), Some((value, unit.to_string())), "{line}");
        }
    }

    #[test]
    fn ranged_later_part_keeps_its_first_number() {
        assert_eq!(
            time_of("Total time: 1 hr 30–45 min"),
            Some((90, "minutes".to_string()))
        );
        assert_eq!(
            time_of("Total time: 2 hours 15-20 minutes"),
            Some((135, "minutes".to_string()))
        );
    }

    #[test]
    fn compound_time_with_a_fractional_part_is_skipped() {
        for line in [
            "Total time: 1hr 1/2",
            "Total time: 1 hour 1/2 hour",
            "Total time: 1 hour and 1/2 hour",
            "Total time: 1 hour ½",
        ] {
            assert_eq!(time_of(line), None, "{line}");
        }
    }

    #[test]
    fn unit_joined_by_a_dash_reads_as_attached() {
        assert_eq!(
            time_of("Prep time: 30-minutes"),
            Some((30, "minutes".to_string()))
        );
        assert_eq!(time_of("Cook time: 1-hour"), Some((1, "hours".to_string())));
    }

    #[test]
    fn compound_time_that_overflows_is_skipped() {
        assert_eq!(time_of("Total time: 2000000 days 5 hours"), None);
    }

    #[test]
    fn time_followed_by_an_unreadable_number_is_skipped() {
        assert_eq!(time_of("Cook time: 1 to 1 1/2 hours"), None);
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
    fn servings_range_with_an_en_dash_keeps_the_first_number() {
        let recipe = parse("# Wings\nServings: 4–6\n\n## Instructions\nFry");
        assert_eq!(recipe.servings, 4);
    }

    #[test]
    fn test_servings_range() {
        let md = "# Wings\nServings: 12-15 pieces\n\n## Instructions\nFry";
        let recipe = parse(md);
        assert_eq!(recipe.servings, 12); // takes first number
    }

    fn only_ingredient(line: &str) -> IngredientDto {
        let recipe = parse(&format!(
            "# Dish\n\n## Ingredients\n{line}\n\n## Instructions\nMix"
        ));
        assert_eq!(recipe.ingredients.len(), 1, "{line}");
        recipe.ingredients.into_iter().next().unwrap()
    }

    #[test]
    fn es_plural_units_split_from_the_name() {
        for (line, unit, name) in [
            ("- 2 bunches kale", "bunches", "kale"),
            ("- 3 pinches salt", "pinches", "salt"),
            ("- 2 dashes bitters", "dashes", "bitters"),
            ("- 4 leaves basil", "leaves", "basil"),
        ] {
            let ing = only_ingredient(line);
            assert_eq!(
                (ing.unit.as_str(), ing.name.as_str()),
                (unit, name),
                "{line}"
            );
        }
        let kale = only_ingredient("- 2 bunches kale");
        assert!(
            matches!(kale.amount, IngredientAmountDto::Single { value } if (value - 2.0).abs() < 0.001)
        );
    }

    #[test]
    fn dotted_letter_and_metric_units_split_from_the_name() {
        for (line, unit, name) in [
            ("- 8 oz. cheese", "oz.", "cheese"),
            ("- 2 Tbsp. butter", "Tbsp.", "butter"),
            ("- 1 lb. beef", "lb.", "beef"),
            ("- 1 T butter", "T", "butter"),
            ("- 1 t salt", "t", "salt"),
            ("- 1 c sugar", "c", "sugar"),
            ("- 2 dl milk", "dl", "milk"),
            ("- 1 kilo potatoes", "kilo", "potatoes"),
        ] {
            let ing = only_ingredient(line);
            assert_eq!(
                (ing.unit.as_str(), ing.name.as_str()),
                (unit, name),
                "{line}"
            );
        }
    }

    #[test]
    fn two_word_fluid_ounce_units_split_from_the_name() {
        for (line, unit) in [
            ("- 8 fluid oz milk", "fluid oz"),
            ("- 8 fl oz milk", "fl oz"),
            ("- 8 fl. oz. milk", "fl. oz."),
            ("- 8 fluid ounces milk", "fluid ounces"),
            ("- 8 fl oz, milk", "fl oz"),
            ("- 8 fl.oz. milk", "fl.oz."),
        ] {
            let ing = only_ingredient(line);
            assert_eq!(
                (ing.unit.as_str(), ing.name.as_str()),
                (unit, "milk"),
                "{line}"
            );
        }
    }

    #[test]
    fn two_word_unit_with_nothing_after_it_stays_in_the_name() {
        let ing = only_ingredient("- 2 fl oz");
        assert_eq!((ing.unit.as_str(), ing.name.as_str()), ("whole", "fl oz"));
    }

    #[test]
    fn double_space_after_the_amount_still_splits_the_unit() {
        let ing = only_ingredient("- 8  oz milk");
        assert_eq!((ing.unit.as_str(), ing.name.as_str()), ("oz", "milk"));
    }

    #[test]
    fn one_letter_label_in_parens_stays_in_the_name() {
        let ing = only_ingredient("- 2 tablets (vitamin C) crushed");
        assert!(ing.name.contains("vitamin C"), "{}", ing.name);
        assert!(ing.notes.is_none(), "{:?}", ing.notes);
    }

    #[test]
    fn words_that_start_like_letter_units_stay_in_the_name() {
        let ing = only_ingredient("- 1 T-bone steak");
        assert_eq!(
            (ing.unit.as_str(), ing.name.as_str()),
            ("whole", "T-bone steak")
        );
        let ing = only_ingredient("- 2 cups. flour");
        assert_eq!(ing.unit, "whole");
    }

    #[test]
    fn words_that_only_look_plural_stay_in_the_name() {
        let ing = only_ingredient("- 2 glasses wine");
        assert_eq!(ing.unit, "whole");
        assert_eq!(ing.name, "glasses wine");
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
        // In "6 cloves garlic, minced" the text after the unit is
        // "garlic, minced", and the splitter pulls "minced" into the prep field.
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
        // The fewd-4i3 hero case: "zucchini," after the amount is not a unit, so
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
        // In "4 medium red onions", "medium" after the amount is not a unit.
        let md = "# Test\n\n## Ingredients\n- 4 medium red onions\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "medium red onions");
        assert_eq!(ing.prep, None);
        assert_eq!(ing.unit, "whole");
    }

    #[test]
    fn test_bay_leaves() {
        // In "2 bay leaves", "bay" is not a unit, so the name is compound.
        let md = "# Test\n\n## Ingredients\n- 2 bay leaves\n\n## Instructions\nMix";
        let recipe = parse(md);
        let ing = &recipe.ingredients[0];
        assert_eq!(ing.name, "bay leaves");
        assert_eq!(ing.unit, "whole");
    }

    #[test]
    fn test_em_dash_range_with_compound_name() {
        // In "12–15 fresh sage leaves" (an en-dash range), "fresh" is not a unit.
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
        // In "3–4 lbs bone-in beef short ribs", "lbs" after the amount is a unit.
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
        // The parenthetical here is a varietal alternative, not trailing
        // notes, and a prep clause follows it. The parser keeps the parens in
        // `name` and puts `grated` in `prep`. "Asian" is a varietal modifier,
        // not a unit, so it stays in `name` too: "Asian pear (or Fuji apple)".
        // Reading the parens as notes would drop `, grated`, and reading
        // "Asian" as the unit would leave name="pear (or Fuji apple)".
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
        // The parens carry per-can size info, and the noun ("crushed San
        // Marzano tomatoes") trails after `)` with no comma. peel_size_paren
        // rewrites the line to `2 cans crushed San Marzano tomatoes` with
        // notes="28 oz each" before tokenization, so the known-unit dispatch
        // reads "cans" as the unit. Without the peel, the whole `(28 oz each)
        // crushed San Marzano tomatoes` substring would land in `name`, and
        // the rendered UI would lose the noun.
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

    // Live-data calibration: parse the original markdown lines that
    // produced the misbucketed prod rows surfaced by the dietpi audit on
    // 2026-04-27. Asserts the post-fewd-4i3 parser produces clean
    // IngredientDto shapes, closing the loop with the m14 backfill, which
    // produces the same shapes from the post-m13 corrupted state.
    #[test]
    fn live_data_calibration() {
        fn line(md_line: &str) -> IngredientDto {
            let md = format!(
                "# Test\n\n## Ingredients\n- {}\n\n## Instructions\nMix",
                md_line
            );
            parse(&md).ingredients.into_iter().next().unwrap()
        }

        // The word after the amount is not a unit (Pattern A's source).
        let zucchini = line("1 zucchini, sliced into half-moons");
        assert_eq!(zucchini.name, "zucchini");
        assert_eq!(zucchini.prep.as_deref(), Some("sliced into half-moons"));
        assert_eq!(zucchini.unit, "whole");

        let scallions = line("1 scallions, thinly sliced");
        assert_eq!(scallions.name, "scallions");
        assert_eq!(scallions.prep.as_deref(), Some("thinly sliced"));

        // In this en-dash range, the word after the amount is a unit.
        let ribs = line("3–4 lbs bone-in beef short ribs");
        assert_eq!(ribs.name, "bone-in beef short ribs");
        assert_eq!(ribs.unit, "lbs");
        assert!(matches!(
            ribs.amount,
            IngredientAmountDto::Range { min, max } if (min - 3.0).abs() < 0.001 && (max - 4.0).abs() < 0.001
        ));

        // In this en-dash range, the word after the amount is not a unit, so
        // the name is compound.
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

        // The name is compound without prep, because a size modifier follows
        // the amount.
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
        //
        // TODO(fewd-prh): the `name` assertion below will need updating when
        // size qualifiers (medium, large, etc.) become a separate field.
        // Expected post-fewd-prh: name=="flour tortillas", size_qualifier=="medium".
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

        // fewd-4nb: degenerate trailing "or" with no right-hand content. The
        // markdown trim strips trailing whitespace, so the byte scan never
        // sees a complete ` or ` (4-byte) pattern — `first_top_level_or`
        // returns None and the line stays unsplit. Pinned so a future change
        // that loosens the trailing-space requirement doesn't silently
        // produce a malformed alt with an empty name.
        let trailing_or = line("8 flour tortillas or");
        assert!(trailing_or.or_alternative.is_none());
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
