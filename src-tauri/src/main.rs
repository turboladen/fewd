#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use fewd_lib::{db, AppState};
use tauri::Manager;

#[tokio::main]
async fn main() {
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
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
