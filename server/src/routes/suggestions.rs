use std::convert::Infallible;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::Json;

use crate::dto::{AiSuggestMealsDto, GetSuggestionsDto};
use crate::error::AppError;
use crate::routes::sse_helpers::{sse_from_channel, SsePayload};
use crate::services::ai_suggestion_service::{AiSuggestionService, SuggestionContext};
use crate::services::claude_client::{ClaudeClient, ProgressEvent};
use crate::services::meal_service::MealService;
use crate::services::person_service::PersonService;
use crate::services::recipe_service::RecipeService;
use crate::services::settings_service::SettingsService;
use crate::services::suggestion_service::{MealSuggestions, SuggestionService};
use crate::AppState;

pub async fn get_suggestions(
    State(state): State<AppState>,
    Json(data): Json<GetSuggestionsDto>,
) -> Result<Json<MealSuggestions>, AppError> {
    let reference_date = chrono::NaiveDate::parse_from_str(&data.reference_date, "%Y-%m-%d")
        .map_err(|e| AppError::BadRequest(format!("Invalid reference_date: {}", e)))?;

    SuggestionService::get_suggestions(&state.db, data.person_ids, reference_date)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub async fn ai_suggest(
    State(state): State<AppState>,
    Json(data): Json<AiSuggestMealsDto>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, AppError> {
    // Validate inputs (runs before SSE stream starts — errors return normal HTTP responses)
    let api_key = SettingsService::get(&state.db, "anthropic_api_key".to_string())
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::BadRequest("No API key configured. Set it in Settings.".into()))?;

    if api_key.is_empty() {
        return Err(AppError::BadRequest(
            "No API key configured. Set it in Settings.".into(),
        ));
    }

    let model = SettingsService::get(&state.db, "claude_model".to_string())
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| ClaudeClient::default_model().to_string());

    let mut people = Vec::new();
    for opt in &data.person_options {
        let person = PersonService::get_by_id(&state.db, opt.person_id.clone())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::NotFound(format!("Person {} not found", opt.person_id)))?;
        people.push(person);
    }

    let today = chrono::Utc::now().date_naive();
    let start = today - chrono::Duration::days(30);
    let meals = MealService::get_all_for_date_range(
        &state.db,
        start.format("%Y-%m-%d").to_string(),
        today.format("%Y-%m-%d").to_string(),
    )
    .await
    .map_err(AppError::from)?;

    let recipes = RecipeService::get_all(&state.db)
        .await
        .map_err(AppError::from)?;

    // Build prompts BEFORE spawning (avoids lifetime issues with SuggestionContext<'a>)
    let ctx = SuggestionContext {
        people: &people,
        person_options: &data.person_options,
        meal_type: &data.meal_type,
        character: &data.character,
        meals: &meals,
        recipes: &recipes,
        feedback: data.feedback.as_deref(),
        previous_suggestions: data
            .previous_suggestion_names
            .as_deref()
            .map(|v| v as &[String]),
    };
    let system_prompt = AiSuggestionService::build_system_prompt();
    let user_message = AiSuggestionService::build_user_message(&ctx);

    // Set up SSE channels
    let (sse_tx, sse_rx) = tokio::sync::mpsc::channel::<SsePayload>(32);
    let (progress_tx, progress_rx) = tokio::sync::mpsc::channel::<ProgressEvent>(32);

    let db = state.db.clone();
    let sse_tx_progress = sse_tx.clone();

    // Forward progress events from Claude client to SSE stream
    tokio::spawn(super::sse_helpers::forward_progress(
        progress_rx,
        sse_tx_progress,
    ));

    // Spawn the AI task with only owned data
    tokio::spawn(async move {
        // Send initial thinking event
        let _ = sse_tx
            .send(SsePayload::Progress(ProgressEvent::Thinking {
                message: "Thinking about meal ideas...".to_string(),
            }))
            .await;

        match ClaudeClient::send_message_streaming(
            &api_key,
            &model,
            Some(&system_prompt),
            &user_message,
            AiSuggestionService::SUGGESTION_MAX_TOKENS,
            progress_tx,
        )
        .await
        {
            Ok(response) => {
                SettingsService::increment_token_usage(
                    &db,
                    response.input_tokens,
                    response.output_tokens,
                )
                .await;

                match AiSuggestionService::parse_response(&response.text) {
                    Ok(suggestions) => {
                        let value = serde_json::to_value(&suggestions).unwrap_or_default();
                        let _ = sse_tx.send(SsePayload::Complete(value)).await;
                    }
                    Err(e) => {
                        tracing::error!("AI suggestion parse failed: {}", e);
                        let _ = sse_tx
                            .send(SsePayload::Error(format!("AI suggestion failed: {}", e)))
                            .await;
                    }
                }
            }
            Err(e) => {
                tracing::error!("AI suggestion API call failed: {}", e);
                let _ = sse_tx
                    .send(SsePayload::Error(format!("AI suggestion failed: {}", e)))
                    .await;
            }
        }
    });

    Ok(sse_from_channel(sse_rx))
}
