use std::fmt;

use serde::Serialize;

use crate::dto::CreateRecipeDto;
use crate::services::claude_client::{ClaudeClient, ClaudeError};
use crate::services::recipe_adapter::strip_code_fences;

/// Result of an AI import: the extracted recipe DTO plus token usage
#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub recipe: CreateRecipeDto,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Errors that can occur during recipe import
#[derive(Debug)]
pub enum ImportError {
    NetworkError(String),
    PdfError(String),
    ContentTooShort,
    AiError(ClaudeError),
    ParseError(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ImportError::PdfError(msg) => write!(f, "PDF error: {}", msg),
            ImportError::ContentTooShort => {
                write!(
                    f,
                    "Could not extract enough text. The file may be scanned or image-based."
                )
            }
            ImportError::AiError(e) => write!(f, "AI error: {}", e),
            ImportError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl From<ClaudeError> for ImportError {
    fn from(e: ClaudeError) -> Self {
        ImportError::AiError(e)
    }
}

const MAX_CONTENT_CHARS: usize = 50_000;
const MIN_CONTENT_CHARS: usize = 50;

pub struct RecipeImportService;

impl RecipeImportService {
    /// Fetch a URL and extract a recipe via AI
    pub async fn import_from_url(
        api_key: &str,
        model: &str,
        url: &str,
    ) -> Result<ImportResult, ImportError> {
        let html = Self::fetch_url(url).await?;
        let content = Self::extract_content(&html);

        if content.len() < MIN_CONTENT_CHARS {
            return Err(ImportError::ContentTooShort);
        }

        Self::extract_recipe_with_ai(api_key, model, &content, "url_import").await
    }

    /// Read a PDF file and extract a recipe via AI
    pub async fn import_from_pdf(
        api_key: &str,
        model: &str,
        file_path: &str,
    ) -> Result<ImportResult, ImportError> {
        let text = Self::read_pdf(file_path).map_err(|e| ImportError::PdfError(e.to_string()))?;

        if text.len() < MIN_CONTENT_CHARS {
            return Err(ImportError::ContentTooShort);
        }

        let truncated = truncate_content(&text, MAX_CONTENT_CHARS);
        Self::extract_recipe_with_ai(api_key, model, truncated, "pdf_import").await
    }

    /// Extract a recipe from in-memory PDF bytes via AI (used for web file uploads)
    pub async fn import_from_pdf_bytes(
        api_key: &str,
        model: &str,
        bytes: &[u8],
    ) -> Result<ImportResult, ImportError> {
        let text = pdf_extract::extract_text_from_mem(bytes).map_err(|e| {
            ImportError::PdfError(format!("Could not extract text from PDF: {}", e))
        })?;

        if text.len() < MIN_CONTENT_CHARS {
            return Err(ImportError::ContentTooShort);
        }

        let truncated = truncate_content(&text, MAX_CONTENT_CHARS);
        Self::extract_recipe_with_ai(api_key, model, truncated, "pdf_import").await
    }

    async fn fetch_url(url: &str) -> Result<String, ImportError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ImportError::NetworkError(e.to_string()))?;

        let response = client
            .get(url)
            .header("User-Agent", "fewd-meal-planner/0.1")
            .send()
            .await
            .map_err(|e| ImportError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ImportError::NetworkError(format!(
                "HTTP {}",
                response.status()
            )));
        }

        response
            .text()
            .await
            .map_err(|e| ImportError::NetworkError(e.to_string()))
    }

    /// Try JSON-LD extraction first (most token-efficient), fall back to html2text
    fn extract_content(html: &str) -> String {
        // Try to find JSON-LD recipe data — recipe sites almost always include this
        if let Some(jsonld) = extract_jsonld_recipe(html) {
            return jsonld;
        }

        // Fall back to html2text conversion
        let text = html2text::from_read(html.as_bytes(), 120).unwrap_or_default();
        truncate_content(&text, MAX_CONTENT_CHARS).to_string()
    }

    fn read_pdf(path: &str) -> Result<String, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("Could not read file: {}", e))?;
        pdf_extract::extract_text_from_mem(&bytes)
            .map_err(|e| format!("Could not extract text from PDF: {}", e))
    }

    async fn extract_recipe_with_ai(
        api_key: &str,
        model: &str,
        content: &str,
        source: &str,
    ) -> Result<ImportResult, ImportError> {
        let system_prompt = Self::build_system_prompt();
        let user_message = format!(
            "Extract the recipe from the following content and return it as JSON:\n\n{}",
            content
        );

        let response =
            ClaudeClient::send_message(api_key, model, Some(&system_prompt), &user_message).await?;

        let cleaned = strip_code_fences(&response.text);
        let mut dto: CreateRecipeDto = serde_json::from_str(&cleaned)
            .map_err(|e| ImportError::ParseError(format!("Failed to parse recipe JSON: {}", e)))?;

        dto.source = source.to_string();
        dto.parent_recipe_id = None;

        Ok(ImportResult {
            recipe: dto,
            input_tokens: response.input_tokens,
            output_tokens: response.output_tokens,
        })
    }

    fn build_system_prompt() -> String {
        r#"You are a recipe extraction assistant. You will be given content from a webpage or document that contains a recipe. Extract the recipe and return it as structured JSON.

Return ONLY valid JSON matching this exact schema (no markdown fences, no commentary, no explanation outside the JSON):

{
  "name": "string",
  "description": "string or null",
  "source": "url_import",
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
  "notes": "string or null",
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
  "portion_size": {"value": number, "unit": "string"} or null
}

Rules:
- Extract all ingredients with accurate amounts, units, and names
- Preserve the full instructions as written
- Include prep/cook/total time if mentioned
- Include nutrition info if available
- Add relevant tags (e.g., "vegetarian", "quick", "dessert")
- If the content contains multiple recipes, extract only the primary/main recipe
- If ingredient amounts are written as fractions (e.g., "1/2 cup"), convert to decimal (0.5)
- Default to 4 servings if not specified
- IMPORTANT: ingredient amount values must ALWAYS be numbers, never null. Use 0 for "to taste" or unspecified amounts
- IMPORTANT: ingredient "unit" must ALWAYS be a string, never null. Use "" (empty string) for unitless items
- If the recipe specifies a yield in countable items (e.g., "makes 36 cookies", "yields 12 muffins"), set portion_size to describe each serving (e.g., {"value": 2, "unit": "cookies"}) and set servings to the total yield divided by the portion value (e.g., 18 servings of 2 cookies). If the recipe just says "serves 4" with no countable yield, set portion_size to null"#
            .to_string()
    }
}

/// Extract JSON-LD recipe data from HTML if present
fn extract_jsonld_recipe(html: &str) -> Option<String> {
    // Look for <script type="application/ld+json"> blocks
    let mut search_from = 0;
    while let Some(start) = html[search_from..].find("<script type=\"application/ld+json\">") {
        let abs_start = search_from + start;
        let content_start = abs_start + "<script type=\"application/ld+json\">".len();

        if let Some(end) = html[content_start..].find("</script>") {
            let json_str = &html[content_start..content_start + end];

            // Check if this JSON-LD contains recipe data
            if json_str.contains("Recipe") || json_str.contains("recipe") {
                return Some(format!(
                    "JSON-LD structured recipe data:\n{}",
                    json_str.trim()
                ));
            }

            search_from = content_start + end;
        } else {
            break;
        }
    }

    None
}

fn truncate_content(content: &str, max_chars: usize) -> &str {
    if content.len() <= max_chars {
        return content;
    }
    // Find a char boundary at or before max_chars
    let mut end = max_chars;
    while !content.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    &content[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_jsonld_recipe() {
        let html = r#"<html><head>
            <script type="application/ld+json">{"@type":"Recipe","name":"Test"}</script>
            </head></html>"#;
        let result = extract_jsonld_recipe(html);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Recipe"));
    }

    #[test]
    fn test_extract_jsonld_no_recipe() {
        let html = r#"<html><head>
            <script type="application/ld+json">{"@type":"WebPage","name":"Home"}</script>
            </head></html>"#;
        let result = extract_jsonld_recipe(html);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_jsonld_missing() {
        let html = "<html><body>No JSON-LD here</body></html>";
        let result = extract_jsonld_recipe(html);
        assert!(result.is_none());
    }

    #[test]
    fn test_truncate_content() {
        let content = "Hello, world!";
        assert_eq!(truncate_content(content, 100), "Hello, world!");
        assert_eq!(truncate_content(content, 5), "Hello");
    }

    #[test]
    fn test_truncate_content_unicode() {
        let content = "Héllo";
        // é is 2 bytes (0xC3 0xA9), so byte index 2 is mid-char — backs up to 1
        let result = truncate_content(content, 2);
        assert_eq!(result, "H");
        // At 3 we get the full "Hé"
        let result = truncate_content(content, 3);
        assert_eq!(result, "Hé");
    }
}
