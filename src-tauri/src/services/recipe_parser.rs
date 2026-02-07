use crate::commands::recipe::{CreateRecipeDto, IngredientAmountDto, IngredientDto, TimeValueDto};

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

    let parts: Vec<&str> = line.splitn(3, ' ').collect();

    match parts.len() {
        // Just a name like "salt"
        1 => Some(IngredientDto {
            name: parts[0].to_string(),
            amount: IngredientAmountDto::Single { value: 1.0 },
            unit: "to taste".to_string(),
            notes,
        }),
        2 => {
            if let Some(amount) = try_parse_amount(parts[0]) {
                // Amount + name like "2 eggs"
                Some(IngredientDto {
                    name: parts[1].to_string(),
                    amount,
                    unit: "whole".to_string(),
                    notes,
                })
            } else {
                // Two-word name like "black pepper"
                Some(IngredientDto {
                    name: line.to_string(),
                    amount: IngredientAmountDto::Single { value: 1.0 },
                    unit: "to taste".to_string(),
                    notes,
                })
            }
        }
        // Amount + unit + name like "2 cups flour" or "6 cloves garlic, minced"
        _ => {
            if let Some(amount) = try_parse_amount(parts[0]) {
                Some(IngredientDto {
                    name: parts[2].to_string(),
                    amount,
                    unit: parts[1].to_string(),
                    notes,
                })
            } else {
                // Entire line is a name (no parseable amount)
                Some(IngredientDto {
                    name: line.to_string(),
                    amount: IngredientAmountDto::Single { value: 1.0 },
                    unit: "to taste".to_string(),
                    notes,
                })
            }
        }
    }
}

/// Extract parenthetical notes from end of string.
/// "orange juice (fresh is best)" → ("orange juice", Some("fresh is best"))
fn extract_notes(s: &str) -> (String, Option<String>) {
    if let Some(open) = s.rfind('(') {
        if let Some(close) = s.rfind(')') {
            if close > open {
                let notes = s[open + 1..close].trim().to_string();
                let name = s[..open].trim().to_string();
                if !notes.is_empty() && !name.is_empty() {
                    return (name, Some(notes));
                }
            }
        }
    }
    (s.to_string(), None)
}

/// Try to parse an amount string, returning None if it's not a number/fraction/range.
fn try_parse_amount(s: &str) -> Option<IngredientAmountDto> {
    // Range like "1-2"
    if let Some((min_s, max_s)) = s.split_once('-') {
        if let (Ok(min), Ok(max)) = (min_s.parse::<f64>(), max_s.parse::<f64>()) {
            return Some(IngredientAmountDto::Range { min, max });
        }
    }

    // Fraction like "1/2"
    if let Some((num_s, den_s)) = s.split_once('/') {
        if let (Ok(num), Ok(den)) = (num_s.parse::<f64>(), den_s.parse::<f64>()) {
            if den != 0.0 {
                return Some(IngredientAmountDto::Single { value: num / den });
            }
        }
    }

    // Regular number
    s.parse::<f64>()
        .ok()
        .map(|value| IngredientAmountDto::Single { value })
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
