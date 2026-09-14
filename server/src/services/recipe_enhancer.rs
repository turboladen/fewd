//! Caroline Chambers-style instruction enhancement.
//!
//! Injects ingredient amounts inline into instruction steps so the cook
//! doesn't have to reference the ingredient list separately.

use std::cmp::Reverse;
use std::ops::Range;

use serde::Serialize;

use crate::dto::{IngredientAmountDto, IngredientDto};

/// Instructions with amounts injected, and how many ingredients were placed.
#[derive(Debug, Serialize)]
pub struct EnhanceResult {
    pub enhanced_text: String,
    /// The number of ingredients whose amount was injected into a step. Zero
    /// means `enhanced_text` carries no added amounts.
    pub injection_count: usize,
}

/// Enhance instructions by injecting ingredient amounts inline, as `**2 cups flour**`.
///
/// Each ingredient is placed at most once. Matching ignores case, and a match
/// must start and end on a word boundary, so "salt" never matches "salted".
/// Ingredients are tried longest name first, where a name is cut at its first
/// ',' or '('. Each tries its full and cut names, then the cut name's singular
/// and plural forms; after every ingredient has, the unplaced ones try the last
/// word of the cut name, with its forms. A name is placed at its first occurrence:
/// the earliest line, then the leftmost position. A singular or plural form, or a
/// last word, is skipped when it is also another ingredient's last word or a form
/// of it; the full and cut names are never skipped. Text inside `**bold**` is
/// ignored. An occurrence with a number right before it, optionally through one
/// unit or size word ("2 cups flour", "1 large egg", "2 cups of flour"), settles
/// the ingredient uncounted, and nothing is placed inside it.
pub fn enhance_instructions(ingredients: &[IngredientDto], instructions: &str) -> EnhanceResult {
    let mut lines: Vec<Line> = instructions.lines().map(Line::new).collect();
    let candidates: Vec<Candidates> = ingredients
        .iter()
        .map(|ing| Candidates::new(&ing.name))
        .collect();

    // Longer names go first, so "kosher salt, divided" claims its words before
    // "salt" can match inside them. The cut name is the key because a suffix
    // after ',' or '(' rarely appears in a step. The sort is stable, which keeps
    // list order for ties.
    let mut order: Vec<usize> = (0..ingredients.len()).collect();
    order.sort_by_key(|&i| Reverse(candidates[i].cut_len));

    let mut settled = vec![false; ingredients.len()];
    let mut injection_count = 0;
    for stage in STAGES {
        for &i in &order {
            for &pass in stage {
                if settled[i] {
                    break;
                }
                let needles: Vec<&str> = candidates[i].passes[pass]
                    .iter()
                    .map(String::as_str)
                    .filter(|needle| pass == 0 || !is_ambiguous(needle, i, &candidates))
                    .collect();
                match place(&mut lines, &ingredients[i], &needles) {
                    Placement::Injected => {
                        settled[i] = true;
                        injection_count += 1;
                    }
                    Placement::AlreadyNumbered => settled[i] = true,
                    Placement::NotFound => {}
                }
            }
        }
    }

    EnhanceResult {
        enhanced_text: lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        injection_count,
    }
}

const PASSES: usize = 3;

// An ingredient runs every pass of a stage before the next ingredient starts,
// so a shorter name never takes a word that a longer name's singular or plural
// form was about to claim. Last words wait until every name has been tried.
const STAGES: [&[usize]; 2] = [&[0, 1], &[2]];

/// One instruction line, with the byte ranges no ingredient may be placed inside.
struct Line {
    text: String,
    claimed: Vec<Range<usize>>,
}

impl Line {
    fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            claimed: Vec::new(),
        }
    }
}

/// The names an ingredient may appear under in the steps, grouped by pass.
#[derive(Default)]
struct Candidates {
    /// The full and cut names; the cut name's singular and plural forms; the
    /// last word of the cut name and its forms.
    passes: [Vec<String>; PASSES],
    /// The char count of the cut name, which orders the ingredients.
    cut_len: usize,
    /// The lowercased last word of the cut name and its singular and plural
    /// forms, which other ingredients' fallbacks must not claim.
    head_forms: Vec<String>,
}

impl Candidates {
    fn new(name: &str) -> Self {
        let full = name.split_whitespace().collect::<Vec<_>>().join(" ");
        if !full.chars().any(char::is_alphanumeric) {
            return Self::default();
        }
        let cut_end = full.find([',', '(']).unwrap_or(full.len());
        let cut = trim_non_alphanumeric(&full[..cut_end]);
        let head = trim_non_alphanumeric(cut.rsplit(' ').next().unwrap_or_default());

        // The cut name is the ingredient's own name, as the full name is, so
        // neither is dropped for matching another ingredient's last word.
        let mut names = vec![full.clone()];
        let mut cut_forms = Vec::new();
        if !cut.is_empty() {
            names.push(cut.to_string());
            // The last word ends the cut name, so its forms are forms of the whole name.
            cut_forms = plural_variants(cut);
        }
        let cut_len = if cut.is_empty() { &full } else { cut }.chars().count();
        let mut head_forms = Vec::new();
        if !head.is_empty() {
            head_forms.push(head.to_string());
            head_forms.extend(plural_variants(head));
        }
        let head_pass = if head == cut {
            Vec::new()
        } else {
            head_forms.clone()
        };

        let mut seen: Vec<String> = Vec::new();
        let passes = [names, cut_forms, head_pass].map(|needles| {
            needles
                .into_iter()
                .filter(|needle| {
                    let lower = needle.to_lowercase();
                    let fresh = !seen.contains(&lower);
                    if fresh {
                        seen.push(lower);
                    }
                    fresh
                })
                .collect()
        });

        Self {
            passes,
            cut_len,
            head_forms: head_forms.iter().map(|form| form.to_lowercase()).collect(),
        }
    }
}

/// Whether `needle` names the head noun of any ingredient other than `owner`.
fn is_ambiguous(needle: &str, owner: usize, candidates: &[Candidates]) -> bool {
    let lower = needle.to_lowercase();
    candidates
        .iter()
        .enumerate()
        .any(|(i, other)| i != owner && other.head_forms.contains(&lower))
}

fn trim_non_alphanumeric(s: &str) -> &str {
    s.trim_matches(|c: char| !c.is_alphanumeric())
}

/// Singular and plural forms of `word`, made by changing its ending.
fn plural_variants(word: &str) -> Vec<String> {
    // The suffixes are ASCII, so slicing them off lands on a char boundary.
    let ends_with = |suffix: &str| {
        word.len() >= suffix.len()
            && word.as_bytes()[word.len() - suffix.len()..].eq_ignore_ascii_case(suffix.as_bytes())
    };
    // A stem shorter than three bytes ("as" from "ass") matches too much.
    let stem = |cut: usize| {
        let stem = &word[..word.len() - cut];
        (stem.len() >= 3).then(|| stem.to_string())
    };
    if ends_with("ss") {
        vec![format!("{word}es")]
    } else if ends_with("es") {
        [stem(2), stem(1)].into_iter().flatten().collect()
    } else if ends_with("s") {
        stem(1).into_iter().collect()
    } else {
        vec![format!("{word}s"), format!("{word}es")]
    }
}

enum Placement {
    Injected,
    AlreadyNumbered,
    NotFound,
}

/// Inject the ingredient at the first occurrence of any needle, unless that
/// occurrence already has a quantity before it.
fn place(lines: &mut [Line], ingredient: &IngredientDto, needles: &[&str]) -> Placement {
    if needles.is_empty() {
        return Placement::NotFound;
    }
    for line in lines.iter_mut() {
        let Some(found) = first_occurrence(line, needles) else {
            continue;
        };
        if has_number_before(&line.text, found.start) {
            // The written quantity covers this text, so a shorter name ("salt"
            // in "1 tbsp kosher salt") must not be placed inside it.
            line.claimed.push(found);
            return Placement::AlreadyNumbered;
        }
        let replacement = format_injection(ingredient, &line.text[found.clone()]);
        let growth = replacement.len() - found.len();
        line.text.replace_range(found.clone(), &replacement);
        for range in line
            .claimed
            .iter_mut()
            .filter(|range| range.start >= found.end)
        {
            *range = range.start + growth..range.end + growth;
        }
        return Placement::Injected;
    }
    Placement::NotFound
}

/// The byte range of the leftmost occurrence of any needle that starts and ends
/// on a word boundary and lies outside `**bold**` and the line's claimed ranges.
/// At one position the longest needle wins.
fn first_occurrence(line: &Line, needles: &[&str]) -> Option<Range<usize>> {
    let text = line.text.as_str();
    let bold_marks: Vec<usize> = text.match_indices("**").map(|(i, _)| i).collect();
    let mut marks_before = 0;
    let mut prev: Option<char> = None;
    for (start, ch) in text.char_indices() {
        let at_boundary = !prev.is_some_and(char::is_alphanumeric);
        prev = Some(ch);
        // An odd count of `**` marks ending at or before `start` means bold is open.
        while bold_marks
            .get(marks_before)
            .is_some_and(|&mark| mark + 2 <= start)
        {
            marks_before += 1;
        }
        if !at_boundary || marks_before % 2 == 1 {
            continue;
        }
        let longest = needles
            .iter()
            .filter_map(|needle| match_len(&text[start..], needle))
            .map(|len| start..start + len)
            .filter(|found| {
                !text[found.end..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphanumeric)
            })
            .filter(|found| {
                !line
                    .claimed
                    .iter()
                    .any(|claimed| found.start < claimed.end && claimed.start < found.end)
            })
            .max_by_key(|found| found.end);
        if longest.is_some() {
            return longest;
        }
    }
    None
}

/// The byte length of the prefix of `hay` that matches `needle`, or `None`.
/// Case is folded per char, and a space in `needle` matches a run of whitespace.
fn match_len(hay: &str, needle: &str) -> Option<usize> {
    // Lowercasing each char, never the whole line, keeps byte offsets pointing
    // into the original text: some chars change byte length when lowercased.
    let mut hay_chars = hay.char_indices().peekable();
    for n in needle.chars() {
        if n == ' ' {
            hay_chars.next_if(|&(_, h)| h.is_whitespace())?;
            while hay_chars.next_if(|&(_, h)| h.is_whitespace()).is_some() {}
        } else {
            let (_, h) = hay_chars.next()?;
            if !h.to_lowercase().eq(n.to_lowercase()) {
                return None;
            }
        }
    }
    Some(hay_chars.peek().map_or(hay.len(), |&(i, _)| i))
}

/// Build the bold replacement for `matched`, the ingredient's text as it appears in the step.
fn format_injection(ingredient: &IngredientDto, matched: &str) -> String {
    let amount = format_amount(&ingredient.amount);
    match ingredient.unit.as_str() {
        "" => format!("**{amount} {matched}**"),
        "to taste" => format!("**{matched} ({amount}, to taste)**"),
        unit => format!("**{amount} {unit} {matched}**"),
    }
}

/// Whether a quantity sits right before `pos`, either directly ("3 eggs", "2-3
/// eggs", "½ lemon") or through one unit or size word and an optional "of" ("2
/// cups flour", "1 large egg", "2 tbsp. oil", "2 cups of flour"). A number
/// earlier in the sentence does not count, and neither does a common connecting
/// word such as "until" or "add" between the number and the name.
fn has_number_before(line: &str, pos: usize) -> bool {
    let before = line[..pos].trim_end();
    if ends_in_quantity(before) {
        return true;
    }
    let Some((rest, word)) = split_last_word(before) else {
        return false;
    };
    let (rest, word) = if word.eq_ignore_ascii_case("of") {
        match split_last_word(rest) {
            Some(split) => split,
            None => return false,
        }
    } else {
        (rest, word)
    };
    is_quantity_word(word) && ends_in_quantity(rest)
}

// A settled ingredient loses its amount for the whole recipe, so these words,
// which join a number to the rest of a sentence ("Bake at 350 until cheese
// melts"), never count as the unit between a quantity and a name.
const CONNECTING_WORDS: &[&str] = &[
    "and", "or", "until", "then", "to", "at", "with", "for", "in", "into", "on", "of", "the", "a",
    "an", "add", "more", "degrees",
];

// A period after one of these is an abbreviation. After any other word, such as
// "min" in "Simmer 10 min. Salt the soup", it ends a sentence.
const UNIT_ABBREVIATIONS: &[&str] = &[
    "tsp", "tsps", "tbsp", "tbsps", "oz", "ozs", "lb", "lbs", "c", "pt", "qt", "g", "kg", "ml", "l",
];

/// Whether `text` ends in a quantity: a digit, which covers "1/2" and "2-3", or a
/// unicode fraction.
fn ends_in_quantity(text: &str) -> bool {
    text.chars()
        .next_back()
        .is_some_and(|c| c.is_ascii_digit() || matches!(c, '½' | '⅓' | '⅔' | '¼' | '¾' | '⅛'))
}

/// Split trimmed `text` into the trimmed text before its last word and the word,
/// which may end in one period. `None` when the text does not end in a word that
/// follows whitespace.
fn split_last_word(text: &str) -> Option<(&str, &str)> {
    let body = text.strip_suffix('.').unwrap_or(text);
    let rest = body.trim_end_matches(|c: char| c.is_alphabetic() || c == '-');
    if rest.len() == body.len() || !rest.ends_with(char::is_whitespace) {
        return None;
    }
    Some((rest.trim_end(), &text[rest.len()..]))
}

/// Whether `word` can stand between a quantity and an ingredient name.
fn is_quantity_word(word: &str) -> bool {
    let lower = word.to_lowercase();
    match lower.strip_suffix('.') {
        Some(abbreviation) => UNIT_ABBREVIATIONS.contains(&abbreviation),
        None => !CONNECTING_WORDS.contains(&lower.as_str()),
    }
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
            or_alternative: None,
        }
    }

    fn enhance(ingredients: &[IngredientDto], instructions: &str) -> String {
        enhance_instructions(ingredients, instructions).enhanced_text
    }

    #[test]
    fn injects_single_occurrence() {
        let ingredients = vec![make_ingredient("flour", 2.0, "cups")];
        let result = enhance_instructions(&ingredients, "Add flour to the bowl.");
        assert_eq!(result.enhanced_text, "Add **2 cups flour** to the bowl.");
        assert_eq!(result.injection_count, 1);
    }

    #[test]
    fn leaves_already_numbered() {
        let ingredients = vec![make_ingredient("flour", 2.0, "cups")];
        let result = enhance_instructions(&ingredients, "Add 2 cups flour to the bowl.");
        // The step already gives a quantity before "flour".
        assert_eq!(result.enhanced_text, "Add 2 cups flour to the bowl.");
        assert_eq!(result.injection_count, 0);
    }

    #[test]
    fn numbered_first_occurrence_consumes_the_ingredient() {
        let ingredients = vec![make_ingredient("flour", 2.0, "cups")];
        let instructions = "Add 2 cups flour to the bowl.\nDust the counter with flour.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result.enhanced_text, instructions);
        assert_eq!(result.injection_count, 0);
    }

    #[test]
    fn multi_step_injects_first_only() {
        let ingredients = vec![make_ingredient("salt", 1.0, "tsp")];
        let result = enhance(
            &ingredients,
            "Add salt to the dough.\nSeason with salt to taste.",
        );
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
        let result = enhance_instructions(&ingredients, "Mix flour and sugar together.");
        assert_eq!(
            result.enhanced_text,
            "Mix **2 cups flour** and **1 cup sugar** together."
        );
        assert_eq!(result.injection_count, 2);
    }

    #[test]
    fn case_insensitive_matching() {
        let ingredients = vec![make_ingredient("Flour", 2.0, "cups")];
        let result = enhance(&ingredients, "Add FLOUR to the bowl.");
        assert_eq!(result, "Add **2 cups FLOUR** to the bowl.");
    }

    #[test]
    fn ingredient_not_in_instructions() {
        let ingredients = vec![make_ingredient("vanilla", 1.0, "tsp")];
        let result = enhance_instructions(&ingredients, "Mix everything together.");
        assert_eq!(result.enhanced_text, "Mix everything together.");
        assert_eq!(result.injection_count, 0);
    }

    #[test]
    fn blank_names_are_never_placed() {
        let ingredients = vec![make_ingredient("  ", 1.0, "tsp")];
        let result = enhance_instructions(&ingredients, "Mix everything together.");
        assert_eq!(result.enhanced_text, "Mix everything together.");
        assert_eq!(result.injection_count, 0);
    }

    #[test]
    fn range_amounts() {
        let ingredients = vec![IngredientDto {
            name: "garlic".to_string(),
            prep: None,
            amount: IngredientAmountDto::Range { min: 2.0, max: 3.0 },
            unit: "clove".to_string(),
            notes: None,
            or_alternative: None,
        }];
        let result = enhance(&ingredients, "Mince garlic finely.");
        assert_eq!(result, "Mince **2-3 clove garlic** finely.");
    }

    #[test]
    fn fractional_amounts() {
        let ingredients = vec![make_ingredient("butter", 0.5, "cup")];
        let result = enhance(&ingredients, "Melt the butter in a pan.");
        assert_eq!(result, "Melt the **0.5 cup butter** in a pan.");
    }

    #[test]
    fn empty_unit() {
        let ingredients = vec![make_ingredient("eggs", 3.0, "")];
        let result = enhance(&ingredients, "Beat the eggs until fluffy.");
        assert_eq!(result, "Beat the **3 eggs** until fluffy.");
    }

    #[test]
    fn to_taste_unit() {
        let ingredients = vec![make_ingredient("salt", 1.0, "to taste")];
        let result = enhance(&ingredients, "Add salt and pepper.");
        assert_eq!(result, "Add **salt (1, to taste)** and pepper.");
    }

    #[test]
    fn preserves_multiline_structure() {
        let ingredients = vec![
            make_ingredient("flour", 2.0, "cups"),
            make_ingredient("eggs", 3.0, "whole"),
        ];
        let instructions = "1. Add flour to bowl.\n2. Crack eggs into mixture.\n3. Stir well.";
        let result = enhance(&ingredients, instructions);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "1. Add **2 cups flour** to bowl.");
        assert_eq!(lines[1], "2. Crack **3 whole eggs** into mixture.");
        assert_eq!(lines[2], "3. Stir well.");
    }

    #[test]
    fn number_before_with_whitespace() {
        let ingredients = vec![make_ingredient("eggs", 3.0, "whole")];
        let result = enhance(&ingredients, "Crack 3 eggs into the bowl.");
        assert_eq!(result, "Crack 3 eggs into the bowl.");
    }

    #[test]
    fn head_noun_fallback_places_a_detailed_name() {
        let ingredients = vec![make_ingredient("large English cucumber", 1.0, "")];
        let result = enhance_instructions(&ingredients, "Slice the cucumber thinly.");
        assert_eq!(result.enhanced_text, "Slice the **1 cucumber** thinly.");
        assert_eq!(result.injection_count, 1);
    }

    #[test]
    fn head_noun_fallback_when_only_the_noun_appears() {
        let ingredients = vec![make_ingredient("ripe avocados", 2.0, "")];
        let result = enhance(&ingredients, "Mash the avocados with a fork.");
        assert_eq!(result, "Mash the **2 avocados** with a fork.");
    }

    #[test]
    fn head_noun_fallback_matches_before_punctuation() {
        let ingredients = vec![make_ingredient("ripe avocados", 2.0, "")];
        let result = enhance(&ingredients, "Halve and pit the avocado.");
        assert_eq!(result, "Halve and pit the **2 avocado**.");
    }

    #[test]
    fn plural_is_stripped_to_match_a_singular_step() {
        let ingredients = vec![make_ingredient("tomatoes", 2.0, "")];
        let result = enhance(&ingredients, "Dice the tomato.");
        assert_eq!(result, "Dice the **2 tomato**.");
    }

    #[test]
    fn singular_name_matches_a_plural_step() {
        let ingredients = vec![make_ingredient("egg", 1.0, "")];
        let result = enhance(&ingredients, "Whisk the eggs.");
        assert_eq!(result, "Whisk the **1 eggs**.");
    }

    #[test]
    fn plural_of_a_detailed_name_matches_its_head_noun() {
        let ingredients = vec![make_ingredient("kalamata olives", 0.5, "cup")];
        let result = enhance(&ingredients, "Scatter the olive halves on top.");
        assert_eq!(result, "Scatter the **0.5 cup olive** halves on top.");
    }

    #[test]
    fn salt_does_not_match_salted() {
        let ingredients = vec![make_ingredient("salt", 1.0, "tsp")];
        let instructions = "Bring a pot of salted water to a boil.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result.enhanced_text, instructions);
        assert_eq!(result.injection_count, 0);
    }

    #[test]
    fn name_is_cut_at_a_parenthesis() {
        let ingredients = vec![make_ingredient("butter (softened)", 0.5, "cup")];
        let result = enhance(&ingredients, "Cream the butter and sugar.");
        assert_eq!(result, "Cream the **0.5 cup butter** and sugar.");
    }

    #[test]
    fn name_is_cut_at_a_comma() {
        let ingredients = vec![make_ingredient("tomatoes, diced", 2.0, "")];
        let result = enhance(&ingredients, "Stir in the tomatoes.");
        assert_eq!(result, "Stir in the **2 tomatoes**.");
    }

    #[test]
    fn full_name_wins_and_a_shared_head_noun_is_not_placed() {
        let ingredients = vec![
            make_ingredient("vegetable oil", 1.0, "cup"),
            make_ingredient("olive oil", 2.0, "tbsp"),
        ];
        let result = enhance_instructions(&ingredients, "Heat the oil.\nDrizzle with olive oil.");
        assert_eq!(
            result.enhanced_text,
            "Heat the oil.\nDrizzle with **2 tbsp olive oil**."
        );
        assert_eq!(result.injection_count, 1);
    }

    #[test]
    fn shared_head_noun_is_placed_for_neither_ingredient() {
        let ingredients = vec![
            make_ingredient("chili powder", 1.0, "tsp"),
            make_ingredient("garlic powder", 0.5, "tsp"),
        ];
        let instructions = "Stir in the powders.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result.enhanced_text, instructions);
        assert_eq!(result.injection_count, 0);
    }

    #[test]
    fn longer_name_is_placed_before_a_name_inside_it() {
        let ingredients = vec![
            make_ingredient("salt", 1.0, "tsp"),
            make_ingredient("kosher salt", 1.0, "tbsp"),
        ];
        let result = enhance(&ingredients, "Add kosher salt.\nSeason with salt.");
        assert_eq!(
            result,
            "Add **1 tbsp kosher salt**.\nSeason with **1 tsp salt**."
        );
    }

    #[test]
    fn matches_at_the_start_and_end_of_a_line() {
        let ingredients = vec![
            make_ingredient("flour", 2.0, "cups"),
            make_ingredient("sugar", 1.0, "cup"),
        ];
        let result = enhance(&ingredients, "Flour goes in first.\nThen add the sugar");
        assert_eq!(
            result,
            "**2 cups Flour** goes in first.\nThen add the **1 cup sugar**"
        );
    }

    #[test]
    fn skips_existing_bold_and_matches_right_after_it() {
        let ingredients = vec![
            make_ingredient("flour", 2.0, "cups"),
            make_ingredient("sugar", 1.0, "cup"),
        ];
        let result = enhance_instructions(&ingredients, "Sift the **flour**.\n**Tip:**sugar last.");
        assert_eq!(
            result.enhanced_text,
            "Sift the **flour**.\n**Tip:****1 cup sugar** last."
        );
        assert_eq!(result.injection_count, 1);
    }

    #[test]
    fn multi_word_name_matches_across_a_run_of_whitespace() {
        let ingredients = vec![make_ingredient("olive oil", 2.0, "tbsp")];
        let result = enhance(&ingredients, "Drizzle with olive  oil.");
        assert_eq!(result, "Drizzle with **2 tbsp olive  oil**.");
    }

    #[test]
    fn char_that_grows_when_lowercased_does_not_shift_the_match() {
        // "İ" (U+0130) lowercases to two chars, so it changes byte length.
        let ingredients = vec![make_ingredient("flour", 2.0, "cups")];
        let result = enhance(&ingredients, "İ add flour.");
        assert_eq!(result, "İ add **2 cups flour**.");
    }

    #[test]
    fn longer_cut_name_is_placed_before_a_full_name_inside_it() {
        let ingredients = vec![
            make_ingredient("salt", 1.0, "tsp"),
            make_ingredient("kosher salt, divided", 1.0, "tbsp"),
        ];
        let result = enhance(&ingredients, "Add kosher salt.\nSeason with salt.");
        assert_eq!(
            result,
            "Add **1 tbsp kosher salt**.\nSeason with **1 tsp salt**."
        );
    }

    #[test]
    fn longer_name_is_placed_before_a_cut_name_inside_it() {
        let ingredients = vec![
            make_ingredient(
                "olive oil, plus more for drizzling over the top",
                2.0,
                "tbsp",
            ),
            make_ingredient("extra virgin olive oil, divided", 1.0, "cup"),
        ];
        let result = enhance(
            &ingredients,
            "Heat the extra virgin olive oil.\nDrizzle with olive oil.",
        );
        assert_eq!(
            result,
            "Heat the **1 cup extra virgin olive oil**.\nDrizzle with **2 tbsp olive oil**."
        );
    }

    #[test]
    fn cut_name_is_placed_when_it_is_another_ingredients_last_word() {
        let ingredients = vec![
            make_ingredient("sugar, divided", 1.0, "cup"),
            make_ingredient("brown sugar", 0.5, "cup"),
        ];
        let result = enhance_instructions(
            &ingredients,
            "Cream the butter and brown sugar.\nAdd the sugar.",
        );
        assert_eq!(
            result.enhanced_text,
            "Cream the butter and **0.5 cup brown sugar**.\nAdd the **1 cup sugar**."
        );
        assert_eq!(result.injection_count, 2);
    }

    #[test]
    fn numbered_name_is_not_claimed_by_a_shorter_name_inside_it() {
        let ingredients = vec![
            make_ingredient("salt", 1.0, "tsp"),
            make_ingredient("kosher salt", 1.0, "tbsp"),
        ];
        let result =
            enhance_instructions(&ingredients, "Add 1 tbsp kosher salt.\nSeason with salt.");
        assert_eq!(
            result.enhanced_text,
            "Add 1 tbsp kosher salt.\nSeason with **1 tsp salt**."
        );
        assert_eq!(result.injection_count, 1);
    }

    #[test]
    fn claimed_numbered_text_moves_with_an_injection_before_it() {
        let ingredients = vec![
            make_ingredient("salt", 1.0, "tsp"),
            make_ingredient("flour", 2.0, "cups"),
            make_ingredient("kosher salt", 1.0, "tbsp"),
        ];
        let result = enhance(&ingredients, "Add flour and 1 tbsp kosher salt, then salt.");
        assert_eq!(
            result,
            "Add **2 cups flour** and 1 tbsp kosher salt, then **1 tsp salt**."
        );
    }

    #[test]
    fn an_earlier_number_in_the_sentence_does_not_count() {
        let ingredients = vec![make_ingredient("butter", 2.0, "tbsp")];
        let result = enhance_instructions(
            &ingredients,
            "Preheat the oven to 350 degrees and grease the pan with butter.\nMelt the butter.",
        );
        assert_eq!(
            result.enhanced_text,
            "Preheat the oven to 350 degrees and grease the pan with **2 tbsp butter**.\nMelt the butter."
        );
        assert_eq!(result.injection_count, 1);
    }

    #[test]
    fn a_number_and_a_unit_word_count_as_numbered() {
        let ingredients = vec![make_ingredient("flour", 2.0, "cups")];
        let result = enhance_instructions(&ingredients, "Whisk 2 cups flour with the salt.");
        assert_eq!(result.enhanced_text, "Whisk 2 cups flour with the salt.");
        assert_eq!(result.injection_count, 0);
    }

    #[test]
    fn a_duration_before_other_words_does_not_count() {
        let ingredients = vec![make_ingredient("eggs", 3.0, "")];
        let result = enhance(&ingredients, "Bake 20 minutes, then add eggs.");
        assert_eq!(result, "Bake 20 minutes, then add **3 eggs**.");
    }

    #[test]
    fn a_number_and_a_size_word_count_as_numbered() {
        let ingredients = vec![make_ingredient("egg", 1.0, "")];
        let instructions = "Beat in 1 large egg.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result.enhanced_text, instructions);
        assert_eq!(result.injection_count, 0);
    }

    #[test]
    fn a_unicode_fraction_counts_as_numbered() {
        let ingredients = vec![make_ingredient("salt", 0.5, "tsp")];
        let instructions = "Stir in ½ tsp salt.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result.enhanced_text, instructions);
        assert_eq!(result.injection_count, 0);
    }

    #[test]
    fn a_connecting_word_after_a_number_does_not_count() {
        let cases = [
            (
                make_ingredient("cheese", 1.0, "cup"),
                "Bake at 350 until cheese melts.\nTop with cheese.",
                "Bake at 350 until **1 cup cheese** melts.\nTop with cheese.",
            ),
            (
                make_ingredient("potatoes", 2.0, "lb"),
                "Roast at 425 until potatoes are tender.",
                "Roast at 425 until **2 lb potatoes** are tender.",
            ),
            (
                make_ingredient("butter", 1.0, "tbsp"),
                "Preheat to 350 and butter the pan.",
                "Preheat to 350 and **1 tbsp butter** the pan.",
            ),
            (
                make_ingredient("flour", 2.0, "cups"),
                "Step 2 add flour.",
                "Step 2 add **2 cups flour**.",
            ),
        ];
        for (ingredient, instructions, expected) in cases {
            let result = enhance_instructions(&[ingredient], instructions);
            assert_eq!(result.enhanced_text, expected);
            assert_eq!(result.injection_count, 1);
        }
    }

    #[test]
    fn a_unit_abbreviation_with_a_period_counts_as_numbered() {
        let ingredients = vec![make_ingredient("oil", 2.0, "tbsp")];
        let instructions = "Heat 2 tbsp. oil.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result.enhanced_text, instructions);
        assert_eq!(result.injection_count, 0);
    }

    #[test]
    fn a_sentence_ending_after_a_number_does_not_count() {
        let ingredients = vec![make_ingredient("salt", 1.0, "tsp")];
        let result = enhance_instructions(&ingredients, "Simmer 10 min. Salt the soup");
        assert_eq!(
            result.enhanced_text,
            "Simmer 10 min. **1 tsp Salt** the soup"
        );
        assert_eq!(result.injection_count, 1);
    }

    #[test]
    fn a_unit_word_followed_by_of_counts_as_numbered() {
        let ingredients = vec![make_ingredient("flour", 2.0, "cups")];
        let instructions = "Add 2 cups of flour.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result.enhanced_text, instructions);
        assert_eq!(result.injection_count, 0);
    }

    #[test]
    fn a_name_without_letters_or_digits_is_skipped() {
        let ingredients = vec![make_ingredient(",", 1.0, "")];
        let instructions = "Stir , then serve.";
        let result = enhance_instructions(&ingredients, instructions);
        assert_eq!(result.enhanced_text, instructions);
        assert_eq!(result.injection_count, 0);
    }

    #[test]
    fn a_match_must_start_on_a_word_boundary() {
        let ingredients = vec![make_ingredient("oil", 2.0, "tbsp")];
        let result = enhance(&ingredients, "Boil the water, then add oil.");
        assert_eq!(result, "Boil the water, then add **2 tbsp oil**.");
    }

    #[test]
    fn a_numbered_first_occurrence_settles_before_later_passes() {
        // Each later pass could match the second line, so only settling the
        // ingredient at its numbered first occurrence keeps the text unchanged.
        let cases = [
            (
                make_ingredient("egg", 1.0, ""),
                "Beat in 1 egg.\nBrush with eggs.",
            ),
            (
                make_ingredient("ripe avocados", 2.0, ""),
                "Mash 2 ripe avocados.\nTop with avocado.",
            ),
        ];
        for (ingredient, instructions) in cases {
            let result = enhance_instructions(&[ingredient], instructions);
            assert_eq!(result.enhanced_text, instructions);
            assert_eq!(result.injection_count, 0);
        }
    }
}
