use serde::{Deserialize, Serialize};
use tauri::State;

use crate::entities::person;
use crate::services::person_service::PersonService;
use crate::AppState;

#[derive(Debug, Deserialize, Serialize)]
pub struct CreatePersonDto {
    pub name: String,
    pub birthdate: String,
    pub dietary_goals: Option<String>,
    pub dislikes: Vec<String>,
    pub favorites: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdatePersonDto {
    pub name: Option<String>,
    pub birthdate: Option<String>,
    pub dietary_goals: Option<String>,
    pub dislikes: Option<Vec<String>>,
    pub favorites: Option<Vec<String>>,
    pub notes: Option<String>,
    pub is_active: Option<bool>,
}

#[tauri::command]
pub async fn get_all_people(state: State<'_, AppState>) -> Result<Vec<person::Model>, String> {
    PersonService::get_all(&state.db).await.map_err(|e| {
        eprintln!("Failed to get all people: {}", e);
        format!("Could not get people: {}", e)
    })
}

#[tauri::command]
pub async fn get_person(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<person::Model>, String> {
    PersonService::get_by_id(&state.db, id).await.map_err(|e| {
        eprintln!("Failed to get person: {}", e);
        format!("Could not get person: {}", e)
    })
}

#[tauri::command]
pub async fn create_person(
    state: State<'_, AppState>,
    data: CreatePersonDto,
) -> Result<person::Model, String> {
    PersonService::create(&state.db, data).await.map_err(|e| {
        eprintln!("Failed to create person: {}", e);
        format!("Could not create person: {}", e)
    })
}

#[tauri::command]
pub async fn update_person(
    state: State<'_, AppState>,
    id: String,
    data: UpdatePersonDto,
) -> Result<person::Model, String> {
    PersonService::update(&state.db, id, data)
        .await
        .map_err(|e| {
            eprintln!("Failed to update person: {}", e);
            format!("Could not update person: {}", e)
        })
}

#[tauri::command]
pub async fn delete_person(state: State<'_, AppState>, id: String) -> Result<(), String> {
    PersonService::delete(&state.db, id).await.map_err(|e| {
        eprintln!("Failed to delete person: {}", e);
        format!("Could not delete person: {}", e)
    })
}
