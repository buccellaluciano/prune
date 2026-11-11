//! Puntos de restauración del sistema (Restaurar sistema de Windows).
//!
//! Se crean con la API de Windows (`SRSetRestorePointW`) y se listan con WMI. Si ya hay uno
//! de las últimas 24 horas no se crea otro: Windows tampoco lo haría (lo limita a uno por
//! día) y respondería como si lo hubiera creado.

use crate::catalog::Hive;
use crate::com::{now_secs, parse_cim_datetime, Com};
use crate::registry as reg;
use serde::Serialize;
use windows::core::{BSTR, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoSetProxyBlanket, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL,
    RPC_C_IMP_LEVEL_IMPERSONATE,
};
use windows::Win32::System::Restore::{
    SRSetRestorePointW, BEGIN_SYSTEM_CHANGE, END_SYSTEM_CHANGE, MODIFY_SETTINGS, RESTOREPOINTINFOW, STATEMGRSTATUS,
};
use windows::Win32::System::Variant::{VariantClear, VariantToString, VARIANT};
use windows::Win32::System::Wmi::{
    IWbemLocator, WbemLocator, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_INFINITE,
};

/// Uno por día, como Windows.
pub const RECENT_SECS: u64 = 24 * 3600;

#[derive(Debug, Clone, Serialize)]
pub struct RestorePoint {
    pub at: u64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreStatus {
    /// Restaurar sistema está activado (en al menos un disco).
    pub enabled: bool,
    /// Una directiva lo apaga.
    pub policy: bool,
    pub elevated: bool,
    /// El más reciente (solo se puede leer como administrador).
    pub last: Option<RestorePoint>,
    pub count: usize,
    /// Se pudo leer la lista (si no, `last` y `count` no dicen nada).
    pub readable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Created {
    pub at: u64,
    /// Ya había uno reciente y se usa ese.
    pub reused: bool,
}

const SR_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore";
const SR_POLICY: &str = r"SOFTWARE\Policies\Microsoft\Windows NT\SystemRestore";

fn dword(h: Hive, key: &str, name: &str) -> Option<u32> {
    reg::read(h, key, name).ok().flatten().and_then(|v| reg::as_dword(&v))
}

/// `DisableSR` apaga Restaurar sistema. (`DisableConfig` solo bloquea la ventana de
/// configuración: los puntos se siguen creando, así que no cuenta.)
fn policy_off() -> bool {
    dword(Hive::HKLM, SR_POLICY, "DisableSR") == Some(1)
}

/// Windows deja `RPSessionInterval` en 1 si la protección está activa en algún disco.
fn enabled() -> bool {
    dword(Hive::HKLM, SR_KEY, "RPSessionInterval").unwrap_or(0) != 0
}

fn variant_text(v: &VARIANT) -> Option<String> {
    let mut buf = [0u16; 512];
    unsafe { VariantToString(v, &mut buf) }.ok()?;
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..n]))
}

/// Puntos existentes, del más nuevo al más viejo.
pub fn list() -> Result<Vec<RestorePoint>, String> {
    let _com = Com::init(COINIT_MULTITHREADED);
    unsafe {
        let loc: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).map_err(|e| e.message())?;
        let empty = BSTR::new();
        let svc = loc
            .ConnectServer(&BSTR::from(r"ROOT\DEFAULT"), &empty, &empty, &empty, 0, &empty, None)
            .map_err(|e| e.message())?;
        // 10 = RPC_C_AUTHN_WINNT, 0 = RPC_C_AUTHZ_NONE.
        CoSetProxyBlanket(&svc, 10, 0, PCWSTR::null(), RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE, None, EOAC_NONE)
            .map_err(|e| e.message())?;
        let rows = svc
            .ExecQuery(
                &BSTR::from("WQL"),
                &BSTR::from("SELECT CreationTime, Description FROM SystemRestore"),
                WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                None,
            )
            .map_err(|e| e.message())?;
        // El enumerador es otro proxy: también necesita permiso de suplantación, o el
        // proveedor de Restaurar sistema responde "acceso denegado" al leer las filas.
        CoSetProxyBlanket(&rows, 10, 0, PCWSTR::null(), RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE, None, EOAC_NONE)
            .map_err(|e| e.message())?;
        let mut out = Vec::new();
        loop {
            let mut objs = [None];
            let mut n = 0;
            // Con WBEM_FLAG_RETURN_IMMEDIATELY los errores de la consulta (p. ej. acceso
            // denegado) llegan acá, no en ExecQuery: no son "fin de la lista".
            let hr = rows.Next(WBEM_INFINITE, &mut objs, &mut n);
            if hr.is_err() {
                return Err(format!("WMI 0x{:08X} {}", hr.0 as u32, hr.message()).trim().to_string());
            }
            if n == 0 {
                break;
            }
            let Some(o) = objs[0].take() else { break };
            let get = |name: &str| {
                let mut v = VARIANT::default();
                let w = windows::core::HSTRING::from(name);
                let r = o.Get(PCWSTR(w.as_ptr()), 0, &mut v, None, None).ok().and_then(|_| variant_text(&v));
                let _ = VariantClear(&mut v);
                r
            };
            if let Some(at) = get("CreationTime").as_deref().and_then(parse_cim_datetime) {
                out.push(RestorePoint { at, description: get("Description").unwrap_or_default() });
            }
            if out.len() > 500 {
                break;
            }
        }
        out.sort_by(|a, b| b.at.cmp(&a.at));
        Ok(out)
    }
}

pub fn status() -> RestoreStatus {
    let elevated = crate::system::is_elevated();
    let listed = if elevated { list().ok() } else { None };
    let readable = listed.is_some();
    let points = listed.unwrap_or_default();
    RestoreStatus {
        enabled: enabled(),
        policy: policy_off(),
        elevated,
        readable,
        count: points.len(),
        last: points.into_iter().next(),
    }
}

/// Crea un punto (o usa el de las últimas 24 horas). Puede tardar varios segundos.
pub fn create(description: &str) -> Result<Created, String> {
    if !crate::system::is_elevated() {
        return Err(t!("requires_admin"));
    }
    if policy_off() {
        return Err(t!("restore_policy"));
    }
    if !enabled() {
        return Err(t!("restore_disabled"));
    }
    // Si la lista no se puede leer, no se puede comprobar nada después: se crea y vale lo que
    // responda Windows, sin esperar un minuto a que aparezca.
    let (before, readable) = match list() {
        Ok(l) => (l, true),
        Err(_) => (vec![], false),
    };
    if let Some(p) = before.first().filter(|p| now_secs().saturating_sub(p.at) < RECENT_SECS) {
        return Ok(Created { at: p.at, reused: true });
    }
    // Windows corta la descripción en 64 caracteres. La estructura es "packed": se arma
    // el texto aparte y se copia entero.
    let mut desc = [0u16; 256];
    for (d, s) in desc.iter_mut().zip(description.encode_utf16().take(63)) {
        *d = s;
    }
    let mut info = RESTOREPOINTINFOW {
        dwEventType: BEGIN_SYSTEM_CHANGE,
        dwRestorePtType: MODIFY_SETTINGS,
        szDescription: desc,
        ..Default::default()
    };
    let mut st = STATEMGRSTATUS::default();
    let ok = unsafe { SRSetRestorePointW(&info, &mut st) }.as_bool();
    let status = st.nStatus.0;
    if !ok || status != 0 {
        return Err(match status {
            1058 => t!("restore_disabled"),
            _ => t!("restore_failed", code = status),
        });
    }
    info.dwEventType = END_SYSTEM_CHANGE;
    info.llSequenceNumber = st.llSequenceNumber;
    let mut end = STATEMGRSTATUS::default();
    unsafe {
        let _ = SRSetRestorePointW(&info, &mut end);
    }
    if !readable {
        return Ok(Created { at: now_secs(), reused: false });
    }
    // Windows termina la instantánea en segundo plano: el punto puede tardar en aparecer en la
    // lista. Se espera un rato antes de decir que no se creó.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut listed = false;
    loop {
        match list() {
            Ok(l) => {
                listed = true;
                if let Some(p) = l.into_iter().next().filter(|p| before.first().is_none_or(|b| p.at > b.at)) {
                    return Ok(Created { at: p.at, reused: false });
                }
            }
            Err(_) => {}
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    // Si la lista no se pudo leer nunca, vale lo que respondió Windows (que fue "creado").
    if listed { Err(t!("restore_not_created")) } else { Ok(Created { at: now_secs(), reused: false }) }
}

/// Abre "Restaurar sistema" de Windows (ruta fija en System32).
pub fn open_rstrui() -> Result<(), String> {
    let exe = crate::startup::windows_dir()
        .map(|w| w.join(r"System32\rstrui.exe"))
        .filter(|p| p.is_file())
        .ok_or_else(|| t!("restore_no_rstrui"))?;
    std::process::Command::new(exe).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Abre "Protección del sistema", donde se activa Restaurar sistema por disco.
pub fn open_settings() -> Result<(), String> {
    let exe = crate::startup::windows_dir()
        .map(|w| w.join(r"System32\SystemPropertiesProtection.exe"))
        .filter(|p| p.is_file())
        .ok_or_else(|| t!("restore_no_rstrui"))?;
    std::process::Command::new(exe).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Como administrador: crea un punto de restauración real (o usa el de hoy).
    /// `cargo test -- --ignored --nocapture restore_crear`
    #[test]
    #[ignore]
    fn restore_crear() {
        println!("lista antes: {:?}", list());
        let r = create("P-Rune: prueba");
        println!("lista después: {:?}", list());
        println!("{r:?}");
        let s = status();
        println!("activado={} puntos={} último={:?}", s.enabled, s.count, s.last);
        r.unwrap();
    }

    /// Muestra el estado real (no crea nada).
    #[test]
    #[ignore]
    fn estado_real() {
        let s = status();
        println!("activado={} directiva={} admin={} puntos={} último={:?}", s.enabled, s.policy, s.elevated, s.count, s.last);
    }
}
