//! Limpieza de archivos descartables (pestaña Disco).
//!
//! Corre como administrador y borra dentro de carpetas donde el usuario (o cualquier
//! programa suyo) puede escribir, así que cada paso está pensado para no borrar nunca
//! fuera de lugar:
//!
//! - Las carpetas son fijas (no vienen de la interfaz) y se resuelven a su ruta real
//!   (siguiendo enlaces). Tienen que quedar dentro del perfil del usuario, de `C:\Windows`
//!   o de `ProgramData`; si `%TEMP%` apunta a otro lado o la carpeta es un enlace a
//!   `System32`, la categoría se descarta.
//! - Nunca se entra en enlaces ni uniones: se cuentan como 0 y no se tocan.
//! - Se borra el contenido, nunca la carpeta raíz. Para subcarpetas se usa
//!   `std::fs::remove_dir_all`, que en Windows no sigue enlaces aunque los cambien a mitad
//!   de camino (CVE-2022-21658).
//! - Lo reciente se respeta (instalaciones en curso usan `%TEMP%`) y lo que está en uso
//!   simplemente falla y se saltea.
//! - El espacio liberado es el tamaño antes menos el tamaño después: no se estima.

use serde::Serialize;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const DAY: Duration = Duration::from_secs(24 * 3600);
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_ATTRIBUTE_READONLY: u32 = 0x1;

/// Advertencia que la interfaz muestra junto a la categoría.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Warn {
    /// Prefetch: Windows lo reconstruye, pero los primeros arranques son más lentos.
    SlowerStart,
    /// Papelera: no se puede recuperar.
    Permanent,
    /// Spotify: hay que cerrarlo; lo descargado sin conexión se vuelve a bajar.
    CloseApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Base {
    /// Perfil del usuario (incluye %TEMP% y %LOCALAPPDATA%).
    Profile,
    Windows,
    ProgramData,
}

/// Una carpeta a vaciar.
struct Dir {
    path: PathBuf,
    base: Base,
    /// Solo entradas sin cambios en este tiempo (las carpetas, por su archivo más nuevo).
    min_age: Duration,
    /// Si está, solo se borran archivos sueltos cuyo nombre cumple esto (no carpetas).
    only: Option<fn(&str) -> bool>,
}

struct Category {
    id: &'static str,
    /// Hace falta admin (carpetas de Windows o de ProgramData).
    admin: bool,
    /// Usa carpetas del usuario: si la app corre con otra cuenta, serían las de esa cuenta.
    per_user: bool,
    warn: Option<Warn>,
    recycle_bin: bool,
}

const CATEGORIES: &[Category] = &[
    Category { id: "temp_user", admin: false, per_user: true, warn: None, recycle_bin: false },
    Category { id: "temp_system", admin: true, per_user: false, warn: None, recycle_bin: false },
    Category { id: "crash", admin: true, per_user: true, warn: None, recycle_bin: false },
    Category { id: "shaders", admin: false, per_user: true, warn: None, recycle_bin: false },
    Category { id: "thumbnails", admin: false, per_user: true, warn: None, recycle_bin: false },
    Category { id: "update_cache", admin: true, per_user: false, warn: None, recycle_bin: false },
    Category { id: "spotify", admin: false, per_user: true, warn: Some(Warn::CloseApp), recycle_bin: false },
    Category { id: "prefetch", admin: true, per_user: false, warn: Some(Warn::SlowerStart), recycle_bin: false },
    Category { id: "recycle_bin", admin: false, per_user: true, warn: Some(Warn::Permanent), recycle_bin: true },
];

fn known(id: &windows::core::GUID) -> Option<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{KF_FLAG_DEFAULT, SHGetKnownFolderPath};
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        s.map(PathBuf::from)
    }
}

fn base_path(b: Base) -> Option<PathBuf> {
    use windows::Win32::UI::Shell::{FOLDERID_ProgramData, FOLDERID_Profile, FOLDERID_Windows};
    match b {
        Base::Profile => known(&FOLDERID_Profile),
        Base::Windows => known(&FOLDERID_Windows),
        Base::ProgramData => known(&FOLDERID_ProgramData),
    }
}

fn dirs(id: &str) -> Vec<Dir> {
    use windows::Win32::UI::Shell::FOLDERID_LocalAppData;
    let local = known(&FOLDERID_LocalAppData);
    let win = base_path(Base::Windows);
    let pd = base_path(Base::ProgramData);
    let mut out = Vec::new();
    let mut add = |p: Option<PathBuf>, base: Base, min_age: Duration, only: Option<fn(&str) -> bool>| {
        if let Some(path) = p {
            out.push(Dir { path, base, min_age, only });
        }
    };
    let l = |rel: &str| local.as_ref().map(|p| p.join(rel));
    match id {
        "temp_user" => add(Some(std::env::temp_dir()), Base::Profile, DAY, None),
        "temp_system" => add(win.as_ref().map(|w| w.join("Temp")), Base::Windows, DAY, None),
        "crash" => {
            add(win.as_ref().map(|w| w.join("Minidump")), Base::Windows, Duration::ZERO, None);
            add(win.clone(), Base::Windows, Duration::ZERO, Some(|n| n.eq_ignore_ascii_case("MEMORY.DMP")));
            add(l("CrashDumps"), Base::Profile, Duration::ZERO, None);
            for sub in [r"Microsoft\Windows\WER\ReportArchive", r"Microsoft\Windows\WER\ReportQueue"] {
                add(pd.as_ref().map(|p| p.join(sub)), Base::ProgramData, Duration::ZERO, None);
                add(l(sub), Base::Profile, Duration::ZERO, None);
            }
        }
        "shaders" => {
            for sub in ["D3DSCache", r"NVIDIA\DXCache", r"NVIDIA\GLCache", r"AMD\DxCache", r"AMD\GLCache"] {
                add(l(sub), Base::Profile, Duration::ZERO, None);
            }
        }
        "thumbnails" => add(
            l(r"Microsoft\Windows\Explorer"),
            Base::Profile,
            Duration::ZERO,
            Some(|n| {
                let n = n.to_ascii_lowercase();
                n.starts_with("thumbcache_") && n.ends_with(".db")
            }),
        ),
        // Lo ya instalado; lo de la última semana puede ser una actualización en curso.
        "update_cache" => add(win.as_ref().map(|w| w.join(r"SoftwareDistribution\Download")), Base::Windows, 7 * DAY, None),
        "spotify" => {
            for sub in [r"Spotify\Data", r"Spotify\Storage", r"Packages\SpotifyAB.SpotifyMusic_zpdnekdrzrea0\LocalCache\Spotify\Data"] {
                add(l(sub), Base::Profile, Duration::ZERO, None);
            }
        }
        "prefetch" => add(
            win.as_ref().map(|w| w.join("Prefetch")),
            Base::Windows,
            Duration::ZERO,
            Some(|n| n.to_ascii_lowercase().ends_with(".pf")),
        ),
        _ => {}
    }
    out
}

/// Ruta real de la carpeta, solo si existe, no es un enlace hacia afuera y queda dentro
/// de su base (y no es la base misma).
fn safe_root(d: &Dir) -> Option<PathBuf> {
    let base = std::fs::canonicalize(base_path(d.base)?).ok()?;
    // Ningún enlace ni unión entre la base y la carpeta (ella incluida): si alguien puso una
    // unión hacia otra carpeta, aunque quede dentro de la misma base, se descarta.
    let mut reached = false;
    for p in d.path.ancestors() {
        let m = std::fs::symlink_metadata(p).ok()?;
        if is_reparse(&m) {
            return None;
        }
        if std::fs::canonicalize(p).ok().as_ref() == Some(&base) {
            reached = true;
            break;
        }
    }
    if !reached {
        return None;
    }
    let real = std::fs::canonicalize(&d.path).ok()?;
    let inside = real.starts_with(&base) && (real != base || d.only.is_some());
    (inside && real.is_dir()).then_some(real)
}

/// Carpeta abierta durante toda la limpieza. Se abre sin permitir borrado ni renombrado
/// (sin `FILE_SHARE_DELETE`), así nadie la puede cambiar por una unión hasta que termine,
/// y se confirma desde el propio handle que es la carpeta esperada y no un enlace.
struct Pinned(windows::Win32::Foundation::HANDLE);

impl Drop for Pinned {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

fn pin(dir: &Path) -> Option<Pinned> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_LIST_DIRECTORY, FILE_NAME_NORMALIZED, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle,
        GetFinalPathNameByHandleW, OPEN_EXISTING,
    };
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    let wide: Vec<u16> = dir.as_os_str().encode_wide_null();
    let h = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .ok()?;
    let pinned = Pinned(h);
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(h, &mut info) }.ok()?;
    let attrs = info.dwFileAttributes;
    if attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0 || attrs & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return None;
    }
    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetFinalPathNameByHandleW(h, &mut buf, FILE_NAME_NORMALIZED) } as usize;
    if n == 0 || n >= buf.len() {
        return None;
    }
    let final_path = String::from_utf16_lossy(&buf[..n]);
    final_path.eq_ignore_ascii_case(&dir.to_string_lossy()).then_some(pinned)
}

trait WideNull {
    fn encode_wide_null(&self) -> Vec<u16>;
}

impl WideNull for std::ffi::OsStr {
    fn encode_wide_null(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().chain(std::iter::once(0)).collect()
    }
}

/// Corre `f` con los permisos de usuario común (sin el grupo Administradores ni privilegios),
/// para las carpetas del perfil: así, pase lo que pase con la ruta, no se puede borrar nada
/// que el usuario no pudiera borrar por su cuenta. Sin elevación, corre tal cual.
fn as_user<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    if !crate::system::is_elevated() {
        return Ok(f());
    }
    let token = restricted_impersonation_token()?;
    struct Revert;
    impl Drop for Revert {
        fn drop(&mut self) {
            unsafe {
                let _ = windows::Win32::Security::RevertToSelf();
            }
        }
    }
    unsafe { windows::Win32::System::Threading::SetThreadToken(None, Some(token.0)) }.map_err(|e| e.message())?;
    let _revert = Revert;
    Ok(f())
}

fn restricted_impersonation_token() -> Result<Pinned, String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::{
        CreateRestrictedToken, CreateWellKnownSid, DISABLE_MAX_PRIVILEGE, DuplicateTokenEx, PSID, SecurityImpersonation,
        SID_AND_ATTRIBUTES, TOKEN_ALL_ACCESS, TOKEN_DUPLICATE, TOKEN_QUERY, TokenImpersonation, WinBuiltinAdministratorsSid,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut process = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_DUPLICATE | TOKEN_QUERY, &mut process).map_err(|e| e.message())?;
        let process = Pinned(process);
        let mut sid = [0u8; 68];
        let mut size = sid.len() as u32;
        let admins = PSID(sid.as_mut_ptr().cast());
        CreateWellKnownSid(WinBuiltinAdministratorsSid, None, Some(admins), &mut size).map_err(|e| e.message())?;
        let disable = [SID_AND_ATTRIBUTES { Sid: admins, Attributes: 0 }];
        let mut restricted = HANDLE::default();
        CreateRestrictedToken(process.0, DISABLE_MAX_PRIVILEGE, Some(&disable), None, None, &mut restricted)
            .map_err(|e| e.message())?;
        let restricted = Pinned(restricted);
        let mut imp = HANDLE::default();
        DuplicateTokenEx(restricted.0, TOKEN_ALL_ACCESS, None, SecurityImpersonation, TokenImpersonation, &mut imp)
            .map_err(|e| e.message())?;
        Ok(Pinned(imp))
    }
}

fn is_reparse(m: &std::fs::Metadata) -> bool {
    m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Tamaño, archivos y fecha del archivo más nuevo de una carpeta, sin entrar en enlaces.
fn measure(dir: &Path) -> (u64, u64, SystemTime) {
    let (mut bytes, mut files, mut newest) = (0u64, 0u64, SystemTime::UNIX_EPOCH);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
            if is_reparse(&m) {
                continue;
            }
            if let Ok(t) = m.modified() {
                newest = newest.max(t);
            }
            if m.is_dir() {
                stack.push(e.path());
            } else {
                bytes += m.len();
                files += 1;
            }
        }
    }
    (bytes, files, newest)
}

/// Entradas de primer nivel que se pueden borrar, con su tamaño y cantidad de archivos.
fn candidates(d: &Dir, root: &Path) -> Vec<(PathBuf, bool, u64, u64)> {
    let cutoff = SystemTime::now().checked_sub(d.min_age).unwrap_or(SystemTime::UNIX_EPOCH);
    let Ok(rd) = std::fs::read_dir(root) else { return vec![] };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let path = e.path();
        let Ok(m) = std::fs::symlink_metadata(&path) else { continue };
        if is_reparse(&m) {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if m.is_dir() {
            if d.only.is_some() {
                continue;
            }
            let (bytes, files, newest) = measure(&path);
            let changed = newest.max(m.modified().unwrap_or(SystemTime::UNIX_EPOCH));
            if changed <= cutoff {
                out.push((path, true, bytes, files));
            }
        } else {
            if d.only.is_some_and(|f| !f(&name)) {
                continue;
            }
            if m.modified().unwrap_or(SystemTime::now()) <= cutoff {
                out.push((path, false, m.len(), 1));
            }
        }
    }
    out
}

fn recycle_bin_size() -> (u64, u64) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{SHQUERYRBINFO, SHQueryRecycleBinW};
    let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
    match unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info) } {
        Ok(()) => (info.i64Size.max(0) as u64, info.i64NumItems.max(0) as u64),
        Err(_) => (0, 0),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanView {
    pub id: String,
    pub bytes: u64,
    pub files: u64,
    pub admin: bool,
    pub per_user: bool,
    pub warn: Option<Warn>,
    /// Se puede limpiar ahora (permisos y cuenta correctos).
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanResult {
    pub id: String,
    pub freed: u64,
    pub deleted: u64,
    /// Lo que no se pudo borrar (en uso o sin permiso).
    pub skipped: u64,
    pub error: Option<String>,
}

fn size_of(c: &Category) -> (u64, u64) {
    if c.recycle_bin {
        return recycle_bin_size();
    }
    let (mut bytes, mut files) = (0, 0);
    for d in dirs(c.id) {
        if let Some(root) = safe_root(&d) {
            for (_, _, b, f) in candidates(&d, &root) {
                bytes += b;
                files += f;
            }
        }
    }
    (bytes, files)
}

fn usable(c: &Category, elevated: bool, foreign_user: bool) -> bool {
    (!c.admin || elevated) && !(c.per_user && foreign_user)
}

/// Cuánto ocupa cada categoría ahora.
pub fn scan(elevated: bool, foreign_user: bool) -> Vec<CleanView> {
    std::thread::scope(|s| {
        let handles: Vec<_> = CATEGORIES.iter().map(|c| (c, s.spawn(move || size_of(c)))).collect();
        handles
            .into_iter()
            .map(|(c, h)| {
                let (bytes, files) = h.join().unwrap_or((0, 0));
                CleanView {
                    id: c.id.into(),
                    bytes,
                    files,
                    admin: c.admin,
                    per_user: c.per_user,
                    warn: c.warn,
                    available: usable(c, elevated, foreign_user),
                }
            })
            .collect()
    })
}

fn clean_one(c: &Category) -> CleanResult {
    let (before, count) = size_of(c);
    let mut skipped = 0;
    let mut error = None;
    if c.recycle_bin {
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::{SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW};
        if count > 0 {
            let flags = SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND;
            // La papelera es del usuario: se vacía con sus permisos, no como admin.
            match as_user(|| unsafe { SHEmptyRecycleBinW(None, PCWSTR::null(), flags) }.is_ok()) {
                Ok(true) => {}
                Ok(false) => skipped = count,
                Err(e) => error = Some(t!("cleanup_unsafe", error = e)),
            }
        }
    } else {
        for d in dirs(c.id) {
            let work = || clean_dir(&d);
            // Carpetas del perfil: con permisos de usuario común. Si no se puede bajar los
            // permisos, no se borra nada (nunca se cae a hacerlo como admin).
            let r = if d.base == Base::Profile { as_user(work) } else { Ok(work()) };
            match r {
                Ok(s) => skipped += s,
                Err(e) => error = Some(t!("cleanup_unsafe", error = e)),
            }
        }
    }
    let (after, left) = size_of(c);
    CleanResult {
        id: c.id.into(),
        freed: before.saturating_sub(after),
        deleted: count.saturating_sub(left),
        skipped: skipped.min(left),
        error,
    }
}

/// Vacía una carpeta (validada y fijada mientras dura). Devuelve los archivos que quedaron.
fn clean_dir(d: &Dir) -> u64 {
    let Some(root) = safe_root(d) else { return 0 };
    let Some(_pinned) = pin(&root) else { return 0 };
    let mut skipped = 0;
    for (path, is_dir, _, files) in candidates(d, &root) {
        let ok = if is_dir { std::fs::remove_dir_all(&path).is_ok() } else { remove_file(&path) };
        if !ok {
            skipped += files;
        }
    }
    skipped
}

/// Borra un archivo; si es de solo lectura, le saca el atributo primero.
fn remove_file(p: &Path) -> bool {
    if std::fs::remove_file(p).is_ok() {
        return true;
    }
    let Ok(m) = std::fs::symlink_metadata(p) else { return false };
    if m.file_attributes() & FILE_ATTRIBUTE_READONLY != 0 && !is_reparse(&m) {
        let mut perm = m.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        let _ = std::fs::set_permissions(p, perm);
        return std::fs::remove_file(p).is_ok();
    }
    false
}

/// Limpia las categorías pedidas. Ignora IDs desconocidos y las que no se pueden ahora.
pub fn run(ids: &[String], elevated: bool, foreign_user: bool) -> Vec<CleanResult> {
    let selected: Vec<&Category> = CATEGORIES.iter().filter(|c| ids.iter().any(|i| i == c.id)).collect();
    selected
        .into_iter()
        .map(|c| {
            if usable(c, elevated, foreign_user) {
                clean_one(c)
            } else {
                let error = Some(if c.admin && !elevated { t!("requires_admin") } else { t!("cleanup_foreign_user") });
                CleanResult { id: c.id.into(), freed: 0, deleted: 0, skipped: 0, error }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_profile_dir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("debloater-cleanup-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn set_old(p: &Path) {
        let old = SystemTime::now() - 3 * DAY;
        let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
        f.set_modified(old).unwrap();
    }

    #[test]
    fn todas_las_categorias_tienen_carpetas_o_papelera() {
        for c in CATEGORIES {
            assert!(c.recycle_bin || !dirs(c.id).is_empty(), "{}", c.id);
        }
    }

    #[test]
    fn respeta_lo_reciente_y_no_entra_en_enlaces() {
        let root = temp_profile_dir("cand");
        // Viejo: se borra. Nuevo: se respeta.
        std::fs::write(root.join("viejo.tmp"), b"12345").unwrap();
        set_old(&root.join("viejo.tmp"));
        std::fs::write(root.join("nuevo.tmp"), b"1").unwrap();
        // Carpeta con un archivo viejo y uno nuevo: queda (manda el más nuevo).
        std::fs::create_dir(root.join("mixta")).unwrap();
        std::fs::write(root.join("mixta").join("a"), b"1").unwrap();
        set_old(&root.join("mixta").join("a"));
        std::fs::write(root.join("mixta").join("b"), b"1").unwrap();
        // Unión hacia afuera: no se cuenta ni se toca lo de adentro.
        let outside = temp_profile_dir("afuera");
        std::fs::write(outside.join("importante"), b"no borrar").unwrap();
        set_old(&outside.join("importante"));
        let junction = root.join("enlace");
        let st = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(st.status.success());

        let d = Dir { path: root.clone(), base: Base::Profile, min_age: DAY, only: None };
        let real = safe_root(&d).expect("raíz válida");
        let c = candidates(&d, &real);
        let names: Vec<String> = c.iter().map(|(p, ..)| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["viejo.tmp"], "{names:?}");

        // Una raíz que es un enlace hacia afuera del perfil no se acepta.
        let d2 = Dir { path: junction.clone(), base: Base::Windows, min_age: Duration::ZERO, only: None };
        assert!(safe_root(&d2).is_none());

        let _ = std::fs::remove_dir(&junction);
        assert!(outside.join("importante").exists());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn la_base_misma_nunca_es_raiz() {
        let d = Dir { path: base_path(Base::Profile).unwrap(), base: Base::Profile, min_age: DAY, only: None };
        assert!(safe_root(&d).is_none());
    }

    #[test]
    fn filtro_de_nombres() {
        let d = dirs("prefetch");
        assert!(d[0].only.unwrap()("CHROME.EXE-1234ABCD.pf"));
        assert!(!d[0].only.unwrap()("Layout.ini"));
        let t = dirs("thumbnails");
        assert!(t[0].only.unwrap()("thumbcache_256.db"));
        assert!(!t[0].only.unwrap()("iconcache_16.db"));
    }

    #[test]
    fn sin_permisos_no_limpia() {
        let r = run(&["temp_system".into(), "spotify".into(), "inventado".into()], false, true);
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|x| x.error.is_some() && x.freed == 0));
    }

    /// Solo lectura: cuánto ocupa cada categoría en esta PC.
    #[test]
    #[ignore]
    fn real_scan() {
        for v in scan(crate::system::is_elevated(), false) {
            println!("{:<14} {:>12} bytes {:>7} archivos  disponible={}", v.id, v.bytes, v.files, v.available);
        }
    }
}

#[cfg(test)]
mod hardening {
    //! Regresión de la auditoría: intentos de hacer borrar a la limpieza fuera de su carpeta.
    use super::*;

    fn junction(link: &Path, target: &Path) {
        let ok = std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(link).arg(target).output().unwrap();
        assert!(ok.status.success(), "mklink /J");
    }

    /// A) La carpeta se reemplaza por una unión después de validarla: con la carpeta fijada,
    /// Windows no deja renombrarla, y lo de afuera queda intacto.
    #[test]
    fn no_se_puede_cambiar_la_carpeta_durante_la_limpieza() {
        let base = std::env::temp_dir().join(format!("hard-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("cache");
        let outside = std::env::temp_dir().join(format!("hard-victima-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("victima.txt"), b"no borrar").unwrap();
        std::fs::write(root.join("basura.tmp"), b"x").unwrap();

        let d = Dir { path: root.clone(), base: Base::Profile, min_age: Duration::ZERO, only: None };
        let real = safe_root(&d).expect("raíz válida");
        let pinned = pin(&real).expect("se fija");
        // El atacante intenta cambiarla: Windows lo rechaza mientras está fijada.
        assert!(std::fs::rename(&root, base.join("cache.old")).is_err(), "pudo renombrar la carpeta fijada");
        for (p, is_dir, ..) in candidates(&d, &real) {
            let _ = if is_dir { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
        }
        drop(pinned);
        assert!(outside.join("victima.txt").exists(), "borró fuera de la carpeta");
        assert!(!root.join("basura.tmp").exists(), "no limpió la carpeta propia");

        // Y si la cambian antes de empezar, `clean_dir` la rechaza entera.
        std::fs::rename(&root, base.join("cache.old")).unwrap();
        junction(&root, &outside);
        assert_eq!(clean_dir(&d), 0);
        assert!(outside.join("victima.txt").exists(), "borró a través de la unión");

        let _ = std::fs::remove_dir(&root);
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&outside);
    }

    /// B) Una unión dentro de la misma base (o en cualquier punto del camino) se rechaza.
    #[test]
    fn rechaza_uniones_en_el_camino() {
        let base = std::env::temp_dir().join(format!("hard-base-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let target = base.join("Documentos-importantes");
        std::fs::create_dir_all(target.join("sub")).unwrap();
        // La carpeta misma es una unión.
        let fake = base.join("D3DSCache");
        junction(&fake, &target);
        assert!(safe_root(&Dir { path: fake.clone(), base: Base::Profile, min_age: Duration::ZERO, only: None }).is_none());
        // Un ancestro es una unión.
        let fake_parent = base.join("NVIDIA");
        junction(&fake_parent, &target);
        let nested = fake_parent.join("sub");
        assert!(safe_root(&Dir { path: nested, base: Base::Profile, min_age: Duration::ZERO, only: None }).is_none());
        // `pin` tampoco acepta una unión.
        assert!(pin(&fake).is_none());
        let _ = std::fs::remove_dir(&fake);
        let _ = std::fs::remove_dir(&fake_parent);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// El token de usuario común se puede usar para borrar lo propio, y al terminar el hilo
    /// vuelve a su identidad normal.
    #[test]
    fn permisos_de_usuario_y_vuelta_atras() {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::Threading::{GetCurrentThread, OpenThreadToken, SetThreadToken};
        use windows::Win32::Security::{RevertToSelf, TOKEN_QUERY};
        let f = std::env::temp_dir().join(format!("hard-user-{}.tmp", std::process::id()));
        std::fs::write(&f, b"x").unwrap();
        let token = restricted_impersonation_token().expect("token restringido");
        unsafe { SetThreadToken(None, Some(token.0)) }.unwrap();
        let borrado = std::fs::remove_file(&f).is_ok();
        unsafe { RevertToSelf() }.unwrap();
        assert!(borrado, "con permisos de usuario no pudo borrar un archivo propio");
        let mut h = HANDLE::default();
        assert!(unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, true, &mut h) }.is_err(), "el hilo sigue suplantando");
        // as_user también vuelve atrás.
        assert_eq!(as_user(|| 7).unwrap(), 7);
        assert!(unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, true, &mut h) }.is_err());
    }
}
