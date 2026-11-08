//! Motor de tweaks: estado real, vista previa, aplicar/revertir y deshacer.

use crate::catalog::{self, Hive, Op, Restart, Risk, Tweak, Value};
use crate::i18n;
use crate::power;
use crate::registry::{self as reg, RawValue};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TweakState {
    Applied,
    Default,
    Partial,
    Unsupported,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpView {
    pub key: String,
    pub name: String,
    pub current: String,
    pub on: String,
    pub off: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TweakView {
    pub id: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub risk: Risk,
    pub restart: Restart,
    pub tags: Vec<String>,
    pub requires_admin: bool,
    /// Toca HKCU (configuración del usuario).
    pub per_user: bool,
    /// Windows protege este valor (UCPD) y no deja que la app lo escriba.
    pub ucpd: bool,
    pub state: TweakState,
    pub ops: Vec<OpView>,
    /// Vacío para tweaks de switch.
    pub choices: Vec<ChoiceView>,
    /// Opción vigente según el registro (None si el valor no coincide con ninguna).
    pub choice: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChoiceView {
    pub id: String,
    pub label: String,
    pub default: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Desired {
    pub id: String,
    /// true = aplicar el tweak, false = volver al valor por defecto. Se ignora si hay `choice`.
    #[serde(default)]
    pub applied: bool,
    /// Para tweaks de opción múltiple: id de la opción elegida.
    #[serde(default)]
    pub choice: Option<String>,
}

/// Qué le va a pasar a cada operación.
enum Target {
    Set(Value),
    DeleteValue,
    DeleteTree(String),
}

fn targets(t: &Tweak, d: &Desired, build: u32) -> Result<Vec<Target>, String> {
    if t.is_choice() {
        let id = d.choice.as_deref().ok_or_else(|| t!("choice_missing"))?;
        let c = t.find_choice(id).ok_or_else(|| t!("choice_unknown", id = id))?;
        if !c.supported_on(build) {
            return Err(t!("choice_unsupported", label = i18n::choice_label(t, c)));
        }
        return Ok(c.values.iter().cloned().map(Target::Set).collect());
    }
    Ok(t.ops
        .iter()
        .map(|op| match (d.applied, &op.delete_key_on_revert, &op.off) {
            // La validación del catálogo garantiza `on` en tweaks de switch.
            (true, _, _) => Target::Set(op.on.clone().expect("op sin on")),
            (false, Some(tree), _) => Target::DeleteTree(tree.clone()),
            (false, None, Some(v)) => Target::Set(v.clone()),
            (false, None, None) => Target::DeleteValue,
        })
        .collect())
}

fn desired_label(t: &Tweak, choice: Option<&str>, applied: bool) -> String {
    match choice.and_then(|c| t.find_choice(c)) {
        Some(c) => format!("{}: {}", i18n::tweak_title(t), i18n::choice_label(t, c)),
        None => format!("{} {}", if applied { "✓" } else { "↺" }, i18n::tweak_title(t)),
    }
}

/// Qué se aplicó en una entrada del historial, para armar su título en el idioma de
/// quien la mira (el texto guardado en `label` queda en el idioma del momento).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelItem {
    pub id: String,
    #[serde(default)]
    pub choice: Option<String>,
    #[serde(default)]
    pub applied: bool,
}

fn item_label(i: &LabelItem) -> Option<String> {
    if i.id == POWER_ID {
        return Some(t!("label_power", plan = power_label(i.choice.as_deref()?)));
    }
    Some(desired_label(catalog::find_tweak(&i.id)?, i.choice.as_deref(), i.applied))
}

/// Título de una entrada en el idioma actual; las entradas viejas conservan el guardado.
fn entry_label(e: &JournalEntry) -> String {
    match e.items.as_slice() {
        [one] => item_label(one).unwrap_or_else(|| e.label.clone()),
        [] if !e.removed_apps.is_empty() && e.changes.is_empty() => t!("label_removed_apps", n = e.removed_apps.len()),
        [] => e.label.clone(),
        many => t!("label_tweaks", n = many.len()),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedChange {
    pub tweak_id: String,
    pub key: String,
    pub name: String,
    pub from: String,
    pub to: String,
    pub noop: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub tweak_id: String,
    pub hive: Hive,
    pub path: String,
    pub name: String,
    pub before: Option<RawValue>,
    /// Claves que no existían y creamos (más profunda primero).
    #[serde(default)]
    pub created_keys: Vec<String>,
    /// Si revertir borró un árbol de claves, guardamos su raíz (informativo).
    #[serde(default)]
    pub deleted_tree: Option<String>,
    /// Valor que dejó este cambio (None = lo borró). Solo es confiable si `tracked`;
    /// las entradas viejas no lo tienen.
    #[serde(default)]
    pub after: Option<RawValue>,
    #[serde(default)]
    pub tracked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: u64,
    pub at: u64,
    pub label: String,
    pub changes: Vec<Change>,
    #[serde(default)]
    pub removed_apps: Vec<String>,
    #[serde(default)]
    pub undone: bool,
    /// Cambio de plan de energía (no es del registro).
    #[serde(default)]
    pub power: Option<power::PowerChange>,
    /// Qué se aplicó (para traducir `label`). Vacío en entradas de versiones anteriores.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<LabelItem>,
}

// ───────────────────────────── Plan de energía ─────────────────────────────
// No es un valor del registro: se maneja con la API de energía, pero se muestra y se
// deshace como cualquier otro tweak (selector, plan, historial).

pub const POWER_ID: &str = "power.plan";

fn power_choices() -> Vec<ChoiceView> {
    let plans = power::list();
    let mut out = Vec::new();
    for (g, label) in [(power::SAVER, t!("power_saver")), (power::BALANCED, t!("power_balanced")), (power::HIGH, t!("power_high"))] {
        if plans.iter().any(|p| p.guid == g) {
            out.push(ChoiceView { id: power::to_string(&g), label, default: g == power::BALANCED });
        }
    }
    let ultimate = power::find_ultimate(&plans).map_or(power::ULTIMATE_NEW.into(), |g| power::to_string(&g));
    out.push(ChoiceView { id: ultimate, label: t!("power_ultimate"), default: false });
    // Un plan propio (del fabricante o del usuario) activo también tiene que verse.
    if let Ok(a) = power::active() {
        let id = power::to_string(&a);
        if !out.iter().any(|c| c.id == id) {
            out.push(ChoiceView { id, label: power::name(&a), default: false });
        }
    }
    out
}

fn power_label(choice: &str) -> String {
    power_choices().into_iter().find(|c| c.id == choice).map_or_else(|| choice.to_string(), |c| c.label)
}

fn power_view() -> Option<TweakView> {
    let active = power::active().ok()?;
    let choices = power_choices();
    let id = power::to_string(&active);
    let is_ultimate = power::find_ultimate(&power::list()) == Some(active);
    let state = if active == power::HIGH || is_ultimate { TweakState::Applied } else { TweakState::Default };
    Some(TweakView {
        id: POWER_ID.into(),
        category: "power".into(),
        title: t!("power_title"),
        description: t!("power_description"),
        risk: Risk::Moderate,
        restart: Restart::None,
        tags: vec!["energia".into(), "rendimiento".into(), "maximo".into(), "ultimate".into()],
        requires_admin: true,
        per_user: false,
        ucpd: false,
        state,
        ops: vec![OpView {
            key: t!("power_key"),
            name: t!("power_active"),
            current: power::name(&active),
            on: choices.iter().map(|c| c.label.as_str()).collect::<Vec<_>>().join(" · "),
            off: String::new(),
        }],
        choice: Some(id),
        choices,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemResult {
    pub id: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplyReport {
    pub entry_id: Option<u64>,
    pub results: Vec<ItemResult>,
    pub restart: Restart,
    /// Deshacer: valores que cambiaron después de esta entrada. Si hay alguno y no se
    /// forzó, no se tocó nada y la UI pide confirmación.
    pub conflicts: Vec<String>,
    /// Aviso para mostrar una vez (por ejemplo, historial dañado y respaldado).
    pub warning: Option<String>,
}

impl ApplyReport {
    fn new(entry_id: Option<u64>, results: Vec<ItemResult>, restart: Restart) -> Self {
        Self { entry_id, results, restart, conflicts: vec![], warning: None }
    }
}

pub struct Engine {
    journal_path: PathBuf,
    lock: Mutex<()>,
    warning: Mutex<Option<String>>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn key_label(op: &Op) -> String {
    format!("{}\\{}", reg::hive_label(op.hive), op.path)
}

fn name_label(op: &Op) -> String {
    if op.name.is_empty() {
        t!("default_value")
    } else {
        op.name.clone()
    }
}

fn off_label(op: &Op) -> String {
    match (&op.off, &op.delete_key_on_revert) {
        (_, Some(_)) => t!("deletes_key"),
        (Some(v), None) => reg::describe_value(op.kind, v),
        (None, None) => t!("deletes_value"),
    }
}

pub struct Detected {
    pub state: TweakState,
    pub ops: Vec<OpView>,
    pub choice: Option<String>,
}

/// Alguna operación pide que su clave exista (p. ej. un servicio) y no está.
fn missing_required_key(t: &Tweak) -> bool {
    t.ops.iter().any(|o| o.must_exist && !reg::key_exists(o.hive, &o.path).unwrap_or(false))
}

pub fn tweak_state(t: &Tweak, build: u32) -> Detected {
    let mut views = Vec::with_capacity(t.ops.len());
    let mut current_values = Vec::with_capacity(t.ops.len());
    let mut error = false;
    for (i, op) in t.ops.iter().enumerate() {
        let cur = reg::read(op.hive, &op.path, &op.name);
        let current = match &cur {
            Ok(v) => reg::describe(v.as_ref()),
            Err(e) => {
                error = true;
                t!("read_error", error = e)
            }
        };
        let (on, off) = if t.is_choice() {
            let opts = t
                .choices
                .iter()
                .map(|c| format!("{} {}", reg::describe_value(op.kind, &c.values[i]), i18n::choice_label(t, c)))
                .collect::<Vec<_>>()
                .join(" · ");
            (opts, String::new())
        } else {
            (reg::describe_value(op.kind, op.on.as_ref().expect("op sin on")), off_label(op))
        };
        views.push(OpView { key: key_label(op), name: name_label(op), current, on, off });
        current_values.push(cur.ok().flatten());
    }

    let mut choice = None;
    let state = if !t.supported_on(build) || missing_required_key(t) {
        TweakState::Unsupported
    } else if error {
        TweakState::Error
    } else if t.is_choice() {
        let all_absent = current_values.iter().all(Option::is_none);
        let found = t.choices.iter().find(|c| {
            t.ops
                .iter()
                .zip(&c.values)
                .zip(&current_values)
                .all(|((op, v), cur)| reg::matches(cur.as_ref(), op.kind, v))
        });
        let found = found.or_else(|| all_absent.then(|| t.choices.iter().find(|c| c.default)).flatten());
        choice = found.map(|c| c.id.clone());
        match found {
            Some(c) if c.default => TweakState::Default,
            Some(_) => TweakState::Applied,
            None => TweakState::Partial,
        }
    } else {
        let hits = t
            .ops
            .iter()
            .zip(&current_values)
            .filter(|(op, cur)| reg::matches(cur.as_ref(), op.kind, op.on.as_ref().expect("op sin on")))
            .count();
        if hits == t.ops.len() {
            TweakState::Applied
        } else if hits == 0 {
            TweakState::Default
        } else {
            TweakState::Partial
        }
    };
    Detected { state, ops: views, choice }
}

pub fn list(build: u32) -> Vec<TweakView> {
    catalog::tweaks()
        .tweak
        .iter()
        .map(|t| {
            let Detected { state, ops, choice } = tweak_state(t, build);
            TweakView {
                id: t.id.clone(),
                category: t.category.clone(),
                title: i18n::tweak_title(t),
                description: i18n::tweak_description(t),
                risk: t.risk,
                restart: t.restart,
                tags: t.tags.clone(),
                requires_admin: t.requires_admin(),
                per_user: t.per_user(),
                ucpd: t.ucpd,
                state,
                ops,
                choices: t
                    .choices
                    .iter()
                    .filter(|c| c.supported_on(build))
                    .map(|c| ChoiceView { id: c.id.clone(), label: i18n::choice_label(t, c), default: c.default })
                    .collect(),
                choice,
            }
        })
        .chain(power_view())
        .collect()
}

pub fn plan(desired: &[Desired], build: u32) -> Result<Vec<PlannedChange>, String> {
    let mut out = Vec::new();
    for d in desired {
        if d.id == POWER_ID {
            let choice = d.choice.as_deref().ok_or_else(|| t!("power_missing"))?;
            let active = power::active()?;
            out.push(PlannedChange {
                tweak_id: POWER_ID.into(),
                key: t!("power_key"),
                name: t!("power_active"),
                from: power::name(&active),
                to: if choice == power::ULTIMATE_NEW {
                    t!("power_ultimate_new")
                } else {
                    power_label(choice)
                },
                noop: choice == power::to_string(&active),
            });
            continue;
        }
        let t = catalog::find_tweak(&d.id).ok_or_else(|| t!("tweak_unknown", id = d.id))?;
        let targets = targets(t, d, build).map_err(|e| format!("{}: {e}", i18n::tweak_title(t)))?;
        for (op, target) in t.ops.iter().zip(&targets) {
            let cur = reg::read(op.hive, &op.path, &op.name).map_err(|e| e.to_string())?;
            let (to, noop) = match target {
                Target::Set(v) => (reg::describe_value(op.kind, v), reg::matches(cur.as_ref(), op.kind, v)),
                Target::DeleteValue => (t!("deletes_value"), cur.is_none()),
                Target::DeleteTree(_) => (t!("deletes_key"), cur.is_none()),
            };
            out.push(PlannedChange {
                tweak_id: t.id.clone(),
                key: key_label(op),
                name: name_label(op),
                from: reg::describe(cur.as_ref()),
                to,
                noop,
            });
        }
    }
    Ok(out)
}

fn apply_op(t: &Tweak, op: &Op, target: &Target) -> Result<Change, String> {
    let before = reg::read(op.hive, &op.path, &op.name).map_err(|e| e.to_string())?;
    let mut change = Change {
        tweak_id: t.id.clone(),
        hive: op.hive,
        path: op.path.clone(),
        name: op.name.clone(),
        before,
        created_keys: Vec::new(),
        deleted_tree: None,
        after: None,
        tracked: true,
    };
    match target {
        Target::DeleteTree(tree) => {
            reg::delete_tree(op.hive, tree).map_err(|e| e.to_string())?;
            change.deleted_tree = Some(tree.clone());
        }
        Target::Set(v) => {
            change.created_keys = reg::missing_keys(op.hive, &op.path).map_err(|e| e.to_string())?;
            if op.must_exist && !change.created_keys.is_empty() {
                return Err(t!("key_missing"));
            }
            let raw = reg::encode(op.kind, v);
            if let Err(e) = reg::write_raw(op.hive, &op.path, &op.name, &raw) {
                // La clave pudo crearse aunque el valor no: no dejar claves vacías colgadas.
                for k in &change.created_keys {
                    let _ = reg::delete_key_if_empty(op.hive, k);
                }
                return Err(write_error(&e));
            }
            change.after = Some(raw);
        }
        Target::DeleteValue => {
            reg::delete_value(op.hive, &op.path, &op.name).map_err(|e| e.to_string())?;
        }
    }
    Ok(change)
}


/// Mensaje de error de escritura: "acceso denegado" con permisos de admin casi siempre
/// es una protección de Windows sobre ese valor, no un problema de permisos.
fn write_error(e: &std::io::Error) -> String {
    if e.raw_os_error() == Some(5) && crate::system::is_elevated() {
        t!("write_protected", error = e)
    } else {
        e.to_string()
    }
}

/// El historial vive en `%APPDATA%`, donde cualquier programa del usuario puede escribir,
/// y "Deshacer" corre como admin. Antes de tocar el registro se verifica que cada cambio
/// corresponda a una operación real del catálogo para ese tweak.
fn validate_change(c: &Change) -> Result<(), String> {
    // Solo en los tests: la clave de prueba propia (no existe en la app compilada).
    #[cfg(test)]
    if c.hive == Hive::HKCU && c.path.starts_with(r"Software\DebloaterTest\") && c.tweak_id == "test.tweak" {
        return Ok(());
    }
    let t = catalog::find_tweak(&c.tweak_id).ok_or_else(|| t!("tweak_unknown", id = c.tweak_id))?;
    let op = t
        .ops
        .iter()
        .find(|o| o.hive == c.hive && o.path.eq_ignore_ascii_case(&c.path) && o.name.eq_ignore_ascii_case(&c.name))
        .ok_or_else(|| t!("journal_not_in_tweak", key = format!("{}\\{}\\{}", reg::hive_label(c.hive), c.path, c.name), id = t.id))?;
    // Deshacer tampoco puede recrear la clave de un servicio que ya no está.
    if op.must_exist && !reg::key_exists(op.hive, &op.path).unwrap_or(false) {
        return Err(t!("key_missing"));
    }
    // Solo se pueden borrar claves que contienen a la de la operación, y nunca de 1-2 niveles.
    let path = op.path.to_ascii_lowercase();
    for k in &c.created_keys {
        let k = k.to_ascii_lowercase();
        let inside = path == k || path.starts_with(&format!("{k}\\"));
        if !inside || k.matches('\\').count() < 2 {
            return Err(t!("journal_bad_key", key = k));
        }
    }
    Ok(())
}

/// Lo mismo para el plan de energía: GUIDs válidos, el anterior tiene que existir y el
/// creado solo puede ser una copia de "Máximo rendimiento".
fn validate_power(p: &power::PowerChange) -> Result<(), String> {
    let plans = power::list();
    let before = power::parse(&p.before).ok_or_else(|| t!("power_guid_invalid"))?;
    power::parse(&p.after).ok_or_else(|| t!("power_guid_invalid"))?;
    if !plans.iter().any(|x| x.guid == before) {
        return Err(t!("power_previous_gone"));
    }
    if let Some(c) = &p.created {
        let g = power::parse(c).ok_or_else(|| t!("power_guid_invalid"))?;
        if power::find_ultimate(&plans) != Some(g) && plans.iter().any(|x| x.guid == g) {
            return Err(t!("power_not_ours"));
        }
    }
    Ok(())
}

/// Deshace un cambio individual devolviendo el valor anterior exacto.
fn undo_change(c: &Change) -> Result<(), String> {
    match &c.before {
        Some(v) => reg::write_raw(c.hive, &c.path, &c.name, v),
        None => reg::delete_value(c.hive, &c.path, &c.name),
    }
    .map_err(|e| e.to_string())?;
    for k in &c.created_keys {
        reg::delete_key_if_empty(c.hive, k).map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl Engine {
    pub fn new(data_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&data_dir);
        Self {
            journal_path: data_dir.join("journal.json"),
            lock: Mutex::new(()),
            warning: Mutex::new(None),
        }
    }

    /// Lee el historial. Si existe pero no se puede interpretar (dañado o de un formato
    /// incompatible), lo aparta con otro nombre en vez de pisarlo, y deja un aviso.
    fn load(&self) -> Vec<JournalEntry> {
        let bytes = match fs::read(&self.journal_path) {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };
        match serde_json::from_slice(&bytes) {
            Ok(entries) => entries,
            Err(e) => {
                let backup = self.journal_path.with_file_name(format!("journal.danado-{}.json", now_ms()));
                let msg = match fs::rename(&self.journal_path, &backup) {
                    Ok(()) => t!("journal_unreadable_backup", error = e, path = backup.display()),
                    Err(re) => t!("journal_unreadable", error = e, backup_error = re),
                };
                *self.warning.lock().unwrap() = Some(msg);
                Vec::new()
            }
        }
    }

    fn take_warning(&self) -> Option<String> {
        self.warning.lock().unwrap().take()
    }

    fn save(&self, entries: &[JournalEntry]) -> Result<(), String> {
        const MAX: usize = 300;
        let start = entries.len().saturating_sub(MAX);
        let json = serde_json::to_vec_pretty(&entries[start..]).map_err(|e| e.to_string())?;
        let tmp = self.journal_path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.journal_path).map_err(|e| e.to_string())
    }

    pub fn history(&self) -> Vec<JournalEntry> {
        let _g = self.lock.lock().unwrap();
        let mut h = self.load();
        h.reverse();
        for e in &mut h {
            e.label = entry_label(e);
        }
        h
    }

    pub fn record_removed_apps(&self, label: String, apps: Vec<String>) -> Result<(), String> {
        let _g = self.lock.lock().unwrap();
        let mut entries = self.load();
        let id = entries.last().map_or(now_ms(), |l| now_ms().max(l.id + 1));
        entries.push(JournalEntry {
            id,
            at: id / 1000,
            label,
            changes: vec![],
            removed_apps: apps,
            undone: false,
            power: None,
            items: vec![],
        });
        self.save(&entries)
    }

    /// `foreign_user`: si la app corre elevada con otra cuenta, el nombre del usuario
    /// real de la sesión. En ese caso HKCU es el de la cuenta admin y no se toca.
    pub fn apply(&self, desired: &[Desired], elevated: bool, build: u32, foreign_user: Option<&str>) -> ApplyReport {
        let _g = self.lock.lock().unwrap();
        let mut results = Vec::new();
        let mut changes = Vec::new();
        let mut restart = Restart::None;
        let mut items: Vec<LabelItem> = Vec::new();
        let mut power_change: Option<power::PowerChange> = None;

        for d in desired {
            if d.id == POWER_ID {
                let r = if !elevated {
                    Err(t!("requires_admin"))
                } else {
                    d.choice.as_deref().ok_or_else(|| t!("power_missing")).and_then(power::activate)
                };
                match r {
                    Ok(c) => {
                        items.push(LabelItem { id: POWER_ID.into(), choice: Some(c.after.clone()), applied: true });
                        power_change = Some(c);
                        results.push(ItemResult { id: POWER_ID.into(), ok: true, error: None });
                    }
                    Err(e) => results.push(ItemResult { id: POWER_ID.into(), ok: false, error: Some(e) }),
                }
                continue;
            }
            let Some(t) = catalog::find_tweak(&d.id) else {
                results.push(ItemResult { id: d.id.clone(), ok: false, error: Some(t!("tweak_unknown", id = d.id)) });
                continue;
            };
            if t.requires_admin() && !elevated {
                results.push(ItemResult { id: t.id.clone(), ok: false, error: Some(t!("requires_admin")) });
                continue;
            }
            if t.ucpd && crate::system::ucpd_active() {
                results.push(ItemResult { id: t.id.clone(), ok: false, error: Some(t!("ucpd")) });
                continue;
            }
            if let (true, Some(user)) = (t.per_user(), foreign_user) {
                results.push(ItemResult {
                    id: t.id.clone(),
                    ok: false,
                    error: Some(t!("foreign_user_apply", user = user)),
                });
                continue;
            }
            if (d.applied || t.is_choice()) && !t.supported_on(build) {
                results.push(ItemResult { id: t.id.clone(), ok: false, error: Some(t!("unsupported_build")) });
                continue;
            }
            // Aplicar o revertir: nunca se crea la clave de algo que no existe (un servicio).
            if missing_required_key(t) {
                results.push(ItemResult { id: t.id.clone(), ok: false, error: Some(t!("key_missing")) });
                continue;
            }
            let targets = match targets(t, d, build) {
                Ok(x) => x,
                Err(e) => {
                    results.push(ItemResult { id: t.id.clone(), ok: false, error: Some(e) });
                    continue;
                }
            };
            // Todo o nada por tweak: si falla una operación, deshacemos las anteriores.
            let mut done: Vec<Change> = Vec::new();
            let mut err = None;
            for (op, target) in t.ops.iter().zip(&targets) {
                match apply_op(t, op, target) {
                    Ok(c) => done.push(c),
                    Err(e) => {
                        err = Some(format!("{}\\{}: {e}", key_label(op), name_label(op)));
                        break;
                    }
                }
            }
            if let Some(e) = err {
                for c in done.iter().rev() {
                    let _ = undo_change(c);
                }
                results.push(ItemResult { id: t.id.clone(), ok: false, error: Some(e) });
            } else {
                restart = restart.max(t.restart);
                items.push(LabelItem { id: t.id.clone(), choice: d.choice.clone(), applied: d.applied });
                changes.extend(done);
                results.push(ItemResult { id: t.id.clone(), ok: true, error: None });
            }
        }

        let entry_id = if changes.is_empty() && power_change.is_none() {
            None
        } else {
            let mut entries = self.load();
            let id = now_ms();
            let label = match items.as_slice() {
                [one] => item_label(one).unwrap_or_default(),
                many => t!("label_tweaks", n = many.len()),
            };
            // Id único aunque dos operaciones caigan en el mismo milisegundo.
            let id = entries.last().map_or(id, |l| id.max(l.id + 1));
            entries.push(JournalEntry {
                id,
                at: id / 1000,
                label,
                changes,
                removed_apps: vec![],
                undone: false,
                power: power_change,
                items,
            });
            if let Err(e) = self.save(&entries) {
                results.push(ItemResult { id: "journal".into(), ok: false, error: Some(t!("journal_save_failed", error = e)) });
            }
            Some(id)
        };
        let mut report = ApplyReport::new(entry_id, results, restart);
        report.warning = self.take_warning();
        report
    }

    /// Deshace una entrada del historial.
    ///
    /// - Si algún valor cambió después de esta entrada (otro cambio posterior, Windows u
    ///   otro programa), no toca nada y devuelve `conflicts`, salvo que `force` sea true.
    /// - Solo marca la entrada como deshecha si todos los valores se restauraron; si no,
    ///   se puede reintentar (restaurar es idempotente).
    pub fn undo(&self, entry_id: u64, force: bool, foreign_user: Option<&str>) -> Result<ApplyReport, String> {
        let _g = self.lock.lock().unwrap();
        let mut entries = self.load();
        let warning = self.take_warning();
        let entry = entries
            .iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or_else(|| t!("entry_missing"))?;
        if entry.undone {
            return Err(t!("entry_undone"));
        }
        if entry.changes.is_empty() && entry.power.is_none() {
            return Err(t!("apps_not_restorable"));
        }
        if let (true, Some(user)) = (entry.changes.iter().any(|c| c.hive == Hive::HKCU), foreign_user) {
            return Err(t!("foreign_user_undo", user = user));
        }
        // Nada se toca si alguna parte de la entrada no coincide con el catálogo.
        for c in &entry.changes {
            validate_change(c).map_err(|e| t!("journal_mismatch", error = e))?;
        }
        if let Some(p) = &entry.power {
            validate_power(p).map_err(|e| t!("journal_power_invalid", error = e))?;
        }

        if !force {
            let mut conflicts = Vec::new();
            for c in &entry.changes {
                if !c.tracked {
                    continue;
                }
                let current = reg::read(c.hive, &c.path, &c.name).map_err(|e| e.to_string())?;
                // Igual a lo que dejamos: se puede deshacer. Igual a lo de antes: ya está restaurado.
                if current != c.after && current != c.before {
                    let title = catalog::find_tweak(&c.tweak_id).map_or(c.tweak_id.clone(), i18n::tweak_title);
                    conflicts.push(t!(
                        "conflict_value",
                        title = title,
                        key = format!("{}\\{}", reg::hive_label(c.hive), c.name),
                        value = reg::describe(current.as_ref())
                    ));
                }
            }
            if let Some(p) = &entry.power {
                let now = power::active().map(|g| power::to_string(&g)).unwrap_or_default();
                if now != p.after && now != p.before {
                    conflicts.push(t!("conflict_power", plan = power_label(&now)));
                }
            }
            if !conflicts.is_empty() {
                let mut r = ApplyReport::new(Some(entry_id), vec![], Restart::None);
                r.conflicts = conflicts;
                r.warning = warning;
                return Ok(r);
            }
        }

        let mut results = Vec::new();
        let mut restart = Restart::None;
        for c in entry.changes.iter().rev() {
            if let Some(t) = catalog::find_tweak(&c.tweak_id) {
                restart = restart.max(t.restart);
            }
            if let Err(e) = undo_change(c) {
                results.push(ItemResult { id: c.tweak_id.clone(), ok: false, error: Some(e) });
            }
        }
        if let Some(p) = &entry.power {
            if let Err(e) = power::revert(p) {
                results.push(ItemResult { id: POWER_ID.into(), ok: false, error: Some(e) });
            } else {
                results.push(ItemResult { id: POWER_ID.into(), ok: true, error: None });
            }
        }
        let mut ids: Vec<String> = entry.changes.iter().map(|c| c.tweak_id.clone()).collect();
        ids.dedup();
        for id in ids {
            if !results.iter().any(|r| r.id == id) {
                results.push(ItemResult { id, ok: true, error: None });
            }
        }
        if results.iter().all(|r| r.ok) {
            entry.undone = true;
            self.save(&entries)?;
        }
        let mut report = ApplyReport::new(Some(entry_id), results, restart);
        report.warning = warning;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Value, ValueKind};

    /// Clave de prueba propia en HKCU; se borra al terminar cada test.
    struct TestKey(String);
    impl TestKey {
        fn new(tag: &str) -> Self {
            Self(format!(r"Software\DebloaterTest\{}-{tag}", std::process::id()))
        }
    }
    impl Drop for TestKey {
        fn drop(&mut self) {
            let _ = reg::delete_tree(Hive::HKCU, &self.0);
            let _ = reg::delete_key_if_empty(Hive::HKCU, r"Software\DebloaterTest");
        }
    }

    fn temp_engine(tag: &str) -> (Engine, PathBuf) {
        let dir = std::env::temp_dir().join(format!("debloater-engine-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        (Engine::new(dir.clone()), dir)
    }

    fn entry_for(key: &str, before: Option<RawValue>, after: Option<RawValue>) -> JournalEntry {
        JournalEntry {
            id: 1,
            at: 0,
            label: "test".into(),
            changes: vec![Change {
                tweak_id: "test.tweak".into(),
                hive: Hive::HKCU,
                path: key.into(),
                name: "Value".into(),
                before,
                created_keys: vec![],
                deleted_tree: None,
                after,
                tracked: true,
            }],
            removed_apps: vec![],
            undone: false,
            power: None,
            items: vec![],
        }
    }

    fn dword(n: i64) -> RawValue {
        reg::encode(ValueKind::Dword, &Value::Int(n))
    }

    #[test]
    fn damaged_journal_is_backed_up_not_overwritten() {
        let (engine, dir) = temp_engine("damaged");
        fs::write(dir.join("journal.json"), b"{ esto no es json").unwrap();
        assert!(engine.history().is_empty());
        let backups: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("journal.danado-"))
            .collect();
        assert_eq!(backups.len(), 1, "debe quedar una copia del historial dañado");
        assert_eq!(fs::read(backups[0].path()).unwrap(), b"{ esto no es json");
        assert!(engine.take_warning().is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn undo_detects_later_changes_and_force_overrides() {
        let key = TestKey::new("conflict");
        let (engine, dir) = temp_engine("conflict");
        // La entrada dice: el valor no existía y lo dejamos en 1. Ahora alguien lo puso en 2.
        engine.save(&[entry_for(&key.0, None, Some(dword(1)))]).unwrap();
        reg::write_raw(Hive::HKCU, &key.0, "Value", &dword(2)).unwrap();

        let r = engine.undo(1, false, None).unwrap();
        assert_eq!(r.conflicts.len(), 1);
        assert_eq!(reg::read(Hive::HKCU, &key.0, "Value").unwrap(), Some(dword(2)), "sin forzar no toca nada");
        assert!(!engine.history()[0].undone);

        let r = engine.undo(1, true, None).unwrap();
        assert!(r.conflicts.is_empty() && r.results.iter().all(|x| x.ok));
        assert_eq!(reg::read(Hive::HKCU, &key.0, "Value").unwrap(), None);
        assert!(engine.history()[0].undone);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn undo_without_conflict_and_already_restored_values() {
        let key = TestKey::new("clean");
        let (engine, dir) = temp_engine("clean");
        // Valor tal como lo dejamos: se deshace sin preguntar.
        engine.save(&[entry_for(&key.0, Some(dword(0)), Some(dword(1)))]).unwrap();
        reg::write_raw(Hive::HKCU, &key.0, "Value", &dword(1)).unwrap();
        let r = engine.undo(1, false, None).unwrap();
        assert!(r.conflicts.is_empty());
        assert_eq!(reg::read(Hive::HKCU, &key.0, "Value").unwrap(), Some(dword(0)));

        // Un reintento sobre un valor que ya volvió a "antes" tampoco es conflicto.
        let mut e = entry_for(&key.0, Some(dword(0)), Some(dword(1)));
        e.id = 2;
        engine.save(&[e]).unwrap();
        let r = engine.undo(2, false, None).unwrap();
        assert!(r.conflicts.is_empty() && r.results.iter().all(|x| x.ok));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn power_plan_is_listed_with_the_active_plan_selected() {
        let v = list(26100).into_iter().find(|t| t.id == POWER_ID).expect("el plan de energía debe listarse");
        assert_eq!(v.category, "power");
        assert!(v.requires_admin);
        let active = power::to_string(&power::active().unwrap());
        assert_eq!(v.choice.as_deref(), Some(active.as_str()));
        assert!(v.choices.iter().any(|c| c.id == active), "la opción activa tiene que estar entre las opciones");
        assert!(v.choices.iter().any(|c| c.label == "Máximo rendimiento"));
        assert_eq!(v.choices.iter().filter(|c| c.default).count(), 1);
    }

    #[test]
    fn old_journal_entries_without_power_still_load() {
        let old = r#"[{"id":1,"at":1,"label":"x","changes":[],"removed_apps":["a"],"undone":false}]"#;
        let e: Vec<JournalEntry> = serde_json::from_str(old).unwrap();
        assert!(e[0].power.is_none());
    }

    /// Un historial manipulado no puede hacer que "Deshacer" (que corre como admin)
    /// escriba claves ajenas al catálogo.
    #[test]
    fn tampered_journal_is_rejected_without_touching_anything() {
        let (engine, dir) = temp_engine("tampered");
        let mut bad = entry_for(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", None, Some(dword(1)));
        bad.changes[0].hive = Hive::HKLM;
        bad.changes[0].tweak_id = "explorer.show_extensions".into();
        bad.changes[0].name = "Malware".into();
        bad.changes[0].before = Some(dword(1));
        engine.save(&[bad]).unwrap();
        let err = engine.undo(1, true, None).unwrap_err();
        assert!(err.contains("no coincide con el catálogo"), "{err}");
        assert!(!engine.history()[0].undone);

        // Clave del catálogo, pero con una clave "creada" demasiado arriba para borrar.
        let mut wide = entry_for(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced", Some(dword(1)), Some(dword(0)));
        wide.changes[0].tweak_id = "explorer.show_extensions".into();
        wide.changes[0].name = "HideFileExt".into();
        wide.changes[0].created_keys = vec![r"Software\Microsoft".into()];
        engine.save(&[wide]).unwrap();
        assert!(engine.undo(1, true, None).unwrap_err().contains("clave a borrar no válida"));

        // Power: el "plan creado" no puede ser uno de los de Windows.
        let mut p = entry_for(r"x", None, None);
        p.changes.clear();
        p.power = Some(power::PowerChange {
            before: power::to_string(&power::active().unwrap()),
            after: power::to_string(&power::HIGH),
            created: Some(power::to_string(&power::BALANCED)),
        });
        engine.save(&[p]).unwrap();
        assert!(engine.undo(1, true, None).unwrap_err().contains("plan de energía no es válido"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Lee (sin modificar) el historial real del usuario y valida cada entrada.
    #[test]
    #[ignore]
    fn real_user_journal_passes_validation() {
        let path = std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join(r"com.debloater.app\journal.json");
        let entries: Vec<JournalEntry> = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        for e in &entries {
            for c in &e.changes {
                validate_change(c).unwrap_or_else(|err| panic!("{}: {err}", e.label));
            }
            if let Some(p) = &e.power {
                validate_power(p).unwrap_or_else(|err| panic!("{}: {err}", e.label));
            }
        }
        println!("{} entradas válidas", entries.len());
    }

    #[test]
    fn real_catalog_changes_pass_validation() {
        for t in &catalog::tweaks().tweak {
            for op in &t.ops {
                let c = Change {
                    tweak_id: t.id.clone(),
                    hive: op.hive,
                    path: op.path.clone(),
                    name: op.name.clone(),
                    before: None,
                    created_keys: reg_ancestors(&op.path),
                    deleted_tree: None,
                    after: None,
                    tracked: true,
                };
                if op.must_exist && !reg::key_exists(op.hive, &op.path).unwrap() {
                    // Un servicio que esta edición no trae: deshacer no lo recrea.
                    assert_eq!(validate_change(&c).unwrap_err(), t!("key_missing"), "{}", t.id);
                    continue;
                }
                validate_change(&c).unwrap_or_else(|e| panic!("{}: {e}", t.id));
            }
        }
    }

    /// `must_exist`: si la clave no está, el tweak no aplica y aplicar nunca la crea.
    #[test]
    fn must_exist_never_creates_the_key() {
        let key = TestKey::new("must-exist");
        let path = format!(r"{}\NoExiste", key.0);
        let op = Op {
            hive: Hive::HKCU,
            path: path.clone(),
            name: "Start".into(),
            kind: catalog::ValueKind::Dword,
            on: Some(Value::Int(4)),
            off: Some(Value::Int(3)),
            delete_key_on_revert: None,
            must_exist: true,
        };
        let t = Tweak {
            id: "test.tweak".into(),
            category: "services".into(),
            title: "t".into(),
            description: "d".into(),
            risk: Risk::Safe,
            restart: Restart::None,
            min_build: None,
            max_build: None,
            tags: vec![],
            ops: vec![op.clone()],
            ucpd: false,
            choices: vec![],
        };
        assert!(missing_required_key(&t));
        assert_eq!(tweak_state(&t, 26100).state, TweakState::Unsupported);
        assert_eq!(apply_op(&t, &op, &Target::Set(Value::Int(4))).unwrap_err(), t!("key_missing"));
        assert!(!reg::key_exists(Hive::HKCU, &path).unwrap(), "se creó la clave");
        assert!(!reg::key_exists(Hive::HKCU, &key.0).unwrap(), "se creó la clave padre");

        // Con la clave presente se aplica normal.
        reg::write_raw(Hive::HKCU, &path, "Start", &reg::encode(catalog::ValueKind::Dword, &Value::Int(3))).unwrap();
        assert!(!missing_required_key(&t));
        apply_op(&t, &op, &Target::Set(Value::Int(4))).unwrap();
        assert!(reg::matches(reg::read(Hive::HKCU, &path, "Start").unwrap().as_ref(), catalog::ValueKind::Dword, &Value::Int(4)));
    }

    /// Claves que `missing_keys` podría devolver en la práctica (con al menos 2 niveles).
    fn reg_ancestors(path: &str) -> Vec<String> {
        let parts: Vec<&str> = path.split('\\').collect();
        (3..=parts.len()).rev().map(|i| parts[..i].join("\\")).collect()
    }

    #[test]
    fn undo_refuses_hkcu_when_running_as_another_account() {
        let key = TestKey::new("foreign");
        let (engine, dir) = temp_engine("foreign");
        engine.save(&[entry_for(&key.0, None, Some(dword(1)))]).unwrap();
        assert!(engine.undo(1, true, Some("otra")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Sondeo de UCPD sin cambiar nada: para cada valor HKCU del catálogo que ya existe,
    /// lo reescribe con exactamente los mismos bytes. Si UCPD lo protegiera, fallaría con
    /// acceso denegado. Correr con `cargo test -- --ignored --nocapture probe_ucpd`.
    #[test]
    #[ignore]
    fn probe_ucpd_on_catalog_values() {
        let mut blocked = Vec::new();
        for t in &catalog::tweaks().tweak {
            for op in t.ops.iter().filter(|o| o.hive == Hive::HKCU) {
                let label = format!("{} · {}\\{}", t.id, op.path, op.name);
                // Las directivas de usuario solo las escriben los admins: no es UCPD.
                let policy = op.path.to_ascii_lowercase().starts_with(r"software\policies\");
                if policy && !crate::system::is_elevated() {
                    println!("req.admin {label}");
                    continue;
                }
                match reg::read(op.hive, &op.path, &op.name) {
                    Ok(Some(v)) => match reg::write_raw(op.hive, &op.path, &op.name, &v) {
                        Ok(()) => println!("OK        {label}"),
                        Err(e) => {
                            println!("BLOQUEADO {label}: {e}");
                            blocked.push(label);
                        }
                    },
                    Ok(None) => println!("sin valor {label}"),
                    Err(e) => println!("error     {label}: {e}"),
                }
            }
        }
        println!("bloqueados: {}", blocked.len());
    }
}
