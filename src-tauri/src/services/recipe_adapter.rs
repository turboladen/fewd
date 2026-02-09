use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::commands::recipe::CreateRecipeDto;
use crate::entities::{person, recipe};
use crate::services::claude_client::{ClaudeClient, ClaudeError, SendMessageResponse};
use crate::services::prompt_builder::PromptBuilder;

/// Controls which profile fields to include per person in the adaptation prompt
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PersonAdaptOptions {
    pub person_id: String,
    pub include_dietary_goals: bool,
    pub include_dislikes: bool,
    pub include_favorites: bool,
}

/// Result of a recipe adaptation: the adapted recipe DTO plus token usage
#[derive(Debug, Serialize)]
pub struct AdaptResult {
    pub recipe: CreateRecipeDto,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

pub struct RecipeAdapter;

impl RecipeAdapter {
    /// Adapt a recipe for the given people's dietary needs
    pub async fn adapt_recipe(
        api_key: &str,
        model: &str,
        recipe: &recipe::Model,
        people: &[person::Model],
        options: &[PersonAdaptOptions],
        user_instructions: &str,
    ) -> Result<AdaptResult, ClaudeError> {
        let system_prompt = Self::build_system_prompt();
        let user_message = Self::build_user_message(recipe, people, options, user_instructions);

        let response: SendMessageResponse =
            ClaudeClient::send_message(api_key, model, Some(&system_prompt), &user_message).await?;

        let adapted = Self::parse_response(&response.text, &recipe.id)?;

        Ok(AdaptResult {
            recipe: adapted,
            input_tokens: response.input_tokens,
            output_tokens: response.output_tokens,
        })
    }

    /// Build the system prompt with JSON schema and adaptation rules
    pub fn build_system_prompt() -> String {
        r#"You are a recipe adaptation assistant. You will be given a recipe and information about the people it should be adapted for. Your job is to adapt the recipe to meet their dietary needs and preferences.

Return ONLY valid JSON matching this exact schema (no markdown fences, no commentary, no explanation outside the JSON):

{
  "name": "string (adapted recipe name)",
  "description": "string or null",
  "source": "ai_adapted",
  "parent_recipe_id": null,
  "servings": integer,
  "instructions": "string (full step-by-step instructions)",
  "ingredients": [
    {
      "name": "string",
      "amount": {"type": "single", "value": number} OR {"type": "range", "min": number, "max": number},
      "unit": "string",
      "notes": "string or null"
    }
  ],
  "tags": ["string"],
  "notes": "string or null (briefly explain key adaptations made)",
  "icon": "string (single emoji) or null",
  "nutrition_per_serving": {
    "calories": integer or null,
    "protein_grams": integer or null,
    "carbs_grams": integer or null,
    "fat_grams": integer or null,
    "notes": "string or null"
  } or null,
  "prep_time": {"value": integer, "unit": "minutes"} or null,
  "cook_time": {"value": integer, "unit": "minutes"} or null,
  "total_time": {"value": integer, "unit": "minutes"} or null,
  "portion_size": null
}

Rules:
- Adapt the recipe to accommodate all listed people's dietary needs and preferences
- Preserve the recipe's general character while making necessary substitutions
- Keep the same serving count unless the user explicitly requests otherwise
- Use the notes field to briefly explain what was changed and why
- If a person dislikes an ingredient, substitute it with something appropriate
- If a person has dietary goals (e.g., high-protein, low-carb, keto), adjust accordingly
- If a person has favorites, try to incorporate them where natural
- Estimate updated nutrition values if the original recipe had them
- Generate an appropriate name for the adapted recipe (e.g., "Keto Grilled Chicken" or "Kid-Friendly Pasta")"#.to_string()
    }

    /// Build the user message with recipe context, filtered people, and instructions
    pub fn build_user_message(
        recipe: &recipe::Model,
        people: &[person::Model],
        options: &[PersonAdaptOptions],
        user_instructions: &str,
    ) -> String {
        let mut message = String::new();

        // Recipe context
        let _ = writeln!(message, "# Original Recipe\n");
        let _ = writeln!(message, "{}", PromptBuilder::build_recipe_context(recipe));
        let _ = writeln!(message);

        // Filtered people context
        let filtered_people = Self::build_filtered_people(people, options);
        if !filtered_people.is_empty() {
            let _ = writeln!(message, "# People to Adapt For\n");
            let _ = writeln!(
                message,
                "{}",
                PromptBuilder::build_person_context(&filtered_people)
            );
            let _ = writeln!(message);
        }

        // User instructions
        if !user_instructions.trim().is_empty() {
            let _ = writeln!(message, "# Additional Instructions\n");
            let _ = writeln!(message, "{}", user_instructions.trim());
        }

        message.trim_end().to_string()
    }

    /// Create filtered copies of people with excluded fields zeroed out
    pub fn build_filtered_people(
        people: &[person::Model],
        options: &[PersonAdaptOptions],
    ) -> Vec<person::Model> {
        people
            .iter()
            .filter_map(|person| {
                let opts = options.iter().find(|o| o.person_id == person.id)?;
                let mut filtered = person.clone();

                if !opts.include_dietary_goals {
                    filtered.dietary_goals = None;
                }
                if !opts.include_dislikes {
                    filtered.dislikes = "[]".to_string();
                }
                if !opts.include_favorites {
                    filtered.favorites = "[]".to_string();
                }

                Some(filtered)
            })
            .collect()
    }

    /// Parse Claude's response text into a CreateRecipeDto
    pub fn parse_response(
        text: &str,
        original_recipe_id: &str,
    ) -> Result<CreateRecipeDto, ClaudeError> {
        // Strip markdown code fences if present
        let cleaned = strip_code_fences(text);

        let mut dto: CreateRecipeDto = serde_json::from_str(&cleaned)
            .map_err(|e| ClaudeError::ParseError(format!("Failed to parse recipe JSON: {}", e)))?;

        // Force correct source and parent link regardless of what Claude returned
        dto.source = "ai_adapted".to_string();
        dto.parent_recipe_id = Some(original_recipe_id.to_string());

        Ok(dto)
    }
}

/// Strip markdown code fences (```json ... ``` or ``` ... ```) from text
pub fn strip_code_fences(text: &str) -> String {
    let trimmed = text.trim();

    // Check for ```json or ``` prefix
    if let Some(rest) = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
    {
        // Find closing ```
        if let Some(content) = rest.strip_suffix("```") {
            return content.trim().to_string();
        }
    }

    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_code_fences_json() {
        let input = "```json\n{\"name\": \"test\"}\n```";
        assert_eq!(strip_code_fences(input), "{\"name\": \"test\"}");
    }

    #[test]
    fn test_strip_code_fences_plain() {
        let input = "```\n{\"name\": \"test\"}\n```";
        assert_eq!(strip_code_fences(input), "{\"name\": \"test\"}");
    }

    #[test]
    fn test_strip_code_fences_none() {
        let input = "{\"name\": \"test\"}";
        assert_eq!(strip_code_fences(input), "{\"name\": \"test\"}");
    }
}
