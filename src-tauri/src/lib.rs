mod appx;
mod catalog;
mod disk;
mod engine;
mod registry;
mod system;
mod user;
mod winget;

use catalog::Category;
use engine::{ApplyReport, Desired, Engine, JournalEntry, PlannedChange, TweakView};
use serde::Serialize;
use tauri::{Emitter, Manager};

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
    Ok(state.apply(&changes, info.elevated, info.build, info.foreign_user.as_deref()))
}

#[tauri::command]
async fn history(state: tauri::State<'_, Engine>) -> Result<Vec<JournalEntry>, String> {
    Ok(state.history())
}

#[tauri::command]
async fn undo(state: tauri::State<'_, Engine>, entry_id: u64, force: Option<bool>) -> Result<ApplyReport, String> {
    state.undo(entry_id, force.unwrap_or(false), system::foreign_user().as_deref())
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
    if let Some(user) = system::foreign_user() {
        return Err(format!(
            "La app corre con otra cuenta de administrador: se quitarían las apps de esa cuenta y no las de {user}"
        ));
    }
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

#[tauri::command]
async fn winget_status() -> Result<winget::AppsOverview, String> {
    tauri::async_runtime::spawn_blocking(winget::status)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Clone, Serialize)]
struct WingetProgress {
    id: String,
    line: String,
}

#[tauri::command]
async fn winget_action(app: tauri::AppHandle, id: String, action: winget::Action) -> Result<winget::ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        winget::act(&id, action, |line| {
            let _ = app.emit("winget-progress", WingetProgress { id: id.clone(), line: line.to_string() });
        })
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_user() -> user::UserInfo {
    tauri::async_runtime::spawn_blocking(user::info).await.unwrap_or_else(|_| user::UserInfo {
        name: String::new(),
        username: String::new(),
        picture: None,
    })
}

// ───────── Disco (solo lectura) ─────────

#[tauri::command]
async fn disk_drives() -> Vec<disk::Drive> {
    tauri::async_runtime::spawn_blocking(disk::drives).await.unwrap_or_default()
}

#[tauri::command]
fn disk_scan(letter: String) -> Result<(), String> {
    disk::start(&letter)
}

#[tauri::command]
fn disk_status() -> disk::ScanStatus {
    disk::status()
}

#[tauri::command]
fn disk_cancel() {
    disk::cancel()
}

#[tauri::command]
fn disk_summary() -> Result<disk::Summary, String> {
    disk::summary()
}

#[tauri::command]
fn disk_dir(id: u32) -> Result<disk::DirView, String> {
    disk::dir(id)
}

#[tauri::command]
fn disk_open(id: u32) -> Result<(), String> {
    disk::open(id)
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
            remove_bloat,
            winget_status,
            winget_action,
            get_user,
            disk_drives,
            disk_scan,
            disk_status,
            disk_cancel,
            disk_summary,
            disk_dir,
            disk_open
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación");
}
