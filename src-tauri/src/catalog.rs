//! Catálogo declarativo embebido en el binario (no se descarga nada en runtime).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::OnceLock;

const TWEAKS_TOML: &str = include_str!("../catalog/tweaks.toml");
const BLOAT_TOML: &str = include_str!("../catalog/bloat.toml");
const APPS_TOML: &str = include_str!("../catalog/apps.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hive {
    HKCU,
    HKLM,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueKind {
    Dword,
    Sz,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Int(i64),
    Str(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Safe,
    Moderate,
    Advanced,
}

/// Qué hay que reiniciar para que el cambio se vea. Ordenado de menor a mayor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Restart {
    #[default]
    None,
    Explorer,
    Logoff,
    Reboot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Op {
    pub hive: Hive,
    pub path: String,
    #[serde(default)]
    pub name: String,
    pub kind: ValueKind,
    /// Valor al aplicar. Los tweaks de opción múltiple no lo usan: los valores van en cada opción.
    #[serde(default)]
    pub on: Option<Value>,
    /// `None` => revertir borra el valor.
    #[serde(default)]
    pub off: Option<Value>,
    #[serde(default)]
    pub delete_key_on_revert: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tweak {
    pub id: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub risk: Risk,
    #[serde(default)]
    pub restart: Restart,
    #[serde(default)]
    pub min_build: Option<u32>,
    #[serde(default)]
    pub max_build: Option<u32>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub ops: Vec<Op>,
    /// Si hay opciones, el tweak se muestra como selector en vez de switch.
    #[serde(default, rename = "choice")]
    pub choices: Vec<Choice>,
}

/// Opción de un tweak de opción múltiple: un valor por cada operación, en orden.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub id: String,
    pub label: String,
    pub values: Vec<Value>,
    /// Lo que usa Windows cuando el valor no existe. Exactamente una por tweak.
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub min_build: Option<u32>,
}

impl Choice {
    pub fn supported_on(&self, build: u32) -> bool {
        self.min_build.is_none_or(|m| build >= m)
    }
}

impl Tweak {
    pub fn is_choice(&self) -> bool {
        !self.choices.is_empty()
    }
    pub fn find_choice(&self, id: &str) -> Option<&Choice> {
        self.choices.iter().find(|c| c.id == id)
    }
    pub fn requires_admin(&self) -> bool {
        self.ops.iter().any(|o| o.hive == Hive::HKLM)
    }
    pub fn supported_on(&self, build: u32) -> bool {
        self.min_build.is_none_or(|m| build >= m) && self.max_build.is_none_or(|m| build <= m)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub id: String,
    pub title: String,
    pub icon: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TweakCatalog {
    pub category: Vec<Category>,
    pub tweak: Vec<Tweak>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    #[default]
    Exact,
    Prefix,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BloatApp {
    pub name: String,
    pub title: String,
    pub tier: Risk,
    #[serde(default, rename = "match")]
    pub match_mode: MatchMode,
    #[serde(default)]
    pub note: Option<String>,
}

impl BloatApp {
    pub fn matches(&self, package_name: &str) -> bool {
        match self.match_mode {
            MatchMode::Exact => package_name.eq_ignore_ascii_case(&self.name),
            MatchMode::Prefix => package_name
                .to_ascii_lowercase()
                .starts_with(&self.name.to_ascii_lowercase()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BloatCatalog {
    pub app: Vec<BloatApp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppGroup {
    pub id: String,
    pub title: String,
}

/// Programa instalable con winget. `id` es el PackageIdentifier exacto.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WingetApp {
    pub id: String,
    pub title: String,
    pub group: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppCatalog {
    pub group: Vec<AppGroup>,
    pub app: Vec<WingetApp>,
}

pub fn apps() -> &'static AppCatalog {
    static C: OnceLock<AppCatalog> = OnceLock::new();
    C.get_or_init(|| {
        let c: AppCatalog = toml::from_str(APPS_TOML).expect("apps.toml inválido");
        validate_apps(&c).expect("apps.toml inconsistente");
        c
    })
}

pub fn find_app(id: &str) -> Option<&'static WingetApp> {
    apps().app.iter().find(|a| a.id == id)
}

/// Un PackageIdentifier válido: "Publicador.Nombre", sin espacios ni nada que winget
/// pueda leer como opción. Se pasa como argumento separado, pero igual lo restringimos.
pub fn is_valid_package_id(id: &str) -> bool {
    id.len() <= 128
        && id.contains('.')
        && !id.starts_with(['-', '.'])
        && !id.ends_with('.')
        && id.chars().all(|c| c.is_ascii_alphanumeric() || "._-+".contains(c))
}

pub fn validate_apps(c: &AppCatalog) -> Result<(), String> {
    let groups: HashSet<_> = c.group.iter().map(|g| g.id.as_str()).collect();
    let mut ids = HashSet::new();
    for a in &c.app {
        if !is_valid_package_id(&a.id) {
            return Err(format!("{}: id de paquete inválido", a.id));
        }
        if !ids.insert(a.id.to_ascii_lowercase()) {
            return Err(format!("{}: duplicado", a.id));
        }
        if !groups.contains(a.group.as_str()) {
            return Err(format!("{}: grupo desconocido '{}'", a.id, a.group));
        }
    }
    Ok(())
}

pub fn tweaks() -> &'static TweakCatalog {
    static C: OnceLock<TweakCatalog> = OnceLock::new();
    C.get_or_init(|| {
        let c: TweakCatalog = toml::from_str(TWEAKS_TOML).expect("tweaks.toml inválido");
        validate_tweaks(&c).expect("tweaks.toml inconsistente");
        c
    })
}

pub fn bloat() -> &'static BloatCatalog {
    static C: OnceLock<BloatCatalog> = OnceLock::new();
    C.get_or_init(|| toml::from_str(BLOAT_TOML).expect("bloat.toml inválido"))
}

pub fn find_tweak(id: &str) -> Option<&'static Tweak> {
    tweaks().tweak.iter().find(|t| t.id == id)
}

fn check_value(kind: ValueKind, v: &Value) -> Result<(), String> {
    match (kind, v) {
        (ValueKind::Dword, Value::Int(n)) if (0..=u32::MAX as i64).contains(n) => Ok(()),
        (ValueKind::Sz, Value::Str(_)) => Ok(()),
        _ => Err(format!("valor {v:?} no corresponde al tipo {kind:?}")),
    }
}

pub fn validate_tweaks(c: &TweakCatalog) -> Result<(), String> {
    let cats: HashSet<_> = c.category.iter().map(|c| c.id.as_str()).collect();
    let mut ids = HashSet::new();
    for t in &c.tweak {
        let ctx = |e: String| format!("{}: {e}", t.id);
        if !ids.insert(t.id.as_str()) {
            return Err(ctx("id duplicado".into()));
        }
        if !cats.contains(t.category.as_str()) {
            return Err(ctx(format!("categoría desconocida '{}'", t.category)));
        }
        if t.ops.is_empty() {
            return Err(ctx("sin operaciones".into()));
        }
        for op in &t.ops {
            if op.path.starts_with('\\') || op.path.ends_with('\\') || op.path.contains("HKEY_") {
                return Err(ctx(format!("ruta mal formada '{}'", op.path)));
            }
            if t.is_choice() {
                if op.on.is_some() || op.off.is_some() || op.delete_key_on_revert.is_some() {
                    return Err(ctx("con opciones, las operaciones no llevan on/off/delete_key_on_revert".into()));
                }
                continue;
            }
            let on = op.on.as_ref().ok_or_else(|| ctx("operación sin 'on'".into()))?;
            check_value(op.kind, on).map_err(ctx)?;
            if let Some(off) = &op.off {
                check_value(op.kind, off).map_err(ctx)?;
            }
            if let Some(k) = &op.delete_key_on_revert {
                // Solo se permite borrar una clave que contiene a la de la operación,
                // y nunca algo de 1-2 niveles (p. ej. "Software\Classes").
                if !op.path.starts_with(k.as_str()) || k.matches('\\').count() < 3 {
                    return Err(ctx(format!("delete_key_on_revert inseguro '{k}'")));
                }
            }
        }
        if t.is_choice() {
            validate_choices(t).map_err(ctx)?;
        }
    }
    Ok(())
}

fn validate_choices(t: &Tweak) -> Result<(), String> {
    if t.choices.len() < 2 {
        return Err("un selector necesita al menos 2 opciones".into());
    }
    if t.choices.iter().filter(|c| c.default).count() != 1 {
        return Err("tiene que haber exactamente una opción default".into());
    }
    let mut ids = HashSet::new();
    for c in &t.choices {
        if !ids.insert(c.id.as_str()) {
            return Err(format!("opción duplicada '{}'", c.id));
        }
        if c.values.len() != t.ops.len() {
            return Err(format!(
                "la opción '{}' tiene {} valores para {} operaciones",
                c.id,
                c.values.len(),
                t.ops.len()
            ));
        }
        for (op, v) in t.ops.iter().zip(&c.values) {
            check_value(op.kind, v).map_err(|e| format!("opción '{}': {e}", c.id))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogs_parse_and_validate() {
        let t = tweaks();
        assert!(t.tweak.len() >= 40, "se esperaban >= 40 tweaks, hay {}", t.tweak.len());
        let b = bloat();
        let names: HashSet<_> = b.app.iter().map(|a| a.name.to_lowercase()).collect();
        assert_eq!(names.len(), b.app.len(), "bloat.toml tiene nombres duplicados");
    }

    fn validate_choice_src(choices: &str) -> Result<(), String> {
        let src = format!(
            r#"
[[category]]
id = "x"
title = "X"
icon = "x"
[[tweak]]
id = "x.mode"
category = "x"
title = "Modo"
description = "d"
risk = "safe"
[[tweak.ops]]
hive = "HKCU"
path = 'Software\A\B'
name = "Mode"
kind = "dword"
{choices}
"#
        );
        let c: TweakCatalog = toml::from_str(&src).map_err(|e| e.to_string())?;
        validate_tweaks(&c)
    }

    #[test]
    fn apps_catalog_parses() {
        assert!(apps().app.len() >= 30);
    }

    #[test]
    fn package_id_rules() {
        for ok in ["Mozilla.Firefox", "Notepad++.Notepad++", "Flow-Launcher.Flow-Launcher", "7zip.7zip"] {
            assert!(is_valid_package_id(ok), "{ok}");
        }
        for bad in ["--override", "-h.x", "Firefox", "a b.c", "a.b;calc", "x.\"y\"", ".a.b", "a.b."] {
            assert!(!is_valid_package_id(bad), "{bad}");
        }
    }

    #[test]
    fn choice_validation() {
        let a = "[[tweak.choice]]\nid = \"a\"\nlabel = \"A\"\nvalues = [0]\ndefault = true\n";
        let b = "[[tweak.choice]]\nid = \"b\"\nlabel = \"B\"\nvalues = [1]\n";
        assert!(validate_choice_src(&format!("{a}{b}")).is_ok());
        assert!(validate_choice_src(a).is_err(), "una sola opción");
        assert!(validate_choice_src(&format!("{}{b}", a.replace("default = true\n", ""))).is_err(), "sin default");
        assert!(validate_choice_src(&format!("{a}{}", b.replace("[1]", "[1, 2]"))).is_err(), "valores != ops");
        assert!(validate_choice_src(&format!("{a}{}", b.replace("[1]", "[\"1\"]"))).is_err(), "tipo incorrecto");
        assert!(validate_choice_src(&format!("{a}{}", b.replace("\"b\"", "\"a\""))).is_err(), "id duplicado");
    }

    #[test]
    fn essentials_are_never_in_bloat_catalog() {
        let essentials = [
            "Microsoft.WindowsStore",
            "Microsoft.DesktopAppInstaller",
            "Microsoft.WindowsTerminal",
            "Microsoft.WindowsCalculator",
            "Microsoft.SecHealthUI",
            "Microsoft.WindowsNotepad",
            "Microsoft.Paint",
            "Microsoft.ScreenSketch",
            "Microsoft.StorePurchaseApp",
            "Microsoft.Xbox.TCUI",
            "Microsoft.VCLibs.140.00",
            "Microsoft.UI.Xaml.2.8",
            "Microsoft.WindowsAppRuntime.1.6",
        ];
        for e in essentials {
            assert!(
                !bloat().app.iter().any(|a| a.matches(e)),
                "{e} no debe poder quitarse"
            );
        }
    }
}
