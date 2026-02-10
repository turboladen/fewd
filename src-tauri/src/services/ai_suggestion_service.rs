use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::commands::recipe::CreateRecipeDto;
use crate::entities::{meal, person, recipe};
use crate::services::claude_client::{ClaudeClient, ClaudeError, SendMessageResponse};
use crate::services::prompt_builder::PromptBuilder;
use crate::services::recipe_adapter::{strip_code_fences, PersonAdaptOptions, RecipeAdapter};

/// The "character" or vibe of the meal suggestions
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum MealCharacter {
    #[serde(rename = "balanced")]
    Balanced,
    #[serde(rename = "indulgent")]
    Indulgent,
    #[serde(rename = "quick")]
    Quick,
    #[serde(rename = "custom")]
    Custom { text: String },
}

/// Result of AI meal suggestion: recipe DTOs plus token usage
#[derive(Debug, Serialize)]
pub struct AiSuggestionResult {
    pub suggestions: Vec<CreateRecipeDto>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Bundles all context needed to generate AI meal suggestions
pub struct SuggestionContext<'a> {
    pub people: &'a [person::Model],
    pub person_options: &'a [PersonAdaptOptions],
    pub meal_type: &'a str,
    pub character: &'a MealCharacter,
    pub meals: &'a [meal::Model],
    pub recipes: &'a [recipe::Model],
    pub feedback: Option<&'a str>,
    pub previous_suggestions: Option<&'a [String]>,
}

pub struct AiSuggestionService;

impl AiSuggestionService {
    /// Max tokens for AI meal suggestions (3-5 full recipes need more than default 4096)
    const SUGGESTION_MAX_TOKENS: u32 = 8192;

    /// Generate 3-5 AI meal suggestions based on people, meal context, and preferences
    pub async fn suggest_meals(
        api_key: &str,
        model: &str,
        ctx: &SuggestionContext<'_>,
    ) -> Result<AiSuggestionResult, ClaudeError> {
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

        Ok(AiSuggestionResult {
            suggestions,
            input_tokens: response.input_tokens,
            output_tokens: response.output_tokens,
        })
    }

    /// Build the system prompt instructing Claude to return a JSON array of recipes
    pub fn build_system_prompt() -> String {
        r#"You are a creative meal suggestion assistant. Generate 3-5 diverse meal suggestions as complete recipes.

Return ONLY a valid JSON array of recipe objects (no markdown fences, no commentary, no explanation outside the JSON):

[
  {
    "name": "string (creative, descriptive recipe name)",
    "description": "string (1-2 sentence appetizing description)",
    "source": "ai_suggested",
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
    "tags": ["string (e.g. high-protein, veggie, quick, kid-friendly, indulgent)"],
    "notes": "string or null",
    "icon": "string (single food emoji)",
    "nutrition_per_serving": {
      "calories": integer or null,
      "protein_grams": integer or null,
      "carbs_grams": integer or null,
      "fat_grams": integer or null,
      "notes": null
    } or null,
    "prep_time": {"value": integer, "unit": "minutes"} or null,
    "cook_time": {"value": integer, "unit": "minutes"} or null,
    "total_time": {"value": integer, "unit": "minutes"} or null,
    "portion_size": null
  }
]

Rules:
- Generate 3-5 diverse suggestions (vary protein sources, cuisines, and cooking methods)
- Consider all listed people's dietary needs, dislikes, and preferences
- Avoid suggesting meals similar to what appears in the recent meal history
- Match the requested meal type (breakfast, lunch, dinner, etc.)
- If a meal character is specified, match the overall vibe (balanced, indulgent, quick, etc.)
- Tag each recipe appropriately (high-protein, veggie, quick, kid-friendly, indulgent, etc.)
- Estimate nutrition values when possible
- Each suggestion should be a complete, cookable recipe with full instructions
- Default serving count to 4 unless the number of people suggests otherwise
- If previous suggestions are listed, generate completely different options"#
            .to_string()
    }

    /// Build the user message with people context, meal type, character, and history
    pub fn build_user_message(ctx: &SuggestionContext<'_>) -> String {
        let mut message = String::new();

        // People context (filtered by field toggles)
        let filtered_people = RecipeAdapter::build_filtered_people(ctx.people, ctx.person_options);
        if !filtered_people.is_empty() {
            let _ = writeln!(message, "# People\n");
            let _ = writeln!(
                message,
                "{}",
                PromptBuilder::build_person_context(&filtered_people)
            );
            let _ = writeln!(message);
        }

        // Meal type
        let _ = writeln!(message, "# Meal Type: {}\n", ctx.meal_type);

        // Meal character
        let char_str = match ctx.character {
            MealCharacter::Balanced => "Balanced / everyday healthy",
            MealCharacter::Indulgent => "Indulgent / treat meal",
            MealCharacter::Quick => "Quick & easy (under 30 minutes)",
            MealCharacter::Custom { text } => text.as_str(),
        };
        let _ = writeln!(message, "# Meal Character: {}\n", char_str);

        // Recent meal history
        let history = PromptBuilder::build_meal_history_context(ctx.meals, ctx.recipes);
        let _ = writeln!(message, "# Recent Meal History\n");
        let _ = writeln!(message, "{}", history);
        let _ = writeln!(message);

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

    /// Parse Claude's response into a Vec of CreateRecipeDto
    pub fn parse_response(text: &str) -> Result<Vec<CreateRecipeDto>, ClaudeError> {
        let cleaned = strip_code_fences(text);

        if cleaned.trim().is_empty() {
            eprintln!(
                "AI suggestion response was empty. Raw text ({} chars): {:?}",
                text.len(),
                &text[..text.len().min(200)]
            );
            return Err(ClaudeError::ParseError(
                "Claude returned an empty response. Try again.".to_string(),
            ));
        }

        let mut suggestions: Vec<CreateRecipeDto> =
            serde_json::from_str(&cleaned).map_err(|e| {
                eprintln!(
                    "Failed to parse suggestions JSON: {}. First 500 chars: {:?}",
                    e,
                    &cleaned[..cleaned.len().min(500)]
                );
                ClaudeError::ParseError(format!("Failed to parse suggestions JSON: {}", e))
            })?;

        if suggestions.is_empty() {
            return Err(ClaudeError::ParseError(
                "No suggestions returned".to_string(),
            ));
        }

        // Force correct source and null parent on every suggestion
        for dto in &mut suggestions {
            dto.source = "ai_suggested".to_string();
            dto.parent_recipe_id = None;
        }

        Ok(suggestions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_json_suggestion() -> String {
        r#"{
            "name": "Test Stir Fry",
            "description": "A quick weeknight stir fry",
            "source": "ai_suggested",
            "parent_recipe_id": null,
            "servings": 4,
            "instructions": "1. Heat oil\n2. Stir fry everything",
            "ingredients": [
                {"name": "chicken breast", "amount": {"type": "single", "value": 1}, "unit": "lb", "notes": null}
            ],
            "tags": ["quick", "high-protein"],
            "notes": null,
            "icon": "🥘",
            "nutrition_per_serving": null,
            "prep_time": null,
            "cook_time": null,
            "total_time": null,
            "portion_size": null
        }"#
        .to_string()
    }

    #[test]
    fn parse_response_valid_array() {
        let json = format!("[{}]", sample_json_suggestion());
        let result = AiSuggestionService::parse_response(&json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Test Stir Fry");
        assert_eq!(result[0].source, "ai_suggested");
        assert!(result[0].parent_recipe_id.is_none());
    }

    #[test]
    fn parse_response_strips_code_fences() {
        let json = format!("```json\n[{}]\n```", sample_json_suggestion());
        let result = AiSuggestionService::parse_response(&json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Test Stir Fry");
    }

    #[test]
    fn parse_response_forces_source_and_parent() {
        let json = r#"[{
            "name": "X",
            "description": null,
            "source": "wrong_source",
            "parent_recipe_id": "some-id",
            "servings": 2,
            "instructions": "Do it",
            "ingredients": [],
            "tags": [],
            "notes": null,
            "icon": null,
            "nutrition_per_serving": null,
            "prep_time": null,
            "cook_time": null,
            "total_time": null,
            "portion_size": null
        }]"#;
        let result = AiSuggestionService::parse_response(json).unwrap();
        assert_eq!(result[0].source, "ai_suggested");
        assert!(result[0].parent_recipe_id.is_none());
    }

    #[test]
    fn parse_response_empty_array_errors() {
        let result = AiSuggestionService::parse_response("[]");
        assert!(result.is_err());
    }

    #[test]
    fn parse_response_invalid_json_errors() {
        let result = AiSuggestionService::parse_response("not json");
        assert!(result.is_err());
    }

    #[test]
    fn build_user_message_includes_meal_type_and_character() {
        let ctx = SuggestionContext {
            people: &[],
            person_options: &[],
            meal_type: "Dinner",
            character: &MealCharacter::Balanced,
            meals: &[],
            recipes: &[],
            feedback: None,
            previous_suggestions: None,
        };
        let message = AiSuggestionService::build_user_message(&ctx);
        assert!(message.contains("Dinner"));
        assert!(message.contains("Balanced"));
    }

    #[test]
    fn build_user_message_includes_feedback_and_previous() {
        let prev = vec!["Chicken Wrap".to_string(), "Pasta Bake".to_string()];
        let ctx = SuggestionContext {
            people: &[],
            person_options: &[],
            meal_type: "Lunch",
            character: &MealCharacter::Quick,
            meals: &[],
            recipes: &[],
            feedback: Some("More vegetarian options please"),
            previous_suggestions: Some(&prev),
        };
        let message = AiSuggestionService::build_user_message(&ctx);
        assert!(message.contains("More vegetarian options"));
        assert!(message.contains("Chicken Wrap"));
        assert!(message.contains("Pasta Bake"));
        assert!(message.contains("Previously Suggested"));
    }

    #[test]
    fn build_system_prompt_contains_schema() {
        let prompt = AiSuggestionService::build_system_prompt();
        assert!(prompt.contains("ai_suggested"));
        assert!(prompt.contains("ingredients"));
        assert!(prompt.contains("3-5"));
        assert!(prompt.contains("Return ONLY a valid JSON array"));
    }
}
