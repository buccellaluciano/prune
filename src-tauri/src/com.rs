//! Ayudas chicas para COM y fechas, compartidas por Inicio, Restaurar sistema y perfiles.

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT};
use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_I4};

/// Inicializa COM en el hilo actual y lo libera al salir, solo si lo inicializó esta llamada.
/// Si el hilo ya tenía COM en otro modo, se usa como está.
pub struct Com(bool);

impl Com {
    pub fn init(mode: COINIT) -> Self {
        Self(unsafe { CoInitializeEx(None, mode) }.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

/// VARIANT con un entero (índices de colecciones COM, que empiezan en 1).
pub fn variant_i4(n: i32) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_I4,
                Anonymous: VARIANT_0_0_0 { lVal: n },
                ..Default::default()
            }),
        },
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Días desde 1970-01-01 para una fecha del calendario civil (algoritmo de H. Hinnant).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Inverso de `days_from_civil`.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// "2026-11-18T15:04:05Z" (UTC), el formato que usa Windows Update.
pub fn iso_utc(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem / 60 % 60, rem % 60)
}

/// Lee "2026-11-18T15:04:05Z" (también sin la Z o con fracción de segundo).
pub fn parse_iso_utc(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<u32>().ok();
    let (y, mo, d, h, mi, se) = (n(0..4)?, n(5..7)?, n(8..10)?, n(11..13)?, n(14..16)?, n(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let days = days_from_civil(y as i64, mo, d);
    u64::try_from(days * 86400 + (h * 3600 + mi * 60 + se) as i64).ok()
}

/// Fecha de WMI (CIM_DATETIME): "20261111153012.123456-180", hora local con el desfase
/// en minutos respecto de UTC.
pub fn parse_cim_datetime(s: &str) -> Option<u64> {
    if s.len() < 25 || !s.is_ascii() {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (n(0..4)?, n(4..6)?, n(6..8)?, n(8..10)?, n(10..12)?, n(12..14)?);
    let sign = match &s[21..22] {
        "+" => 1,
        "-" => -1,
        _ => return None,
    };
    let offset = sign * n(22..25)?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let local = days_from_civil(y, mo as u32, d as u32) * 86400 + h * 3600 + mi * 60 + se;
    u64::try_from(local - offset * 60).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_roundtrip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(days_from_civil(2026, 11, 11)), (2026, 11, 11));
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        let t = parse_iso_utc("2026-11-18T15:04:05Z").unwrap();
        assert_eq!(iso_utc(t), "2026-11-18T15:04:05Z");
        assert_eq!(parse_iso_utc("2026-11-18T15:04:05"), Some(t));
        assert!(parse_iso_utc("ayer").is_none());
        assert!(parse_iso_utc("2026-13-18T15:04:05Z").is_none());
    }

    #[test]
    fn cim_datetime_uses_the_offset() {
        // 12:00 en UTC-3 son las 15:00 UTC.
        let a = parse_cim_datetime("20261111120000.000000-180").unwrap();
        assert_eq!(iso_utc(a), "2026-11-11T15:00:00Z");
        let b = parse_cim_datetime("20261111150000.000000+000").unwrap();
        assert_eq!(a, b);
        assert!(parse_cim_datetime("2026").is_none());
    }
}
