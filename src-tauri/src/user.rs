//! Usuario de Windows que abrió la app: nombre y foto de cuenta (solo lectura).

use crate::catalog::Hive;
use crate::registry as reg;
use serde::Serialize;
use std::path::Path;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
use windows::Win32::NetworkManagement::NetManagement::{NetApiBufferFree, NetUserGetInfo, USER_INFO_10};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::System::WindowsProgramming::GetUserNameW;

#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    /// Nombre completo si la cuenta lo tiene; si no, el nombre de usuario.
    pub name: String,
    pub username: String,
    /// Foto de la cuenta como data URL, si el usuario configuró una.
    pub picture: Option<String>,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(crate) fn username() -> Option<String> {
    let mut buf = [0u16; 257];
    let mut len = buf.len() as u32;
    unsafe { GetUserNameW(Some(PWSTR(buf.as_mut_ptr())), &mut len) }.ok()?;
    // `len` incluye el terminador nulo.
    Some(String::from_utf16_lossy(&buf[..len.saturating_sub(1) as usize]))
}

/// Nombre completo de la cuenta local (las cuentas Microsoft lo traen de la cuenta).
fn full_name(user: &str) -> Option<String> {
    let name = wide(user);
    let mut ptr: *mut u8 = std::ptr::null_mut();
    let rc = unsafe { NetUserGetInfo(PCWSTR::null(), PCWSTR(name.as_ptr()), 10, &mut ptr) };
    if rc != 0 || ptr.is_null() {
        return None;
    }
    let full = unsafe {
        let info = &*(ptr as *const USER_INFO_10);
        let s = info.usri10_full_name.to_string().ok();
        NetApiBufferFree(Some(ptr as _));
        s
    };
    full.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn current_sid() -> Option<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
        let mut len = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        let got = GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr() as _), len, &mut len);
        let _ = CloseHandle(token);
        got.ok()?;
        let tu = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s = PWSTR::null();
        ConvertSidToStringSidW(tu.User.Sid, &mut s).ok()?;
        let out = s.to_string().ok();
        let _ = LocalFree(Some(HLOCAL(s.0 as _)));
        out
    }
}

/// Windows guarda la ruta de la foto en varios tamaños; usamos la más cercana a 208 px.
fn picture(sid: &str) -> Option<String> {
    let key = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\AccountPicture\Users\{sid}");
    for name in ["Image208", "Image240", "Image192", "Image448", "Image96"] {
        let Some(v) = reg::read(Hive::HKLM, &key, name).ok().flatten() else { continue };
        let path = reg::describe(Some(&v)).trim_matches('"').to_string();
        if let Some(url) = data_url(Path::new(&path)) {
            return Some(url);
        }
    }
    None
}

fn data_url(path: &Path) -> Option<String> {
    let mime = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "bmp" => "image/bmp",
        _ => return None,
    };
    if std::fs::metadata(path).ok()?.len() > 4 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    Some(format!("data:{mime};base64,{}", base64(&bytes)))
}

pub(crate) fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn info() -> UserInfo {
    let username = username().unwrap_or_else(|| std::env::var("USERNAME").unwrap_or_default());
    UserInfo {
        name: full_name(&username).unwrap_or_else(|| username.clone()),
        picture: current_sid().as_deref().and_then(picture),
        username,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn reads_current_user() {
        let u = info();
        assert!(!u.username.is_empty());
        assert!(current_sid().is_some_and(|s| s.starts_with("S-1-5-")));
    }
}
