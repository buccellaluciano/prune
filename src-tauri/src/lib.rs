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
mod com;
mod disk;
mod engine;
mod http;
mod i18n;
mod icons;
mod network;
mod power;
mod profile;
mod proxy;
mod registry;
mod restore;
mod sign;
mod startup;
mod system;
mod updater;
mod verify;
mod user;
mod vt;
mod winget;
mod wmo;
mod wupdate;

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
    // Deshacer a mano un cambio de DNS sin confirmar también cancela la vuelta atrás automática.
    network::take(entry_id);
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

// ───────── Red (DNS) ─────────

#[tauri::command]
async fn network_status() -> Result<network::NetworkStatus, String> {
    tauri::async_runtime::spawn_blocking(network::status).await.map_err(|e| e.to_string())
}

/// Prueba cada servidor pedido antes de aplicar (no cambia nada).
#[tauri::command]
async fn dns_test(adapters: Vec<network::AdapterDns>) -> Result<Vec<network::TestResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let target = network::validate_request(&adapters, &network::adapters())?;
        Ok(network::test(&target))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
struct DnsApplied {
    entry_id: u64,
    seconds: u64,
}

/// Aplica, guarda en el historial y arma la vuelta atrás: si en `CONFIRM_SECS` no llega
/// `dns_confirm`, se restaura solo (aunque la interfaz no responda).
#[tauri::command]
async fn dns_apply(app: tauri::AppHandle, adapters: Vec<network::AdapterDns>) -> Result<DnsApplied, String> {
    let entry_id = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || -> Result<u64, String> {
            // Dos pedidos a la vez no pueden pasar los dos el control de "hay uno pendiente".
            let _lock = network::APPLY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            network::guard()?;
            let target = network::validate_request(&adapters, &network::adapters())?;
            let change = network::apply(&target)?;
            let engine = app.state::<Engine>();
            match engine.record_network(change.clone()) {
                Ok(id) => {
                    network::arm(id);
                    Ok(id)
                }
                Err(e) => {
                    // Sin historial no hay cómo deshacer: mejor no dejar el cambio.
                    let _ = network::restore(&change.before);
                    Err(t!("journal_save_failed", error = e))
                }
            }
        }
    })
    .await
    .map_err(|e| e.to_string())??;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(network::CONFIRM_SECS));
        if network::take(entry_id) {
            // `undo` devuelve Ok aunque restaurar haya fallado (va en `results`): la interfaz
            // tiene que enterarse de la falla, no mostrar "volvió a como estaba".
            let r = app.state::<Engine>().undo(entry_id, true, None).and_then(|rep| {
                match rep.results.iter().find(|x| !x.ok) {
                    Some(f) => Err(f.error.clone().unwrap_or_default()),
                    None => Ok(entry_id),
                }
            });
            let _ = app.emit("dns-reverted", r);
        }
    });
    Ok(DnsApplied { entry_id, seconds: network::CONFIRM_SECS })
}

/// La persona confirmó que internet anda: el cambio queda.
#[tauri::command]
fn dns_confirm(entry_id: u64) -> bool {
    network::take(entry_id)
}

/// "Volver" en el diálogo de confirmación.
#[tauri::command]
async fn dns_revert(state: tauri::State<'_, Engine>, entry_id: u64) -> Result<ApplyReport, String> {
    // Solo el cambio de DNS que está esperando confirmación. Sin esto, este comando forzaría
    // el deshacer de cualquier entrada del historial salteando el aviso de conflictos. Si ya
    // no está pendiente es porque la vuelta atrás automática ganó: no hay nada más que hacer.
    if !network::take(entry_id) {
        return Ok(ApplyReport::default_for(entry_id));
    }
    state.undo(entry_id, true, None)
}

// ───────── Red (proxy del usuario) ─────────

#[tauri::command]
async fn proxy_status() -> Result<proxy::ProxyStatus, String> {
    tauri::async_runtime::spawn_blocking(proxy::status).await.map_err(|e| e.to_string())?
}

/// Prende o apaga un interruptor del proxy ya configurado. Devuelve la entrada del historial
/// (None si ya estaba así).
#[tauri::command]
async fn proxy_set(app: tauri::AppHandle, kind: proxy::Kind, enabled: bool) -> Result<Option<u64>, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<u64>, String> {
        let Some(change) = proxy::set(kind, enabled)? else { return Ok(None) };
        match app.state::<Engine>().record_proxy(change.clone()) {
            Ok(id) => Ok(Some(id)),
            Err(e) => {
                let _ = proxy::restore(&change);
                Err(t!("journal_save_failed", error = e))
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

// ───────── Inicio de Windows ─────────

#[tauri::command]
async fn startup_status() -> startup::StartupStatus {
    tauri::async_runtime::spawn_blocking(startup::status).await.unwrap_or_else(|_| startup::StartupStatus {
        items: vec![],
        elevated: system::is_elevated(),
        foreign_user: None,
    })
}

/// Habilita o deshabilita algo que ya arranca con Windows. Devuelve la entrada del historial
/// (None si ya estaba así).
#[tauri::command]
async fn startup_set(app: tauri::AppHandle, source: startup::Source, name: String, enabled: bool) -> Result<Option<u64>, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<u64>, String> {
        let Some(change) = startup::set(source, &name, enabled)? else { return Ok(None) };
        match app.state::<Engine>().record_startup(change.clone()) {
            Ok(id) => Ok(Some(id)),
            Err(e) => {
                let _ = startup::restore(&change);
                Err(t!("journal_save_failed", error = e))
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

// ───────── Puntos de restauración ─────────

#[tauri::command]
async fn restore_status() -> restore::RestoreStatus {
    tauri::async_runtime::spawn_blocking(restore::status).await.unwrap_or_else(|_| restore::RestoreStatus {
        enabled: false,
        policy: false,
        elevated: false,
        last: None,
        count: 0,
        readable: false,
    })
}

#[tauri::command]
async fn restore_create() -> Result<restore::Created, String> {
    tauri::async_runtime::spawn_blocking(|| restore::create(&t!("restore_description")))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn restore_open() -> Result<(), String> {
    restore::open_rstrui()
}

#[tauri::command]
fn restore_settings() -> Result<(), String> {
    restore::open_settings()
}

// ───────── Windows Update ─────────

#[tauri::command]
async fn wu_status() -> wupdate::WuStatus {
    tauri::async_runtime::spawn_blocking(wupdate::status).await.unwrap_or_else(|_| wupdate::WuStatus {
        paused_until: None,
        max_pause_days: wupdate::DEFAULT_MAX_DAYS,
        active_start: None,
        active_end: None,
        smart_hours: None,
        pause_policy: false,
        hours_policy: false,
        elevated: false,
    })
}

#[derive(serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum WuRequest {
    Pause { days: u32 },
    Resume,
    Hours { start: Option<u32>, end: Option<u32> },
}

/// Pausa, reanuda o cambia las horas activas. Devuelve la entrada del historial.
#[tauri::command]
async fn wu_set(app: tauri::AppHandle, request: WuRequest) -> Result<Option<u64>, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<u64>, String> {
        let change = match request {
            WuRequest::Pause { days } => wupdate::pause(days)?,
            WuRequest::Resume => wupdate::resume()?,
            WuRequest::Hours { start: Some(s), end: Some(e) } => wupdate::hours(Some((s, e)))?,
            WuRequest::Hours { start: None, end: None } => wupdate::hours(None)?,
            // Solo una de las dos no es "automáticas": se rechaza.
            WuRequest::Hours { .. } => return Err(t!("wu_hours_invalid", max = wupdate::MAX_ACTIVE_SPAN)),
        };
        // Ya estaba así: no se escribió nada ni hace falta una entrada en el historial.
        let Some(change) = change else { return Ok(None) };
        app.state::<Engine>().record_wu(change.clone()).map(Some).map_err(|e| {
            let _ = wupdate::restore(&change);
            t!("journal_save_failed", error = e)
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ───────── Cambios que Windows volvió atrás ─────────

#[tauri::command]
async fn drift_check(app: tauri::AppHandle) -> Vec<engine::Drift> {
    tauri::async_runtime::spawn_blocking(move || {
        let installed: Vec<(String, String)> = appx::list().unwrap_or_default().into_iter().map(|b| (b.id, b.title)).collect();
        app.state::<Engine>().drift(system::info().build, &installed)
    })
    .await
    .unwrap_or_default()
}

// ───────── Exportar / importar configuración ─────────

fn main_hwnd(app: &tauri::AppHandle) -> Result<isize, String> {
    let w = app.get_webview_window("main").ok_or("sin ventana")?;
    Ok(w.hwnd().map_err(|e| e.to_string())?.0 as isize)
}

#[tauri::command]
async fn profile_export(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let hwnd = main_hwnd(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        profile::export(&app.state::<Engine>(), windows::Win32::Foundation::HWND(hwnd as *mut core::ffi::c_void))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn profile_import(app: tauri::AppHandle) -> Result<Option<profile::ImportPreview>, String> {
    let hwnd = main_hwnd(&app)?;
    tauri::async_runtime::spawn_blocking(move || profile::import(windows::Win32::Foundation::HWND(hwnd as *mut core::ffi::c_void)))
        .await
        .map_err(|e| e.to_string())?
}

// ───────── WMO (otra app del mismo autor) ─────────

#[tauri::command]
async fn wmo_status() -> wmo::Status {
    tauri::async_runtime::spawn_blocking(wmo::status).await.unwrap_or_else(|_| wmo::Status {
        installed: None,
        available: wmo::VERSION.into(),
        update: false,
        running: false,
        sha256: String::new(),
        repo: wmo::REPO.into(),
    })
}

/// Instala o actualiza a la versión fijada en `wmo.rs` (sin parámetros: URL y hash son fijos).
#[tauri::command]
async fn wmo_install() -> Result<wmo::Status, String> {
    tauri::async_runtime::spawn_blocking(wmo::install).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn wmo_uninstall() -> Result<wmo::Status, String> {
    tauri::async_runtime::spawn_blocking(wmo::uninstall).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn wmo_launch() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(wmo::launch).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn wmo_open_repo() -> Result<(), String> {
    wmo::open_repo()
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
            wmo_status,
            wmo_install,
            wmo_uninstall,
            wmo_launch,
            wmo_open_repo,
            network_status,
            dns_test,
            dns_apply,
            dns_confirm,
            dns_revert,
            proxy_status,
            proxy_set,
            startup_status,
            startup_set,
            restore_status,
            restore_create,
            restore_open,
            restore_settings,
            wu_status,
            wu_set,
            drift_check,
            profile_export,
            profile_import,
            play_theme_sound
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación");
}
