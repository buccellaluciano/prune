//! Traducciones del backend: mensajes que llegan a la interfaz y textos del catálogo.
//!
//! El español es la fuente: los textos del catálogo viven en los `.toml` y los mensajes en
//! `locales/es.json`. Cada idioma tiene su `locales/<idioma>.json` con los mismos mensajes
//! y la traducción de cada tweak, categoría, app y programa, por id. Los tests verifican
//! que ningún idioma tenga huecos ni ids que no existen.

use crate::catalog::{BloatApp, Choice, Tweak, WingetApp};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const LANGS: [&str; 9] = ["es", "en", "pt-BR", "fr", "de", "it", "zh-CN", "ja", "ru"];

const SOURCES: [&str; 9] = [
    include_str!("../locales/es.json"),
    include_str!("../locales/en.json"),
    include_str!("../locales/pt-BR.json"),
    include_str!("../locales/fr.json"),
    include_str!("../locales/de.json"),
    include_str!("../locales/it.json"),
    include_str!("../locales/zh-CN.json"),
    include_str!("../locales/ja.json"),
    include_str!("../locales/ru.json"),
];

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    #[serde(default)]
    pub msg: HashMap<String, String>,
    #[serde(default)]
    pub category: HashMap<String, String>,
    #[serde(default)]
    pub tweak: HashMap<String, TweakText>,
    #[serde(default)]
    pub bloat: HashMap<String, BloatText>,
    #[serde(default)]
    pub app_group: HashMap<String, String>,
    #[serde(default)]
    pub app: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TweakText {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub choices: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BloatText {
    pub title: String,
    #[serde(default)]
    pub note: Option<String>,
}

static CURRENT: AtomicUsize = AtomicUsize::new(0);

fn bundles() -> &'static [Bundle] {
    static B: OnceLock<Vec<Bundle>> = OnceLock::new();
    B.get_or_init(|| {
        SOURCES
            .iter()
            .zip(LANGS)
            .map(|(s, l)| serde_json::from_str(s).unwrap_or_else(|e| panic!("locales/{l}.json inválido: {e}")))
            .collect()
    })
}

/// Cambia el idioma de los mensajes y del catálogo. Solo acepta los de `LANGS`.
pub fn set(lang: &str) -> Result<(), String> {
    let i = LANGS.iter().position(|l| *l == lang).ok_or_else(|| format!("idioma no soportado: {lang}"))?;
    CURRENT.store(i, Ordering::Relaxed);
    Ok(())
}

fn cur() -> &'static Bundle {
    &bundles()[CURRENT.load(Ordering::Relaxed)]
}

fn es() -> &'static Bundle {
    &bundles()[0]
}

/// Mensaje en el idioma actual (o en español si faltara, o la clave si tampoco existe).
pub fn msg(key: &str) -> String {
    msg_in(cur(), key)
}

fn msg_in(b: &Bundle, key: &str) -> String {
    b.msg.get(key).or_else(|| es().msg.get(key)).cloned().unwrap_or_else(|| key.to_string())
}

/// Mensaje con valores: cada `{nombre}` del texto se reemplaza por su valor.
pub fn fmt(key: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut s = msg(key);
    for (name, value) in args {
        s = s.replace(&format!("{{{name}}}"), &value.to_string());
    }
    s
}

// ───────── Textos del catálogo ─────────

pub fn category(id: &str, fallback: &str) -> String {
    cur().category.get(id).cloned().unwrap_or_else(|| fallback.to_string())
}

pub fn tweak_title(t: &Tweak) -> String {
    cur().tweak.get(&t.id).map_or_else(|| t.title.clone(), |x| x.title.clone())
}

pub fn tweak_description(t: &Tweak) -> String {
    cur().tweak.get(&t.id).map_or_else(|| t.description.clone(), |x| x.description.clone())
}

pub fn choice_label(t: &Tweak, c: &Choice) -> String {
    cur().tweak.get(&t.id).and_then(|x| x.choices.get(&c.id)).cloned().unwrap_or_else(|| c.label.clone())
}

pub fn bloat_title(a: &BloatApp) -> String {
    cur().bloat.get(&a.name).map_or_else(|| a.title.clone(), |x| x.title.clone())
}

pub fn bloat_note(a: &BloatApp) -> Option<String> {
    a.note.as_ref()?;
    cur().bloat.get(&a.name).and_then(|x| x.note.clone()).or_else(|| a.note.clone())
}

pub fn app_group(id: &str, fallback: &str) -> String {
    cur().app_group.get(id).cloned().unwrap_or_else(|| fallback.to_string())
}

pub fn app_description(a: &WingetApp) -> String {
    cur().app.get(&a.id).cloned().unwrap_or_else(|| a.description.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;
    use std::collections::BTreeSet;

    fn placeholders(s: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut rest = s;
        while let Some(i) = rest.find('{') {
            let Some(j) = rest[i..].find('}') else { break };
            out.insert(rest[i + 1..i + j].to_string());
            rest = &rest[i + j + 1..];
        }
        out
    }

    fn keys<V>(m: &HashMap<String, V>) -> BTreeSet<&str> {
        m.keys().map(String::as_str).collect()
    }

    #[test]
    fn todos_los_idiomas_cargan() {
        assert_eq!(bundles().len(), LANGS.len());
        assert!(es().category.is_empty() && es().tweak.is_empty(), "el catálogo en español vive en los .toml");
    }

    #[test]
    fn mensajes_completos_y_con_los_mismos_valores() {
        let base = &es().msg;
        for (lang, b) in LANGS.iter().zip(bundles()).skip(1) {
            assert_eq!(keys(&b.msg), keys(base), "{lang}: mensajes distintos de es");
            for (k, v) in base {
                assert_eq!(placeholders(&b.msg[k]), placeholders(v), "{lang}: {k} cambia los {{valores}}");
            }
        }
    }

    #[test]
    fn catalogo_traducido_completo() {
        let tw = catalog::tweaks();
        let bl = catalog::bloat();
        let ap = catalog::apps();
        for (lang, b) in LANGS.iter().zip(bundles()).skip(1) {
            let cats: BTreeSet<&str> = tw.category.iter().map(|c| c.id.as_str()).collect();
            assert_eq!(keys(&b.category), cats, "{lang}: categorías");
            let ids: BTreeSet<&str> = tw.tweak.iter().map(|t| t.id.as_str()).collect();
            assert_eq!(keys(&b.tweak), ids, "{lang}: tweaks");
            for t in &tw.tweak {
                let x = &b.tweak[&t.id];
                assert!(!x.title.trim().is_empty() && !x.description.trim().is_empty(), "{lang}: {} vacío", t.id);
                let choices: BTreeSet<&str> = t.choices.iter().map(|c| c.id.as_str()).collect();
                assert_eq!(keys(&x.choices), choices, "{lang}: opciones de {}", t.id);
            }
            let names: BTreeSet<&str> = bl.app.iter().map(|a| a.name.as_str()).collect();
            assert_eq!(keys(&b.bloat), names, "{lang}: bloatware");
            for a in &bl.app {
                assert_eq!(b.bloat[&a.name].note.is_some(), a.note.is_some(), "{lang}: nota de {}", a.name);
            }
            let groups: BTreeSet<&str> = ap.group.iter().map(|g| g.id.as_str()).collect();
            assert_eq!(keys(&b.app_group), groups, "{lang}: grupos de programas");
            let apps: BTreeSet<&str> = ap.app.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(keys(&b.app), apps, "{lang}: programas");
        }
    }

    /// Sin cambiar el idioma global: los tests corren en paralelo y otros esperan español.
    #[test]
    fn idiomas_y_valores() {
        assert!(set("xx").is_err());
        let en = &bundles()[LANGS.iter().position(|l| *l == "en").unwrap()];
        assert_eq!(msg_in(en, "requires_admin"), "Requires running as administrator");
        assert_eq!(msg_in(en, "no_existe"), "no_existe");
        assert_eq!(fmt("drive_unavailable", &[("letter", &"Z")]), "Unidad no disponible: Z");
    }

    /// Vuelca los textos del catálogo en español a JSON, como base para traducir:
    /// `cargo test dump_catalogo -- --ignored` (escribe en %TEMP%\catalog-es.json).
    #[test]
    #[ignore]
    fn dump_catalogo() {
        use serde_json::{Map, Value, json};
        let tw = catalog::tweaks();
        let mut out = Map::new();
        out.insert("category".into(), tw.category.iter().map(|c| (c.id.clone(), json!(c.title))).collect::<Map<_, _>>().into());
        let tweaks: Map<String, Value> = tw
            .tweak
            .iter()
            .map(|t| {
                let mut v = json!({ "title": t.title, "description": t.description });
                if !t.choices.is_empty() {
                    v["choices"] = t.choices.iter().map(|c| (c.id.clone(), json!(c.label))).collect::<Map<_, _>>().into();
                }
                (t.id.clone(), v)
            })
            .collect();
        out.insert("tweak".into(), tweaks.into());
        let bloat: Map<String, Value> = catalog::bloat()
            .app
            .iter()
            .map(|a| {
                let mut v = json!({ "title": a.title });
                if let Some(n) = &a.note {
                    v["note"] = json!(n);
                }
                (a.name.clone(), v)
            })
            .collect();
        out.insert("bloat".into(), bloat.into());
        let ap = catalog::apps();
        out.insert("app_group".into(), ap.group.iter().map(|g| (g.id.clone(), json!(g.title))).collect::<Map<_, _>>().into());
        out.insert("app".into(), ap.app.iter().map(|a| (a.id.clone(), json!(a.description))).collect::<Map<_, _>>().into());
        let path = std::env::temp_dir().join("catalog-es.json");
        std::fs::write(&path, serde_json::to_string_pretty(&Value::Object(out)).unwrap()).unwrap();
        println!("{}", path.display());
    }
}
