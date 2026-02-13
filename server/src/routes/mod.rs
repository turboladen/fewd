mod meal_templates;
mod meals;
mod people;
mod recipes;
mod settings;
mod shopping;
mod suggestions;

use axum::routing::{get, post, put};
use axum::Router;

use crate::AppState;

pub fn api_routes() -> Router<AppState> {
    Router::new()
        // People
        .route("/people", get(people::list).post(people::create))
        .route(
            "/people/{id}",
            get(people::get_one)
                .put(people::update)
                .delete(people::remove),
        )
        // Recipes
        .route("/recipes", get(recipes::list).post(recipes::create))
        .route("/recipes/search", get(recipes::search))
        .route("/recipes/import/markdown", post(recipes::import_markdown))
        .route("/recipes/import/url", post(recipes::import_url))
        .route("/recipes/import/file", post(recipes::import_file))
        .route(
            "/recipes/{id}",
            get(recipes::get_one)
                .put(recipes::update)
                .delete(recipes::remove),
        )
        .route("/recipes/{id}/favorite", post(recipes::toggle_favorite))
        .route("/recipes/{id}/scale", post(recipes::preview_scale))
        .route("/recipes/{id}/enhance", post(recipes::enhance))
        .route("/recipes/{id}/adapt", post(recipes::adapt))
        // Meals
        .route("/meals", get(meals::list).post(meals::create))
        .route(
            "/meals/{id}",
            get(meals::get_one).put(meals::update).delete(meals::remove),
        )
        // Meal Templates
        .route(
            "/meal-templates",
            get(meal_templates::list).post(meal_templates::create),
        )
        .route(
            "/meal-templates/from-meal",
            post(meal_templates::create_from_meal),
        )
        .route(
            "/meal-templates/{id}",
            put(meal_templates::update).delete(meal_templates::remove),
        )
        // Shopping
        .route("/shopping-list", get(shopping::get_shopping_list))
        // Suggestions
        .route("/suggestions", post(suggestions::get_suggestions))
        .route("/suggestions/ai", post(suggestions::ai_suggest))
        // Settings
        .route("/settings/models", get(settings::available_models))
        .route("/settings/test-connection", post(settings::test_connection))
        .route("/settings/token-usage", get(settings::token_usage))
        .route(
            "/settings/{key}",
            get(settings::get_setting).put(settings::set_setting),
        )
}
