//! Análisis de uso de disco. Solo lectura: mide y ordena, nunca mueve ni borra nada.
//!
//! Recorre el disco con `FindFirstFileExW` (modo "large fetch", sin abrir archivos) en
//! varios hilos que comparten una cola de carpetas pendientes.
//! - No sigue uniones ni enlaces simbólicos (evita contar dos veces y bucles).
//! - Los archivos de OneDrive "solo en la nube" se cuentan aparte y nunca se abren.
//! - Archivos comprimidos/dispersos: se usa el tamaño real en disco.
//! - Las carpetas sin permiso se marcan y el escaneo sigue.

use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::Instant;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FileIdBothDirectoryInfo, GetDiskFreeSpaceExW, GetDriveTypeW, GetFileInformationByHandleEx,
    GetLogicalDrives, GetVolumeInformationW, FILE_FLAG_BACKUP_SEMANTICS, FILE_ID_BOTH_DIR_INFO, FILE_LIST_DIRECTORY,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const ATTR_DIRECTORY: u32 = 0x10;
const ATTR_REPARSE_POINT: u32 = 0x400;
const ATTR_OFFLINE: u32 = 0x1000;
const ATTR_RECALL_ON_OPEN: u32 = 0x4_0000;
const ATTR_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
const TAG_MOUNT_POINT: u32 = 0xA000_0003;
const TAG_SYMLINK: u32 = 0xA000_000C;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}

// ───────────────────────────── Unidades ─────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Drive {
    pub letter: String,
    pub label: String,
    pub fs: String,
    pub total: u64,
    pub free: u64,
    pub removable: bool,
}

pub fn drives() -> Vec<Drive> {
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .filter_map(|i| {
            let letter = ((b'A' + i) as char).to_string();
            let root = wide(&format!("{letter}:\\"));
            let kind = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
            if kind != DRIVE_FIXED && kind != DRIVE_REMOVABLE {
                return None;
            }
            let (mut total, mut free) = (0u64, 0u64);
            unsafe { GetDiskFreeSpaceExW(PCWSTR(root.as_ptr()), None, Some(&mut total), Some(&mut free)) }.ok()?;
            let (mut label, mut fs) = ([0u16; 261], [0u16; 64]);
            let _ = unsafe {
                GetVolumeInformationW(PCWSTR(root.as_ptr()), Some(&mut label), None, None, None, Some(&mut fs))
            };
            Some(Drive {
                letter,
                label: from_wide(&label),
                fs: from_wide(&fs),
                total,
                free,
                removable: kind == DRIVE_REMOVABLE,
            })
        })
        .collect()
}

// ───────────────────────────── Árbol ─────────────────────────────

#[derive(Debug, Clone, Default)]
struct Node {
    name: Box<str>,
    parent: u32,
    /// Tamaño total en disco (archivos propios + subcarpetas).
    size: u64,
    /// Tamaño de los archivos sueltos de esta carpeta.
    own: u64,
    files: u64,
    own_files: u64,
    /// Archivos que están solo en la nube (no ocupan espacio local).
    cloud: u64,
    denied: bool,
    /// Unión o enlace simbólico: no se recorre.
    link: bool,
    children: Vec<u32>,
}

pub struct Tree {
    root: String,
    nodes: Vec<Node>,
    denied_dirs: u64,
}

impl Tree {
    fn path(&self, mut id: u32) -> String {
        let mut parts = Vec::new();
        loop {
            parts.push(&*self.nodes[id as usize].name);
            if id == 0 {
                break;
            }
            id = self.nodes[id as usize].parent;
        }
        parts.reverse();
        let mut p = parts.join("\\");
        if parts.len() == 1 {
            p.push('\\');
        }
        p
    }
}

// ───────────────────────────── Escaneo ─────────────────────────────

#[derive(Default)]
struct Scan {
    running: AtomicBool,
    cancel: AtomicBool,
    files: AtomicU64,
    dirs: AtomicU64,
    bytes: AtomicU64,
    denied: AtomicU64,
    elapsed_ms: AtomicU64,
    current: Mutex<String>,
    root: Mutex<String>,
    error: Mutex<Option<String>>,
    tree: Mutex<Option<Arc<Tree>>>,
}

static SCAN: LazyLock<Scan> = LazyLock::new(Scan::default);

#[derive(Debug, Clone, Serialize)]
pub struct ScanStatus {
    pub running: bool,
    pub done: bool,
    pub cancelled: bool,
    pub root: String,
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub denied: u64,
    pub current: String,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

pub fn status() -> ScanStatus {
    let s = &*SCAN;
    ScanStatus {
        running: s.running.load(Relaxed),
        done: s.tree.lock().unwrap().is_some(),
        cancelled: s.cancel.load(Relaxed) && !s.running.load(Relaxed),
        root: s.root.lock().unwrap().clone(),
        files: s.files.load(Relaxed),
        dirs: s.dirs.load(Relaxed),
        bytes: s.bytes.load(Relaxed),
        denied: s.denied.load(Relaxed),
        current: s.current.lock().unwrap().clone(),
        elapsed_ms: s.elapsed_ms.load(Relaxed),
        error: s.error.lock().unwrap().clone(),
    }
}

pub fn cancel() {
    SCAN.cancel.store(true, Relaxed);
}

/// Inicia el escaneo de una unidad (solo letras de `drives()`). Corre en segundo plano.
pub fn start(letter: &str) -> Result<(), String> {
    let letter = letter.trim().trim_end_matches([':', '\\']).to_ascii_uppercase();
    let Some(drive) = drives().into_iter().find(|d| d.letter == letter) else {
        return Err(t!("drive_unavailable", letter = letter));
    };
    // Solo NTFS y ReFS dan IDs de archivo únicos y estables. En FAT32/exFAT pueden venir
    // en 0 o repetidos: deduplicar ahí colapsaría archivos distintos en uno.
    let fs = drive.fs.to_ascii_uppercase();
    DEDUPE.store(fs == "NTFS" || fs == "REFS", Relaxed);
    let s = &*SCAN;
    if s.running.swap(true, Relaxed) {
        return Err(t!("scan_running"));
    }
    s.cancel.store(false, Relaxed);
    for c in [&s.files, &s.dirs, &s.bytes, &s.denied, &s.elapsed_ms] {
        c.store(0, Relaxed);
    }
    *s.tree.lock().unwrap() = None;
    *s.error.lock().unwrap() = None;
    reset_seen();
    *s.root.lock().unwrap() = format!("{letter}:\\");

    std::thread::spawn(move || {
        let t0 = Instant::now();
        let tree = scan_tree(&format!("{letter}:"));
        SCAN.elapsed_ms.store(t0.elapsed().as_millis() as u64, Relaxed);
        if !SCAN.cancel.load(Relaxed) {
            *SCAN.tree.lock().unwrap() = Some(Arc::new(tree));
        }
        reset_seen(); // libera la memoria de IDs: ya no hace falta
        SCAN.running.store(false, Relaxed);
    });
    Ok(())
}

struct Queue {
    jobs: VecDeque<(u32, String)>,
    active: usize,
}

fn scan_tree(root: &str) -> Tree {
    let nodes = Mutex::new(vec![Node { name: root.into(), ..Default::default() }]);
    let queue = Mutex::new(Queue { jobs: VecDeque::from([(0u32, format!(r"\\?\{root}\"))]), active: 0 });
    let cv = Condvar::new();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 8);

    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let job = {
                    let mut q = queue.lock().unwrap();
                    loop {
                        if SCAN.cancel.load(Relaxed) {
                            q.jobs.clear();
                        }
                        if let Some(j) = q.jobs.pop_front() {
                            q.active += 1;
                            break Some(j);
                        }
                        if q.active == 0 {
                            break None;
                        }
                        q = cv.wait(q).unwrap();
                    }
                };
                let Some((id, path)) = job else {
                    cv.notify_all();
                    return;
                };
                let listing = list_dir(&path);
                let mut new_jobs = Vec::with_capacity(listing.dirs.len());
                {
                    let mut n = nodes.lock().unwrap();
                    let base = n.len() as u32;
                    for (i, d) in listing.dirs.iter().enumerate() {
                        n.push(Node {
                            name: d.name.as_str().into(),
                            parent: id,
                            link: d.link,
                            cloud: 0,
                            ..Default::default()
                        });
                        if !d.link && !d.cloud_dir {
                            let sep = if path.ends_with('\\') { "" } else { "\\" };
                            new_jobs.push((base + i as u32, format!("{path}{sep}{}", d.name)));
                        }
                    }
                    let me = &mut n[id as usize];
                    me.own = listing.own;
                    me.own_files = listing.files;
                    me.cloud = listing.cloud;
                    me.denied = listing.denied;
                    me.children = (base..base + listing.dirs.len() as u32).collect();
                }
                let s = &*SCAN;
                let dirs = s.dirs.fetch_add(1, Relaxed);
                s.files.fetch_add(listing.files, Relaxed);
                s.bytes.fetch_add(listing.own, Relaxed);
                if listing.denied {
                    s.denied.fetch_add(1, Relaxed);
                }
                if dirs % 64 == 0 {
                    *s.current.lock().unwrap() = path.trim_start_matches(r"\\?\").to_string();
                }
                let mut q = queue.lock().unwrap();
                q.jobs.extend(new_jobs);
                q.active -= 1;
                drop(q);
                cv.notify_all();
            });
        }
    });

    let mut nodes = nodes.into_inner().unwrap();
    // Los hijos siempre tienen índice mayor que su padre: sumar de atrás hacia adelante.
    for n in nodes.iter_mut() {
        n.size = n.own;
        n.files = n.own_files;
    }
    for i in (1..nodes.len()).rev() {
        let (size, files, cloud, p) = (nodes[i].size, nodes[i].files, nodes[i].cloud, nodes[i].parent as usize);
        nodes[p].size += size;
        nodes[p].files += files;
        nodes[p].cloud += cloud;
    }
    let sizes: Vec<u64> = nodes.iter().map(|n| n.size).collect();
    for n in nodes.iter_mut() {
        n.children.sort_unstable_by(|a, b| sizes[*b as usize].cmp(&sizes[*a as usize]));
    }
    Tree { root: format!("{root}\\"), denied_dirs: SCAN.denied.load(Relaxed), nodes }
}

struct SubDir {
    name: String,
    link: bool,
    cloud_dir: bool,
}

#[derive(Default)]
struct Listing {
    dirs: Vec<SubDir>,
    own: u64,
    files: u64,
    cloud: u64,
    denied: bool,
}

/// Si se deduplican enlaces duros por ID de archivo (solo NTFS/ReFS).
static DEDUPE: AtomicBool = AtomicBool::new(true);

/// Tamaño ya contado por ID de archivo. Un archivo con varios enlaces duros (típico entre
/// WinSxS y System32) aparece en varias carpetas pero ocupa espacio una sola vez.
/// NTFS solo mantiene al día el tamaño en una de las entradas de directorio del archivo;
/// las demás pueden quedar viejas (incluso en 0), así que nos quedamos con el mayor.
static SEEN: LazyLock<[Mutex<HashMap<i64, u64>>; 64]> =
    LazyLock::new(|| std::array::from_fn(|_| Mutex::new(HashMap::new())));

/// Cuánto suma esta aparición del archivo al total: todo la primera vez y, si una
/// aparición posterior trae un tamaño mayor, solo la diferencia.
fn claim(file_id: i64, size: u64) -> u64 {
    let mut m = SEEN[(file_id as u64 % 64) as usize].lock().unwrap();
    let prev = m.entry(file_id).or_insert(0);
    let delta = size.saturating_sub(*prev);
    *prev = (*prev).max(size);
    delta
}

fn reset_seen() {
    for s in SEEN.iter() {
        *s.lock().unwrap() = HashMap::new();
    }
}

/// Lista una carpeta con `GetFileInformationByHandleEx(FileIdBothDirectoryInfo)`: en una
/// sola pasada trae atributos, tamaño ocupado en disco e ID de cada archivo, sin abrirlos.
fn list_dir(path: &str) -> Listing {
    let mut out = Listing::default();
    let p = wide(path);
    let handle = unsafe {
        CreateFileW(
            PCWSTR(p.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    };
    let Ok(handle) = handle else {
        out.denied = true;
        return out;
    };
    // 64 KB alineados a 8 bytes, como pide la estructura.
    let mut buf = vec![0u64; 8192];
    while unsafe {
        GetFileInformationByHandleEx(handle, FileIdBothDirectoryInfo, buf.as_mut_ptr() as *mut _, (buf.len() * 8) as u32)
    }
    .is_ok()
    {
        let base = buf.as_ptr() as *const u8;
        let mut offset = 0usize;
        loop {
            // SAFETY: el sistema llenó el buffer con entradas encadenadas por NextEntryOffset.
            let info = unsafe { &*(base.add(offset) as *const FILE_ID_BOTH_DIR_INFO) };
            let name = unsafe {
                let ptr = std::ptr::addr_of!(info.FileName) as *const u16;
                String::from_utf16_lossy(std::slice::from_raw_parts(ptr, info.FileNameLength as usize / 2))
            };
            let attr = info.FileAttributes;
            if name != "." && name != ".." {
                if attr & ATTR_DIRECTORY != 0 {
                    // Para puntos de reparación, EaSize trae la etiqueta del punto.
                    let tag = if attr & ATTR_REPARSE_POINT != 0 { info.EaSize } else { 0 };
                    out.dirs.push(SubDir {
                        link: tag == TAG_MOUNT_POINT || tag == TAG_SYMLINK,
                        // Carpeta que se descargaría de la nube al listarla: no se entra.
                        cloud_dir: attr & ATTR_RECALL_ON_OPEN != 0,
                        name,
                    });
                } else {
                    out.files += 1;
                    if attr & (ATTR_RECALL_ON_DATA_ACCESS | ATTR_RECALL_ON_OPEN | ATTR_OFFLINE) != 0 {
                        out.cloud += info.EndOfFile.max(0) as u64;
                    } else {
                        // En entradas de enlaces duros NTFS suele dejar AllocationSize en 0
                        // (dato viejo) aunque EndOfFile esté bien: en ese caso estimamos
                        // con EndOfFile redondeado al clúster de 4 KB.
                        let alloc = info.AllocationSize.max(0) as u64;
                        let eof = info.EndOfFile.max(0) as u64;
                        let size = if alloc > 0 || eof == 0 { alloc } else { eof.div_ceil(4096) * 4096 };
                        out.own += if DEDUPE.load(Relaxed) { claim(info.FileId, size) } else { size };
                    }
                }
            }
            if info.NextEntryOffset == 0 {
                break;
            }
            offset += info.NextEntryOffset as usize;
        }
    }
    let _ = unsafe { CloseHandle(handle) };
    out
}

// ───────────────────────────── Consultas ─────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub id: u32,
    pub name: String,
    pub size: u64,
    pub files: u64,
    pub cloud: u64,
    pub denied: bool,
    pub link: bool,
    pub has_children: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DirView {
    pub id: u32,
    pub path: String,
    pub size: u64,
    pub files: u64,
    pub own: u64,
    pub own_files: u64,
    pub cloud: u64,
    pub crumbs: Vec<(u32, String)>,
    pub children: Vec<Entry>,
    /// Cuántas subcarpetas no se muestran (se listan las más pesadas).
    pub hidden: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hotspot {
    pub id: u32,
    pub path: String,
    pub size: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub root: String,
    pub size: u64,
    pub files: u64,
    pub dirs: u64,
    pub cloud: u64,
    pub denied_dirs: u64,
    pub elapsed_ms: u64,
    pub hotspots: Vec<Hotspot>,
}

fn tree() -> Result<Arc<Tree>, String> {
    SCAN.tree.lock().unwrap().clone().ok_or_else(|| t!("scan_missing"))
}

/// "Dónde está el peso": carpetas pesadas en las que ninguna subcarpeta concentra
/// más de la mitad del tamaño. Señalan el lugar concreto en vez de sus ancestros.
fn hotspots(t: &Tree, limit: usize) -> Vec<Hotspot> {
    let total = t.nodes[0].size;
    let threshold = (total / 200).max(100 * 1024 * 1024);
    let mut ids = Vec::new();
    for (i, n) in t.nodes.iter().enumerate().skip(1) {
        if n.size < threshold {
            continue;
        }
        let biggest_child = n.children.first().map_or(0, |c| t.nodes[*c as usize].size);
        if biggest_child * 2 < n.size {
            ids.push(i as u32);
        }
    }
    let mut pairs: Vec<(u32, u64)> = ids.into_iter().map(|i| (i, t.nodes[i as usize].size)).collect();
    pairs.sort_unstable_by(|a, b| b.1.cmp(&a.1));
    pairs
        .into_iter()
        .take(limit)
        .map(|(id, size)| Hotspot { id, path: t.path(id), size, files: t.nodes[id as usize].files })
        .collect()
}

pub fn summary() -> Result<Summary, String> {
    let t = tree()?;
    let r = &t.nodes[0];
    Ok(Summary {
        root: t.root.clone(),
        size: r.size,
        files: r.files,
        dirs: t.nodes.len() as u64,
        cloud: r.cloud,
        denied_dirs: t.denied_dirs,
        elapsed_ms: SCAN.elapsed_ms.load(Relaxed),
        hotspots: hotspots(&t, 40),
    })
}

pub fn dir(id: u32) -> Result<DirView, String> {
    const MAX: usize = 300;
    let t = tree()?;
    let n = t.nodes.get(id as usize).ok_or_else(|| t!("folder_missing"))?;
    let mut crumbs = Vec::new();
    let mut cur = id;
    loop {
        crumbs.push((cur, t.nodes[cur as usize].name.to_string()));
        if cur == 0 {
            break;
        }
        cur = t.nodes[cur as usize].parent;
    }
    crumbs.reverse();
    let children = n
        .children
        .iter()
        .take(MAX)
        .map(|&c| {
            let ch = &t.nodes[c as usize];
            Entry {
                id: c,
                name: ch.name.to_string(),
                size: ch.size,
                files: ch.files,
                cloud: ch.cloud,
                denied: ch.denied,
                link: ch.link,
                has_children: !ch.children.is_empty(),
            }
        })
        .collect();
    Ok(DirView {
        id,
        path: t.path(id),
        size: n.size,
        files: n.files,
        own: n.own,
        own_files: n.own_files,
        cloud: n.cloud,
        crumbs,
        children,
        hidden: n.children.len().saturating_sub(MAX),
    })
}

/// Abre la carpeta en el Explorador. Solo carpetas del árbol escaneado.
pub fn open(id: u32) -> Result<(), String> {
    let t = tree()?;
    if id as usize >= t.nodes.len() {
        return Err(t!("folder_missing"));
    }
    let path = wide(&t.path(id));
    let r = unsafe { ShellExecuteW(None, w!("explore"), PCWSTR(path.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    // ShellExecute devuelve un valor > 32 si tuvo éxito.
    if r.0 as usize > 32 {
        Ok(())
    } else {
        Err(t!("explorer_failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_fixed_drives() {
        let d = drives();
        assert!(d.iter().any(|d| d.total > 0 && d.free <= d.total));
    }

    /// Escaneo real de C: (solo lectura). Correr con `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn scan_c_drive() {
        start("C").unwrap();
        while status().running {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        let s = summary().unwrap();
        let used = drives().into_iter().find(|d| d.letter == "C").map(|d| d.total - d.free).unwrap();
        println!(
            "{} carpetas, {} archivos, {:.1} GB medidos de {:.1} GB usados, {:.1} GB en la nube, {} sin acceso, {} ms",
            s.dirs,
            s.files,
            s.size as f64 / 1e9,
            used as f64 / 1e9,
            s.cloud as f64 / 1e9,
            s.denied_dirs,
            s.elapsed_ms
        );
        for h in s.hotspots.iter().take(12) {
            println!("{:>8.2} GB  {}", h.size as f64 / 1e9, h.path);
        }
        let root = dir(0).unwrap();
        assert_eq!(root.path, "C:\\");
        assert_eq!(root.size, s.size);
    }

    #[test]
    fn lists_a_directory_without_following_links() {
        let tmp = std::env::temp_dir().join(format!("debloater-disk-test-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("sub")).unwrap();
        std::fs::write(tmp.join("a.bin"), vec![1u8; 10_000]).unwrap();
        std::fs::write(tmp.join("sub").join("b.bin"), vec![1u8; 500]).unwrap();
        // Un enlace duro al mismo archivo: no debe sumar dos veces.
        std::fs::hard_link(tmp.join("a.bin"), tmp.join("a-link.bin")).unwrap();
        let l = list_dir(&format!(r"\\?\{}", tmp.display()));
        assert_eq!(l.files, 2);
        assert!((10_000..=16_384).contains(&l.own), "own = {}", l.own);
        assert_eq!(l.dirs.len(), 1);
        assert!(!l.dirs[0].link);
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
