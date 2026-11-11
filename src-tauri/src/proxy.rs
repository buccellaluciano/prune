//! Proxy del usuario (el de Windows y los navegadores): activar o desactivar lo que ya está
//! configurado. P-Rune no crea ni edita proxies: un proxy puesto por un tercero es una vía
//! clásica para espiar el tráfico, así que esa puerta queda cerrada.
//!
//! - Se lee y se escribe con WinINet (`INTERNET_OPTION_PER_CONNECTION_OPTION`) y después se
//!   avisa con `SETTINGS_CHANGED` + `REFRESH`, así el navegador y las apps abiertas lo toman.
//! - Solo se cambian los interruptores (`INTERNET_PER_CONN_FLAGS`); servidor, excepciones y
//!   dirección del script quedan como estaban, para reactivarlos tal cual.

use serde::{Deserialize, Serialize};

const DIRECT: u32 = 1;
const MANUAL: u32 = 2;
const PAC: u32 = 4;
const AUTO_DETECT: u32 = 8;
const KNOWN: u32 = DIRECT | MANUAL | PAC | AUTO_DETECT;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Proxy manual (servidor y puerto).
    Manual,
    /// Script de configuración (PAC).
    Pac,
    /// Detectar la configuración automáticamente (WPAD).
    AutoDetect,
}

impl Kind {
    fn bit(self) -> u32 {
        match self {
            Kind::Manual => MANUAL,
            Kind::Pac => PAC,
            Kind::AutoDetect => AUTO_DETECT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Settings {
    flags: u32,
    server: String,
    bypass: String,
    pac_url: String,
}

/// Lo que se guarda en el historial: los interruptores antes y después.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyChange {
    pub kind: Kind,
    pub before: u32,
    pub after: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProxyStatus {
    pub manual: bool,
    pub server: String,
    pub bypass: String,
    pub pac: bool,
    pub pac_url: String,
    pub auto_detect: bool,
    /// El proxy manual apunta a esta misma PC (suele ser de un programa: Fiddler, antivirus, VPN).
    pub local: bool,
    /// Una directiva bloquea estos ajustes.
    pub policy: bool,
    /// Proxy de WinHTTP (el de algunos servicios del sistema), solo informativo.
    pub winhttp: Option<String>,
    /// La app corre con otra cuenta de admin: se cambiaría el proxy de esa cuenta.
    pub foreign_user: Option<String>,
}

// ───────────────────────────── WinINet ─────────────────────────────

fn take_string(p: windows::core::PWSTR) -> String {
    use windows::Win32::Foundation::{GlobalFree, HGLOBAL};
    if p.is_null() {
        return String::new();
    }
    let s = unsafe { p.to_string() }.unwrap_or_default();
    unsafe {
        let _ = GlobalFree(Some(HGLOBAL(p.0.cast())));
    }
    s
}

fn read() -> Result<Settings, String> {
    use windows::Win32::Networking::WinInet::{
        InternetQueryOptionW, INTERNET_OPTION_PER_CONNECTION_OPTION, INTERNET_PER_CONN, INTERNET_PER_CONN_AUTOCONFIG_URL,
        INTERNET_PER_CONN_FLAGS, INTERNET_PER_CONN_FLAGS_UI, INTERNET_PER_CONN_OPTIONW, INTERNET_PER_CONN_OPTION_LISTW,
        INTERNET_PER_CONN_PROXY_BYPASS, INTERNET_PER_CONN_PROXY_SERVER,
    };
    // FLAGS_UI refleja lo que muestra Configuración; si esta versión no lo entiende, FLAGS.
    for flags_opt in [INTERNET_PER_CONN(INTERNET_PER_CONN_FLAGS_UI), INTERNET_PER_CONN_FLAGS] {
        let mut opts = [flags_opt, INTERNET_PER_CONN_PROXY_SERVER, INTERNET_PER_CONN_PROXY_BYPASS, INTERNET_PER_CONN_AUTOCONFIG_URL]
            .map(|o| INTERNET_PER_CONN_OPTIONW { dwOption: o, ..Default::default() });
        let mut list = INTERNET_PER_CONN_OPTION_LISTW {
            dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
            dwOptionCount: opts.len() as u32,
            pOptions: opts.as_mut_ptr(),
            ..Default::default()
        };
        let mut size = list.dwSize;
        let r = unsafe {
            InternetQueryOptionW(None, INTERNET_OPTION_PER_CONNECTION_OPTION, Some((&mut list as *mut INTERNET_PER_CONN_OPTION_LISTW).cast()), &mut size)
        };
        if r.is_err() {
            continue;
        }
        return Ok(unsafe {
            Settings {
                flags: opts[0].Value.dwValue,
                server: take_string(opts[1].Value.pszValue),
                bypass: take_string(opts[2].Value.pszValue),
                pac_url: take_string(opts[3].Value.pszValue),
            }
        });
    }
    Err(t!("proxy_read_failed"))
}

/// Escribe solo los interruptores y avisa a las apps abiertas.
fn write_flags(flags: u32) -> Result<(), String> {
    use windows::Win32::Networking::WinInet::{
        InternetSetOptionW, INTERNET_OPTION_PER_CONNECTION_OPTION, INTERNET_OPTION_REFRESH,
        INTERNET_OPTION_SETTINGS_CHANGED, INTERNET_PER_CONN_FLAGS, INTERNET_PER_CONN_OPTIONW, INTERNET_PER_CONN_OPTIONW_0,
        INTERNET_PER_CONN_OPTION_LISTW,
    };
    let mut opt = INTERNET_PER_CONN_OPTIONW {
        dwOption: INTERNET_PER_CONN_FLAGS,
        Value: INTERNET_PER_CONN_OPTIONW_0 { dwValue: (flags & KNOWN) | DIRECT },
    };
    let list = INTERNET_PER_CONN_OPTION_LISTW {
        dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
        dwOptionCount: 1,
        pOptions: &mut opt,
        ..Default::default()
    };
    unsafe {
        InternetSetOptionW(None, INTERNET_OPTION_PER_CONNECTION_OPTION, Some((&list as *const INTERNET_PER_CONN_OPTION_LISTW).cast()), list.dwSize)
            .map_err(|e| t!("proxy_write_failed", error = e.message()))?;
        let _ = InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0);
        let _ = InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0);
    }
    Ok(())
}

// ───────────────────────────── Estado ─────────────────────────────

/// Proxy de WinHTTP, si hay uno con nombre.
fn winhttp() -> Option<String> {
    use windows::Win32::Networking::WinHttp::{WinHttpGetDefaultProxyConfiguration, WINHTTP_ACCESS_TYPE_NAMED_PROXY, WINHTTP_PROXY_INFO};
    let mut info = WINHTTP_PROXY_INFO::default();
    unsafe { WinHttpGetDefaultProxyConfiguration(&mut info) }.ok()?;
    let proxy = take_string(info.lpszProxy);
    let _ = take_string(info.lpszProxyBypass);
    (info.dwAccessType == WINHTTP_ACCESS_TYPE_NAMED_PROXY && !proxy.is_empty()).then_some(proxy)
}

/// Directivas que fijan el proxy (Internet Explorer / Windows) o lo pasan a todo el equipo.
fn policy() -> bool {
    use crate::catalog::Hive;
    let on = |h: Hive, path: &str, name: &str, want: u32| {
        matches!(crate::registry::read(h, path, name), Ok(Some(v)) if v.bytes.len() >= 4 && u32::from_le_bytes([v.bytes[0], v.bytes[1], v.bytes[2], v.bytes[3]]) == want)
    };
    let cp = r"Software\Policies\Microsoft\Internet Explorer\Control Panel";
    [Hive::HKCU, Hive::HKLM].into_iter().any(|h| on(h, cp, "Proxy", 1) || on(h, cp, "Autoconfig", 1))
        || on(Hive::HKLM, r"SOFTWARE\Policies\Microsoft\Windows\CurrentVersion\Internet Settings", "ProxySettingsPerUser", 0)
}

/// El servidor apunta a esta PC: `127.0.0.1:8888`, `localhost:3128`, `http=[::1]:8080;https=…`.
fn is_local(server: &str) -> bool {
    server.split(';').filter(|s| !s.is_empty()).any(|entry| {
        let host = entry.rsplit('=').next().unwrap_or(entry);
        let host = host.trim_start_matches("http://").trim_start_matches("https://").trim_start_matches("socks=");
        let host = if let Some(rest) = host.strip_prefix('[') {
            rest.split(']').next().unwrap_or("")
        } else {
            host.rsplit_once(':').map_or(host, |(h, _)| h)
        };
        host.eq_ignore_ascii_case("localhost")
            || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
    })
}

pub fn status() -> Result<ProxyStatus, String> {
    let s = read()?;
    Ok(ProxyStatus {
        manual: s.flags & MANUAL != 0,
        local: is_local(&s.server),
        pac: s.flags & PAC != 0,
        auto_detect: s.flags & AUTO_DETECT != 0,
        server: s.server,
        bypass: s.bypass,
        pac_url: s.pac_url,
        policy: policy(),
        winhttp: winhttp(),
        foreign_user: crate::system::foreign_user(),
    })
}

/// Qué no deja cambiar el proxy ahora.
fn guard() -> Result<(), String> {
    if let Some(user) = crate::system::foreign_user() {
        return Err(t!("foreign_user_apply", user = user));
    }
    if policy() {
        return Err(t!("proxy_policy"));
    }
    Ok(())
}

/// Activar con algo configurado; desactivar, siempre.
fn check_target(kind: Kind, enabled: bool, s: &Settings) -> Result<(), String> {
    if enabled && kind == Kind::Manual && s.server.trim().is_empty() {
        return Err(t!("proxy_no_server"));
    }
    if enabled && kind == Kind::Pac && s.pac_url.trim().is_empty() {
        return Err(t!("proxy_no_pac"));
    }
    Ok(())
}

/// Prende o apaga un interruptor. Devuelve el cambio para el historial (None si ya estaba así).
pub fn set(kind: Kind, enabled: bool) -> Result<Option<ProxyChange>, String> {
    guard()?;
    let s = read()?;
    check_target(kind, enabled, &s)?;
    let before = s.flags & KNOWN;
    let after = if enabled { before | kind.bit() } else { before & !kind.bit() } | DIRECT;
    if after & !DIRECT == before & !DIRECT {
        return Ok(None);
    }
    write_flags(after)?;
    let now = read().map(|s| s.flags & KNOWN).unwrap_or(after);
    Ok(Some(ProxyChange { kind, before, after: now }))
}

/// Para deshacer desde el historial.
pub fn validate_saved(c: &ProxyChange) -> Result<(), String> {
    if c.before & !KNOWN != 0 || c.after & !KNOWN != 0 {
        return Err(t!("proxy_bad_entry"));
    }
    guard()?;
    let s = read()?;
    check_target(Kind::Manual, c.before & MANUAL != 0, &s)?;
    check_target(Kind::Pac, c.before & PAC != 0, &s)
}

/// Interruptores actuales (para detectar cambios posteriores).
pub fn current_flags() -> Option<u32> {
    read().ok().map(|s| s.flags & KNOWN)
}

pub fn restore(c: &ProxyChange) -> Result<(), String> {
    write_flags(c.before)
}

pub fn label(c: &ProxyChange) -> String {
    let what = match c.kind {
        Kind::Manual => t!("proxy_manual"),
        Kind::Pac => t!("proxy_pac"),
        Kind::AutoDetect => t!("proxy_auto_detect"),
    };
    let on = c.after & c.kind.bit() != 0;
    t!("label_proxy", what = what, state = if on { t!("proxy_on") } else { t!("proxy_off") })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_proxies_locales() {
        for local in ["127.0.0.1:8888", "localhost:3128", "http=127.0.0.1:8080;https=127.0.0.1:8080", "[::1]:8080", "LOCALHOST"] {
            assert!(is_local(local), "{local}");
        }
        for remoto in ["proxy.empresa.com:8080", "10.0.0.5:3128", "http=proxy:80", ""] {
            assert!(!is_local(remoto), "{remoto}");
        }
    }

    #[test]
    fn solo_activa_lo_configurado() {
        let vacio = Settings::default();
        assert!(check_target(Kind::Manual, true, &vacio).is_err());
        assert!(check_target(Kind::Pac, true, &vacio).is_err());
        assert!(check_target(Kind::AutoDetect, true, &vacio).is_ok(), "detectar no necesita nada");
        assert!(check_target(Kind::Manual, false, &vacio).is_ok(), "apagar siempre se puede");
        let con = Settings { server: "proxy:8080".into(), pac_url: "http://wpad/wpad.dat".into(), ..Default::default() };
        assert!(check_target(Kind::Manual, true, &con).is_ok());
        assert!(check_target(Kind::Pac, true, &con).is_ok());
    }

    #[test]
    fn entradas_raras_se_rechazan() {
        let c = ProxyChange { kind: Kind::Manual, before: 0x100 | DIRECT, after: DIRECT };
        assert!(validate_saved(&c).is_err());
    }

    /// Lectura real (no cambia nada).
    #[test]
    #[ignore]
    fn lectura_real() {
        println!("{:?}", status());
        assert!(status().is_ok());
    }

    /// Prende y apaga "detectar automáticamente" y deja todo como estaba (no necesita admin).
    #[test]
    #[ignore]
    fn alternar_y_restaurar() {
        let before = read().unwrap();
        let c = set(Kind::AutoDetect, before.flags & AUTO_DETECT == 0).unwrap().expect("tenía que cambiar");
        assert_ne!(current_flags().unwrap() & AUTO_DETECT, before.flags & AUTO_DETECT);
        restore(&c).unwrap();
        let after = read().unwrap();
        assert_eq!(after.flags & KNOWN, before.flags & KNOWN);
        assert_eq!((after.server, after.bypass, after.pac_url), (before.server, before.bypass, before.pac_url));
    }
}
