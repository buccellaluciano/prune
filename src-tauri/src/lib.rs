mod appx;
mod catalog;
mod engine;
mod registry;
mod system;

use catalog::Category;
use engine::{ApplyReport, Desired, Engine, JournalEntry, PlannedChange, TweakView};
use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
struct Overview {
    system: system::SystemInfo,
    categories: Vec<Category>,
    tweaks: Vec<TweakView>,
}

#[tauri::command]
async fn get_overview() -> Overview {
    let system = system::info();
    let tweaks = engine::list(system.build);
    Overview { categories: catalog::tweaks().category.clone(), tweaks, system }
}

#[tauri::command]
async fn plan_changes(changes: Vec<Desired>) -> Result<Vec<PlannedChange>, String> {
    engine::plan(&changes, system::info().build)
}

#[tauri::command]
async fn apply_changes(state: tauri::State<'_, Engine>, changes: Vec<Desired>) -> Result<ApplyReport, String> {
    let info = system::info();
    Ok(state.apply(&changes, info.elevated, info.build))
}

#[tauri::command]
async fn history(state: tauri::State<'_, Engine>) -> Result<Vec<JournalEntry>, String> {
    Ok(state.history())
}

#[tauri::command]
async fn undo(state: tauri::State<'_, Engine>, entry_id: u64) -> Result<ApplyReport, String> {
    state.undo(entry_id)
}

#[tauri::command]
async fn list_bloat() -> Result<Vec<appx::BloatView>, String> {
    tauri::async_runtime::spawn_blocking(appx::list)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn remove_bloat(
    state: tauri::State<'_, Engine>,
    ids: Vec<String>,
    deprovision: bool,
) -> Result<Vec<appx::RemoveResult>, String> {
    let deprovision = deprovision && system::is_elevated();
    let results = tauri::async_runtime::spawn_blocking(move || appx::remove(&ids, deprovision))
        .await
        .map_err(|e| e.to_string())??;
    let removed: Vec<String> = results.iter().flat_map(|r| r.removed.clone()).collect();
    if !removed.is_empty() {
        let _ = state.record_removed_apps(format!("Quitadas {} apps", removed.len()), removed);
    }
    Ok(results)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            app.manage(Engine::new(dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_overview,
            plan_changes,
            apply_changes,
            history,
            undo,
            list_bloat,
            remove_bloat
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación");
}
