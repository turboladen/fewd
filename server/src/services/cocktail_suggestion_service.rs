use std::fmt::Write;

use chrono::Datelike;
use serde::Serialize;

use crate::dto::{CreateDrinkRecipeDto, DrinkMood};
use crate::entities::{bar_item, drink_recipe, person};
use crate::services::claude_client::{ClaudeClient, ClaudeError, SendMessageResponse};
use crate::services::recipe_adapter::strip_code_fences;

#[derive(Debug, Serialize)]
pub struct CocktailSuggestionResult {
    pub suggestions: Vec<CreateDrinkRecipeDto>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

pub struct CocktailContext<'a> {
    pub people: &'a [person::Model],
    pub bar_items: &'a [bar_item::Model],
    pub mood: &'a DrinkMood,
    pub include_non_alcoholic: bool,
    pub drink_recipes: &'a [drink_recipe::Model],
    pub feedback: Option<&'a str>,
    pub previous_suggestions: Option<&'a [String]>,
}

pub struct CocktailSuggestionService;

impl CocktailSuggestionService {
    pub(crate) const SUGGESTION_MAX_TOKENS: u32 = 8192;

    pub async fn suggest_cocktails(
        api_key: &str,
        model: &str,
        ctx: &CocktailContext<'_>,
    ) -> Result<CocktailSuggestionResult, ClaudeError> {
        let system_prompt = Self::build_system_prompt();
        let user_message = Self::build_user_message(ctx);

        let response: SendMessageResponse = ClaudeClient::send_message_with_max_tokens(
            api_key,
            model,
            Some(&system_prompt),
            &user_message,
            Self::SUGGESTION_MAX_TOKENS,
        )
        .await?;

        let suggestions = Self::parse_response(&response.text)?;

        Ok(CocktailSuggestionResult {
            suggestions,
            input_tokens: response.input_tokens,
            output_tokens: response.output_tokens,
        })
    }

    pub fn build_system_prompt() -> String {
        r#"You are a creative cocktail and drink suggestion assistant and bartender, specializing in bespoke cocktails. Generate 3-5 diverse drink suggestions as complete recipes.

Return ONLY a valid JSON array of drink recipe objects (no markdown fences, no commentary, no explanation outside the JSON):

[
  {
    "name": "string (creative, descriptive drink name)",
    "description": "string (1-2 sentence enticing description)",
    "source": "ai_suggested",
    "servings": 1,
    "instructions": "string (step-by-step mixing instructions)",
    "ingredients": [
      {
        "name": "string",
        "amount": {"type": "single", "value": number} OR {"type": "range", "min": number, "max": number},
        "unit": "string (oz, dash, ml, splash, etc.)",
        "notes": "string or null"
      }
    ],
    "technique": "string or null (one of: stirred, shaken, built, blended, muddled)",
    "glassware": "string or null (e.g., coupe, rocks, highball, martini, tiki mug, collins)",
    "garnish": "string or null (e.g., lemon twist, cherry, mint sprig)",
    "tags": ["string (e.g. classic, tiki, sour, spirit-forward, refreshing, non-alcoholic)"],
    "notes": "string or null",
    "icon": "string (single drink emoji)",
    "is_non_alcoholic": boolean
  }
]

Rules:
- Generate 3-5 diverse suggestions (vary styles, base spirits, and flavor profiles)
- Use ONLY ingredients from the provided bar inventory (or very common pantry items like water, ice, sugar, salt, eggs)
- Every suggestion must work for the ENTIRE group: avoid ALL listed dislikes and lean into shared preferences
- Do NOT generate per-person drinks; every drink in the list should be enjoyable by everyone
- If the group includes anyone under 21, ALL suggestions MUST be non-alcoholic (set is_non_alcoholic to true)
- This is a safety requirement: when a minor is present, do not suggest any alcoholic drinks
- Match the requested cocktail style(s) closely. If multiple styles are listed, spread suggestions across them
- Provide accurate measurements (typically in oz for spirits, dashes for bitters)
- Include proper technique, glassware, and garnish recommendations
- If previous suggestions are listed, generate completely different options
- Keep instructions concise but complete"#
            .to_string()
    }

    pub fn build_user_message(ctx: &CocktailContext<'_>) -> String {
        let mut message = String::new();

        // Bar inventory grouped by category
        if !ctx.bar_items.is_empty() {
            let _ = writeln!(message, "# Bar Inventory\n");
            let mut current_category = "";
            for item in ctx.bar_items {
                if item.category != current_category {
                    current_category = &item.category;
                    let _ = writeln!(message, "**{}:**", capitalize(current_category));
                }
                let _ = writeln!(message, "- {}", item.name);
            }
            let _ = writeln!(message);
        }

        // People context with drink preferences
        if !ctx.people.is_empty() {
            let _ = writeln!(message, "# People\n");
            for person in ctx.people {
                let age = compute_age(person.birthdate);
                let _ = writeln!(message, "## {} (age {})", person.name, age);

                if let Some(ref prefs) = person.drink_preferences {
                    let parsed = parse_string_array(prefs);
                    if !parsed.is_empty() {
                        let _ = writeln!(message, "- Drink preferences: {}", parsed.join(", "));
                    }
                }

                if let Some(ref dislikes) = person.drink_dislikes {
                    let parsed = parse_string_array(dislikes);
                    if !parsed.is_empty() {
                        let _ = writeln!(message, "- Drink dislikes: {}", dislikes_str(&parsed));
                    }
                }

                if age < 21 {
                    let _ = writeln!(message, "- **Non-alcoholic drinks only**");
                }

                let _ = writeln!(message);
            }
        }

        // Mood / style
        let mood_str = match ctx.mood {
            DrinkMood::Style { label } => label.as_str(),
            DrinkMood::Custom { text } => text.as_str(),
        };
        let _ = writeln!(message, "# Style: {}\n", mood_str);

        // Non-alcoholic flag (set when a minor is in the group)
        if ctx.include_non_alcoholic {
            let _ = writeln!(
                message,
                "⚠️ IMPORTANT: The group includes someone under 21. ALL suggestions MUST be non-alcoholic. Do not suggest any drinks containing alcohol.\n"
            );
        }

        // Previously made drinks (avoid repeats)
        if !ctx.drink_recipes.is_empty() {
            let _ = writeln!(message, "# Previously Made Drinks\n");
            for recipe in ctx.drink_recipes.iter().take(20) {
                let _ = writeln!(message, "- {}", recipe.name);
            }
            let _ = writeln!(message);
        }

        // Previous suggestions to avoid (for regeneration)
        if let Some(names) = ctx.previous_suggestions {
            if !names.is_empty() {
                let _ = writeln!(message, "# Previously Suggested (avoid these)\n");
                for name in names {
                    let _ = writeln!(message, "- {}", name);
                }
                let _ = writeln!(message);
            }
        }

        // User feedback for regeneration
        if let Some(fb) = ctx.feedback {
            if !fb.trim().is_empty() {
                let _ = writeln!(message, "# User Feedback\n");
                let _ = writeln!(message, "{}", fb.trim());
            }
        }

        message.trim_end().to_string()
    }

    pub fn parse_response(text: &str) -> Result<Vec<CreateDrinkRecipeDto>, ClaudeError> {
        let cleaned = strip_code_fences(text);

        if cleaned.trim().is_empty() {
            eprintln!(
                "Cocktail suggestion response was empty. Raw text ({} chars): {:?}",
                text.len(),
                &text[..text.len().min(200)]
            );
            return Err(ClaudeError::ParseError(
                "Claude returned an empty response. Try again.".to_string(),
            ));
        }

        let mut suggestions: Vec<CreateDrinkRecipeDto> =
            serde_json::from_str(&cleaned).map_err(|e| {
                eprintln!(
                    "Failed to parse cocktail suggestions JSON: {}. First 500 chars: {:?}",
                    e,
                    &cleaned[..cleaned.len().min(500)]
                );
                ClaudeError::ParseError(format!("Failed to parse cocktail suggestions JSON: {}", e))
            })?;

        if suggestions.is_empty() {
            return Err(ClaudeError::ParseError(
                "No cocktail suggestions returned".to_string(),
            ));
        }

        // Force correct source on every suggestion
        for dto in &mut suggestions {
            dto.source = "ai_suggested".to_string();
        }

        Ok(suggestions)
    }
}

fn compute_age(birthdate: chrono::NaiveDate) -> i32 {
    let today = chrono::Utc::now().date_naive();
    let mut age = today.year() - birthdate.year();
    if (today.month(), today.day()) < (birthdate.month(), birthdate.day()) {
        age -= 1;
    }
    age
}

fn parse_string_array(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

fn dislikes_str(items: &[String]) -> String {
    items.join(", ")
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}
