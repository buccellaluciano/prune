//! Exportar e importar la configuración de P-Rune a un archivo.
//!
//! El archivo solo guarda ids del catálogo (tweaks y su opción, apps de Windows a quitar,
//! programas de winget a instalar), nunca rutas, claves ni comandos. Al importar se descarta
//! todo lo que no esté en el catálogo de esta versión y se muestra una vista previa: nada
//! se aplica sin pasar por el plan de siempre.
//!
//! La ruta la elige la persona en el diálogo de Windows, que abre el backend: la interfaz
//! no puede pedir leer ni escribir un archivo cualquiera.

use crate::catalog;
use crate::com::{now_secs, Com};
use crate::engine::{self, Engine, TweakState, POWER_ID};
use crate::power;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use windows::core::{Interface, HSTRING, PCWSTR};
use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FileOpenDialog, FileSaveDialog, IFileDialog, IFileOpenDialog, IFileSaveDialog, FOS_FILEMUSTEXIST,
    FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
};

pub const FORMAT: &str = "p-rune-profile";
pub const VERSION: u32 = 1;
const MAX_FILE: u64 = 1024 * 1024;
const MAX_ITEMS: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileTweak {
    pub id: String,
    #[serde(default)]
    pub applied: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub app_version: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub tweaks: Vec<ProfileTweak>,
    /// Apps de Windows (ids de `bloat.toml`) que no tienen que estar.
    #[serde(default)]
    pub bloat: Vec<String>,
    /// Programas de winget (ids de `apps.toml`).
    #[serde(default)]
    pub apps: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportTweak {
    pub id: String,
    pub title: String,
    pub applied: bool,
    pub choice: Option<String>,
    pub choice_label: Option<String>,
    /// Ya está así en esta PC.
    pub current: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportBloat {
    pub id: String,
    pub title: String,
    pub tier: catalog::Risk,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportApp {
    pub id: String,
    pub title: String,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportPreview {
    pub file: String,
    pub app_version: String,
    pub created: u64,
    pub tweaks: Vec<ImportTweak>,
    /// Solo las que esta PC tiene instaladas (las demás no hacen falta).
    pub bloat: Vec<ImportBloat>,
    pub apps: Vec<ImportApp>,
    /// Ids que esta versión no conoce o no se pueden usar acá.
    pub skipped: usize,
}

// ───────────────────────── Diálogo de archivos ─────────────────────────

fn dialog(hwnd: HWND, save: bool) -> Result<Option<PathBuf>, String> {
    let _com = Com::init(COINIT_APARTMENTTHREADED);
    unsafe {
        let d: IFileDialog = if save {
            CoCreateInstance::<_, IFileSaveDialog>(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).and_then(|x| x.cast())
        } else {
            CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).and_then(|x| x.cast())
        }
        .map_err(|e| e.message())?;
        let name = HSTRING::from(t!("profile_filter"));
        let spec = HSTRING::from("*.json");
        d.SetFileTypes(&[COMDLG_FILTERSPEC { pszName: PCWSTR(name.as_ptr()), pszSpec: PCWSTR(spec.as_ptr()) }])
            .map_err(|e| e.message())?;
        d.SetDefaultExtension(&HSTRING::from("json")).map_err(|e| e.message())?;
        if save {
            d.SetFileName(&HSTRING::from("p-rune.json")).map_err(|e| e.message())?;
        }
        let extra = if save { FOS_OVERWRITEPROMPT } else { FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST };
        let opts = d.GetOptions().map_err(|e| e.message())?;
        d.SetOptions(opts | FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR | extra).map_err(|e| e.message())?;
        if let Err(e) = d.Show(Some(hwnd)) {
            return if e.code() == ERROR_CANCELLED.to_hresult() { Ok(None) } else { Err(e.message()) };
        }
        let item = d.GetResult().map_err(|e| e.message())?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).map_err(|e| e.message())?;
        let s = p.to_string().map_err(|e| e.to_string());
        CoTaskMemFree(Some(p.0 as _));
        Ok(Some(PathBuf::from(s?)))
    }
}

// ───────────────────────── Exportar ─────────────────────────

/// Paquetes quitados según el historial (nombres completos) → ids del catálogo.
fn removed_bloat(engine: &Engine) -> HashSet<String> {
    let mut out = HashSet::new();
    for e in engine.history() {
        for full in &e.removed_apps {
            let name = full.split('_').next().unwrap_or(full);
            if let Some(a) = catalog::bloat().app.iter().find(|a| a.matches(name)) {
                out.insert(a.name.clone());
            }
        }
    }
    out
}

fn installed_bloat() -> HashSet<String> {
    crate::appx::list().unwrap_or_default().into_iter().map(|b| b.id).collect()
}

pub fn build(engine: &Engine) -> Profile {
    let build = crate::system::info().build;
    let mut tweaks = Vec::new();
    for v in engine::list(build) {
        if v.id == POWER_ID {
            if v.state == TweakState::Applied {
                let high = power::to_string(&power::HIGH);
                let choice = if v.choice.as_deref() == Some(high.as_str()) { high } else { power::ULTIMATE_NEW.into() };
                tweaks.push(ProfileTweak { id: v.id, applied: true, choice: Some(choice) });
            }
        } else if !v.choices.is_empty() {
            let default = v.choices.iter().find(|c| c.default).map(|c| c.id.clone());
            if let Some(c) = v.choice.filter(|c| Some(c) != default.as_ref()) {
                tweaks.push(ProfileTweak { id: v.id, applied: true, choice: Some(c) });
            }
        } else if v.state == TweakState::Applied {
            tweaks.push(ProfileTweak { id: v.id, applied: true, choice: None });
        }
    }
    let installed = installed_bloat();
    let mut bloat: Vec<String> = removed_bloat(engine).into_iter().filter(|b| !installed.contains(b)).collect();
    bloat.sort();
    let apps = crate::winget::status().apps.into_iter().filter(|a| a.installed.is_some()).map(|a| a.id).collect();
    Profile {
        format: FORMAT.into(),
        version: VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        created: now_secs(),
        tweaks,
        bloat,
        apps,
    }
}

/// Abre "Guardar como" y escribe el archivo. None si se canceló.
pub fn export(engine: &Engine, hwnd: HWND) -> Result<Option<String>, String> {
    // Primero el diálogo: armar el perfil consulta winget y tarda unos segundos.
    let Some(path) = dialog(hwnd, true)? else { return Ok(None) };
    let profile = build(engine);
    let json = serde_json::to_vec_pretty(&profile).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| t!("profile_write_failed", error = e))?;
    Ok(Some(path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())))
}

// ───────────────────────── Importar ─────────────────────────

pub fn parse(bytes: &[u8]) -> Result<Profile, String> {
    let p: Profile = serde_json::from_slice(bytes).map_err(|_| t!("profile_invalid"))?;
    if p.format != FORMAT {
        return Err(t!("profile_invalid"));
    }
    if p.version > VERSION {
        return Err(t!("profile_newer"));
    }
    if p.tweaks.len() > MAX_ITEMS || p.bloat.len() > MAX_ITEMS || p.apps.len() > MAX_ITEMS {
        return Err(t!("profile_invalid"));
    }
    Ok(p)
}

/// Cruza el perfil con el catálogo y el estado de esta PC.
pub fn preview(p: &Profile, file: String) -> ImportPreview {
    let build = crate::system::info().build;
    let views = engine::list(build);
    let mut skipped = 0;
    let mut seen = HashSet::new();

    let mut tweaks = Vec::new();
    for t in &p.tweaks {
        let Some(v) = views.iter().find(|v| v.id == t.id) else {
            skipped += 1;
            continue;
        };
        if !seen.insert(t.id.clone()) || v.state == TweakState::Unsupported {
            skipped += 1;
            continue;
        }
        if v.choices.is_empty() {
            if !t.applied || t.choice.is_some() {
                skipped += 1;
                continue;
            }
            tweaks.push(ImportTweak {
                id: v.id.clone(),
                title: v.title.clone(),
                applied: true,
                choice: None,
                choice_label: None,
                current: v.state == TweakState::Applied,
            });
            continue;
        }
        // "ultimate" en el archivo = la copia de Máximo rendimiento que tenga (o cree) esta PC.
        let wanted = t.choice.as_deref().unwrap_or_default();
        let found = v.choices.iter().find(|c| c.id == wanted).or_else(|| {
            (v.id == POWER_ID && wanted == power::ULTIMATE_NEW)
                .then(|| v.choices.iter().find(|c| c.label == t!("power_ultimate")))
                .flatten()
        });
        let Some(c) = found else {
            skipped += 1;
            continue;
        };
        tweaks.push(ImportTweak {
            id: v.id.clone(),
            title: v.title.clone(),
            applied: true,
            choice: Some(c.id.clone()),
            choice_label: Some(c.label.clone()),
            current: v.choice.as_deref() == Some(c.id.as_str()),
        });
    }

    let installed = crate::appx::list().unwrap_or_default();
    let mut bloat = Vec::new();
    for id in &p.bloat {
        if !seen.insert(format!("bloat:{id}")) {
            continue;
        }
        match catalog::bloat().app.iter().find(|a| &a.name == id) {
            // Lo que esta PC no tiene no hace falta quitarlo.
            Some(_) => {
                if let Some(b) = installed.iter().find(|b| &b.id == id) {
                    bloat.push(ImportBloat { id: b.id.clone(), title: b.title.clone(), tier: b.tier });
                }
            }
            None => skipped += 1,
        }
    }

    let status = if p.apps.is_empty() { None } else { Some(crate::winget::status()) };
    let mut apps = Vec::new();
    for id in &p.apps {
        if !seen.insert(format!("app:{}", id.to_ascii_lowercase())) {
            continue;
        }
        let Some(a) = catalog::is_valid_package_id(id).then(|| catalog::find_app(id)).flatten() else {
            skipped += 1;
            continue;
        };
        let installed = status.as_ref().and_then(|s| s.apps.iter().find(|x| x.id == a.id)).is_some_and(|x| x.installed.is_some());
        apps.push(ImportApp { id: a.id.clone(), title: a.title.clone(), installed });
    }

    // La versión viene del archivo: solo se muestra si tiene forma de versión.
    let app_version = Some(p.app_version.as_str())
        .filter(|v| v.len() <= 32 && v.chars().all(|c| c.is_ascii_alphanumeric() || ".-+".contains(c)))
        .unwrap_or_default()
        .to_string();
    ImportPreview { file, app_version, created: p.created, tweaks, bloat, apps, skipped }
}

/// Abre "Abrir", lee y valida el archivo. None si se canceló.
pub fn import(hwnd: HWND) -> Result<Option<ImportPreview>, String> {
    use std::io::Read;
    let Some(path) = dialog(hwnd, false)? else { return Ok(None) };
    // Se lee con tope (no alcanza con mirar el tamaño antes: el archivo puede crecer en el medio).
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|f| f.take(MAX_FILE + 1).read_to_end(&mut bytes))
        .map_err(|e| t!("profile_read_failed", error = e))?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(t!("profile_too_big"));
    }
    let p = parse(&bytes)?;
    let file = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    Ok(Some(preview(&p, file)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_foreign_or_newer_files() {
        assert!(parse(b"{}").is_err());
        assert!(parse(br#"{"format":"otra-cosa","version":1}"#).is_err());
        assert!(parse(br#"{"format":"p-rune-profile","version":99}"#).is_err());
        assert!(parse(b"no es json").is_err());
        let ok = parse(br#"{"format":"p-rune-profile","version":1,"tweaks":[{"id":"x","applied":true}],"extra":1}"#).unwrap();
        assert_eq!(ok.tweaks.len(), 1);
    }

    #[test]
    fn preview_drops_unknown_ids() {
        let p = parse(
            br#"{"format":"p-rune-profile","version":1,
                "tweaks":[{"id":"no.existe","applied":true},{"id":"explorer.show_extensions","applied":true},
                          {"id":"explorer.show_extensions","applied":true},{"id":"search.taskbar_mode","choice":"nada"}],
                "bloat":["No.Existe"],
                "apps":["--override","Fake.App"]}"#,
        )
        .unwrap();
        let v = preview(&p, "x.json".into());
        assert_eq!(v.tweaks.len(), 1, "{:?}", v.tweaks);
        assert_eq!(v.tweaks[0].id, "explorer.show_extensions");
        assert!(v.bloat.is_empty() && v.apps.is_empty());
        assert_eq!(v.skipped, 6);
        let odd = parse(br#"{"format":"p-rune-profile","version":1,"app_version":"<b>x</b> y otra cosa muy larga"}"#).unwrap();
        assert_eq!(preview(&odd, "x.json".into()).app_version, "");
        let ok = parse(br#"{"format":"p-rune-profile","version":1,"app_version":"0.5.0"}"#).unwrap();
        assert_eq!(preview(&ok, "x.json".into()).app_version, "0.5.0");
    }
}
