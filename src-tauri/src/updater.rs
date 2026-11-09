//! Actualización de P-Rune desde GitHub Releases.
//!
//! - Dónde buscar y con qué clave verificar está fijo en `tauri.conf.json`
//!   (`plugins.updater`): la interfaz no puede cambiar ni la dirección ni la clave.
//! - `requireSignedVersion`: la firma tiene que cubrir la versión anunciada, así nadie puede
//!   presentar un instalador viejo (firmado de verdad) como si fuera nuevo.
//! - Del plugin solo se usa la consulta y la descarga, que verifica la firma en memoria.
//!   El instalador no se ejecuta con `Update::install`: ese lo guarda en `%TEMP%` del
//!   usuario y lo corre como admin, y un programa sin privilegios podría cambiarlo entre
//!   medio. Acá se escribe en una carpeta que solo pueden modificar los administradores y
//!   se ejecuta desde ahí.
//! - La interfaz no tiene permisos del plugin: solo estos dos comandos.

use serde::Serialize;
use std::process::Command;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Actualización encontrada por el último `update_check`, lista para instalar.
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

#[derive(Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub current: String,
    pub notes: Option<String>,
}

#[derive(Clone, Serialize)]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

#[tauri::command]
pub async fn update_check(app: AppHandle, pending: State<'_, Pending>) -> Result<Option<UpdateInfo>, String> {
    let updater = app.updater().map_err(|e| t!("update_failed", error = e))?;
    let found = updater.check().await.map_err(|e| match e {
        // Sin conexión, GitHub caído o el release todavía no es público: no es un error de P-Rune.
        tauri_plugin_updater::Error::ReleaseNotFound
        | tauri_plugin_updater::Error::Reqwest(_)
        | tauri_plugin_updater::Error::Network(_) => t!("update_unreachable"),
        other => t!("update_failed", error = other),
    })?;
    let info = found.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current: u.current_version.clone(),
        notes: u.body.clone(),
    });
    *pending.0.lock().unwrap() = found;
    Ok(info)
}

/// Descarga (verificando la firma), lanza el instalador y cierra la app para que pueda
/// reemplazarla. El instalador vuelve a abrir P-Rune al terminar.
#[tauri::command]
pub async fn update_install(app: AppHandle, pending: State<'_, Pending>) -> Result<(), String> {
    let update = pending.0.lock().unwrap().clone().ok_or_else(|| t!("update_none"))?;

    // Antes de nada, la persona confirma que es ella con el diálogo de Windows. Se hace acá
    // (no en la interfaz) para que no haya forma de saltearlo.
    use tauri::Manager;
    let window = app.get_webview_window("main").ok_or_else(|| t!("update_verify_failed"))?;
    let hwnd = window.hwnd().map_err(|_| t!("update_verify_failed"))?.0 as isize;
    let message = t!("update_verify_message", version = update.version);
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        crate::verify::confirm(windows::Win32::Foundation::HWND(hwnd as *mut core::ffi::c_void), &message)
    })
    .await
    .map_err(|e| t!("update_failed", error = e))??;
    if outcome != crate::verify::Outcome::Verified {
        return Err(t!("update_not_verified"));
    }

    let mut downloaded = 0u64;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = app.emit("update-progress", Progress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| t!("update_failed", error = e))?;
    launch(&bytes, &update.version)?;
    app.exit(0);
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Nsis,
    Msi,
}

fn kind_of(bytes: &[u8]) -> Option<Kind> {
    const OLE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    if bytes.starts_with(&OLE) {
        Some(Kind::Msi)
    } else if bytes.starts_with(b"MZ") {
        Some(Kind::Nsis)
    } else {
        None
    }
}

/// Solo caracteres de versión semántica para el nombre del archivo.
fn safe_version(v: &str) -> String {
    v.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')).take(40).collect()
}

fn system32(exe: &str) -> Result<std::path::PathBuf, String> {
    use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut buf = [0u16; 260];
    let n = unsafe { GetSystemDirectoryW(Some(&mut buf)) } as usize;
    if n == 0 || n >= buf.len() {
        return Err(t!("update_failed", error = "System32"));
    }
    Ok(std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n])).join(exe))
}

/// Mismos argumentos que usa el plugin en modo "passive": barra de progreso sin preguntas,
/// actualización en el lugar y reapertura de la app al terminar.
fn installer_command(kind: &Kind, path: &std::path::Path) -> Result<Command, String> {
    Ok(match kind {
        Kind::Nsis => {
            let mut c = Command::new(path);
            c.args(["/P", "/UPDATE", "/R", "/ARGS"]);
            c
        }
        Kind::Msi => {
            let mut c = Command::new(system32("msiexec.exe")?);
            c.arg("/i").arg(path).args(["/passive", "/promptrestart", "AUTOLAUNCHAPP=True", "LAUNCHAPPARGS=\"\""]);
            c
        }
    })
}

fn launch(bytes: &[u8], version: &str) -> Result<(), String> {
    let kind = kind_of(bytes).ok_or_else(|| t!("update_bad_format"))?;
    let dir = crate::winget::private_dir()?;
    let ext = if kind == Kind::Msi { "msi" } else { "exe" };
    let path = dir.join(format!("P-Rune-{}-setup.{ext}", safe_version(version)));
    std::fs::write(&path, bytes).map_err(|e| t!("update_failed", error = e))?;
    installer_command(&kind, &path)?.spawn().map_err(|e| t!("update_failed", error = e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tipo_de_instalador() {
        assert_eq!(kind_of(b"MZ\x90\x00"), Some(Kind::Nsis));
        assert_eq!(kind_of(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0]), Some(Kind::Msi));
        assert_eq!(kind_of(b"<html>"), None);
        assert_eq!(kind_of(b""), None);
    }

    #[test]
    fn version_para_nombre_de_archivo() {
        assert_eq!(safe_version("0.3.0"), "0.3.0");
        assert_eq!(safe_version("1.0.0-beta.2+build"), "1.0.0-beta.2+build");
        assert_eq!(safe_version(r"..\..\evil 1.0"), "....evil1.0");
    }

    #[test]
    fn argumentos_del_instalador() {
        let p = std::path::Path::new(r"C:\x\P-Rune-0.3.0-setup.exe");
        let nsis = installer_command(&Kind::Nsis, p).unwrap();
        assert_eq!(nsis.get_program(), p.as_os_str());
        assert_eq!(nsis.get_args().collect::<Vec<_>>(), ["/P", "/UPDATE", "/R", "/ARGS"]);
        let msi = installer_command(&Kind::Msi, p).unwrap();
        assert!(msi.get_program().to_string_lossy().to_ascii_lowercase().ends_with(r"\system32\msiexec.exe"));
        let args: Vec<_> = msi.get_args().map(|a| a.to_string_lossy().to_string()).collect();
        assert_eq!(args[..3], ["/i".to_string(), p.to_string_lossy().to_string(), "/passive".to_string()]);
    }
}
