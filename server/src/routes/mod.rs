mod cocktails;
mod meal_templates;
mod meals;
mod people;
mod recipes;
mod settings;
mod shopping;
mod suggestions;

use axum::routing::{delete, get, post, put};
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
        // Bar Items
        .route(
            "/bar-items",
            get(cocktails::list_bar_items).post(cocktails::create_bar_item),
        )
        .route("/bar-items/bulk", post(cocktails::bulk_create_bar_items))
        .route("/bar-items/all", delete(cocktails::delete_all_bar_items))
        .route("/bar-items/{id}", delete(cocktails::delete_bar_item))
        // Drink Recipes
        .route(
            "/drink-recipes",
            get(cocktails::list_drink_recipes).post(cocktails::create_drink_recipe),
        )
        .route(
            "/drink-recipes/{id}",
            get(cocktails::get_drink_recipe)
                .put(cocktails::update_drink_recipe)
                .delete(cocktails::delete_drink_recipe),
        )
        .route(
            "/drink-recipes/{id}/favorite",
            post(cocktails::toggle_drink_favorite),
        )
        .route(
            "/drink-recipes/import/url",
            post(cocktails::import_drink_recipe_url),
        )
        // Cocktail AI Suggestions
        .route("/cocktails/suggest", post(cocktails::ai_suggest_cocktails))
        // Settings
        .route("/settings/models", get(settings::available_models))
        .route("/settings/test-connection", post(settings::test_connection))
        .route("/settings/token-usage", get(settings::token_usage))
        .route(
            "/settings/{key}",
            get(settings::get_setting).put(settings::set_setting),
        )
}
