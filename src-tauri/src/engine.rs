//! Motor de tweaks: estado real, vista previa, aplicar/revertir y deshacer.

use crate::catalog::{self, Hive, Op, Restart, Risk, Tweak, Value};
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
        let id = d.choice.as_deref().ok_or("Falta elegir una opción")?;
        let c = t.find_choice(id).ok_or_else(|| format!("Opción desconocida: {id}"))?;
        if !c.supported_on(build) {
            return Err(format!("'{}' no está disponible en esta versión de Windows", c.label));
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

fn desired_label(t: &Tweak, d: &Desired) -> String {
    match d.choice.as_deref().and_then(|c| t.find_choice(c)) {
        Some(c) => format!("{}: {}", t.title, c.label),
        None => format!("{} {}", if d.applied { "✓" } else { "↺" }, t.title),
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
}

pub struct Engine {
    journal_path: PathBuf,
    lock: Mutex<()>,
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
        "(Predeterminado)".into()
    } else {
        op.name.clone()
    }
}

fn off_label(op: &Op) -> String {
    match (&op.off, &op.delete_key_on_revert) {
        (_, Some(_)) => "(borra la clave)".into(),
        (Some(v), None) => reg::describe_value(v),
        (None, None) => "(borra el valor)".into(),
    }
}

pub struct Detected {
    pub state: TweakState,
    pub ops: Vec<OpView>,
    pub choice: Option<String>,
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
                format!("error: {e}")
            }
        };
        let (on, off) = if t.is_choice() {
            let opts = t
                .choices
                .iter()
                .map(|c| format!("{} {}", reg::describe_value(&c.values[i]), c.label))
                .collect::<Vec<_>>()
                .join(" · ");
            (opts, String::new())
        } else {
            (reg::describe_value(op.on.as_ref().expect("op sin on")), off_label(op))
        };
        views.push(OpView { key: key_label(op), name: name_label(op), current, on, off });
        current_values.push(cur.ok().flatten());
    }

    let mut choice = None;
    let state = if !t.supported_on(build) {
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
                title: t.title.clone(),
                description: t.description.clone(),
                risk: t.risk,
                restart: t.restart,
                tags: t.tags.clone(),
                requires_admin: t.requires_admin(),
                state,
                ops,
                choices: t
                    .choices
                    .iter()
                    .filter(|c| c.supported_on(build))
                    .map(|c| ChoiceView { id: c.id.clone(), label: c.label.clone(), default: c.default })
                    .collect(),
                choice,
            }
        })
        .collect()
}

pub fn plan(desired: &[Desired], build: u32) -> Result<Vec<PlannedChange>, String> {
    let mut out = Vec::new();
    for d in desired {
        let t = catalog::find_tweak(&d.id).ok_or_else(|| format!("tweak desconocido: {}", d.id))?;
        let targets = targets(t, d, build).map_err(|e| format!("{}: {e}", t.title))?;
        for (op, target) in t.ops.iter().zip(&targets) {
            let cur = reg::read(op.hive, &op.path, &op.name).map_err(|e| e.to_string())?;
            let (to, noop) = match target {
                Target::Set(v) => (reg::describe_value(v), reg::matches(cur.as_ref(), op.kind, v)),
                Target::DeleteValue => ("(borra el valor)".into(), cur.is_none()),
                Target::DeleteTree(_) => ("(borra la clave)".into(), cur.is_none()),
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
    };
    match target {
        Target::DeleteTree(tree) => {
            reg::delete_tree(op.hive, tree).map_err(|e| e.to_string())?;
            change.deleted_tree = Some(tree.clone());
        }
        Target::Set(v) => {
            change.created_keys = reg::missing_keys(op.hive, &op.path).map_err(|e| e.to_string())?;
            reg::write_raw(op.hive, &op.path, &op.name, &reg::encode(op.kind, v))
                .map_err(|e| e.to_string())?;
        }
        Target::DeleteValue => {
            reg::delete_value(op.hive, &op.path, &op.name).map_err(|e| e.to_string())?;
        }
    }
    Ok(change)
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
        }
    }

    fn load(&self) -> Vec<JournalEntry> {
        fs::read(&self.journal_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
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
        h
    }

    pub fn record_removed_apps(&self, label: String, apps: Vec<String>) -> Result<(), String> {
        let _g = self.lock.lock().unwrap();
        let mut entries = self.load();
        entries.push(JournalEntry {
            id: now_ms(),
            at: now_ms() / 1000,
            label,
            changes: vec![],
            removed_apps: apps,
            undone: false,
        });
        self.save(&entries)
    }

    pub fn apply(&self, desired: &[Desired], elevated: bool, build: u32) -> ApplyReport {
        let _g = self.lock.lock().unwrap();
        let mut results = Vec::new();
        let mut changes = Vec::new();
        let mut restart = Restart::None;
        let mut applied_titles = Vec::new();

        for d in desired {
            let Some(t) = catalog::find_tweak(&d.id) else {
                results.push(ItemResult { id: d.id.clone(), ok: false, error: Some("tweak desconocido".into()) });
                continue;
            };
            if t.requires_admin() && !elevated {
                results.push(ItemResult { id: t.id.clone(), ok: false, error: Some("Requiere ejecutar como administrador".into()) });
                continue;
            }
            if (d.applied || t.is_choice()) && !t.supported_on(build) {
                results.push(ItemResult { id: t.id.clone(), ok: false, error: Some("No aplica a esta versión de Windows".into()) });
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
                applied_titles.push(desired_label(t, d));
                changes.extend(done);
                results.push(ItemResult { id: t.id.clone(), ok: true, error: None });
            }
        }

        let entry_id = if changes.is_empty() {
            None
        } else {
            let mut entries = self.load();
            let id = now_ms();
            let label = match applied_titles.len() {
                1 => applied_titles.remove(0),
                n => format!("{n} tweaks"),
            };
            entries.push(JournalEntry { id, at: id / 1000, label, changes, removed_apps: vec![], undone: false });
            if let Err(e) = self.save(&entries) {
                results.push(ItemResult { id: "journal".into(), ok: false, error: Some(format!("No se pudo guardar el historial: {e}")) });
            }
            Some(id)
        };
        ApplyReport { entry_id, results, restart }
    }

    pub fn undo(&self, entry_id: u64) -> Result<ApplyReport, String> {
        let _g = self.lock.lock().unwrap();
        let mut entries = self.load();
        let entry = entries
            .iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or("Entrada de historial inexistente")?;
        if entry.undone {
            return Err("Ya se deshizo".into());
        }
        if entry.changes.is_empty() {
            return Err("Las apps quitadas no se pueden restaurar desde acá: reinstalalas desde Microsoft Store.".into());
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
        let mut ids: Vec<String> = entry.changes.iter().map(|c| c.tweak_id.clone()).collect();
        ids.dedup();
        for id in ids {
            if !results.iter().any(|r| r.id == id) {
                results.push(ItemResult { id, ok: true, error: None });
            }
        }
        entry.undone = true;
        self.save(&entries)?;
        Ok(ApplyReport { entry_id: Some(entry_id), results, restart })
    }
}
