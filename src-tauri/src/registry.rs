//! Acceso al registro de bajo nivel. Siempre la vista de 64 bits.

use crate::catalog::{Hive, Value, ValueKind};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::io;
use winreg::enums::*;
use winreg::{RegKey, RegValue};

/// Valor crudo tal como está en el registro (para el journal y para deshacer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawValue {
    pub vtype: u32,
    pub bytes: Vec<u8>,
}

fn root(h: Hive) -> RegKey {
    RegKey::predef(match h {
        Hive::HKCU => HKEY_CURRENT_USER,
        Hive::HKLM => HKEY_LOCAL_MACHINE,
    })
}

pub fn hive_label(h: Hive) -> &'static str {
    match h {
        Hive::HKCU => "HKCU",
        Hive::HKLM => "HKLM",
    }
}

fn regtype_from_u32(v: u32) -> RegType {
    match v {
        1 => REG_SZ,
        2 => REG_EXPAND_SZ,
        3 => REG_BINARY,
        4 => REG_DWORD,
        5 => REG_DWORD_BIG_ENDIAN,
        6 => REG_LINK,
        7 => REG_MULTI_SZ,
        8 => REG_RESOURCE_LIST,
        9 => REG_FULL_RESOURCE_DESCRIPTOR,
        10 => REG_RESOURCE_REQUIREMENTS_LIST,
        11 => REG_QWORD,
        _ => REG_NONE,
    }
}

fn is_not_found(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::NotFound
}

pub fn key_exists(h: Hive, path: &str) -> io::Result<bool> {
    match root(h).open_subkey_with_flags(path, KEY_READ | KEY_WOW64_64KEY) {
        Ok(_) => Ok(true),
        Err(e) if is_not_found(&e) => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn read(h: Hive, path: &str, name: &str) -> io::Result<Option<RawValue>> {
    let key = match root(h).open_subkey_with_flags(path, KEY_READ | KEY_WOW64_64KEY) {
        Ok(k) => k,
        Err(e) if is_not_found(&e) => return Ok(None),
        Err(e) => return Err(e),
    };
    match key.get_raw_value(name) {
        Ok(v) => Ok(Some(RawValue {
            vtype: v.vtype as u32,
            bytes: v.bytes.into_owned(),
        })),
        Err(e) if is_not_found(&e) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Devuelve las claves (de la más profunda a la más alta) que no existían y
/// se crearon para poder escribir en `path`.
pub fn missing_keys(h: Hive, path: &str) -> io::Result<Vec<String>> {
    let parts: Vec<&str> = path.split('\\').collect();
    let mut missing = Vec::new();
    for i in (1..=parts.len()).rev() {
        let p = parts[..i].join("\\");
        if key_exists(h, &p)? {
            break;
        }
        missing.push(p);
    }
    Ok(missing)
}

pub fn write_raw(h: Hive, path: &str, name: &str, v: &RawValue) -> io::Result<()> {
    let (key, _) = root(h).create_subkey_with_flags(path, KEY_WRITE | KEY_WOW64_64KEY)?;
    key.set_raw_value(
        name,
        &RegValue {
            bytes: Cow::Borrowed(&v.bytes),
            vtype: regtype_from_u32(v.vtype),
        },
    )
}

pub fn delete_value(h: Hive, path: &str, name: &str) -> io::Result<()> {
    let key = match root(h).open_subkey_with_flags(path, KEY_WRITE | KEY_WOW64_64KEY) {
        Ok(k) => k,
        Err(e) if is_not_found(&e) => return Ok(()),
        Err(e) => return Err(e),
    };
    match key.delete_value(name) {
        Err(e) if !is_not_found(&e) => Err(e),
        _ => Ok(()),
    }
}

pub fn delete_tree(h: Hive, path: &str) -> io::Result<()> {
    match root(h).delete_subkey_all(path) {
        Err(e) if !is_not_found(&e) => Err(e),
        _ => Ok(()),
    }
}

/// Borra la clave solo si quedó vacía (sin valores ni subclaves).
pub fn delete_key_if_empty(h: Hive, path: &str) -> io::Result<()> {
    let key = match root(h).open_subkey_with_flags(path, KEY_READ | KEY_WOW64_64KEY) {
        Ok(k) => k,
        Err(e) if is_not_found(&e) => return Ok(()),
        Err(e) => return Err(e),
    };
    let info = key.query_info()?;
    drop(key);
    if info.values == 0 && info.sub_keys == 0 {
        root(h).delete_subkey_with_flags(path, KEY_WOW64_64KEY)?;
    }
    Ok(())
}

pub fn encode(kind: ValueKind, v: &Value) -> RawValue {
    match (kind, v) {
        (ValueKind::Dword, Value::Int(n)) => RawValue {
            vtype: REG_DWORD as u32,
            bytes: (*n as u32).to_le_bytes().to_vec(),
        },
        (ValueKind::Sz, Value::Str(s)) => {
            let mut bytes = Vec::with_capacity((s.len() + 1) * 2);
            for u in s.encode_utf16().chain(std::iter::once(0)) {
                bytes.extend_from_slice(&u.to_le_bytes());
            }
            RawValue { vtype: REG_SZ as u32, bytes }
        }
        (ValueKind::Binary, Value::Str(s)) => RawValue {
            vtype: REG_BINARY as u32,
            bytes: parse_hex(s).expect("hex validado en el catálogo"),
        },
        // El catálogo se valida al cargar, esto no debería pasar.
        _ => panic!("tipo de valor inconsistente"),
    }
}

fn decode_dword(v: &RawValue) -> Option<u32> {
    (v.vtype == REG_DWORD as u32 && v.bytes.len() >= 4)
        .then(|| u32::from_le_bytes([v.bytes[0], v.bytes[1], v.bytes[2], v.bytes[3]]))
}

fn decode_sz(v: &RawValue) -> Option<String> {
    if v.vtype != REG_SZ as u32 && v.vtype != REG_EXPAND_SZ as u32 {
        return None;
    }
    let units: Vec<u16> = v
        .bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let s = String::from_utf16_lossy(&units);
    Some(s.trim_end_matches('\0').to_string())
}

/// ¿El valor actual equivale al valor deseado?
pub fn matches(current: Option<&RawValue>, kind: ValueKind, want: &Value) -> bool {
    let Some(cur) = current else { return false };
    match (kind, want) {
        (ValueKind::Dword, Value::Int(n)) => decode_dword(cur) == Some(*n as u32),
        (ValueKind::Sz, Value::Str(s)) => decode_sz(cur).as_deref() == Some(s.as_str()),
        (ValueKind::Binary, Value::Str(s)) => {
            cur.vtype == REG_BINARY as u32 && parse_hex(s).is_some_and(|b| b == cur.bytes)
        }
        _ => false,
    }
}

/// "90 12 03 80" → bytes. Exige pares hex separados por espacios y al menos uno.
pub fn parse_hex(s: &str) -> Option<Vec<u8>> {
    let bytes: Option<Vec<u8>> = s
        .split_whitespace()
        .map(|b| (b.len() == 2).then(|| u8::from_str_radix(b, 16).ok()).flatten())
        .collect();
    bytes.filter(|b| !b.is_empty())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// Representación legible para la vista previa.
pub fn describe(v: Option<&RawValue>) -> String {
    match v {
        None => "(no existe)".into(),
        Some(r) => {
            if let Some(n) = decode_dword(r) {
                format!("{n}")
            } else if let Some(s) = decode_sz(r) {
                format!("\"{s}\"")
            } else if r.vtype == REG_BINARY as u32 {
                hex(&r.bytes)
            } else {
                format!("<tipo {} · {} bytes>", r.vtype, r.bytes.len())
            }
        }
    }
}

pub fn describe_value(kind: ValueKind, v: &Value) -> String {
    match (kind, v) {
        (ValueKind::Binary, Value::Str(s)) => parse_hex(s).map_or_else(|| s.clone(), |b| hex(&b)),
        (_, Value::Int(n)) => n.to_string(),
        (_, Value::Str(s)) => format!("\"{s}\""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dword_roundtrip() {
        let r = encode(ValueKind::Dword, &Value::Int(1));
        assert!(matches(Some(&r), ValueKind::Dword, &Value::Int(1)));
        assert!(!matches(Some(&r), ValueKind::Dword, &Value::Int(0)));
        assert!(!matches(None, ValueKind::Dword, &Value::Int(0)));
    }

    #[test]
    fn sz_roundtrip_and_empty_default() {
        let r = encode(ValueKind::Sz, &Value::Str("506".into()));
        assert!(matches(Some(&r), ValueKind::Sz, &Value::Str("506".into())));
        let empty = encode(ValueKind::Sz, &Value::Str(String::new()));
        assert!(matches(Some(&empty), ValueKind::Sz, &Value::Str(String::new())));
        // Un REG_SZ vacío sin terminador nulo también cuenta como "".
        let bare = RawValue { vtype: REG_SZ as u32, bytes: vec![] };
        assert!(matches(Some(&bare), ValueKind::Sz, &Value::Str(String::new())));
    }

    #[test]
    fn type_mismatch_is_not_a_match() {
        let s = encode(ValueKind::Sz, &Value::Str("1".into()));
        assert!(!matches(Some(&s), ValueKind::Dword, &Value::Int(1)));
    }

    #[test]
    fn binary_roundtrip_and_hex_parsing() {
        let want = Value::Str("90 12 03 80 10 00 00 00".into());
        let r = encode(ValueKind::Binary, &want);
        assert_eq!(r.vtype, REG_BINARY as u32);
        assert_eq!(r.bytes, [0x90, 0x12, 0x03, 0x80, 0x10, 0, 0, 0]);
        assert!(matches(Some(&r), ValueKind::Binary, &want));
        // Minúsculas y espacios extra son el mismo valor.
        assert!(matches(Some(&r), ValueKind::Binary, &Value::Str(" 90 12 03 80 10 00 00 00 ".to_lowercase())));
        assert!(!matches(Some(&r), ValueKind::Binary, &Value::Str("9E 1E 07 80 12 00 00 00".into())));
        assert_eq!(describe(Some(&r)), "90 12 03 80 10 00 00 00");
        assert_eq!(describe_value(ValueKind::Binary, &Value::Str("9e 1e".into())), "9E 1E");
        for bad in ["", "9", "zz", "901203", "90 1"] {
            assert!(parse_hex(bad).is_none(), "{bad:?}");
        }
    }
}
