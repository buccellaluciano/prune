//! Windows Update: pausar y horas activas.
//!
//! Se escriben exactamente los mismos valores que escribe Configuración > Windows Update
//! (`HKLM\SOFTWARE\Microsoft\WindowsUpdate\UX\Settings`), dentro de los límites que pone
//! Windows: la pausa dura como máximo lo que Windows permite (35 días salvo que diga otra
//! cosa) y las horas activas cubren como máximo 18 horas. Si una directiva de la
//! organización maneja algo de esto, no se toca. Cada cambio va al historial con los
//! valores de antes, para deshacerlo exacto.

use crate::catalog::{Hive, Value, ValueKind};
use crate::com::{iso_utc, now_secs, parse_iso_utc};
use crate::registry::{self as reg, RawValue};
use serde::{Deserialize, Serialize};

const KEY: &str = r"SOFTWARE\Microsoft\WindowsUpdate\UX\Settings";
const POLICY: &str = r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate";

const PAUSE: [&str; 6] = [
    "PauseFeatureUpdatesStartTime",
    "PauseFeatureUpdatesEndTime",
    "PauseQualityUpdatesStartTime",
    "PauseQualityUpdatesEndTime",
    "PauseUpdatesStartTime",
    "PauseUpdatesExpiryTime",
];
const HOURS: [&str; 3] = ["ActiveHoursStart", "ActiveHoursEnd", "SmartActiveHoursState"];

/// Lo que permite Windows si no dice otra cosa.
pub const DEFAULT_MAX_DAYS: u32 = 35;
pub const MAX_ACTIVE_SPAN: u32 = 18;

#[derive(Debug, Clone, Serialize)]
pub struct WuStatus {
    /// Pausado hasta (segundos Unix), si la pausa sigue vigente.
    pub paused_until: Option<u64>,
    pub max_pause_days: u32,
    pub active_start: Option<u32>,
    pub active_end: Option<u32>,
    /// None: Windows no lo dejó escrito.
    pub smart_hours: Option<bool>,
    /// Una directiva no deja pausar desde Configuración.
    pub pause_policy: bool,
    /// Una directiva fija las horas activas.
    pub hours_policy: bool,
    pub elevated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WuKind {
    Pause,
    Resume,
    Hours,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WuValue {
    pub name: String,
    pub before: Option<RawValue>,
    pub after: Option<RawValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WuChange {
    pub kind: WuKind,
    pub values: Vec<WuValue>,
    /// Para el título del historial: días de pausa u horas "inicio-fin" (0-0 = automáticas).
    #[serde(default)]
    pub detail: String,
}

fn read(name: &str) -> Option<RawValue> {
    reg::read(Hive::HKLM, KEY, name).ok().flatten()
}

fn policy_dword(name: &str) -> Option<u32> {
    reg::read(Hive::HKLM, POLICY, name).ok().flatten().and_then(|v| reg::as_dword(&v))
}

fn max_pause_days() -> u32 {
    // Windows lo amplía o achica con este valor; nunca se pasa de lo que dice.
    read("FlightSettingsMaxPauseDays")
        .and_then(|v| reg::as_dword(&v))
        .filter(|d| (1..=DEFAULT_MAX_DAYS).contains(d))
        .unwrap_or(DEFAULT_MAX_DAYS)
}

fn pause_policy() -> bool {
    policy_dword("SetDisablePauseUXAccess") == Some(1)
}

fn hours_policy() -> bool {
    policy_dword("SetActiveHours") == Some(1)
}

pub fn status() -> WuStatus {
    let now = now_secs();
    let paused_until = read("PauseUpdatesExpiryTime")
        .and_then(|v| reg::as_text(&v))
        .and_then(|s| parse_iso_utc(&s))
        .filter(|&t| t > now);
    let dw = |n: &str| read(n).and_then(|v| reg::as_dword(&v));
    WuStatus {
        paused_until,
        max_pause_days: max_pause_days(),
        active_start: dw("ActiveHoursStart").filter(|h| *h < 24),
        active_end: dw("ActiveHoursEnd").filter(|h| *h < 24),
        smart_hours: dw("SmartActiveHoursState").map(|v| v != 0),
        pause_policy: pause_policy(),
        hours_policy: hours_policy(),
        elevated: crate::system::is_elevated(),
    }
}

fn sz(s: &str) -> RawValue {
    reg::encode(ValueKind::Sz, &Value::Str(s.into()))
}

fn dword(n: u32) -> RawValue {
    reg::encode(ValueKind::Dword, &Value::Int(n as i64))
}

/// ¿Es un valor que el historial acepta para este nombre? Lo usa tanto deshacer (contra un
/// historial manipulado) como aplicar (para no guardar nada que después no se pueda deshacer).
fn value_ok(name: &str, r: &Option<RawValue>) -> bool {
    // Una fecha legítima nunca pasa del máximo de Windows contado desde hoy (con un día de
    // margen): así un historial manipulado no puede "restaurar" una pausa eterna.
    let latest = now_secs() + (DEFAULT_MAX_DAYS as u64 + 1) * 86400;
    let Some(r) = r else { return true };
    if PAUSE.contains(&name) {
        (r.vtype == 1 || r.vtype == 2)
            && r.bytes.len() <= 128
            && reg::as_text(r).and_then(|s| parse_iso_utc(&s)).is_some_and(|t| t <= latest)
    } else if HOURS.contains(&name) {
        let max = if name == "SmartActiveHoursState" { 1 } else { 23 };
        r.vtype == 4 && r.bytes.len() == 4 && reg::as_dword(r).is_some_and(|n| n <= max)
    } else {
        false
    }
}

/// Escribe todo o nada: si algo falla, vuelve lo ya escrito a como estaba. None si ya estaba así.
fn write(kind: WuKind, wanted: Vec<(&str, Option<RawValue>)>, detail: String) -> Result<Option<WuChange>, String> {
    if !crate::system::is_elevated() {
        return Err(t!("requires_admin"));
    }
    // Antes de tocar nada: si lo que hay hoy no se podría restaurar desde el historial (por
    // ejemplo, una pausa más larga que la de Windows puesta con otro programa), no se cambia.
    let mut befores = Vec::new();
    for (name, _) in &wanted {
        let before = reg::read(Hive::HKLM, KEY, name).map_err(|e| e.to_string())?;
        if !value_ok(name, &before) {
            return Err(t!("wu_unrestorable"));
        }
        befores.push(before);
    }
    if wanted.iter().zip(&befores).all(|((_, after), before)| after == before) {
        return Ok(None);
    }
    let mut done: Vec<WuValue> = Vec::new();
    for ((name, after), before) in wanted.into_iter().zip(befores) {
        let r = match &after {
            Some(v) => reg::write_raw(Hive::HKLM, KEY, name, v),
            None => reg::delete_value(Hive::HKLM, KEY, name),
        };
        if let Err(e) = r {
            for d in done.iter().rev() {
                let _ = put(&d.name, d.before.as_ref());
            }
            return Err(e.to_string());
        }
        done.push(WuValue { name: name.into(), before, after });
    }
    Ok(Some(WuChange { kind, values: done, detail }))
}

fn put(name: &str, v: Option<&RawValue>) -> std::io::Result<()> {
    match v {
        Some(v) => reg::write_raw(Hive::HKLM, KEY, name, v),
        None => reg::delete_value(Hive::HKLM, KEY, name),
    }
}

pub fn pause(days: u32) -> Result<Option<WuChange>, String> {
    if pause_policy() {
        return Err(t!("wu_policy"));
    }
    let max = max_pause_days();
    if days == 0 || days > max {
        return Err(t!("wu_pause_days", max = max));
    }
    let now = now_secs();
    let start = iso_utc(now);
    let end = iso_utc(now + days as u64 * 86400);
    let wanted = PAUSE
        .iter()
        .map(|&n| (n, Some(sz(if n.ends_with("StartTime") { &start } else { &end }))))
        .collect();
    write(WuKind::Pause, wanted, days.to_string())
}

pub fn resume() -> Result<Option<WuChange>, String> {
    if pause_policy() {
        return Err(t!("wu_policy"));
    }
    write(WuKind::Resume, PAUSE.iter().map(|&n| (n, None)).collect(), String::new())
}

/// Horas activas: `None` = que Windows las ajuste solo según el uso.
pub fn hours(manual: Option<(u32, u32)>) -> Result<Option<WuChange>, String> {
    if hours_policy() {
        return Err(t!("wu_policy"));
    }
    match manual {
        None => write(WuKind::Hours, vec![("SmartActiveHoursState", Some(dword(1)))], "auto".into()),
        Some((start, end)) => {
            let span = (end + 24 - start) % 24;
            if start > 23 || end > 23 || span == 0 || span > MAX_ACTIVE_SPAN {
                return Err(t!("wu_hours_invalid", max = MAX_ACTIVE_SPAN));
            }
            write(
                WuKind::Hours,
                vec![
                    ("SmartActiveHoursState", Some(dword(0))),
                    ("ActiveHoursStart", Some(dword(start))),
                    ("ActiveHoursEnd", Some(dword(end))),
                ],
                format!("{start}-{end}"),
            )
        }
    }
}

/// El historial lo puede escribir el usuario y deshacer corre como admin: solo nombres de
/// esta lista y con el tipo que usa Windows para cada uno.
pub fn validate_saved(c: &WuChange) -> Result<(), String> {
    if c.values.is_empty() || c.values.len() > PAUSE.len() + HOURS.len() {
        return Err(t!("wu_bad_entry"));
    }
    for v in &c.values {
        if !value_ok(&v.name, &v.before) || !value_ok(&v.name, &v.after) {
            return Err(t!("wu_bad_entry"));
        }
    }
    if !crate::system::is_elevated() {
        return Err(t!("requires_admin"));
    }
    Ok(())
}

/// Nombres cuyo valor cambió después (ni el que dejamos ni el de antes).
pub fn conflicts(c: &WuChange) -> bool {
    c.values.iter().any(|v| {
        let now = read(&v.name);
        now != v.after && now != v.before
    })
}

pub fn restore(c: &WuChange) -> Result<(), String> {
    let mut err = None;
    for v in c.values.iter().rev() {
        if let Err(e) = put(&v.name, v.before.as_ref()) {
            err.get_or_insert(e.to_string());
        }
    }
    err.map_or(Ok(()), Err)
}

pub fn label(c: &WuChange) -> String {
    match c.kind {
        WuKind::Pause => t!("label_wu_pause", days = c.detail),
        WuKind::Resume => t!("label_wu_resume"),
        WuKind::Hours if c.detail == "auto" => t!("label_wu_hours_auto"),
        WuKind::Hours => {
            let (a, b) = c.detail.split_once('-').unwrap_or(("?", "?"));
            t!("label_wu_hours", start = a, end = b)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tampered_entries_are_rejected() {
        let bad = |name: &str, after: Option<RawValue>| WuChange {
            kind: WuKind::Pause,
            values: vec![WuValue { name: name.into(), before: None, after }],
            detail: String::new(),
        };
        assert!(validate_saved(&bad("NoAutoUpdate", Some(dword(1)))).is_err(), "nombre fuera de la lista");
        assert!(validate_saved(&bad("ActiveHoursStart", Some(sz("x")))).is_err(), "tipo incorrecto");
        assert!(validate_saved(&bad("PauseUpdatesExpiryTime", Some(dword(1)))).is_err(), "tipo incorrecto");
        let long = RawValue { vtype: 1, bytes: vec![b'a'; 400] };
        assert!(validate_saved(&bad("PauseUpdatesExpiryTime", Some(long))).is_err(), "demasiado largo");
        let forever = sz("2099-01-01T00:00:00Z");
        assert_eq!(validate_saved(&bad("PauseUpdatesExpiryTime", Some(forever))).unwrap_err(), t!("wu_bad_entry"), "pausa eterna");
        assert_eq!(validate_saved(&bad("PauseUpdatesExpiryTime", Some(sz("mañana")))).unwrap_err(), t!("wu_bad_entry"));
        assert_eq!(validate_saved(&bad("ActiveHoursStart", Some(dword(40)))).unwrap_err(), t!("wu_bad_entry"));
        assert_eq!(validate_saved(&bad("SmartActiveHoursState", Some(dword(7)))).unwrap_err(), t!("wu_bad_entry"));
        assert!(validate_saved(&WuChange { kind: WuKind::Pause, values: vec![], detail: String::new() }).is_err());
    }

    #[test]
    fn hour_ranges() {
        // Sin admin, `hours` falla antes de escribir; acá solo se prueba la regla de 18 horas.
        let span = |s: u32, e: u32| (e + 24 - s) % 24;
        assert_eq!(span(8, 17), 9);
        assert_eq!(span(20, 2), 6);
        assert_eq!(span(5, 23), 18);
        assert_eq!(span(9, 9), 0);
    }

    /// Como administrador: pausa 1 día y cambia las horas activas, y vuelve todo exacto.
    /// `cargo test -- --ignored --nocapture wu_aplicar`
    #[test]
    #[ignore]
    fn wu_aplicar_y_restaurar() {
        let names: Vec<&str> = PAUSE.iter().chain(HOURS.iter()).copied().collect();
        let snapshot = || names.iter().map(|n| read(n)).collect::<Vec<_>>();
        let before = snapshot();

        let p = pause(1).unwrap().expect("tenía que cambiar");
        let s = status();
        let until = s.paused_until.expect("tiene que quedar pausado");
        assert!(until > now_secs() + 86000 && until < now_secs() + 86800, "{until}");
        println!("pausado hasta {}", iso_utc(until));
        assert!(!conflicts(&p));
        validate_saved(&p).unwrap();
        restore(&p).unwrap();
        assert_eq!(snapshot(), before, "la pausa no volvió exacta");

        let h = hours(Some((9, 18))).unwrap().expect("tenía que cambiar");
        let s = status();
        assert_eq!((s.active_start, s.active_end, s.smart_hours), (Some(9), Some(18), Some(false)));
        assert!(hours(Some((9, 18))).unwrap().is_none(), "repetir lo mismo no escribe ni anota nada");
        restore(&h).unwrap();
        assert_eq!(snapshot(), before, "las horas no volvieron exactas");

        assert!(hours(Some((5, 0))).is_err(), "19 horas no");
        assert!(pause(36).is_err(), "más de lo que permite Windows no");
        assert_eq!(snapshot(), before);
        println!("ok: todo volvió a como estaba");
    }

    #[test]
    #[ignore]
    fn estado_real() {
        println!("{:?}", status());
    }
}
