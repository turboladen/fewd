//! Canonical diet-tag vocabulary — the single source of truth the
//! `list_diet_tags` tool and the `fewd://diet-tags` resource both serve.
//!
//! The LLM translates a person's free-form `dietary_goals` into these tags,
//! then filters via `search_recipes(tags=[...])`. Tags are stored on recipes
//! as plain strings in the existing free-form `tags` array; nothing here is
//! enforced server-side (soft convention — see fewd-08x). `create_recipe`'s
//! description merely encourages applying applicable tags.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One canonical diet tag plus a one-line meaning the LLM uses to map a
/// person's free-form goals onto it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DietTag {
    /// The exact lowercase string to pass in `search_recipes`'s `tags` filter
    /// and to apply in `create_recipe`. Matching is case-insensitive, but emit
    /// lowercase for consistency with stored tags.
    pub tag: String,
    /// What dietary constraint this tag asserts about a recipe — use it to
    /// decide which tag(s) a person's free-form `dietary_goals` maps to.
    pub meaning: String,
}

/// The vocabulary. Single source of truth — the tool, the resource, and the
/// README taxonomy all derive from this. Keep entries lowercase, hyphenated
/// (no spaces), and stable: recipes are tagged against these exact strings.
pub const DIET_TAGS: &[(&str, &str)] = &[
    (
        "vegetarian",
        "No meat, poultry, or seafood. May include dairy and eggs.",
    ),
    (
        "vegan",
        "No animal products at all — no meat, dairy, eggs, or honey.",
    ),
    ("pescatarian", "No meat or poultry, but seafood is allowed."),
    (
        "gluten-free",
        "Contains no wheat, barley, rye, or other gluten sources.",
    ),
    (
        "dairy-free",
        "Contains no milk, cheese, butter, or other dairy.",
    ),
    ("nut-free", "Contains no tree nuts or peanuts."),
    (
        "low-carb",
        "Low in carbohydrates; minimizes grains, sugars, and starches.",
    ),
    (
        "keto",
        "Very low carb, high fat — suitable for a ketogenic diet.",
    ),
    (
        "paleo",
        "No grains, legumes, dairy, or refined sugar (paleo template).",
    ),
    (
        "low-sodium",
        "Prepared with little added salt; suitable for sodium-restricted diets.",
    ),
    (
        "high-protein",
        "Notably high protein per serving; suits muscle-gain or satiety goals.",
    ),
    (
        "whole30",
        "No added sugar, grains, dairy, legumes, or alcohol (Whole30 elimination template).",
    ),
    (
        "mediterranean",
        "Emphasizes vegetables, whole grains, fish, and olive oil; minimal red meat.",
    ),
    (
        "low-fodmap",
        "Low in fermentable carbs (FODMAPs) that can trigger IBS symptoms.",
    ),
    (
        "halal",
        "Permissible under Islamic dietary law (no pork or alcohol; meat slaughtered halal).",
    ),
    (
        "kosher",
        "Conforms to Jewish dietary law (no pork or shellfish; meat and dairy not mixed).",
    ),
];

/// Materialize the vocabulary into the serializable shape the
/// `list_diet_tags` tool returns as JSON.
pub fn diet_tags_payload() -> Vec<DietTag> {
    DIET_TAGS
        .iter()
        .map(|(tag, meaning)| DietTag {
            tag: (*tag).to_string(),
            meaning: (*meaning).to_string(),
        })
        .collect()
}

/// Render the same vocabulary as Markdown for the `fewd://diet-tags` resource
/// (user-attachment surface), mirroring `render_family_overview`.
pub fn render_diet_tags_markdown() -> String {
    let mut out = String::from(
        "# fewd diet-tag vocabulary\n\n\
         Apply these tags on recipes (`create_recipe`) and filter by them \
         (`search_recipes` `tags`). Translate a person's free-form dietary \
         goals into one or more of these, then search per-constraint — \
         multiple tags AND together.\n\n",
    );
    for (tag, meaning) in DIET_TAGS {
        out.push_str(&format!("- **{tag}** — {meaning}\n"));
    }
    out
}
