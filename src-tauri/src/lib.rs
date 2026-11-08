/// Mensaje traducido: `t!("clave")` o `t!("clave", nombre = valor, ...)` (ver `i18n`).
macro_rules! t {
    ($key:literal) => {
        $crate::i18n::msg($key)
    };
    ($key:literal, $($name:ident = $val:expr),+ $(,)?) => {
        $crate::i18n::fmt($key, &[$((stringify!($name), &$val as &dyn ::std::fmt::Display)),+])
    };
}

mod appx;
mod catalog;
mod cleanup;
mod disk;
mod engine;
mod http;
mod i18n;
mod icons;
mod power;
mod registry;
mod sign;
mod system;
mod updater;
mod verify;
mod user;
mod vt;
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

/// Versión instalada (la de Cargo.toml, que es la de tauri.conf.json).
#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Idioma de los mensajes y del catálogo (uno de `i18n::LANGS`).
#[tauri::command]
fn set_language(lang: String) -> Result<(), String> {
    i18n::set(&lang)
}

#[tauri::command]
async fn get_overview() -> Overview {
    let system = system::info();
    let tweaks = engine::list(system.build);
    let categories = catalog::tweaks()
        .category
        .iter()
        .map(|c| Category { title: i18n::category(&c.id, &c.title), ..c.clone() })
        .collect();
    Overview { categories, tweaks, system }
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
        return Err(t!("foreign_user_remove", user = user));
    }
    let deprovision = deprovision && system::is_elevated();
    let results = tauri::async_runtime::spawn_blocking(move || appx::remove(&ids, deprovision))
        .await
        .map_err(|e| e.to_string())??;
    let removed: Vec<String> = results.iter().flat_map(|r| r.removed.clone()).collect();
    if !removed.is_empty() {
        let _ = state.record_removed_apps(t!("label_removed_apps", n = removed.len()), removed);
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
    phase: Option<winget::Phase>,
    fraction: Option<f32>,
}

#[tauri::command]
async fn winget_action(app: tauri::AppHandle, id: String, action: winget::Action) -> Result<winget::ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        winget::act(&id, action, |p| {
            let _ = app.emit(
                "winget-progress",
                WingetProgress { id: id.clone(), line: p.line, phase: p.phase, fraction: p.fraction },
            );
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

// ───────── Limpieza de archivos descartables ─────────

#[tauri::command]
async fn cleanup_scan() -> Vec<cleanup::CleanView> {
    let info = system::info();
    let foreign = info.foreign_user.is_some();
    tauri::async_runtime::spawn_blocking(move || cleanup::scan(info.elevated, foreign)).await.unwrap_or_default()
}

/// Borra las categorías elegidas. Los IDs se validan contra la lista fija del backend.
#[tauri::command]
async fn cleanup_run(ids: Vec<String>) -> Result<Vec<cleanup::CleanResult>, String> {
    let info = system::info();
    let foreign = info.foreign_user.is_some();
    tauri::async_runtime::spawn_blocking(move || cleanup::run(&ids, info.elevated, foreign))
        .await
        .map_err(|e| e.to_string())
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

// ───────── Íconos y VirusTotal (Programas) ─────────

/// Íconos del catálogo. Con `fetch` busca en internet los que falten.
#[tauri::command]
async fn winget_icons(app: tauri::AppHandle, fetch: bool) -> std::collections::HashMap<String, String> {
    let Ok(dir) = app.path().app_data_dir() else { return Default::default() };
    tauri::async_runtime::spawn_blocking(move || icons::all(&dir.join("icons"), fetch)).await.unwrap_or_default()
}

#[derive(Serialize)]
struct SecurityCheck {
    installer: Option<winget::InstallerInfo>,
    error: Option<String>,
}

/// Datos del instalador según el manifiesto de winget (URL y SHA-256), para que la
/// persona lo revise antes de instalar.
#[tauri::command]
async fn winget_check(id: String) -> SecurityCheck {
    tauri::async_runtime::spawn_blocking(move || match winget::installer_info(&id) {
        Ok(i) => SecurityCheck { installer: Some(i), error: None },
        Err(e) => SecurityCheck { installer: None, error: Some(e) },
    })
    .await
    .unwrap_or_else(|e| SecurityCheck { installer: None, error: Some(e.to_string()) })
}

/// Abre en el navegador el informe de VirusTotal de un hash.
#[tauri::command]
fn vt_open(sha256: String) -> Result<(), String> {
    vt::open_report(&sha256)
}

/// Sonido al activar el tema Aero 7: el `tada.wav` clásico que trae el propio Windows
/// (`%windir%\Media`). Ruta fija, sin parámetros; no se incluye ningún sonido en la app.
/// Asíncrono: no frena la interfaz. Si el archivo no está, no suena nada.
#[tauri::command]
fn play_theme_sound() {
    use windows::core::PCWSTR;
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};
    use windows::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;
    let mut buf = [0u16; 260];
    let n = unsafe { GetSystemWindowsDirectoryW(Some(&mut buf)) } as usize;
    if n == 0 || n >= buf.len() {
        return;
    }
    let path = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n])).join(r"Media\tada.wav");
    if !path.is_file() {
        return;
    }
    let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    let _ = unsafe { PlaySoundW(PCWSTR(wide.as_ptr()), None, SND_FILENAME | SND_ASYNC | SND_NODEFAULT) };
}

/// Dónde guardar el historial.
///
/// Instalada en Program Files y corriendo como admin, va en `<instalación>\data`: ahí solo
/// escriben los administradores, así que ningún programa del usuario puede manipularlo.
/// La primera vez se trae el historial que hubiera en `%APPDATA%` (igual se valida contra el
/// catálogo antes de deshacer). En desarrollo, sin admin, queda en `%APPDATA%`.
fn journal_dir(user_dir: &std::path::Path) -> std::path::PathBuf {
    let protected = (|| {
        if !system::is_elevated() {
            return None;
        }
        let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        let pf = winget::program_files()?.to_string_lossy().to_ascii_lowercase();
        if !exe_dir.to_string_lossy().to_ascii_lowercase().starts_with(&format!("{pf}\\")) {
            return None;
        }
        let dir = exe_dir.join("data");
        std::fs::create_dir_all(&dir).ok()?;
        Some(dir)
    })();
    let Some(dir) = protected else { return user_dir.to_path_buf() };
    let new = dir.join("journal.json");
    // La 0.1.0 se llamaba "Prune" y se instalaba en Program Files\Prune: su historial
    // (solo escribible por admins, igual que este) se trae a la carpeta nueva.
    let legacy = winget::program_files().map(|pf| pf.join("Prune").join("data").join("journal.json"));
    if !new.exists() {
        if let Some(legacy) = legacy.filter(|p| p.exists() && *p != new) {
            if std::fs::copy(&legacy, &new).is_ok() {
                let _ = std::fs::rename(&legacy, legacy.with_file_name("journal.migrado.json"));
            }
        }
    }
    let old = user_dir.join("journal.json");
    if !new.exists() && old.exists() && std::fs::copy(&old, &new).is_ok() {
        let _ = std::fs::rename(&old, user_dir.join("journal.migrado.json"));
    }
    dir
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::Pending::default())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            app.manage(Engine::new(journal_dir(&dir)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_language,
            app_version,
            updater::update_check,
            updater::update_install,
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
            disk_open,
            cleanup_scan,
            cleanup_run,
            winget_icons,
            winget_check,
            vt_open,
            play_theme_sound
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación");
}
