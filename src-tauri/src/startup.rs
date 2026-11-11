//! Inicio de Windows: lo que arranca solo al iniciar sesión.
//!
//! Desactivar nunca borra nada. Para las entradas `Run` del registro y la carpeta Inicio se
//! usa el mismo mecanismo que el Administrador de tareas y Configuración
//! (`Explorer\StartupApproved`), así que el cambio se ve y se revierte también desde ahí.
//! Las tareas programadas que arrancan al iniciar sesión o con el equipo se marcan como
//! deshabilitadas en el Programador de tareas. Las de Windows (carpeta `\Microsoft` o que
//! ejecutan algo dentro de la carpeta de Windows) no se listan ni se tocan.

use crate::catalog::Hive;
use crate::com::{variant_i4, Com};
use crate::registry::{self as reg, RawValue};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use windows::core::{Interface, BSTR, HSTRING, PCWSTR};
use windows::Win32::Foundation::{VARIANT_BOOL, VARIANT_FALSE, VARIANT_TRUE};
use windows::Win32::System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, STGM_READ};
use windows::Win32::System::TaskScheduler::{
    IExecAction, IRegisteredTask, ITaskFolder, ITaskService, TaskScheduler, TASK_ACTION_EXEC, TASK_ENUM_HIDDEN,
    TASK_TRIGGER_BOOT, TASK_TRIGGER_LOGON, TASK_TRIGGER_TYPE2,
};
use windows::Win32::System::Variant::VARIANT;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// `HKCU\...\Run`: solo esta cuenta.
    RunUser,
    /// `HKLM\...\Run`: todas las cuentas.
    RunMachine,
    /// `HKLM\...\WOW6432Node\...\Run`: programas de 32 bits, todas las cuentas.
    RunMachine32,
    /// Carpeta Inicio de esta cuenta.
    FolderUser,
    /// Carpeta Inicio común (todas las cuentas).
    FolderMachine,
    /// Tarea programada al iniciar sesión o con el equipo.
    Task,
}

const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

impl Source {
    fn run_key(self) -> Option<(Hive, &'static str)> {
        match self {
            Source::RunUser => Some((Hive::HKCU, r"Software\Microsoft\Windows\CurrentVersion\Run")),
            Source::RunMachine => Some((Hive::HKLM, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run")),
            Source::RunMachine32 => Some((Hive::HKLM, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run")),
            _ => None,
        }
    }

    /// Dónde guarda Windows si está habilitada (no aplica a tareas).
    fn approved(self) -> Option<(Hive, String)> {
        let (hive, sub) = match self {
            Source::RunUser => (Hive::HKCU, "Run"),
            Source::RunMachine => (Hive::HKLM, "Run"),
            Source::RunMachine32 => (Hive::HKLM, "Run32"),
            Source::FolderUser => (Hive::HKCU, "StartupFolder"),
            Source::FolderMachine => (Hive::HKLM, "StartupFolder"),
            Source::Task => return None,
        };
        Some((hive, format!(r"{APPROVED}\{sub}")))
    }

    pub fn per_user(self) -> bool {
        matches!(self, Source::RunUser | Source::FolderUser)
    }

    pub fn requires_admin(self) -> bool {
        !self.per_user()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StartupItem {
    pub source: Source,
    /// Identifica la entrada dentro de su origen: nombre del valor, del archivo o ruta de la tarea.
    pub name: String,
    pub title: String,
    pub command: String,
    /// Ejecutable que arranca, si se pudo determinar.
    pub path: Option<String>,
    /// El ejecutable existe.
    pub exists: bool,
    /// Firmante (Authenticode) válido, si tiene.
    pub publisher: Option<String>,
    /// Es parte de Windows (está dentro de la carpeta de Windows).
    pub system: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartupStatus {
    pub items: Vec<StartupItem>,
    pub elevated: bool,
    /// La app corre con otra cuenta: lo de "esta cuenta" no se muestra.
    pub foreign_user: Option<String>,
}

/// Cambio guardado en el historial.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupChange {
    pub source: Source,
    pub name: String,
    pub title: String,
    pub enabled_before: bool,
    pub enabled_after: bool,
    /// Valor de `StartupApproved` antes del cambio (None = no existía). No aplica a tareas.
    #[serde(default)]
    pub raw_before: Option<RawValue>,
}

const MAX_NAME: usize = 520;

// ───────────────────────── Estado "habilitada" (StartupApproved) ─────────────────────────

/// Sin valor = habilitada. Windows usa 02 (habilitada) y 03 (deshabilitada) en el primer
/// byte, seguido de la fecha del cambio; un primer byte impar es "deshabilitada".
fn approved_enabled(v: Option<&RawValue>) -> bool {
    match v {
        Some(r) if r.vtype == 3 && !r.bytes.is_empty() => r.bytes[0] & 1 == 0,
        _ => true,
    }
}

fn approved_value(enabled: bool) -> RawValue {
    let mut bytes = vec![if enabled { 2 } else { 3 }, 0, 0, 0];
    // Igual que el Administrador de tareas: la fecha del cambio (FILETIME) al deshabilitar.
    let ft = if enabled { 0u64 } else { (crate::com::now_secs() + 11_644_473_600) * 10_000_000 };
    bytes.extend_from_slice(&ft.to_le_bytes());
    RawValue { vtype: 3, bytes }
}

fn read_approved(source: Source, name: &str) -> Result<Option<RawValue>, String> {
    let (h, key) = source.approved().ok_or("sin estado")?;
    reg::read(h, &key, name).map_err(|e| e.to_string())
}

// ───────────────────────── Rutas ─────────────────────────

fn known_folder(id: &windows::core::GUID) -> Option<PathBuf> {
    use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as _));
        s.map(PathBuf::from)
    }
}

fn startup_folder(source: Source) -> Option<PathBuf> {
    use windows::Win32::UI::Shell::{FOLDERID_CommonStartup, FOLDERID_Startup};
    match source {
        Source::FolderUser => known_folder(&FOLDERID_Startup),
        Source::FolderMachine => known_folder(&FOLDERID_CommonStartup),
        _ => None,
    }
}

pub fn windows_dir() -> Option<PathBuf> {
    use windows::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;
    let mut buf = [0u16; 260];
    let n = unsafe { GetSystemWindowsDirectoryW(Some(&mut buf)) } as usize;
    (n > 0 && n < buf.len()).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..n])))
}

fn expand_env(s: &str) -> String {
    use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
    let src = HSTRING::from(s);
    let mut buf = vec![0u16; 2048];
    let n = unsafe { ExpandEnvironmentStringsW(PCWSTR(src.as_ptr()), Some(&mut buf)) } as usize;
    if n == 0 || n > buf.len() {
        return s.to_string();
    }
    String::from_utf16_lossy(&buf[..n.saturating_sub(1)])
}

/// El ejecutable de una línea de comandos: entre comillas, o lo más largo que termine en
/// `.exe` y exista (las rutas sin comillas pueden tener espacios).
pub fn exe_from_command(cmd: &str) -> Option<PathBuf> {
    let cmd = expand_env(cmd.trim());
    let path = if let Some(rest) = cmd.strip_prefix('"') {
        PathBuf::from(rest.split('"').next()?)
    } else {
        let lower = cmd.to_ascii_lowercase();
        let found = lower
            .match_indices(".exe")
            .map(|(i, _)| &cmd[..i + 4])
            .find(|p| Path::new(p).is_file());
        PathBuf::from(found.or_else(|| cmd.split_whitespace().next())?)
    };
    if path.as_os_str().is_empty() {
        return None;
    }
    if path.is_relative() {
        // "rundll32.exe ..." y similares: se buscan en System32.
        let sys = windows_dir()?.join("System32").join(&path);
        return Some(if sys.is_file() { sys } else { path });
    }
    Some(path)
}

/// Programa de Windows: directo en la carpeta de Windows o en una de sus carpetas de sistema,
/// donde solo escriben los administradores y TrustedInstaller. Se compara por componentes:
/// `C:\Windows\System32\..\Temp\x.exe` o `C:\Windows\Temp\x.exe` (donde cualquiera escribe)
/// no cuentan, para que nada se esconda de la lista haciéndose pasar por Windows.
fn inside_windows(p: &Path) -> bool {
    use std::path::Component;
    const SYSTEM_DIRS: [&str; 5] = ["system32", "syswow64", "systemapps", "immersivecontrolpanel", "winsxs"];
    let Some(win) = windows_dir() else { return false };
    if p.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return false;
    }
    let lower = |c: Component| c.as_os_str().to_string_lossy().to_ascii_lowercase();
    let win: Vec<String> = win.components().map(lower).collect();
    let path: Vec<String> = p.components().map(lower).collect();
    if path.len() <= win.len() || path[..win.len()] != win[..] {
        return false;
    }
    let rest = &path[win.len()..];
    rest.len() == 1 || SYSTEM_DIRS.contains(&rest[0].as_str())
}

/// Destino de un acceso directo (.lnk).
fn shortcut_target(lnk: &Path) -> Option<(PathBuf, String)> {
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        link.cast::<IPersistFile>().ok()?.Load(&HSTRING::from(lnk.as_os_str()), STGM_READ).ok()?;
        let mut path = [0u16; 1024];
        link.GetPath(&mut path, std::ptr::null_mut(), 0).ok()?;
        let mut args = [0u16; 1024];
        let _ = link.GetArguments(&mut args);
        let s = |b: &[u16]| String::from_utf16_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]);
        let target = s(&path);
        (!target.is_empty()).then(|| (PathBuf::from(&target), s(&args)))
    }
}

fn describe_exe(path: Option<PathBuf>, sign: bool) -> (Option<String>, bool, Option<String>, bool) {
    match path {
        Some(p) => {
            let exists = p.is_file();
            let publisher = (exists && sign).then(|| crate::sign::signer(&p).ok()).flatten();
            let system = inside_windows(&p);
            (Some(p.to_string_lossy().into_owned()), exists, publisher, system)
        }
        None => (None, false, None, false),
    }
}

// ───────────────────────── Listado ─────────────────────────

fn run_items(source: Source, sign: bool, out: &mut Vec<StartupItem>) {
    let Some((h, key)) = source.run_key() else { return };
    let Ok(values) = reg::values(h, key) else { return };
    for (name, v) in values {
        let Some(command) = reg::as_text(&v) else { continue };
        if name.is_empty() || command.trim().is_empty() {
            continue;
        }
        let enabled = approved_enabled(read_approved(source, &name).ok().flatten().as_ref());
        let (path, exists, publisher, system) = describe_exe(exe_from_command(&command), sign);
        out.push(StartupItem { source, title: name.clone(), name, command, path, exists, publisher, system, enabled });
    }
}

fn folder_items(source: Source, sign: bool, out: &mut Vec<StartupItem>) {
    let Some(dir) = startup_folder(source) else { return };
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if !p.is_file() || name.eq_ignore_ascii_case("desktop.ini") {
            continue;
        }
        let enabled = approved_enabled(read_approved(source, &name).ok().flatten().as_ref());
        let is_lnk = p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk"));
        let (target, command) = if is_lnk {
            match shortcut_target(&p) {
                Some((t, args)) => (Some(t.clone()), format!("{} {args}", t.display()).trim().to_string()),
                None => (None, p.display().to_string()),
            }
        } else {
            (Some(p.clone()), p.display().to_string())
        };
        let title = p.file_stem().map_or(name.clone(), |s| s.to_string_lossy().into_owned());
        let (path, exists, publisher, system) = describe_exe(target, sign);
        out.push(StartupItem { source, name, title, command, path, exists, publisher, system, enabled });
    }
}

struct TaskInfo {
    path: String,
    name: String,
    exe: String,
    args: String,
    enabled: bool,
}

fn task_service() -> windows::core::Result<ITaskService> {
    unsafe {
        let svc: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
        let empty = VARIANT::default();
        svc.Connect(&empty, &empty, &empty, &empty)?;
        Ok(svc)
    }
}

fn is_windows_task_path(path: &str) -> bool {
    path.to_ascii_lowercase().starts_with(r"\microsoft\")
}

/// Datos de la tarea si es de las que listamos: arranca al iniciar sesión o con el equipo,
/// ejecuta un programa y no es de Windows.
fn describe_task(t: &IRegisteredTask) -> Option<TaskInfo> {
    unsafe {
        let path = t.Path().ok()?.to_string();
        if is_windows_task_path(&path) {
            return None;
        }
        let def = t.Definition().ok()?;
        let triggers = def.Triggers().ok()?;
        let mut n = 0;
        triggers.Count(&mut n).ok()?;
        let at_start = (1..=n).any(|i| {
            let mut ty = TASK_TRIGGER_TYPE2::default();
            triggers.get_Item(i).and_then(|tr| tr.Type(&mut ty)).is_ok() && (ty == TASK_TRIGGER_LOGON || ty == TASK_TRIGGER_BOOT)
        });
        if !at_start {
            return None;
        }
        let actions = def.Actions().ok()?;
        let mut count = 0;
        actions.Count(&mut count).ok()?;
        let mut exe = None;
        for i in 1..=count {
            let Ok(a) = actions.get_Item(i) else { continue };
            let mut ty = Default::default();
            if a.Type(&mut ty).is_err() || ty != TASK_ACTION_EXEC {
                continue;
            }
            let Ok(x) = a.cast::<IExecAction>() else { continue };
            let (mut p, mut args) = (BSTR::new(), BSTR::new());
            if x.Path(&mut p).is_ok() {
                let _ = x.Arguments(&mut args);
                exe = Some((p.to_string(), args.to_string()));
                break;
            }
        }
        let (exe, args) = exe?;
        let resolved = exe_from_command(&format!("\"{}\"", exe.trim_matches('"')));
        if resolved.as_deref().is_some_and(inside_windows) {
            return None;
        }
        Some(TaskInfo {
            name: t.Name().ok()?.to_string(),
            path,
            exe,
            args,
            enabled: t.Enabled().ok()? != VARIANT_FALSE,
        })
    }
}

fn walk(folder: &ITaskFolder, depth: u32, out: &mut Vec<TaskInfo>) {
    unsafe {
        let path = folder.Path().map(|p| p.to_string()).unwrap_or_default();
        if depth > 8 || is_windows_task_path(&format!("{path}\\")) {
            return;
        }
        if let Ok(tasks) = folder.GetTasks(TASK_ENUM_HIDDEN.0) {
            for i in 1..=tasks.Count().unwrap_or(0) {
                if let Some(info) = tasks.get_Item(&variant_i4(i)).ok().and_then(|t| describe_task(&t)) {
                    out.push(info);
                }
            }
        }
        if let Ok(subs) = folder.GetFolders(0) {
            for i in 1..=subs.Count().unwrap_or(0) {
                if let Ok(f) = subs.get_Item(&variant_i4(i)) {
                    walk(&f, depth + 1, out);
                }
            }
        }
    }
}

fn task_items(sign: bool, out: &mut Vec<StartupItem>) {
    let Ok(svc) = task_service() else { return };
    let Ok(root) = (unsafe { svc.GetFolder(&BSTR::from("\\")) }) else { return };
    let mut tasks = Vec::new();
    walk(&root, 0, &mut tasks);
    for t in tasks {
        let command = format!("{} {}", t.exe, t.args).trim().to_string();
        let (path, exists, publisher, system) = describe_exe(exe_from_command(&format!("\"{}\"", t.exe.trim_matches('"'))), sign);
        out.push(StartupItem {
            source: Source::Task,
            name: t.path,
            title: t.name,
            command,
            path,
            exists,
            publisher,
            system,
            enabled: t.enabled,
        });
    }
}

/// Lo que arranca desde un origen. `sign`: verificar firmas (lento, solo para mostrar).
fn collect(source: Source, sign: bool, out: &mut Vec<StartupItem>) {
    match source {
        Source::RunUser | Source::RunMachine | Source::RunMachine32 => run_items(source, sign, out),
        Source::FolderUser | Source::FolderMachine => folder_items(source, sign, out),
        Source::Task => task_items(sign, out),
    }
}

pub fn status() -> StartupStatus {
    let _com = Com::init(COINIT_MULTITHREADED);
    let foreign_user = crate::system::foreign_user();
    let mut items = Vec::new();
    for source in [Source::RunUser, Source::FolderUser, Source::RunMachine, Source::RunMachine32, Source::FolderMachine, Source::Task] {
        if !(source.per_user() && foreign_user.is_some()) {
            collect(source, true, &mut items);
        }
    }
    items.sort_by(|a, b| b.enabled.cmp(&a.enabled).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())));
    StartupStatus { items, elevated: crate::system::is_elevated(), foreign_user }
}

// ───────────────────────── Cambios ─────────────────────────

fn guard(source: Source) -> Result<(), String> {
    if source.requires_admin() && !crate::system::is_elevated() {
        return Err(t!("requires_admin"));
    }
    if source.per_user() {
        if let Some(user) = crate::system::foreign_user() {
            return Err(t!("foreign_user_apply", user = user));
        }
    }
    Ok(())
}

fn find_task(path: &str) -> Result<(IRegisteredTask, TaskInfo), String> {
    let svc = task_service().map_err(|e| e.message())?;
    unsafe {
        let task = svc
            .GetFolder(&BSTR::from("\\"))
            .and_then(|root| root.GetTask(&BSTR::from(path)))
            .map_err(|_| t!("startup_missing"))?;
        // Solo las tareas que la app lista: nunca una de Windows ni una que no sea de inicio.
        let info = describe_task(&task).ok_or_else(|| t!("startup_not_eligible"))?;
        Ok((task, info))
    }
}

fn set_task(path: &str, enabled: bool) -> Result<(), String> {
    let (task, _) = find_task(path)?;
    let v: VARIANT_BOOL = if enabled { VARIANT_TRUE } else { VARIANT_FALSE };
    unsafe { task.SetEnabled(v) }.map_err(|e| if e.code().0 as u32 == 0x8007_0005 { t!("requires_admin") } else { e.message() })
}

fn current_task(path: &str) -> Option<bool> {
    find_task(path).ok().map(|(_, i)| i.enabled)
}

/// Habilita o deshabilita una entrada que existe hoy. Devuelve None si ya estaba así.
pub fn set(source: Source, name: &str, enabled: bool) -> Result<Option<StartupChange>, String> {
    guard(source)?;
    let _com = Com::init(COINIT_MULTITHREADED);
    let mut items = Vec::new();
    collect(source, false, &mut items);
    let item = items
        .into_iter()
        .find(|i| i.source == source && i.name == name)
        .ok_or_else(|| t!("startup_missing"))?;
    if item.enabled == enabled {
        return Ok(None);
    }
    let raw_before = if source == Source::Task {
        set_task(name, enabled)?;
        None
    } else {
        let before = read_approved(source, name)?;
        let (h, key) = source.approved().expect("origen con estado");
        reg::write_raw(h, &key, name, &approved_value(enabled)).map_err(|e| e.to_string())?;
        before
    };
    Ok(Some(StartupChange {
        source,
        name: name.to_string(),
        title: item.title,
        enabled_before: item.enabled,
        enabled_after: enabled,
        raw_before,
    }))
}

/// Estado actual de lo que tocó un cambio (None si ya no se puede leer).
pub fn current(c: &StartupChange) -> Option<bool> {
    if c.source == Source::Task {
        let _com = Com::init(COINIT_MULTITHREADED);
        return current_task(&c.name);
    }
    read_approved(c.source, &c.name).ok().map(|v| approved_enabled(v.as_ref()))
}

/// El historial lo puede escribir el usuario y deshacer corre como admin: solo se acepta
/// un valor de estado con la forma de los de Windows, y tareas que la app listaría.
pub fn validate_saved(c: &StartupChange) -> Result<(), String> {
    if c.name.is_empty() || c.name.len() > MAX_NAME || c.name.contains('\0') {
        return Err(t!("startup_bad_entry"));
    }
    if let Some(r) = &c.raw_before {
        if c.source == Source::Task || r.vtype != 3 || r.bytes.is_empty() || r.bytes.len() > 16 {
            return Err(t!("startup_bad_entry"));
        }
    }
    guard(c.source)?;
    if c.source == Source::Task {
        let _com = Com::init(COINIT_MULTITHREADED);
        find_task(&c.name)?;
    }
    Ok(())
}

pub fn restore(c: &StartupChange) -> Result<(), String> {
    if c.source == Source::Task {
        let _com = Com::init(COINIT_MULTITHREADED);
        return set_task(&c.name, c.enabled_before);
    }
    let (h, key) = c.source.approved().ok_or_else(|| t!("startup_bad_entry"))?;
    match &c.raw_before {
        Some(v) => reg::write_raw(h, &key, &c.name, v),
        None => reg::delete_value(h, &key, &c.name),
    }
    .map_err(|e| e.to_string())
}

pub fn label(c: &StartupChange) -> String {
    if c.enabled_after {
        t!("label_startup_on", name = c.title)
    } else {
        t!("label_startup_off", name = c.title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approved_values() {
        assert!(approved_enabled(None));
        assert!(!approved_enabled(Some(&approved_value(false))));
        assert!(approved_enabled(Some(&approved_value(true))));
        assert_eq!(approved_value(true).bytes.len(), 12);
        assert_eq!(approved_value(false).bytes[0], 3);
        // Otro tipo de valor: Windows lo ignora, cuenta como habilitada.
        assert!(approved_enabled(Some(&RawValue { vtype: 4, bytes: vec![3, 0, 0, 0] })));
    }

    #[test]
    fn exe_from_commands() {
        let win = windows_dir().unwrap();
        let notepad = win.join(r"System32\notepad.exe");
        assert_eq!(exe_from_command(&format!("\"{}\" /x", notepad.display())).unwrap(), notepad);
        assert_eq!(exe_from_command(&format!("{} /x", notepad.display())).unwrap(), notepad);
        assert_eq!(exe_from_command(r"%windir%\System32\notepad.exe").unwrap().to_string_lossy().to_lowercase(), notepad.to_string_lossy().to_lowercase());
        assert!(inside_windows(&exe_from_command("rundll32.exe shell32.dll,Foo").unwrap()));
        assert!(inside_windows(&win.join("explorer.exe")));
        // Carpetas de Windows donde cualquiera escribe, o rutas con "..": no son de Windows.
        assert!(!inside_windows(&win.join(r"Temp\x.exe")));
        assert!(!inside_windows(&win.join(r"Tasks\x.exe")));
        assert!(!inside_windows(&win.join(r"System32\..\Temp\x.exe")));
        assert!(!inside_windows(Path::new(r"C:\WindowsApps\x.exe")));
        let fwd = win.join("System32").join("notepad.exe").to_string_lossy().replace('\\', "/");
        assert!(inside_windows(Path::new(&fwd)), "barras normales siguen siendo la misma ruta");
        assert!(exe_from_command("   ").is_none());
    }

    #[test]
    fn tampered_entries_are_rejected() {
        let bad = |source, name: &str, raw: Option<RawValue>| StartupChange {
            source,
            name: name.into(),
            title: "x".into(),
            enabled_before: true,
            enabled_after: false,
            raw_before: raw,
        };
        assert!(validate_saved(&bad(Source::RunUser, "", None)).is_err());
        assert!(validate_saved(&bad(Source::RunUser, "x", Some(RawValue { vtype: 1, bytes: vec![1, 0] }))).is_err());
        assert!(validate_saved(&bad(Source::RunUser, "x", Some(RawValue { vtype: 3, bytes: vec![0; 64] }))).is_err());
        assert!(validate_saved(&bad(Source::Task, r"\Microsoft\Windows\Defrag\ScheduledDefrag", None)).is_err());
    }

    /// Crea una entrada de prueba en HKCU\...\Run (apunta a un programa que no existe), la
    /// desactiva, la restaura y la borra. Correr con `cargo test -- --ignored alternar`.
    #[test]
    #[ignore]
    fn alternar_y_restaurar() {
        const NAME: &str = "P-Rune prueba";
        let (h, run) = Source::RunUser.run_key().unwrap();
        let (ah, approved) = Source::RunUser.approved().unwrap();
        let cmd = reg::encode(crate::catalog::ValueKind::Sz, &crate::catalog::Value::Str(r"C:\no-existe\p-rune-prueba.exe".into()));
        reg::write_raw(h, run, NAME, &cmd).unwrap();
        let cleanup = || {
            let _ = reg::delete_value(h, run, NAME);
            let _ = reg::delete_value(ah, &approved, NAME);
        };
        let r = std::panic::catch_unwind(|| {
            let c = set(Source::RunUser, NAME, false).unwrap().expect("tenía que cambiar");
            assert!(c.enabled_before && !c.enabled_after && c.raw_before.is_none());
            assert_eq!(current(&c), Some(false));
            let listed = status().items.into_iter().find(|i| i.name == NAME).unwrap();
            assert!(!listed.enabled && !listed.exists);
            assert!(set(Source::RunUser, NAME, false).unwrap().is_none(), "ya estaba desactivada");
            validate_saved(&c).unwrap();
            restore(&c).unwrap();
            assert_eq!(reg::read(ah, &approved, NAME).unwrap(), None, "volvió exacto: sin valor");
            assert_eq!(current(&c), Some(true));
        });
        cleanup();
        r.unwrap();
    }

    /// Lista lo que arranca en esta PC (no cambia nada).
    #[test]
    #[ignore]
    fn listado_real() {
        for i in status().items {
            println!("{:?} {} | {} | {:?} | {}", i.source, i.title, i.command, i.publisher, if i.enabled { "on" } else { "off" });
        }
    }
}
