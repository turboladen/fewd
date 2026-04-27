//! Caroline Chambers-style instruction enhancement.
//!
//! Injects ingredient amounts inline into instruction steps so the cook
//! doesn't have to reference the ingredient list separately.

use crate::dto::{IngredientAmountDto, IngredientDto};

/// Enhance instructions by injecting ingredient amounts inline.
///
/// Rules:
/// - If a step already has a number directly before the ingredient name, skip it
/// - If an ingredient appears in only one step, inject full amount on that occurrence
/// - If an ingredient appears in multiple steps, inject on the first occurrence only
/// - Injected amounts are bold-formatted: **2 cups flour**
pub fn enhance_instructions(ingredients: &[IngredientDto], instructions: &str) -> String {
    // Track which ingredients have already been injected
    let mut injected: Vec<bool> = vec![false; ingredients.len()];

    // Process each line independently
    let lines: Vec<&str> = instructions.lines().collect();
    let mut result_lines: Vec<String> = Vec::with_capacity(lines.len());

    for line in &lines {
        let mut enhanced_line = line.to_string();

        for (i, ing) in ingredients.iter().enumerate() {
            if injected[i] {
                continue;
            }
            if ing.name.trim().is_empty() {
                continue;
            }

            if let Some(new_line) = try_inject(&enhanced_line, ing) {
                enhanced_line = new_line;
                injected[i] = true;
            }
        }

        result_lines.push(enhanced_line);
    }

    result_lines.join("\n")
}

/// Try to inject an ingredient's amount into a line.
/// Returns Some(new_line) if injection happened, None if ingredient not found or already numbered.
fn try_inject(line: &str, ingredient: &IngredientDto) -> Option<String> {
    let lower_line = line.to_lowercase();
    let lower_name = ingredient.name.trim().to_lowercase();

    let pos = lower_line.find(&lower_name)?;

    // Check if there's already a number before the ingredient name
    if has_number_before(line, pos) {
        return None;
    }

    // Build the replacement: **amount unit name**
    let amount_str = format_amount(&ingredient.amount);
    let unit = &ingredient.unit;
    let original_name = &line[pos..pos + lower_name.len()];

    let replacement = if unit.is_empty() || unit == "to taste" {
        if unit == "to taste" {
            format!("**{original_name} ({amount_str}, to taste)**")
        } else {
            format!("**{amount_str} {original_name}**")
        }
    } else {
        format!("**{amount_str} {unit} {original_name}**")
    };

    let mut result = String::with_capacity(line.len() + replacement.len());
    result.push_str(&line[..pos]);
    result.push_str(&replacement);
    result.push_str(&line[pos + lower_name.len()..]);

    Some(result)
}

/// Check if there's a quantity number preceding the ingredient at `pos`.
/// Walks backwards through unit words and whitespace to find a digit,
/// but ignores step numbers like "1." at the start of a line.
fn has_number_before(line: &str, pos: usize) -> bool {
    if pos == 0 {
        return false;
    }
    let before = &line[..pos];

    // Walk backwards looking for a digit within the preceding text
    let mut found_digit_pos: Option<usize> = None;
    for (i, ch) in before.char_indices().rev() {
        if ch.is_ascii_digit() || ch == '⅓' || ch == '⅔' || ch == '½' || ch == '¼' || ch == '¾'
        {
            found_digit_pos = Some(i);
            break;
        }
        if ch.is_alphabetic() || ch == ' ' || ch == '/' || ch == '.' {
            continue;
        }
        // Hit other punctuation — stop
        break;
    }

    let digit_pos = match found_digit_pos {
        Some(p) => p,
        None => return false,
    };

    // Check if this digit is just a step number (e.g., "1. " at line start)
    // A step number is: optional whitespace, then digits, then ". " or ") "
    let trimmed_start = line.trim_start();
    let step_prefix_len = line.len() - trimmed_start.len();

    // Find where the step number ends
    let mut step_end = step_prefix_len;
    for ch in trimmed_start.chars() {
        if ch.is_ascii_digit() {
            step_end += ch.len_utf8();
        } else {
            break;
        }
    }

    // Check if text after the step digits is ". " or ") "
    if step_end > step_prefix_len && step_end < line.len() {
        let after_digits = &line[step_end..];
        if after_digits.starts_with(". ") || after_digits.starts_with(") ") {
            // The digit we found is at or before step_end — it's a step number
            if digit_pos < step_end {
                return false;
            }
        }
    }

    true
}

fn format_amount(amount: &IngredientAmountDto) -> String {
    match amount {
        IngredientAmountDto::Single { value } => format_number(*value),
        IngredientAmountDto::Range { min, max } => {
            format!("{}-{}", format_number(*min), format_number(*max))
        }
    }
}

fn format_number(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        let s = format!("{:.2}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ingredient(name: &str, value: f64, unit: &str) -> IngredientDto {
        IngredientDto {
            name: name.to_string(),
            prep: None,
            amount: IngredientAmountDto::Single { value },
            unit: unit.to_string(),
            notes: None,
        }
    }

    #[test]
    fn injects_single_occurrence() {
        let ingredients = vec![make_ingredient("flour", 2.0, "cups")];
        let instructions = "Add flour to the bowl.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result, "Add **2 cups flour** to the bowl.");
    }

    #[test]
    fn leaves_already_numbered() {
        let ingredients = vec![make_ingredient("flour", 2.0, "cups")];
        let instructions = "Add 2 cups flour to the bowl.";
        let result = enhance_instructions(&ingredients, instructions);
        // No change — already has a number before "flour"
        assert_eq!(result, "Add 2 cups flour to the bowl.");
    }

    #[test]
    fn multi_step_injects_first_only() {
        let ingredients = vec![make_ingredient("salt", 1.0, "tsp")];
        let instructions = "Add salt to the dough.\nSeason with salt to taste.";
        let result = enhance_instructions(&ingredients, instructions);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "Add **1 tsp salt** to the dough.");
        assert_eq!(lines[1], "Season with salt to taste.");
    }

    #[test]
    fn multiple_ingredients_same_line() {
        let ingredients = vec![
            make_ingredient("flour", 2.0, "cups"),
            make_ingredient("sugar", 1.0, "cup"),
        ];
        let instructions = "Mix flour and sugar together.";
        let result = enhance_instructions(&ingredients, instructions);
        assert!(result.contains("**2 cups flour**"));
        assert!(result.contains("**1 cup sugar**"));
    }

    #[test]
    fn case_insensitive_matching() {
        let ingredients = vec![make_ingredient("Flour", 2.0, "cups")];
        let instructions = "Add FLOUR to the bowl.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result, "Add **2 cups FLOUR** to the bowl.");
    }

    #[test]
    fn ingredient_not_in_instructions() {
        let ingredients = vec![make_ingredient("vanilla", 1.0, "tsp")];
        let instructions = "Mix everything together.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result, "Mix everything together.");
    }

    #[test]
    fn range_amounts() {
        let ingredients = vec![IngredientDto {
            name: "garlic".to_string(),
            prep: None,
            amount: IngredientAmountDto::Range { min: 2.0, max: 3.0 },
            unit: "clove".to_string(),
            notes: None,
        }];
        let instructions = "Mince garlic finely.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result, "Mince **2-3 clove garlic** finely.");
    }

    #[test]
    fn fractional_amounts() {
        let ingredients = vec![make_ingredient("butter", 0.5, "cup")];
        let instructions = "Melt the butter in a pan.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result, "Melt the **0.5 cup butter** in a pan.");
    }

    #[test]
    fn empty_unit() {
        let ingredients = vec![make_ingredient("eggs", 3.0, "")];
        let instructions = "Beat the eggs until fluffy.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result, "Beat the **3 eggs** until fluffy.");
    }

    #[test]
    fn to_taste_unit() {
        let ingredients = vec![make_ingredient("salt", 1.0, "to taste")];
        let instructions = "Add salt and pepper.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result, "Add **salt (1, to taste)** and pepper.");
    }

    #[test]
    fn preserves_multiline_structure() {
        let ingredients = vec![
            make_ingredient("flour", 2.0, "cups"),
            make_ingredient("eggs", 3.0, "whole"),
        ];
        let instructions = "1. Add flour to bowl.\n2. Crack eggs into mixture.\n3. Stir well.";
        let result = enhance_instructions(&ingredients, instructions);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("**2 cups flour**"));
        assert!(lines[1].contains("**3 whole eggs**"));
        assert_eq!(lines[2], "3. Stir well.");
    }

    #[test]
    fn number_before_with_whitespace() {
        let ingredients = vec![make_ingredient("eggs", 3.0, "whole")];
        let instructions = "Crack 3 eggs into the bowl.";
        let result = enhance_instructions(&ingredients, instructions);
        // Already has a number before "eggs"
        assert_eq!(result, "Crack 3 eggs into the bowl.");
    }
}
