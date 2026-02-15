use serde::Serialize;

use crate::dto::CreateDrinkRecipeDto;
use crate::services::claude_client::ClaudeClient;
use crate::services::recipe_adapter::strip_code_fences;
use crate::services::recipe_import_service::{ImportError, RecipeImportService, MIN_CONTENT_CHARS};

/// Result of an AI drink recipe import: the extracted DTO plus token usage
#[derive(Debug, Serialize)]
pub struct DrinkImportResult {
    pub recipe: CreateDrinkRecipeDto,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

pub struct DrinkRecipeImportService;

impl DrinkRecipeImportService {
    /// Fetch a URL and extract a drink recipe via AI
    pub async fn import_from_url(
        api_key: &str,
        model: &str,
        url: &str,
    ) -> Result<DrinkImportResult, ImportError> {
        let html = RecipeImportService::fetch_url(url).await?;
        let content = RecipeImportService::extract_content(&html);

        if content.len() < MIN_CONTENT_CHARS {
            return Err(ImportError::ContentTooShort);
        }

        Self::extract_drink_recipe_with_ai(api_key, model, &content).await
    }

    async fn extract_drink_recipe_with_ai(
        api_key: &str,
        model: &str,
        content: &str,
    ) -> Result<DrinkImportResult, ImportError> {
        let system_prompt = Self::build_system_prompt();
        let user_message = format!(
            "Extract the drink recipe from the following content and return it as JSON:\n\n{}",
            content
        );

        let response =
            ClaudeClient::send_message(api_key, model, Some(&system_prompt), &user_message).await?;

        let cleaned = strip_code_fences(&response.text);
        let json_str = extract_json_object(&cleaned).unwrap_or(&cleaned);
        let mut dto: CreateDrinkRecipeDto = serde_json::from_str(json_str).map_err(|e| {
            tracing::error!(
                "Failed to parse AI response as drink recipe. Error: {}. First 500 chars: {}",
                e,
                &json_str[..json_str.len().min(500)]
            );
            ImportError::ParseError(
                "AI returned an unparseable response. Try again or try a different URL."
                    .to_string(),
            )
        })?;

        dto.source = "url_import".to_string();

        Ok(DrinkImportResult {
            recipe: dto,
            input_tokens: response.input_tokens,
            output_tokens: response.output_tokens,
        })
    }

    /// Build the cocktail-specific extraction prompt
    pub(crate) fn build_system_prompt() -> String {
        r#"You are a cocktail recipe extraction assistant. You will be given content from a webpage that contains a drink or cocktail recipe. Extract the recipe and return it as structured JSON.

Return ONLY valid JSON matching this exact schema (no markdown fences, no commentary, no explanation outside the JSON):

{
  "name": "string",
  "description": "string or null — a brief 1-2 sentence description of the drink",
  "source": "url_import",
  "servings": integer (default 1),
  "instructions": "string (full step-by-step mixing instructions)",
  "ingredients": [
    {
      "name": "string",
      "amount": {"type": "single", "value": number} OR {"type": "range", "min": number, "max": number},
      "unit": "string",
      "notes": "string or null"
    }
  ],
  "technique": "string or null — the primary mixing method (e.g. stirred, shaken, built, blended, muddled, swizzled)",
  "glassware": "string or null — the recommended glass (e.g. coupe, rocks, highball, nick & nora, flute)",
  "garnish": "string or null — the garnish description (e.g. lemon twist, cherry, mint sprig)",
  "tags": ["string"],
  "notes": "string or null — any additional tips, variations, or history",
  "icon": "string (single emoji representing the drink) or null",
  "is_non_alcoholic": boolean
}

Rules:
- Extract all ingredients with accurate amounts, units, and names
- Preserve the full instructions as written
- IMPORTANT: ingredient amount values must ALWAYS be numbers, never null. Use 0 for "to taste" or unspecified amounts
- IMPORTANT: ingredient "unit" must ALWAYS be a string, never null. Use "" (empty string) for unitless items (e.g. dashes, drops counted by name)
- If ingredient amounts are written as fractions (e.g., "3/4 oz"), convert to decimal (0.75)
- Default to 1 serving if not specified
- Extract the mixing technique if mentioned or inferable from instructions (shaken, stirred, built, blended, etc.)
- Extract the glassware if mentioned
- Extract the garnish separately from the ingredients list
- Set is_non_alcoholic to true only if the drink contains NO alcohol whatsoever (mocktails, virgin drinks)
- Add relevant tags — e.g. "classic", "tiki", "sour", "stirred", "shaken", "spirit-forward", "refreshing", "bitter", "creamy", "hot", "frozen", "punch", "non-alcoholic"
- If the content contains multiple recipes, extract only the primary/featured recipe
- Choose a fitting emoji icon (🍸 martini, 🥃 spirit-forward, 🍹 tropical, 🍋 sour, 🫧 fizzy, ☕ hot, 🥂 champagne, etc.)"#
            .to_string()
    }
}

/// Extract a JSON object from text that may contain surrounding prose.
/// Finds the first `{` and the matching closing `}` using brace-depth tracking.
fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, ch) in text[start..].char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..start + i + 1]);
                }
            }
            _ => {}
        }
    }

    None
}
