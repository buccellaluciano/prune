//! Plan de energía activo con la API de Windows (powrprof.dll), la misma que usa
//! Configuración. Sin powercfg ni consola.

use serde::{Deserialize, Serialize};
use windows::core::GUID;
use windows::Win32::Foundation::{LocalFree, ERROR_SUCCESS, HLOCAL, WIN32_ERROR};
use windows::Win32::System::Power::{
    PowerDeleteScheme, PowerDuplicateScheme, PowerEnumerate, PowerGetActiveScheme, PowerReadFriendlyName,
    PowerSetActiveScheme, ACCESS_SCHEME,
};

pub const SAVER: GUID = GUID::from_u128(0xa1841308_3541_4fab_bc81_f71556f20b4a);
pub const BALANCED: GUID = GUID::from_u128(0x381b4222_f694_41f0_9685_ff5bb260df2e);
pub const HIGH: GUID = GUID::from_u128(0x8c5e7fda_e8bf_4a96_9a85_a6e23a8c635c);
/// Plantilla oculta de "Máximo rendimiento" (Ultimate Performance). Para usarla hay
/// que duplicarla: la copia es un plan normal con un GUID nuevo.
pub const ULTIMATE_TEMPLATE: GUID = GUID::from_u128(0xe9a42b02_d5df_448d_aa00_03f14749eb61);
/// Id de opción para "Máximo rendimiento" cuando todavía no existe la copia.
pub const ULTIMATE_NEW: &str = "ultimate";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PowerChange {
    /// GUID del plan activo antes del cambio.
    pub before: String,
    /// GUID del plan que dejamos activo.
    pub after: String,
    /// Si tuvimos que crear "Máximo rendimiento", su GUID (para borrarlo al deshacer).
    #[serde(default)]
    pub created: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub guid: GUID,
    pub name: String,
}

pub fn to_string(g: &GUID) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        g.data1, g.data2, g.data3, g.data4[0], g.data4[1], g.data4[2], g.data4[3], g.data4[4], g.data4[5], g.data4[6], g.data4[7]
    )
}

pub fn parse(s: &str) -> Option<GUID> {
    let hex: String = s.trim().trim_matches(['{', '}']).chars().filter(|c| *c != '-').collect();
    (hex.len() == 32).then(|| u128::from_str_radix(&hex, 16).ok()).flatten().map(GUID::from_u128)
}

fn check(e: WIN32_ERROR, what: &str) -> Result<(), String> {
    if e == ERROR_SUCCESS {
        Ok(())
    } else {
        let msg = windows::core::Error::from(e.to_hresult()).message();
        Err(format!("{what}: {msg} ({})", e.0))
    }
}

/// GUID devuelto por powrprof en memoria que hay que liberar con LocalFree.
unsafe fn take_guid(p: *mut GUID) -> GUID {
    let g = unsafe { *p };
    let _ = unsafe { LocalFree(Some(HLOCAL(p as _))) };
    g
}

pub fn active() -> Result<GUID, String> {
    let mut p: *mut GUID = std::ptr::null_mut();
    check(unsafe { PowerGetActiveScheme(None, &mut p) }, "No se pudo leer el plan activo")?;
    Ok(unsafe { take_guid(p) })
}

pub fn name(g: &GUID) -> String {
    let mut size = 0u32;
    let _ = unsafe { PowerReadFriendlyName(None, Some(g), None, None, None, &mut size) };
    if size == 0 {
        return to_string(g);
    }
    let mut buf = vec![0u8; size as usize];
    let r = unsafe { PowerReadFriendlyName(None, Some(g), None, None, Some(buf.as_mut_ptr()), &mut size) };
    if r != ERROR_SUCCESS {
        return to_string(g);
    }
    let units: Vec<u16> = buf.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units).trim_end_matches('\0').to_string()
}

/// Planes visibles (los que lista Configuración / powercfg /list).
pub fn list() -> Vec<Plan> {
    let mut out = Vec::new();
    for i in 0.. {
        let mut g = GUID::zeroed();
        let mut size = std::mem::size_of::<GUID>() as u32;
        let r = unsafe { PowerEnumerate(None, None, None, ACCESS_SCHEME, i, Some(&mut g as *mut GUID as *mut u8), &mut size) };
        if r != ERROR_SUCCESS {
            break;
        }
        out.push(Plan { name: name(&g), guid: g });
    }
    out
}

/// La copia de "Máximo rendimiento" que ya exista, reconocida por el nombre de la plantilla.
pub fn find_ultimate(plans: &[Plan]) -> Option<GUID> {
    let template_name = name(&ULTIMATE_TEMPLATE);
    plans
        .iter()
        .find(|p| p.guid == ULTIMATE_TEMPLATE || (p.name == template_name && ![SAVER, BALANCED, HIGH].contains(&p.guid)))
        .map(|p| p.guid)
}

/// Activa un plan. `choice` es un GUID o `ULTIMATE_NEW` (crea "Máximo rendimiento").
pub fn activate(choice: &str) -> Result<PowerChange, String> {
    let before = active()?;
    let plans = list();
    let (target, created) = if choice == ULTIMATE_NEW {
        match find_ultimate(&plans) {
            Some(g) => (g, None),
            None => {
                let mut p: *mut GUID = std::ptr::null_mut();
                check(
                    unsafe { PowerDuplicateScheme(None, &ULTIMATE_TEMPLATE, &mut p) },
                    "Windows no permitió crear el plan Máximo rendimiento",
                )?;
                let g = unsafe { take_guid(p) };
                (g, Some(g))
            }
        }
    } else {
        let g = parse(choice).ok_or("Plan de energía inválido")?;
        if !plans.iter().any(|p| p.guid == g) {
            return Err("Ese plan de energía no existe en este equipo".into());
        }
        (g, None)
    };
    if let Err(e) = check(unsafe { PowerSetActiveScheme(None, Some(&target)) }, "No se pudo activar el plan") {
        if let Some(c) = created {
            let _ = unsafe { PowerDeleteScheme(None, &c) };
        }
        return Err(e);
    }
    Ok(PowerChange { before: to_string(&before), after: to_string(&target), created: created.map(|g| to_string(&g)) })
}

/// Vuelve al plan anterior y borra el que hayamos creado.
pub fn revert(c: &PowerChange) -> Result<(), String> {
    let before = parse(&c.before).ok_or("GUID inválido en el historial")?;
    check(unsafe { PowerSetActiveScheme(None, Some(&before)) }, "No se pudo volver al plan anterior")?;
    if let Some(created) = c.created.as_deref().and_then(parse) {
        if active().ok() != Some(created) {
            check(unsafe { PowerDeleteScheme(None, &created) }, "No se pudo borrar el plan creado")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_roundtrip() {
        assert_eq!(to_string(&BALANCED), "381b4222-f694-41f0-9685-ff5bb260df2e");
        assert_eq!(parse("{381B4222-F694-41F0-9685-FF5BB260DF2E}"), Some(BALANCED));
        assert_eq!(parse("no-es-un-guid"), None);
    }

    #[test]
    fn reads_plans_without_changing_anything() {
        let plans = list();
        assert!(!plans.is_empty());
        let a = active().unwrap();
        assert!(plans.iter().any(|p| p.guid == a), "el plan activo debe estar en la lista");
        // El nombre de la plantilla oculta es lo que usamos para reconocer la copia.
        assert_ne!(name(&ULTIMATE_TEMPLATE), to_string(&ULTIMATE_TEMPLATE));
    }
}
