//! Instalar, actualizar y desinstalar programas del catálogo con winget.exe.
//!
//! Seguridad: solo se aceptan IDs del catálogo (`apps.toml`), siempre con `--exact`
//! y `--source winget`, y los argumentos van separados (sin shell de por medio).

use crate::catalog::{self, AppGroup};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const COMMON: [&str; 2] = ["--accept-source-agreements", "--disable-interactivity"];

// Códigos de salida de winget (APPINSTALLER_CLI_ERROR_*).
const UPDATE_NOT_APPLICABLE: u32 = 0x8A15_002B;
const NO_APPLICATIONS_FOUND: u32 = 0x8A15_0014;
const PACKAGE_ALREADY_INSTALLED: u32 = 0x8A15_0061;

/// Una operación de winget a la vez: evita que dos instaladores se pisen.
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Install,
    Upgrade,
    Uninstall,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppStatus {
    pub id: String,
    pub title: String,
    pub group: String,
    pub description: String,
    /// Versión instalada, si está instalada.
    pub installed: Option<String>,
    /// Versión nueva disponible, si hay actualización.
    pub available: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppsOverview {
    pub winget: Option<String>,
    pub error: Option<String>,
    pub groups: Vec<AppGroup>,
    pub apps: Vec<AppStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionResult {
    pub id: String,
    pub ok: bool,
    pub message: Option<String>,
}

fn winget_path() -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA no está definido")?;
    let p = PathBuf::from(base).join(r"Microsoft\WindowsApps\winget.exe");
    // Es un alias de ejecución (reparse point): `metadata` falla, `symlink_metadata` no.
    if std::fs::symlink_metadata(&p).is_ok() {
        Ok(p)
    } else {
        Err("No se encontró winget. Instalá o actualizá \"Instalador de aplicación\" desde Microsoft Store.".into())
    }
}

/// Ejecuta winget y devuelve (código de salida, líneas de salida).
/// `on_line` recibe cada línea a medida que llega (incluye barras de progreso).
fn run(args: &[&str], mut on_line: impl FnMut(&str)) -> Result<(u32, Vec<String>), String> {
    let mut child = Command::new(winget_path()?)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("No se pudo ejecutar winget: {e}"))?;

    let mut out = child.stdout.take().expect("stdout");
    let mut lines = Vec::new();
    let mut cur: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    let mut flush = |cur: &mut Vec<u8>, lines: &mut Vec<String>| {
        let s = String::from_utf8_lossy(cur).trim().to_string();
        cur.clear();
        // Ignora el "spinner" de winget (- \ | /).
        if s.chars().count() > 1 {
            on_line(&s);
            lines.push(s);
        }
    };
    loop {
        let n = out.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        for &b in &buf[..n] {
            if b == b'\r' || b == b'\n' {
                flush(&mut cur, &mut lines);
            } else {
                cur.push(b);
            }
        }
    }
    flush(&mut cur, &mut lines);
    let status = child.wait().map_err(|e| e.to_string())?;
    Ok((status.code().unwrap_or(-1) as u32, lines))
}

fn version() -> Result<String, String> {
    let (code, lines) = run(&["--version"], |_| {})?;
    match (code, lines.first()) {
        (0, Some(v)) => Ok(v.clone()),
        _ => Err(format!("winget respondió con error 0x{code:08X}")),
    }
}

/// Versiones instaladas, por ID en minúsculas. Usa `winget export`: JSON, sin columnas truncadas.
fn installed() -> Result<HashMap<String, String>, String> {
    let tmp = std::env::temp_dir().join(format!("debloater-winget-{}.json", std::process::id()));
    let tmp_s = tmp.to_string_lossy().to_string();
    let mut args = vec!["export", "-o", &tmp_s, "--source", "winget", "--include-versions"];
    args.extend(COMMON);
    let (code, lines) = run(&args, |_| {})?;
    let data = std::fs::read(&tmp);
    let _ = std::fs::remove_file(&tmp);
    let data = data.map_err(|_| format!("winget export falló (0x{code:08X}): {}", lines.last().cloned().unwrap_or_default()))?;
    let json: serde_json::Value = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    let mut map = HashMap::new();
    for src in json["Sources"].as_array().into_iter().flatten() {
        for p in src["Packages"].as_array().into_iter().flatten() {
            if let Some(id) = p["PackageIdentifier"].as_str() {
                let v = p["Version"].as_str().unwrap_or("?").to_string();
                map.insert(id.to_ascii_lowercase(), v);
            }
        }
    }
    Ok(map)
}

/// Actualizaciones disponibles para IDs del catálogo, a partir de la tabla de `winget upgrade`.
/// Busca el ID exacto como palabra y toma la última columna (Disponible). Independiente del idioma.
fn upgrades() -> Result<HashMap<String, String>, String> {
    let mut args = vec!["upgrade", "--source", "winget"];
    args.extend(COMMON);
    let (_, lines) = run(&args, |_| {})?;
    Ok(parse_upgrades(&lines))
}

fn parse_upgrades(lines: &[String]) -> HashMap<String, String> {
    let ids: HashMap<String, ()> = catalog::apps().app.iter().map(|a| (a.id.to_ascii_lowercase(), ())).collect();
    let mut map = HashMap::new();
    for line in lines {
        let toks: Vec<&str> = line.split_whitespace().collect();
        if let Some(pos) = toks.iter().position(|t| ids.contains_key(&t.to_ascii_lowercase())) {
            let rest = &toks[pos + 1..];
            if rest.len() >= 2 {
                map.insert(toks[pos].to_ascii_lowercase(), rest[rest.len() - 1].to_string());
            }
        }
    }
    map
}

pub fn status() -> AppsOverview {
    let c = catalog::apps();
    let mut ov = AppsOverview { winget: None, error: None, groups: c.group.clone(), apps: Vec::new() };
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    match version() {
        Ok(v) => ov.winget = Some(v),
        Err(e) => {
            ov.error = Some(e);
            return ov;
        }
    }
    let (inst, upg) = std::thread::scope(|s| {
        let a = s.spawn(installed);
        let b = s.spawn(upgrades);
        (a.join().unwrap_or_else(|_| Err("pánico".into())), b.join().unwrap_or_else(|_| Err("pánico".into())))
    });
    let inst = inst.unwrap_or_else(|e| {
        ov.error = Some(e);
        HashMap::new()
    });
    let upg = upg.unwrap_or_default();
    ov.apps = c
        .app
        .iter()
        .map(|a| {
            let k = a.id.to_ascii_lowercase();
            AppStatus {
                id: a.id.clone(),
                title: a.title.clone(),
                group: a.group.clone(),
                description: a.description.clone(),
                installed: inst.get(&k).cloned(),
                available: if inst.contains_key(&k) { upg.get(&k).cloned() } else { None },
            }
        })
        .collect();
    ov
}

pub fn act(id: &str, action: Action, mut progress: impl FnMut(&str)) -> ActionResult {
    let fail = |m: String| ActionResult { id: id.to_string(), ok: false, message: Some(m) };
    let Some(app) = catalog::find_app(id) else {
        return fail("El programa no está en el catálogo".into());
    };
    if !catalog::is_valid_package_id(&app.id) {
        return fail("ID de paquete inválido".into());
    }
    let mut args: Vec<&str> = match action {
        Action::Install => vec!["install"],
        Action::Upgrade => vec!["upgrade"],
        Action::Uninstall => vec!["uninstall"],
    };
    args.extend(["--id", &app.id, "--exact", "--source", "winget", "--silent"]);
    if !matches!(action, Action::Uninstall) {
        args.push("--accept-package-agreements");
    }
    args.extend(COMMON);

    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut last = Instant::now() - Duration::from_secs(1);
    let res = run(&args, |line| {
        // Limita la frecuencia de eventos de progreso a la UI.
        if last.elapsed() >= Duration::from_millis(120) {
            last = Instant::now();
            progress(line);
        }
    });
    let (code, lines) = match res {
        Ok(x) => x,
        Err(e) => return fail(e),
    };
    let ok = |m: &str| ActionResult { id: id.to_string(), ok: true, message: Some(m.to_string()) };
    match code {
        0 => ActionResult { id: id.to_string(), ok: true, message: None },
        UPDATE_NOT_APPLICABLE => ok("Ya estaba en la última versión"),
        PACKAGE_ALREADY_INSTALLED => ok("Ya estaba instalado"),
        NO_APPLICATIONS_FOUND => fail("winget no encontró el paquete (¿ya se desinstaló?)".into()),
        _ => {
            let detail = lines
                .iter()
                .rev()
                .find(|l| !l.contains('█') && !l.contains('▒') && !l.starts_with("--"))
                .cloned()
                .unwrap_or_default();
            fail(format!("{detail} (código 0x{code:08X})").trim_start().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_upgrade_table() {
        let lines: Vec<String> = [
            "Name             Id                        Version              Available",
            "-----------------------------------------------------------------------------------",
            "App Installer    Microsoft.AppInstaller    1.29.379.0           1.29.380",
            "Mozilla Firefox (x64 es-AR) Mozilla.Firefox 143.0 144.0.1",
            "Git              Git.Git                   < 2.50.0             2.56.0",
            "3 upgrades available.",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let m = parse_upgrades(&lines);
        assert_eq!(m.get("mozilla.firefox").map(String::as_str), Some("144.0.1"));
        assert_eq!(m.get("git.git").map(String::as_str), Some("2.56.0"));
        // Fuera del catálogo: se ignora.
        assert!(!m.contains_key("microsoft.appinstaller"));
    }
}
