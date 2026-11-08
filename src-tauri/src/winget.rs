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
use std::sync::mpsc::{self, RecvTimeoutError};
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

const APP_INSTALLER_FAMILY: &str = "Microsoft.DesktopAppInstaller_8wekyb3d8bbwe";

/// Carpeta "Program Files" según Windows (no según variables de entorno, que se pueden cambiar).
pub(crate) fn program_files() -> Option<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{SHGetKnownFolderPath, FOLDERID_ProgramFiles, KF_FLAG_DEFAULT};
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_ProgramFiles, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        s.map(PathBuf::from)
    }
}

/// winget.exe real, dentro del paquete "Instalador de aplicación" en Program Files\WindowsApps.
///
/// No se usa el alias de `%LOCALAPPDATA%\Microsoft\WindowsApps`: esa carpeta la puede
/// modificar cualquier programa del usuario, y la app (como admin) lo ejecutaría sin aviso.
fn locate_winget() -> Result<PathBuf, String> {
    use windows::core::HSTRING;
    use windows::ApplicationModel::PackageSignatureKind;
    use windows::Management::Deployment::PackageManager;
    let not_found = || t!("winget_not_found");
    let pm = PackageManager::new().map_err(|e| e.to_string())?;
    let family = HSTRING::from(APP_INSTALLER_FAMILY);
    let mut dirs: Vec<PathBuf> = Vec::new();
    // Del usuario actual (no requiere admin); si no, de cualquier usuario (requiere admin).
    let lists = [pm.FindPackagesByUserSecurityIdPackageFamilyName(&HSTRING::new(), &family), pm.FindPackagesByPackageFamilyName(&family)];
    for list in lists.into_iter().flatten() {
        for p in list {
            // Paquete firmado por Microsoft (Store o sistema), no uno con el mismo nombre.
            let from_microsoft = p.Id().and_then(|id| id.Publisher()).is_ok_and(|pb| pb.to_string().starts_with("CN=Microsoft Corporation,"));
            let signed = p.SignatureKind().is_ok_and(|k| k == PackageSignatureKind::Store || k == PackageSignatureKind::System);
            if !(from_microsoft && signed) {
                continue;
            }
            if let Ok(path) = p.InstalledPath() {
                dirs.push(PathBuf::from(path.to_string()));
            }
        }
        if !dirs.is_empty() {
            break;
        }
    }
    let protected = program_files().ok_or_else(not_found)?.join("WindowsApps");
    let protected = protected.to_string_lossy().to_ascii_lowercase();
    let exe = dirs
        .into_iter()
        .map(|d| d.join("winget.exe"))
        .find(|exe| exe.to_string_lossy().to_ascii_lowercase().starts_with(&protected) && exe.is_file())
        .ok_or_else(not_found)?;
    // Y el ejecutable en sí, con firma válida de Microsoft.
    crate::sign::require_microsoft(&exe).map_err(|e| t!("winget_bad_signature", error = e))?;
    Ok(exe)
}

#[cfg(test)]
pub(crate) fn winget_exe_for_tests() -> PathBuf {
    winget_path().unwrap()
}

fn winget_path() -> Result<PathBuf, String> {
    static PATH: std::sync::OnceLock<Result<PathBuf, String>> = std::sync::OnceLock::new();
    PATH.get_or_init(locate_winget).clone()
}

/// Tiempo máximo para consultas (listar, versiones) y para instalar/actualizar/desinstalar.
const QUERY_TIMEOUT: Duration = Duration::from_secs(3 * 60);
const ACTION_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Separa la salida en líneas (winget usa `\r` para las barras de progreso).
fn split_lines(cur: &mut Vec<u8>, bytes: &[u8], mut emit: impl FnMut(String)) {
    for &b in bytes {
        if b == b'\r' || b == b'\n' {
            let s = String::from_utf8_lossy(cur).trim().to_string();
            cur.clear();
            // Ignora el "spinner" de winget (- \ | /).
            if s.chars().count() > 1 {
                emit(s);
            }
        } else {
            cur.push(b);
        }
    }
}

/// Ejecuta winget y devuelve (código de salida, líneas de salida).
/// `on_line` recibe cada línea a medida que llega (incluye barras de progreso).
///
/// Se termina cuando termina **el proceso de winget**, no cuando se cierra su salida:
/// algunos instaladores abren el programa al final y ese proceso puede heredar la
/// salida de winget y mantenerla abierta indefinidamente. Además hay un tiempo límite.
fn run(args: &[&str], timeout: Duration, mut on_line: impl FnMut(&str)) -> Result<(u32, Vec<String>), String> {
    let mut child = Command::new(winget_path()?)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| t!("winget_run_failed", error = e))?;

    // Un hilo lee la salida y la manda por un canal; si queda bloqueado por un proceso
    // heredero, simplemente se abandona (termina solo cuando ese proceso cierre).
    let mut out = child.stdout.take().expect("stdout");
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut cur = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok(n) = out.read(&mut buf) {
            if n == 0 {
                break;
            }
            split_lines(&mut cur, &buf[..n], |s| {
                let _ = tx.send(s);
            });
        }
        split_lines(&mut cur, b"\n", |s| {
            let _ = tx.send(s);
        });
    });

    let started = Instant::now();
    let mut lines = Vec::new();
    let mut take = |line: String, lines: &mut Vec<String>| {
        on_line(&line);
        lines.push(line);
    };
    let status = loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => take(line, &mut lines),
            Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {}
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(t!("winget_timeout", minutes = timeout.as_secs() / 60));
        }
    };
    // Lo que quedó en el canal, sin esperar a que se cierre la salida.
    let drain_until = Instant::now() + Duration::from_millis(300);
    while let Some(left) = drain_until.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(line) => take(line, &mut lines),
            Err(_) => break,
        }
    }
    Ok((status.code().unwrap_or(-1) as u32, lines))
}

fn version() -> Result<String, String> {
    let (code, lines) = run(&["--version"], QUERY_TIMEOUT, |_| {})?;
    match (code, lines.first()) {
        (0, Some(v)) => Ok(v.clone()),
        _ => Err(format!("winget respondió con error 0x{code:08X}")),
    }
}

/// Carpeta temporal propia para `winget export`.
///
/// `%TEMP%` es del usuario: cualquier programa suyo podría dejar ahí un enlace con el nombre
/// que vamos a usar y hacer que winget (como admin) escriba en otro lado. Por eso se crea
/// una carpeta nueva (falla si ya existe algo con ese nombre) y, si la app corre como
/// admin, nace con permisos solo para Administradores y SYSTEM.
pub(crate) fn private_dir() -> Result<PathBuf, String> {
    private_dir_with(crate::system::is_elevated())
}

fn private_dir_with(elevated: bool) -> Result<PathBuf, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
    use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
    use windows::Win32::Storage::FileSystem::CreateDirectoryW;

    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    for attempt in 0..8u128 {
        let path = std::env::temp_dir().join(format!("debloater-{:x}", seed.wrapping_mul(2654435761).wrapping_add(attempt)));
        let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        let created = unsafe {
            if elevated {
                // P = protegida (no hereda de %TEMP%); acceso total solo a SYSTEM y Administradores.
                let sddl: Vec<u16> = "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)".encode_utf16().chain(std::iter::once(0)).collect();
                let mut sd = PSECURITY_DESCRIPTOR::default();
                ConvertStringSecurityDescriptorToSecurityDescriptorW(PCWSTR(sddl.as_ptr()), SDDL_REVISION_1, &mut sd, None)
                    .map_err(|e| e.message())?;
                let sa = SECURITY_ATTRIBUTES {
                    nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                    lpSecurityDescriptor: sd.0,
                    bInheritHandle: false.into(),
                };
                let r = CreateDirectoryW(PCWSTR(wide.as_ptr()), Some(&sa));
                let _ = LocalFree(Some(HLOCAL(sd.0)));
                r
            } else {
                CreateDirectoryW(PCWSTR(wide.as_ptr()), None)
            }
        };
        if created.is_ok() {
            return Ok(path);
        }
    }
    Err("No se pudo crear la carpeta temporal".into())
}

/// Versiones instaladas, por ID en minúsculas. Usa `winget export`: JSON, sin columnas truncadas.
fn installed() -> Result<HashMap<String, String>, String> {
    let dir = private_dir()?;
    let tmp = dir.join("export.json");
    let tmp_s = tmp.to_string_lossy().to_string();
    let mut args = vec!["export", "-o", &tmp_s, "--source", "winget", "--include-versions"];
    args.extend(COMMON);
    let res = run(&args, QUERY_TIMEOUT, |_| {});
    let data = std::fs::read(&tmp);
    let _ = std::fs::remove_dir_all(&dir);
    let (code, lines) = res?;
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
    let (_, lines) = run(&args, QUERY_TIMEOUT, |_| {})?;
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

// ───────── Programas instalados por fuera de winget ─────────
//
// `winget export` (y `winget list` completo) solo identifican un programa si la
// correspondencia con el repositorio no es ambigua. Brave instalado desde su web, por
// ejemplo, coincide con `Brave.Brave` de winget y con el de Microsoft Store, así que en el
// listado general queda como "ARP\Machine\X86\BraveSoftware Brave-Browser" y no aparece.
// Preguntando por el ID exacto (`winget list --id Brave.Brave --exact`) winget sí lo
// confirma. Para no hacer 39 consultas, solo se pregunta por los programas del catálogo
// cuyo nombre aparece en "Aplicaciones instaladas" del registro.

/// Nombres visibles de lo instalado ("Agregar o quitar programas"), 64 y 32 bits y del usuario.
fn arp_names() -> Vec<String> {
    use winreg::enums::*;
    use winreg::RegKey;
    const PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
    let roots = [
        (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_64KEY),
        (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_32KEY),
        (HKEY_CURRENT_USER, KEY_READ),
    ];
    let mut out = Vec::new();
    for (hive, flags) in roots {
        let Ok(k) = RegKey::predef(hive).open_subkey_with_flags(PATH, flags) else { continue };
        for name in k.enum_keys().flatten() {
            let Ok(sub) = k.open_subkey_with_flags(&name, flags) else { continue };
            if sub.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1 {
                continue;
            }
            if let Ok(d) = sub.get_value::<String, _>("DisplayName") {
                out.push(d);
            }
        }
    }
    out
}

fn name_tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| c.is_whitespace() || "()[],:;/-_".contains(c))
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

/// ¿El nombre registrado puede ser este programa? Es generoso a propósito ("VLC" con
/// "VLC media player", "Node.js LTS" con "Node.js"): la confirmación la da winget.
fn title_matches(title: &str, display: &str) -> bool {
    let (t, d) = (name_tokens(title), name_tokens(display));
    if t.is_empty() || d.is_empty() {
        return false;
    }
    d.windows(t.len()).any(|w| w == t.as_slice()) || (t[0].len() >= 3 && d[0] == t[0])
}

/// Versión instalada y disponible (si hay actualización) de la fila de `id` en la tabla de
/// `winget list --id <id> --exact --source winget`. Independiente del idioma.
fn parse_list_row(lines: &[String], id: &str) -> Option<(String, Option<String>)> {
    for line in lines {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let Some(pos) = toks.iter().position(|t| t.eq_ignore_ascii_case(id)) else { continue };
        let mut rest = toks[pos + 1..].iter().copied();
        let mut version = rest.next()?.to_string();
        // Versiones aproximadas: "< 1.2" / "> 3.0".
        if version == "<" || version == ">" {
            version = format!("{version} {}", rest.next()?);
        }
        let available = rest.next().filter(|t| !t.eq_ignore_ascii_case("winget")).map(String::from);
        return Some((version, available));
    }
    None
}

/// Le pregunta a winget por un ID exacto: si lo reconoce como instalado, devuelve versión y
/// actualización disponible.
fn confirm_installed(id: &str) -> Option<(String, Option<String>)> {
    let mut args = vec!["list", "--id", id, "--exact", "--source", "winget"];
    args.extend(COMMON);
    let (code, lines) = run(&args, QUERY_TIMEOUT, |_| {}).ok()?;
    if code != 0 {
        return None;
    }
    parse_list_row(&lines, id)
}

/// Completa `inst`/`upg` con los programas del catálogo que `export` no identificó.
fn add_unmatched(inst: &mut HashMap<String, String>, upg: &mut HashMap<String, String>) {
    let missing: Vec<&catalog::WingetApp> =
        catalog::apps().app.iter().filter(|a| !inst.contains_key(&a.id.to_ascii_lowercase())).collect();
    if missing.is_empty() {
        return;
    }
    let names = arp_names();
    let candidates: Vec<&catalog::WingetApp> =
        missing.into_iter().filter(|a| names.iter().any(|n| title_matches(&a.title, n))).collect();
    let found: Vec<(String, Option<(String, Option<String>)>)> = std::thread::scope(|s| {
        let handles: Vec<_> = candidates.iter().map(|a| (a.id.to_ascii_lowercase(), s.spawn(|| confirm_installed(&a.id)))).collect();
        handles.into_iter().map(|(k, h)| (k, h.join().ok().flatten())).collect()
    });
    for (k, r) in found {
        if let Some((version, available)) = r {
            if let Some(v) = available {
                upg.insert(k.clone(), v);
            }
            inst.insert(k, version);
        }
    }
}

pub fn status() -> AppsOverview {
    let c = catalog::apps();
    let groups = c
        .group
        .iter()
        .map(|g| catalog::AppGroup { id: g.id.clone(), title: crate::i18n::app_group(&g.id, &g.title) })
        .collect();
    let mut ov = AppsOverview { winget: None, error: None, groups, apps: Vec::new() };
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
    let mut inst = inst.unwrap_or_else(|e| {
        ov.error = Some(e);
        HashMap::new()
    });
    let mut upg = upg.unwrap_or_default();
    if ov.error.is_none() {
        add_unmatched(&mut inst, &mut upg);
    }
    ov.apps = c
        .app
        .iter()
        .map(|a| {
            let k = a.id.to_ascii_lowercase();
            AppStatus {
                id: a.id.clone(),
                title: a.title.clone(),
                group: a.group.clone(),
                description: crate::i18n::app_description(a),
                installed: inst.get(&k).cloned(),
                available: if inst.contains_key(&k) { upg.get(&k).cloned() } else { None },
            }
        })
        .collect();
    ov
}

/// Etapa de una operación de winget, deducida de lo que va imprimiendo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Download,
    Hash,
    Install,
    Uninstall,
    Done,
}

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub line: String,
    pub phase: Option<Phase>,
    /// 0.0–1.0 si la línea trae avance (bytes descargados o porcentaje).
    pub fraction: Option<f32>,
}

fn unit(u: &str) -> Option<f64> {
    match u.to_ascii_uppercase().as_str() {
        "B" => Some(1.0),
        "KB" => Some(1024.0),
        "MB" => Some(1024.0 * 1024.0),
        "GB" => Some(1024.0 * 1024.0 * 1024.0),
        _ => None,
    }
}

/// Avance de una barra de winget: "12.0 MB / 50.0 MB" (o con coma decimal) o "45%".
fn fraction(line: &str) -> Option<f32> {
    let clean: String = line.chars().map(|c| if c == '█' || c == '▒' { ' ' } else { c }).collect();
    let toks: Vec<&str> = clean.split_whitespace().collect();
    // número unidad / número unidad
    for w in toks.windows(5) {
        if w[2] == "/" {
            let a = w[0].replace(',', ".").parse::<f64>().ok().zip(unit(w[1]));
            let b = w[3].replace(',', ".").parse::<f64>().ok().zip(unit(w[4]));
            if let (Some((a, ua)), Some((b, ub))) = (a, b) {
                if b > 0.0 {
                    return Some(((a * ua) / (b * ub)).clamp(0.0, 1.0) as f32);
                }
            }
        }
    }
    toks.iter()
        .rev()
        .find_map(|t| t.strip_suffix('%')?.replace(',', ".").parse::<f32>().ok())
        .map(|p| (p / 100.0).clamp(0.0, 1.0))
}

/// Clasifica una línea de winget (en inglés o en español) en una etapa.
pub fn classify(line: &str) -> Progress {
    let l = line.to_lowercase();
    let frac = fraction(line);
    let phase = if l.contains("successfully installed")
        || l.contains("instalado correctamente")
        || l.contains("successfully uninstalled")
        || l.contains("desinstalado correctamente")
    {
        Some(Phase::Done)
    } else if l.contains("uninstall") || l.contains("desinstal") {
        Some(Phase::Uninstall)
    } else if l.contains("hash") {
        Some(Phase::Hash)
    } else if l.contains("download") || l.contains("descarg") || (frac.is_some() && (l.contains("mb") || l.contains("kb"))) {
        Some(Phase::Download)
    } else if l.contains("install") || l.contains("instal") {
        Some(Phase::Install)
    } else {
        None
    };
    Progress { line: line.to_string(), phase, fraction: frac }
}

/// Datos del instalador según el manifiesto de winget (para verificarlo antes).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InstallerInfo {
    pub url: Option<String>,
    pub sha256: Option<String>,
    pub version: Option<String>,
}

/// Interpreta la salida de `winget show`, sin depender del idioma de las etiquetas:
/// el SHA-256 es el valor de 64 hex; la URL del instalador es la que va justo antes.
pub fn parse_installer(lines: &[String]) -> InstallerInfo {
    let value = |l: &str| l.split_once(": ").map(|(_, v)| v.trim().to_string());
    let is_sha = |v: &str| v.len() == 64 && v.chars().all(|c| c.is_ascii_hexdigit());
    let mut info = InstallerInfo { url: None, sha256: None, version: None };
    for (i, l) in lines.iter().enumerate() {
        let Some(v) = value(l) else { continue };
        if info.version.is_none() && (l.trim_start().starts_with("Version:") || l.trim_start().starts_with("Versión:")) {
            info.version = Some(v.clone());
        }
        if info.sha256.is_none() && is_sha(&v) {
            info.sha256 = Some(v.to_ascii_lowercase());
            info.url = lines[..i]
                .iter()
                .rev()
                .take(3)
                .filter_map(|p| value(p))
                .find(|u| u.starts_with("https://") || u.starts_with("http://"));
        }
    }
    info
}

pub fn installer_info(id: &str) -> Result<InstallerInfo, String> {
    let app = catalog::find_app(id).ok_or_else(|| t!("app_not_in_catalog"))?;
    let mut args = vec!["show", "--id", &app.id, "--exact", "--source", "winget"];
    args.extend(COMMON);
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (code, lines) = run(&args, QUERY_TIMEOUT, |_| {})?;
    if code != 0 {
        return Err(t!("winget_show_failed", code = format!("0x{code:08X}")));
    }
    let info = parse_installer(&lines);
    if info.sha256.is_none() {
        return Err(t!("winget_no_hash"));
    }
    Ok(info)
}

pub fn act(id: &str, action: Action, mut progress: impl FnMut(Progress)) -> ActionResult {
    let fail = |m: String| ActionResult { id: id.to_string(), ok: false, message: Some(m) };
    let Some(app) = catalog::find_app(id) else {
        return fail(t!("app_not_in_catalog"));
    };
    if !catalog::is_valid_package_id(&app.id) {
        return fail(t!("package_id_invalid"));
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
    let mut last_phase = None;
    let res = run(&args, ACTION_TIMEOUT, |line| {
        let p = classify(line);
        // Los cambios de etapa siempre llegan; el resto, como mucho cada 120 ms.
        let changed = p.phase.is_some() && p.phase != last_phase;
        if changed || last.elapsed() >= Duration::from_millis(120) {
            last = Instant::now();
            if p.phase.is_some() {
                last_phase = p.phase;
            }
            progress(p);
        }
    });
    let (code, lines) = match res {
        Ok(x) => x,
        Err(e) => return fail(e),
    };
    let ok = |m: &str| ActionResult { id: id.to_string(), ok: true, message: Some(m.to_string()) };
    match code {
        0 => ActionResult { id: id.to_string(), ok: true, message: None },
        UPDATE_NOT_APPLICABLE => ok(&t!("already_latest")),
        PACKAGE_ALREADY_INSTALLED => ok(&t!("already_installed")),
        NO_APPLICATIONS_FOUND => fail(t!("package_not_found")),
        _ => {
            let detail = lines
                .iter()
                .rev()
                .find(|l| !l.contains('█') && !l.contains('▒') && !l.starts_with("--"))
                .cloned()
                .unwrap_or_default();
            fail(t!("winget_exit_code", detail = detail, code = format!("0x{code:08X}")).trim_start().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_winget_lines_in_both_languages() {
        let c = |l: &str| classify(l);
        assert_eq!(c("Downloading https://x/7z.msi").phase, Some(Phase::Download));
        assert_eq!(c("Descargando https://x/7z.msi").phase, Some(Phase::Download));
        let p = c("  ██████████▒▒▒▒▒▒▒▒▒▒  12.0 MB / 48.0 MB");
        assert_eq!(p.phase, Some(Phase::Download));
        assert!((p.fraction.unwrap() - 0.25).abs() < 0.001);
        assert!((c("  ███▒▒▒  1,5 MB / 3,0 MB").fraction.unwrap() - 0.5).abs() < 0.001);
        assert!((c("  ████▒▒▒  1024 KB / 2.0 MB").fraction.unwrap() - 0.5).abs() < 0.001);
        assert_eq!(c("Successfully verified installer hash").phase, Some(Phase::Hash));
        assert_eq!(c("Se verificó correctamente el hash del instalador").phase, Some(Phase::Hash));
        assert_eq!(c("Starting package install...").phase, Some(Phase::Install));
        assert_eq!(c("Iniciando la instalación del paquete...").phase, Some(Phase::Install));
        assert_eq!(c("Successfully installed").phase, Some(Phase::Done));
        assert_eq!(c("Instalado correctamente").phase, Some(Phase::Done));
        assert_eq!(c("Starting package uninstall...").phase, Some(Phase::Uninstall));
        assert_eq!(c("Desinstalado correctamente").phase, Some(Phase::Done));
        assert_eq!(c("  ████████  45%").fraction, Some(0.45));
        assert_eq!(c("Found 7-Zip [7zip.7zip] Version 26.05").phase, None);
    }

    #[test]
    fn parses_installer_from_winget_show() {
        // Salida real de `winget show --id 7zip.7zip` (recortada).
        let out: Vec<String> = [
            "Found 7-Zip [7zip.7zip]",
            "Version: 26.05",
            "Homepage: https://7-zip.org/download.html",
            "Installer:",
            "  Installer Type: wix",
            "  Installer Url: https://github.com/ip7z/7zip/releases/download/26.05/7z2605-x64.msi",
            "  Installer SHA256: 3FE4AB99FA15629C652944309A2B2BC6E41FC933FC6B63F5DF1EBCF3AEA68263",
            "  Release Date: 2026-10-21",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let i = parse_installer(&out);
        assert_eq!(i.sha256.as_deref(), Some("3fe4ab99fa15629c652944309a2b2bc6e41fc933fc6b63f5df1ebcf3aea68263"));
        assert_eq!(i.url.as_deref(), Some("https://github.com/ip7z/7zip/releases/download/26.05/7z2605-x64.msi"));
        assert_eq!(i.version.as_deref(), Some("26.05"));
        // Mismas líneas con etiquetas en español: el resultado no depende del idioma.
        let es: Vec<String> = out
            .iter()
            .map(|l| l.replace("Installer Url", "Dirección URL del instalador").replace("Installer SHA256", "SHA256 del instalador"))
            .collect();
        assert_eq!(parse_installer(&es).sha256, i.sha256);
        assert_eq!(parse_installer(&es).url, i.url);
    }

    /// `winget show` real (solo lectura).
    #[test]
    #[ignore]
    fn real_installer_info() {
        let i = installer_info("7zip.7zip").unwrap();
        println!("{i:?}");
        assert!(i.url.is_some() && i.sha256.is_some());
    }

    #[test]
    fn splits_progress_and_lines() {
        let mut cur = Vec::new();
        let mut out = Vec::new();
        split_lines(&mut cur, b"  -\r  \\\r  Descargando 10%\r  Descargando 55%\rListo\nresto", |s| out.push(s));
        assert_eq!(out, ["Descargando 10%", "Descargando 55%", "Listo"]);
        assert_eq!(cur, b"resto");
    }

    /// Corre winget de verdad (solo `--version`) con el nuevo `run`.
    #[test]
    #[ignore]
    fn real_winget_version() {
        let v = version().unwrap();
        assert!(v.starts_with('v'), "{v}");
    }

    #[test]
    fn private_dir_is_new_each_time_and_cleaned() {
        let a = private_dir().unwrap();
        let b = private_dir().unwrap();
        assert_ne!(a, b);
        assert!(a.is_dir() && b.is_dir());
        std::fs::remove_dir_all(&a).unwrap();
        std::fs::remove_dir_all(&b).unwrap();
    }

    /// La versión protegida no deja escribir a un usuario sin admin (lo que haría un
    /// programa malicioso del usuario contra la app elevada).
    #[test]
    fn protected_dir_rejects_non_admin_writes() {
        if crate::system::is_elevated() {
            return; // como admin sí se puede escribir: la prueba solo tiene sentido sin admin
        }
        let d = private_dir_with(true).unwrap();
        let err = std::fs::write(d.join("plantado.txt"), b"x").unwrap_err();
        // Limpieza: ni quien la creó puede borrarla sin admin; como dueño, se le devuelven
        // los permisos heredados y recién ahí se borra.
        let _ = Command::new("icacls").arg(&d).arg("/reset").stdout(Stdio::null()).status();
        let cleaned = std::fs::remove_dir(&d);
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        cleaned.unwrap();
    }

    /// Lista real de instalados con la carpeta privada (solo lectura).
    #[test]
    #[ignore]
    fn real_installed_list() {
        let m = installed().unwrap();
        assert!(!m.is_empty());
    }

    #[test]
    fn nombres_registrados() {
        assert!(title_matches("Brave", "Brave"));
        assert!(title_matches("VLC", "VLC media player"));
        assert!(title_matches("7-Zip", "7-Zip 24.08 (x64)"));
        assert!(title_matches("Notepad++", "Notepad++ (64-bit x64)"));
        assert!(title_matches("Node.js LTS", "Node.js"));
        assert!(title_matches("PowerShell 7", "PowerShell 7-x64"));
        assert!(title_matches("Visual Studio Code", "Microsoft Visual Studio Code (User)"));
        assert!(!title_matches("Git", "GitHub Desktop"));
        assert!(!title_matches("Zoom", "Zoomit"));
        assert!(!title_matches("Brave", ""));
    }

    #[test]
    fn fila_de_winget_list() {
        let lines = |s: &str| s.lines().map(String::from).collect::<Vec<_>>();
        let plain = lines("Name  Id          Version\n-----------------------------\nBrave Brave.Brave 157.1.99.55");
        assert_eq!(parse_list_row(&plain, "Brave.Brave"), Some(("157.1.99.55".into(), None)));
        let upd = lines("Nombre Id Versión Disponible\n----\nBrave Brave.Brave 157.1.99.55 157.1.100.2");
        assert_eq!(parse_list_row(&upd, "brave.brave"), Some(("157.1.99.55".into(), Some("157.1.100.2".into()))));
        let src = lines("Brave Brave.Brave 1.0 winget");
        assert_eq!(parse_list_row(&src, "Brave.Brave"), Some(("1.0".into(), None)));
        let approx = lines("Foo Foo.Bar < 3.1 3.2");
        assert_eq!(parse_list_row(&approx, "Foo.Bar"), Some(("< 3.1".into(), Some("3.2".into()))));
        assert_eq!(parse_list_row(&lines("No se encontró ningún paquete instalado"), "Brave.Brave"), None);
    }

    /// En una PC con Brave instalado desde su web: `export` no lo trae y el complemento sí.
    #[test]
    #[ignore]
    fn real_brave_fuera_de_export() {
        let mut inst = installed().unwrap();
        let mut upg = HashMap::new();
        println!("export trae brave: {}", inst.contains_key("brave.brave"));
        add_unmatched(&mut inst, &mut upg);
        println!("después: {:?} / actualización: {:?}", inst.get("brave.brave"), upg.get("brave.brave"));
        assert!(inst.contains_key("brave.brave"));
    }

    /// winget se busca en el paquete protegido, nunca en la carpeta del usuario.
    #[test]
    fn winget_comes_from_protected_program_files() {
        let p = winget_path().unwrap().to_string_lossy().to_ascii_lowercase();
        assert!(p.contains(r"\windowsapps\microsoft.desktopappinstaller_"), "{p}");
        assert!(!p.contains(r"\appdata\"), "{p}");
    }

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
