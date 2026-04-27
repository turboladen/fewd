//! Splits a free-form ingredient string into a purchasable `name` and an
//! optional `prep` clause.
//!
//! Strategy: find the first comma at paren-depth 0. Left half is the name
//! (the thing you buy), right half is the preparation form (how the recipe
//! uses it). If there's no comma, or the split would produce an empty side,
//! the input is returned as-is with `prep = None`.
//!
//! The shopping aggregator keys on `name.to_lowercase()`, so cleanly peeling
//! off prep (`"garlic, minced"` → `"garlic"`) lets multiple recipes that
//! share a purchasable identity collapse into one shopping-list line.

/// Split a raw ingredient name string into `(name, Option<prep>)`.
///
/// Idempotent: calling on an already-split name (no commas) returns
/// `(name, None)`.
pub fn split_name_and_prep(raw: &str) -> (String, Option<String>) {
    let trimmed = raw.trim();

    let Some(idx) = first_top_level_comma(trimmed) else {
        return (trimmed.to_string(), None);
    };

    let name = trimmed[..idx].trim();
    let prep = trimmed[idx + 1..].trim();

    if name.is_empty() || prep.is_empty() {
        return (trimmed.to_string(), None);
    }

    (name.to_string(), Some(prep.to_string()))
}

fn first_top_level_comma(s: &str) -> Option<usize> {
    let mut depth: i32 = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            ',' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(s: &str) -> (String, Option<String>) {
        split_name_and_prep(s)
    }

    #[test]
    fn no_comma_returns_input_unchanged() {
        assert_eq!(split("garlic"), ("garlic".to_string(), None));
        assert_eq!(split("olive oil"), ("olive oil".to_string(), None));
        assert_eq!(
            split("Salt and black pepper"),
            ("Salt and black pepper".to_string(), None)
        );
    }

    #[test]
    fn simple_split() {
        assert_eq!(
            split("garlic, minced"),
            ("garlic".to_string(), Some("minced".to_string()))
        );
        assert_eq!(
            split("eggs, beaten"),
            ("eggs".to_string(), Some("beaten".to_string()))
        );
    }

    #[test]
    fn multi_word_name() {
        assert_eq!(
            split("Garlic cloves, minced"),
            ("Garlic cloves".to_string(), Some("minced".to_string()))
        );
        assert_eq!(
            split("boneless skinless chicken thighs, cut into 1.5-inch chunks"),
            (
                "boneless skinless chicken thighs".to_string(),
                Some("cut into 1.5-inch chunks".to_string())
            )
        );
    }

    #[test]
    fn paren_depth_aware_split() {
        // The comma inside the parens is NOT the split point; the one after `)` is.
        assert_eq!(
            split("pear (or Fuji apple), grated"),
            (
                "pear (or Fuji apple)".to_string(),
                Some("grated".to_string())
            )
        );
    }

    #[test]
    fn first_comma_consumed_only() {
        // Multi-comma prep: only the first comma splits; the rest are part of prep.
        assert_eq!(
            split("Parmesan, freshly grated, for serving"),
            (
                "Parmesan".to_string(),
                Some("freshly grated, for serving".to_string())
            )
        );
    }

    #[test]
    fn degenerate_empty_sides_left_alone() {
        assert_eq!(
            split(", missing left"),
            (", missing left".to_string(), None)
        );
        assert_eq!(
            split("missing right,"),
            ("missing right,".to_string(), None)
        );
        assert_eq!(split(","), (",".to_string(), None));
    }

    #[test]
    fn idempotent_on_already_split_name() {
        let (name, prep) = split("garlic, minced");
        let (name2, prep2) = split(&name);
        assert_eq!(name, name2);
        assert_eq!(prep2, None);
        assert_eq!(prep, Some("minced".to_string()));
    }

    #[test]
    fn whitespace_trimmed() {
        assert_eq!(
            split("  garlic ,  minced  "),
            ("garlic".to_string(), Some("minced".to_string()))
        );
    }

    /// Table-driven test pinning behavior against actual production
    /// ingredient strings pulled from the dietpi DB on 2026-04-27. The audit
    /// surfaced 59 comma-bearing rows. The fixtures embedded here cover:
    ///
    /// - 43 clean `<name>, <prep>` splits in `clean_splits`.
    /// - 2 recipe-author meta-prose strings in `meta_prose` (the splitter
    ///   still consumes their first comma — see the loop comment for why
    ///   that's harmless).
    ///
    /// The remaining ~14 rows are corrupted by upstream parser bugs (em-dash
    /// ranges, Unicode fractions, compound units mis-bucketed by
    /// `splitn(3, ' ')`). They're intentionally NOT covered here — those
    /// rows look ugly today and our splitter can't meaningfully fix them.
    /// Tracked separately as fewd-4i3.
    #[test]
    fn live_data_calibration() {
        let clean_splits: &[(&str, &str, &str)] = &[
            (
                "Celery stalks, finely diced",
                "Celery stalks",
                "finely diced",
            ),
            (
                "Fresh mozzarella, torn into pieces",
                "Fresh mozzarella",
                "torn into pieces",
            ),
            (
                "Fresh parsley or basil, chopped",
                "Fresh parsley or basil",
                "chopped",
            ),
            ("Garlic cloves, minced", "Garlic cloves", "minced"),
            (
                "Medium carrots, finely diced",
                "Medium carrots",
                "finely diced",
            ),
            ("Medium onion, finely diced", "Medium onion", "finely diced"),
            (
                "Nori sheets, cut into strips",
                "Nori sheets",
                "cut into strips",
            ),
            (
                "Parmesan or Pecorino Romano, finely grated",
                "Parmesan or Pecorino Romano",
                "finely grated",
            ),
            ("Parmesan, freshly grated", "Parmesan", "freshly grated"),
            (
                "Parmesan, freshly grated, for serving",
                "Parmesan",
                "freshly grated, for serving",
            ),
            (
                "Parmesan, freshly grated, plus more for serving",
                "Parmesan",
                "freshly grated, plus more for serving",
            ),
            ("Parmesan, grated", "Parmesan", "grated"),
            ("Parmesan, shaved or grated", "Parmesan", "shaved or grated"),
            ("bell pepper, sliced", "bell pepper", "sliced"),
            (
                "boneless skinless chicken thighs, cut into 1.5-inch chunks",
                "boneless skinless chicken thighs",
                "cut into 1.5-inch chunks",
            ),
            ("broccoli, cut into florets", "broccoli", "cut into florets"),
            (
                "cannellini beans, drained and rinsed",
                "cannellini beans",
                "drained and rinsed",
            ),
            ("carrots, finely diced", "carrots", "finely diced"),
            ("celery, finely diced", "celery", "finely diced"),
            (
                "dried wakame seaweed, rehydrated in water",
                "dried wakame seaweed",
                "rehydrated in water",
            ),
            ("eggs, beaten", "eggs", "beaten"),
            (
                "firm tofu, cut into 1/2-inch cubes",
                "firm tofu",
                "cut into 1/2-inch cubes",
            ),
            (
                "fresh cilantro, finely chopped",
                "fresh cilantro",
                "finely chopped",
            ),
            ("fresh ginger, grated", "fresh ginger", "grated"),
            (
                "fresh parsley, finely chopped",
                "fresh parsley",
                "finely chopped",
            ),
            (
                "fresh rosemary, finely chopped",
                "fresh rosemary",
                "finely chopped",
            ),
            ("frozen edamame, thawed", "frozen edamame", "thawed"),
            (
                "garlic clove, minced or grated",
                "garlic clove",
                "minced or grated",
            ),
            ("garlic, minced", "garlic", "minced"),
            ("garlic, thinly sliced", "garlic", "thinly sliced"),
            (
                "melted butter, plus more for the pan",
                "melted butter",
                "plus more for the pan",
            ),
            ("onion, finely diced", "onion", "finely diced"),
            ("onion, very thinly sliced", "onion", "very thinly sliced"),
            ("onions, chopped", "onions", "chopped"),
            ("onions, sliced", "onions", "sliced"),
            ("pasta water, reserved", "pasta water", "reserved"),
            (
                "pear (or Fuji apple), grated",
                "pear (or Fuji apple)",
                "grated",
            ),
            ("peppers, sliced", "peppers", "sliced"),
            (
                "ribeye steak, thinly sliced",
                "ribeye steak",
                "thinly sliced",
            ),
            ("thick-cut bacon, diced", "thick-cut bacon", "diced"),
            ("tomatoes, sliced", "tomatoes", "sliced"),
            (
                "unsalted butter, melted and slightly cooled",
                "unsalted butter",
                "melted and slightly cooled",
            ),
            (
                "yellow onions, finely diced",
                "yellow onions",
                "finely diced",
            ),
        ];

        for (raw, expected_name, expected_prep) in clean_splits {
            let (name, prep) = split_name_and_prep(raw);
            assert_eq!(&name, expected_name, "name mismatch for input {:?}", raw);
            assert_eq!(
                prep.as_deref(),
                Some(*expected_prep),
                "prep mismatch for input {:?}",
                raw
            );
        }

        // Recipe-author meta-prose: strings that aren't really `name, prep`
        // but contain top-level commas anyway. The splitter has no way to
        // tell these apart from genuine prep clauses, so it splits them too,
        // producing a "garbage prep" that doesn't collide with real prep
        // clauses (the aggregator keys on `name` only). Asserting:
        //   1. the function never panics or produces empty fields
        //   2. recombination is lossless (so an operator inspecting the
        //      DB can still see the original string)
        let meta_prose: &[&str] = &[
            "All tossed with olive oil, salt, and pepper",
            "Pizza sauce or crushed tomatoes seasoned with salt, olive oil, and oregano",
        ];
        for raw in meta_prose {
            let (name, prep) = split_name_and_prep(raw);
            assert!(!name.is_empty(), "name unexpectedly empty for {:?}", raw);
            assert!(
                prep.is_some(),
                "expected meta-prose to split, input {:?}",
                raw
            );
            let recombined = match &prep {
                Some(p) => format!("{}, {}", name, p),
                None => name.clone(),
            };
            assert_eq!(&recombined, raw, "recombination should round-trip");
        }
    }
}
