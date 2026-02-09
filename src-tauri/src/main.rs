#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use fewd_lib::commands::{meal, meal_template, person, recipe, settings, shopping, suggestion};
use fewd_lib::{db, AppState};
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let db = tauri::async_runtime::block_on(async {
                db::init(app.handle())
                    .await
                    .expect("Failed to initialize database")
            });

            app.manage(AppState { db });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            person::get_all_people,
            person::get_person,
            person::create_person,
            person::update_person,
            person::delete_person,
            recipe::get_all_recipes,
            recipe::get_recipe,
            recipe::create_recipe,
            recipe::update_recipe,
            recipe::delete_recipe,
            recipe::search_recipes,
            recipe::toggle_favorite_recipe,
            recipe::import_recipe_from_markdown,
            recipe::preview_scale_recipe,
            recipe::enhance_recipe_instructions,
            meal::get_meals_for_date_range,
            meal::get_meal,
            meal::create_meal,
            meal::update_meal,
            meal::delete_meal,
            shopping::get_shopping_list,
            meal_template::get_all_meal_templates,
            meal_template::create_meal_template,
            meal_template::update_meal_template,
            meal_template::delete_meal_template,
            meal_template::create_template_from_meal,
            suggestion::get_meal_suggestions,
            settings::get_setting,
            settings::set_setting,
            settings::get_available_models,
            settings::test_claude_connection,
            settings::get_token_usage,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
