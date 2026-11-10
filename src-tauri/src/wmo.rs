//! WMO (Win11 Media Overlay), otra app del mismo autor: estado, instalar/actualizar,
//! abrir y desinstalar.
//!
//! - Solo se instala la versión fijada acá: URL de GitHub Releases, tamaño exacto y
//!   SHA-256 revisados al publicar P-Rune. El hash no se toma de GitHub en el momento,
//!   así que una release reemplazada o una descarga alterada nunca se instala.
//! - WMO es una app del usuario y no pide admin: va en `%LOCALAPPDATA%\Programs\WMO` y
//!   todo lo que se escribe o se borra ahí se hace con permisos de usuario común
//!   (`cleanup::as_user`), así una unión plantada en el camino no lleva a ningún lado
//!   donde el usuario no pudiera escribir por su cuenta.
//! - Se abre sin elevación: con el token del Explorador de la sesión, no con el de P-Rune.
//! - Del zip solo se extraen los archivos esperados (nada de .bat ni rutas raras).

use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Versión que ofrece esta versión de P-Rune. Para subirla: nueva URL, tamaño y SHA-256
/// del zip de la release (`gh api repos/buccellaluciano/win-media-overlay/releases/latest`).
pub const VERSION: &str = "1.1.0";
const URL: &str = "https://github.com/buccellaluciano/win-media-overlay/releases/download/1.1.0/WMO-1.1.0-win64.zip";
const SIZE: usize = 2_071_540;
const SHA256: &str = "779c311fb6d3cb1124c1cc8e4c881feff171f6fda40594d2c2b7aeca2d999956";
pub const REPO: &str = "https://github.com/buccellaluciano/win-media-overlay";

const EXE: &str = "wmo.exe";
/// Lo único que se extrae del zip.
const FILES: [&str; 3] = [EXE, "LICENSE", "LEEME.txt"];
const SHORTCUT: &str = "WMO.lnk";

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    /// Versión instalada (del recurso de versión de wmo.exe), o None si no está.
    pub installed: Option<String>,
    /// La que instala este P-Rune.
    pub available: String,
    /// La instalada es más vieja que `available`.
    pub update: bool,
    /// wmo.exe está abierto (no se puede actualizar ni desinstalar sin cerrarlo).
    pub running: bool,
    pub sha256: String,
    pub repo: String,
}

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

/// `%LOCALAPPDATA%\Programs` según Windows.
fn programs_dir() -> Result<PathBuf, String> {
    known(&windows::Win32::UI::Shell::FOLDERID_UserProgramFiles).ok_or_else(|| t!("wmo_no_folder"))
}

fn install_dir() -> Result<PathBuf, String> {
    Ok(programs_dir()?.join("WMO"))
}

/// Acceso directo en el menú Inicio del usuario.
fn shortcut_path() -> Option<PathBuf> {
    known(&windows::Win32::UI::Shell::FOLDERID_Programs).map(|p| p.join(SHORTCUT))
}

fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(std::iter::once(0)).collect()
}

fn is_reparse(p: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_attributes() & 0x400 != 0)
}

/// Versión de producto de un ejecutable ("1.0.1"), del recurso VS_FIXEDFILEINFO.
fn file_version(exe: &Path) -> Option<String> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO};
    let path = wide(exe.as_os_str());
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(path.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(path.as_ptr()), None, size, buf.as_mut_ptr().cast()).ok()?;
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        if !VerQueryValueW(buf.as_ptr().cast(), w!("\\"), &mut ptr, &mut len).as_bool()
            || (len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>()
        {
            return None;
        }
        let info = &*(ptr as *const VS_FIXEDFILEINFO);
        let (ms, ls) = (info.dwProductVersionMS, info.dwProductVersionLS);
        Some(format!("{}.{}.{}", ms >> 16, ms & 0xffff, ls >> 16))
    }
}

/// "1.0.1" → [1, 0, 1]; lo que no sea número cuenta como 0.
fn parse_version(v: &str) -> Vec<u32> {
    v.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect()
}

fn older(installed: &str, available: &str) -> bool {
    let (mut a, mut b) = (parse_version(installed), parse_version(available));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a < b
}

/// Un exe en uso no se puede abrir para escribir (error 32, archivo en uso).
fn exe_running(exe: &Path) -> bool {
    std::fs::OpenOptions::new()
        .write(true)
        .open(exe)
        .is_err_and(|e| e.raw_os_error() == Some(32))
}

pub fn status() -> Status {
    let exe = install_dir().ok().map(|d| d.join(EXE)).filter(|e| e.is_file());
    let installed = exe.as_deref().map(|e| file_version(e).unwrap_or_else(|| "?".into()));
    Status {
        update: installed.as_deref().is_some_and(|v| older(v, VERSION)),
        running: exe.as_deref().is_some_and(exe_running),
        installed,
        available: VERSION.into(),
        sha256: SHA256.into(),
        repo: REPO.into(),
    }
}

fn sha256(data: &[u8]) -> Result<String, String> {
    use windows::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};
    let mut out = [0u8; 32];
    let st = unsafe { BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, data, &mut out) };
    if st.is_err() {
        return Err(t!("wmo_hash_failed"));
    }
    Ok(out.iter().map(|b| format!("{b:02x}")).collect())
}

/// El zip tiene que ser exactamente el fijado: mismo tamaño y mismo SHA-256.
fn verify(zip: &[u8]) -> Result<(), String> {
    if zip.len() != SIZE || sha256(zip)? != SHA256 {
        return Err(t!("wmo_hash_mismatch"));
    }
    Ok(())
}

/// Saca del zip solo los archivos de `FILES` (en la raíz, sin carpetas), con un tope de
/// tamaño. Devuelve (nombre, contenido); wmo.exe es obligatorio.
fn unpack(zip: &[u8]) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    const MAX: u64 = 64 * 1024 * 1024;
    let bad = |e: zip::result::ZipError| t!("wmo_bad_zip", error = e);
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).map_err(bad)?;
    let mut out = Vec::new();
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(bad)?;
        // Nombres que no son UTF-8 válido no pueden ser ninguno de los esperados.
        let Ok(entry) = f.name() else { continue };
        let Some(name) = FILES.iter().find(|n| entry == **n) else { continue };
        if f.size() > MAX {
            return Err(t!("wmo_bad_zip", error = name));
        }
        let mut data = Vec::with_capacity(f.size() as usize);
        f.by_ref().take(MAX).read_to_end(&mut data).map_err(|e| t!("wmo_bad_zip", error = e))?;
        out.push((*name, data));
    }
    if !out.iter().any(|(n, _)| *n == EXE) {
        return Err(t!("wmo_bad_zip", error = EXE));
    }
    Ok(out)
}

/// Escribe los archivos en una carpeta nueva al lado y la pone en lugar de la vieja.
/// Si WMO está abierto, la carpeta vieja no se puede mover y no se toca nada.
fn place(files: &[(&str, Vec<u8>)]) -> Result<(), String> {
    let base = programs_dir()?;
    let dir = base.join("WMO");
    let staging = base.join("WMO.p-rune-new");
    let old = base.join("WMO.p-rune-old");
    crate::cleanup::as_user(|| -> Result<(), String> {
        std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
        for p in [&dir, &staging, &old] {
            if is_reparse(p) {
                return Err(t!("wmo_link"));
            }
        }
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_dir_all(&old);
        std::fs::create_dir(&staging).map_err(|e| e.to_string())?;
        for (name, data) in files {
            if let Err(e) = std::fs::write(staging.join(name), data) {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(e.to_string());
            }
        }
        if dir.exists() && std::fs::rename(&dir, &old).is_err() {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(t!("wmo_close_first"));
        }
        if let Err(e) = std::fs::rename(&staging, &dir) {
            // Volver a dejar la versión anterior en su lugar.
            let _ = std::fs::rename(&old, &dir);
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e.to_string());
        }
        let _ = std::fs::remove_dir_all(&old);
        Ok(())
    })?
}

/// Crea (o rehace) el acceso directo del menú Inicio. Si falla, WMO igual queda instalado.
fn create_shortcut(exe: &Path) -> Result<(), String> {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    let lnk = shortcut_path().ok_or_else(|| t!("wmo_no_folder"))?;
    let dir = exe.parent().unwrap_or(exe).to_path_buf();
    let exe = exe.to_path_buf();
    crate::cleanup::as_user(move || -> Result<(), String> {
        unsafe {
            let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let r = (|| -> windows::core::Result<()> {
                let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
                link.SetPath(&HSTRING::from(exe.as_os_str()))?;
                link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
                link.SetDescription(&HSTRING::from("WMO - Win11 Media Overlay"))?;
                link.cast::<IPersistFile>()?.Save(&HSTRING::from(lnk.as_os_str()), true)
            })();
            if init.is_ok() {
                CoUninitialize();
            }
            r.map_err(|e| e.message())
        }
    })?
}

/// Descarga, verifica e instala (o actualiza) la versión fijada.
pub fn install() -> Result<Status, String> {
    if let Some(user) = crate::system::foreign_user() {
        return Err(t!("wmo_foreign_user", user = user));
    }
    // Con WMO abierto, Windows igual deja mover su carpeta y el exe en uso quedaría
    // huérfano en WMO.p-rune-old: se pide cerrarlo antes de descargar nada.
    if exe_running(&install_dir()?.join(EXE)) {
        return Err(t!("wmo_close_first"));
    }
    let res = crate::http::get(URL, &[], SIZE + 1024)?;
    if res.status != 200 {
        return Err(t!("wmo_download_failed", status = res.status));
    }
    verify(&res.body)?;
    let files = unpack(&res.body)?;
    place(&files)?;
    let exe = install_dir()?.join(EXE);
    let _ = create_shortcut(&exe);
    Ok(status())
}

/// Borra la carpeta de WMO y su acceso directo. La configuración (`%APPDATA%\WMO`) queda.
pub fn uninstall() -> Result<Status, String> {
    if let Some(user) = crate::system::foreign_user() {
        return Err(t!("wmo_foreign_user", user = user));
    }
    let dir = install_dir()?;
    let exe = dir.join(EXE);
    if exe_running(&exe) {
        return Err(t!("wmo_close_first"));
    }
    let lnk = shortcut_path();
    crate::cleanup::as_user(|| -> Result<(), String> {
        if is_reparse(&dir) {
            return Err(t!("wmo_link"));
        }
        // Solo si es la carpeta de WMO (tiene wmo.exe): nunca se borra otra cosa.
        if dir.join(EXE).is_file() {
            std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        }
        if let Some(l) = lnk.filter(|l| l.is_file()) {
            let _ = std::fs::remove_file(l);
        }
        Ok(())
    })??;
    Ok(status())
}

/// Abre WMO sin permisos de administrador.
pub fn launch() -> Result<(), String> {
    let exe = install_dir()?.join(EXE);
    if !exe.is_file() || is_reparse(&exe) {
        return Err(t!("wmo_not_installed"));
    }
    let dir = exe.parent().unwrap_or(&exe).to_path_buf();
    if !crate::system::is_elevated() {
        std::process::Command::new(&exe).current_dir(&dir).spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }
    if let Some(user) = crate::system::foreign_user() {
        return Err(t!("wmo_foreign_user", user = user));
    }
    launch_as_shell_user(&exe, &dir)
}

/// Crea el proceso con el token del Explorador (el de la persona, sin elevar).
fn launch_as_shell_user(exe: &Path, dir: &Path) -> Result<(), String> {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        DuplicateTokenEx, SecurityImpersonation, TokenPrimary, TOKEN_ACCESS_MASK, TOKEN_ADJUST_DEFAULT,
        TOKEN_ADJUST_SESSIONID, TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{
        CreateProcessWithTokenW, OpenProcess, OpenProcessToken, CREATE_PROCESS_LOGON_FLAGS, PROCESS_INFORMATION,
        PROCESS_QUERY_LIMITED_INFORMATION, STARTUPINFOW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetShellWindow, GetWindowThreadProcessId};

    struct H(HANDLE);
    impl Drop for H {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                let _ = unsafe { CloseHandle(self.0) };
            }
        }
    }
    let fail = |e: windows::core::Error| t!("wmo_launch_failed", error = e.message());
    unsafe {
        let shell = GetShellWindow();
        if shell.is_invalid() {
            return Err(t!("wmo_no_shell"));
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(shell, Some(&mut pid));
        if pid == 0 {
            return Err(t!("wmo_no_shell"));
        }
        let process = H(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).map_err(fail)?);
        let mut token = HANDLE::default();
        OpenProcessToken(process.0, TOKEN_DUPLICATE, &mut token).map_err(fail)?;
        let token = H(token);
        let access = TOKEN_ACCESS_MASK(
            TOKEN_QUERY.0 | TOKEN_DUPLICATE.0 | TOKEN_ASSIGN_PRIMARY.0 | TOKEN_ADJUST_DEFAULT.0 | TOKEN_ADJUST_SESSIONID.0,
        );
        let mut primary = HANDLE::default();
        DuplicateTokenEx(token.0, access, None, SecurityImpersonation, TokenPrimary, &mut primary).map_err(fail)?;
        let primary = H(primary);

        let app = wide(exe.as_os_str());
        let mut cmd = wide(std::ffi::OsStr::new(&format!("\"{}\"", exe.display())));
        let cwd = wide(dir.as_os_str());
        let si = STARTUPINFOW { cb: std::mem::size_of::<STARTUPINFOW>() as u32, ..Default::default() };
        let mut pi = PROCESS_INFORMATION::default();
        CreateProcessWithTokenW(
            primary.0,
            CREATE_PROCESS_LOGON_FLAGS(0),
            PCWSTR(app.as_ptr()),
            Some(PWSTR(cmd.as_mut_ptr())),
            Default::default(),
            None,
            PCWSTR(cwd.as_ptr()),
            &si,
            &mut pi,
        )
        .map_err(fail)?;
        let _ = CloseHandle(pi.hThread);
        let _ = CloseHandle(pi.hProcess);
    }
    Ok(())
}

/// Abre el repositorio de WMO en el navegador (dirección fija).
pub fn open_repo() -> Result<(), String> {
    use windows::core::{w, PCWSTR};
    let u = wide(std::ffi::OsStr::new(REPO));
    let r = unsafe {
        windows::Win32::UI::Shell::ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(u.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };
    if r.0 as usize > 32 { Ok(()) } else { Err(t!("wmo_browser_failed")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compara_versiones() {
        assert!(older("1.0.0", "1.0.1"));
        assert!(older("0.9", "1.0.1"));
        assert!(!older("1.0.1", "1.0.1"));
        assert!(!older("1.1.0", "1.0.1"));
        assert!(older("?", "1.0.1"));
    }

    #[test]
    fn hash_conocido() {
        assert_eq!(sha256(b"abc").unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn rechaza_zips_que_no_son_el_fijado() {
        assert!(verify(b"cualquier cosa").is_err());
        assert!(verify(&vec![0u8; SIZE]).is_err());
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            for (n, d) in entries {
                w.start_file(*n, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))
                    .unwrap();
                std::io::Write::write_all(&mut w, d).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn solo_extrae_lo_esperado() {
        let z = zip_with(&[
            ("wmo.exe", b"MZ"),
            ("Iniciar.bat", b"start wmo.exe"),
            ("../../evil.exe", b"x"),
            ("sub/wmo.exe", b"y"),
            ("LICENSE", b"MIT"),
        ]);
        let files = unpack(&z).unwrap();
        let names: Vec<_> = files.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, ["wmo.exe", "LICENSE"]);
        assert_eq!(files[0].1, b"MZ");
    }

    #[test]
    fn sin_exe_no_sirve() {
        assert!(unpack(&zip_with(&[("LICENSE", b"MIT")])).is_err());
    }

    /// Descarga real de la release fijada: confirma que la URL, el tamaño y el hash coinciden.
    #[test]
    #[ignore]
    fn release_fijada_real() {
        let res = crate::http::get(URL, &[], SIZE + 1024).unwrap();
        assert_eq!(res.status, 200);
        verify(&res.body).unwrap();
        let files = unpack(&res.body).unwrap();
        assert!(files.iter().any(|(n, d)| *n == EXE && d.starts_with(b"MZ")));
    }
}
