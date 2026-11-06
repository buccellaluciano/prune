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
    /// Toca HKCU (configuración del usuario).
    pub per_user: bool,
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
                per_user: t.per_user(),
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
            let raw = reg::encode(op.kind, v);
            reg::write_raw(op.hive, &op.path, &op.name, &raw).map_err(|e| e.to_string())?;
            change.after = Some(raw);
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
                    Ok(()) => format!(
                        "El historial no se pudo leer ({e}). Se guardó una copia en {} y se empezó uno nuevo.",
                        backup.display()
                    ),
                    Err(re) => format!("El historial no se pudo leer ({e}) ni respaldar ({re})."),
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
            if let (true, Some(user)) = (t.per_user(), foreign_user) {
                results.push(ItemResult {
                    id: t.id.clone(),
                    ok: false,
                    error: Some(format!("La app corre con otra cuenta de administrador: se aplicaría a esa cuenta y no a {user}")),
                });
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
            // Id único aunque dos operaciones caigan en el mismo milisegundo.
            let id = entries.last().map_or(id, |l| id.max(l.id + 1));
            entries.push(JournalEntry { id, at: id / 1000, label, changes, removed_apps: vec![], undone: false });
            if let Err(e) = self.save(&entries) {
                results.push(ItemResult { id: "journal".into(), ok: false, error: Some(format!("No se pudo guardar el historial: {e}")) });
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
            .ok_or("Entrada de historial inexistente")?;
        if entry.undone {
            return Err("Ya se deshizo".into());
        }
        if entry.changes.is_empty() {
            return Err("Las apps quitadas no se pueden restaurar desde acá: reinstalalas desde Microsoft Store.".into());
        }
        if let (true, Some(user)) = (entry.changes.iter().any(|c| c.hive == Hive::HKCU), foreign_user) {
            return Err(format!(
                "La app corre con otra cuenta de administrador: deshacer tocaría esa cuenta y no la de {user}"
            ));
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
                    let title = catalog::find_tweak(&c.tweak_id).map_or(c.tweak_id.clone(), |t| t.title.clone());
                    conflicts.push(format!(
                        "{title}: {}\\{} vale {}",
                        reg::hive_label(c.hive),
                        c.name,
                        reg::describe(current.as_ref())
                    ));
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
    fn undo_refuses_hkcu_when_running_as_another_account() {
        let key = TestKey::new("foreign");
        let (engine, dir) = temp_engine("foreign");
        engine.save(&[entry_for(&key.0, None, Some(dword(1)))]).unwrap();
        assert!(engine.undo(1, true, Some("otra")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
