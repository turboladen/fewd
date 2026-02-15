use std::convert::Infallible;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::Json;

use crate::dto::{
    AiSuggestCocktailsDto, BulkBarItemsDto, CreateBarItemDto, CreateDrinkRecipeDto,
    ImportRecipeFromUrlDto, UpdateDrinkRecipeDto,
};
use crate::error::AppError;
use crate::routes::sse_helpers::{sse_from_channel, SsePayload};
use crate::services::bar_item_service::BarItemService;
use crate::services::claude_client::{ClaudeClient, ProgressEvent};
use crate::services::cocktail_suggestion_service::{CocktailContext, CocktailSuggestionService};
use crate::services::drink_recipe_import_service::DrinkRecipeImportService;
use crate::services::drink_recipe_service::DrinkRecipeService;
use crate::services::person_service::PersonService;
use crate::services::recipe_import_service::RecipeImportService;
use crate::services::settings_service::SettingsService;
use crate::AppState;

// ─── Bar Items ─────────────────────────────────────────────────

pub async fn list_bar_items(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::entities::bar_item::Model>>, AppError> {
    BarItemService::get_all(&state.db)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub async fn create_bar_item(
    State(state): State<AppState>,
    Json(data): Json<CreateBarItemDto>,
) -> Result<(StatusCode, Json<crate::entities::bar_item::Model>), AppError> {
    BarItemService::create(&state.db, data)
        .await
        .map(|item| (StatusCode::CREATED, Json(item)))
        .map_err(AppError::from)
}

pub async fn bulk_create_bar_items(
    State(state): State<AppState>,
    Json(data): Json<BulkBarItemsDto>,
) -> Result<(StatusCode, Json<Vec<crate::entities::bar_item::Model>>), AppError> {
    BarItemService::bulk_create(&state.db, data)
        .await
        .map(|items| (StatusCode::CREATED, Json(items)))
        .map_err(AppError::from)
}

pub async fn delete_bar_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    BarItemService::delete(&state.db, id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}

pub async fn delete_all_bar_items(State(state): State<AppState>) -> Result<StatusCode, AppError> {
    BarItemService::delete_all(&state.db)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}

// ─── Drink Recipes ─────────────────────────────────────────────

pub async fn list_drink_recipes(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::entities::drink_recipe::Model>>, AppError> {
    DrinkRecipeService::get_all(&state.db)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub async fn get_drink_recipe(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::entities::drink_recipe::Model>, AppError> {
    DrinkRecipeService::get_by_id(&state.db, id)
        .await
        .map_err(AppError::from)?
        .map(Json)
        .ok_or_else(|| AppError::NotFound("Drink recipe not found".into()))
}

pub async fn create_drink_recipe(
    State(state): State<AppState>,
    Json(data): Json<CreateDrinkRecipeDto>,
) -> Result<(StatusCode, Json<crate::entities::drink_recipe::Model>), AppError> {
    DrinkRecipeService::create(&state.db, data)
        .await
        .map(|recipe| (StatusCode::CREATED, Json(recipe)))
        .map_err(AppError::from)
}

pub async fn update_drink_recipe(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(data): Json<UpdateDrinkRecipeDto>,
) -> Result<Json<crate::entities::drink_recipe::Model>, AppError> {
    DrinkRecipeService::update(&state.db, id, data)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub async fn delete_drink_recipe(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    DrinkRecipeService::delete(&state.db, id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}

pub async fn toggle_drink_favorite(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::entities::drink_recipe::Model>, AppError> {
    DrinkRecipeService::toggle_favorite(&state.db, id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

// ─── Drink Recipe Import ──────────────────────────────────────

pub async fn import_drink_recipe_url(
    State(state): State<AppState>,
    Json(data): Json<ImportRecipeFromUrlDto>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, AppError> {
    // Validate inputs before SSE stream starts
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

    // Set up SSE channels
    let (sse_tx, sse_rx) = tokio::sync::mpsc::channel::<SsePayload>(32);
    let (progress_tx, progress_rx) = tokio::sync::mpsc::channel::<ProgressEvent>(32);

    let db = state.db.clone();
    let url = data.url.clone();
    let sse_tx_progress = sse_tx.clone();

    tokio::spawn(super::sse_helpers::forward_progress(
        progress_rx,
        sse_tx_progress,
    ));

    // Spawn the import task — URL fetch + AI extraction + DB save
    tokio::spawn(async move {
        // Phase 1: Fetch the page
        let _ = sse_tx
            .send(SsePayload::Progress(ProgressEvent::Thinking {
                message: "Fetching page...".to_string(),
            }))
            .await;

        let html = match RecipeImportService::fetch_url(&url).await {
            Ok(html) => html,
            Err(e) => {
                tracing::error!("URL fetch failed for {}: {}", url, e);
                let _ = sse_tx
                    .send(SsePayload::Error(format!("Import failed: {}", e)))
                    .await;
                return;
            }
        };

        let content = RecipeImportService::extract_content(&html);
        if content.len() < crate::services::recipe_import_service::MIN_CONTENT_CHARS {
            tracing::warn!("Insufficient content extracted from URL: {}", url);
            let _ = sse_tx
                .send(SsePayload::Error(
                    "Could not extract enough text from the page.".to_string(),
                ))
                .await;
            return;
        }

        // Phase 2: Extract drink recipe with AI (streaming)
        let _ = sse_tx
            .send(SsePayload::Progress(ProgressEvent::Thinking {
                message: "Extracting drink recipe...".to_string(),
            }))
            .await;

        let system_prompt = DrinkRecipeImportService::build_system_prompt();
        let user_message = format!(
            "Extract the drink recipe from the following content and return it as JSON:\n\n{}",
            content
        );

        match ClaudeClient::send_message_streaming(
            &api_key,
            &model,
            Some(&system_prompt),
            &user_message,
            crate::services::claude_client::DEFAULT_MAX_TOKENS,
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

                let cleaned = crate::services::recipe_adapter::strip_code_fences(&response.text);
                match serde_json::from_str::<CreateDrinkRecipeDto>(&cleaned) {
                    Ok(mut dto) => {
                        dto.source = "url_import".to_string();
                        dto.source_url = Some(url);

                        match DrinkRecipeService::create(&db, dto).await {
                            Ok(recipe) => {
                                let value = serde_json::to_value(&recipe).unwrap_or_default();
                                let _ = sse_tx.send(SsePayload::Complete(value)).await;
                            }
                            Err(e) => {
                                tracing::error!("Failed to save imported drink recipe: {}", e);
                                let _ = sse_tx
                                    .send(SsePayload::Error(format!(
                                        "Failed to save recipe: {}",
                                        e
                                    )))
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Drink recipe import AI response unparseable: {}", e);
                        let _ = sse_tx
                            .send(SsePayload::Error(format!(
                                "AI returned an unparseable response: {}",
                                e
                            )))
                            .await;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Drink recipe import API call failed: {}", e);
                let _ = sse_tx
                    .send(SsePayload::Error(format!("Import failed: {}", e)))
                    .await;
            }
        }
    });

    Ok(sse_from_channel(sse_rx))
}

// ─── AI Cocktail Suggestions ───────────────────────────────────

pub async fn ai_suggest_cocktails(
    State(state): State<AppState>,
    Json(data): Json<AiSuggestCocktailsDto>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, AppError> {
    // Validate inputs before SSE stream starts
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

    // Fetch selected people
    let mut people = Vec::new();
    for person_id in &data.person_ids {
        let person = PersonService::get_by_id(&state.db, person_id.clone())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::NotFound(format!("Person {} not found", person_id)))?;
        people.push(person);
    }

    // Fetch selected bar items
    let all_bar_items = BarItemService::get_all(&state.db)
        .await
        .map_err(AppError::from)?;
    let selected_bar_items: Vec<_> = all_bar_items
        .into_iter()
        .filter(|item| data.bar_item_ids.contains(&item.id))
        .collect();

    // Fetch existing drink recipes for context
    let drink_recipes = DrinkRecipeService::get_all(&state.db)
        .await
        .map_err(AppError::from)?;

    // Build prompts BEFORE spawning (avoids lifetime issues with CocktailContext<'a>)
    let ctx = CocktailContext {
        people: &people,
        bar_items: &selected_bar_items,
        mood: &data.mood,
        include_non_alcoholic: data.include_non_alcoholic,
        drink_recipes: &drink_recipes,
        feedback: data.feedback.as_deref(),
        previous_suggestions: data
            .previous_suggestion_names
            .as_deref()
            .map(|v| v as &[String]),
    };
    let system_prompt = CocktailSuggestionService::build_system_prompt();
    let user_message = CocktailSuggestionService::build_user_message(&ctx);

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
        let _ = sse_tx
            .send(SsePayload::Progress(ProgressEvent::Thinking {
                message: "Thinking about cocktails...".to_string(),
            }))
            .await;

        match ClaudeClient::send_message_streaming(
            &api_key,
            &model,
            Some(&system_prompt),
            &user_message,
            CocktailSuggestionService::SUGGESTION_MAX_TOKENS,
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

                match CocktailSuggestionService::parse_response(&response.text) {
                    Ok(suggestions) => {
                        let value = serde_json::to_value(&suggestions).unwrap_or_default();
                        let _ = sse_tx.send(SsePayload::Complete(value)).await;
                    }
                    Err(e) => {
                        tracing::error!("Cocktail suggestion parse failed: {}", e);
                        let _ = sse_tx
                            .send(SsePayload::Error(format!(
                                "Cocktail suggestion failed: {}",
                                e
                            )))
                            .await;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Cocktail suggestion API call failed: {}", e);
                let _ = sse_tx
                    .send(SsePayload::Error(format!(
                        "Cocktail suggestion failed: {}",
                        e
                    )))
                    .await;
            }
        }
    });

    Ok(sse_from_channel(sse_rx))
}
