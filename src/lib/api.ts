import { invoke } from "@tauri-apps/api/core";
import { mock, mockProgress, mockUpdateProgress } from "./mock";

export type Risk = "safe" | "moderate" | "advanced";
export type Restart = "none" | "explorer" | "logoff" | "reboot";
export type TweakState = "applied" | "default" | "partial" | "unsupported" | "error";

export interface SystemInfo {
  product: string;
  display_version: string;
  build: number;
  ubr: number;
  edition: string;
  elevated: boolean;
  /** Usuario real de la sesión si la app corre con otra cuenta de admin. */
  foreign_user: string | null;
  /** UCPD activo: Windows bloquea ciertos valores para programas de terceros. */
  ucpd_active: boolean;
}

export interface Category {
  id: string;
  title: string;
  icon: string;
}

export interface OpView {
  key: string;
  name: string;
  current: string;
  on: string;
  off: string;
}

export interface Tweak {
  id: string;
  category: string;
  title: string;
  description: string;
  risk: Risk;
  restart: Restart;
  tags: string[];
  requires_admin: boolean;
  /** Toca configuración del usuario (HKCU). */
  per_user: boolean;
  /** Valor protegido por UCPD. */
  ucpd: boolean;
  state: TweakState;
  ops: OpView[];
  /** Vacío para tweaks de switch; con opciones se muestra un selector. */
  choices: ChoiceView[];
  /** Opción vigente según el registro (null si no coincide con ninguna). */
  choice: string | null;
}

export interface ChoiceView {
  id: string;
  label: string;
  default: boolean;
}

export interface Overview {
  system: SystemInfo;
  categories: Category[];
  tweaks: Tweak[];
}

export interface Desired {
  id: string;
  applied: boolean;
  choice?: string;
}

export interface PlannedChange {
  tweak_id: string;
  key: string;
  name: string;
  from: string;
  to: string;
  noop: boolean;
}

export interface ItemResult {
  id: string;
  ok: boolean;
  error: string | null;
}

export interface ApplyReport {
  entry_id: number | null;
  results: ItemResult[];
  restart: Restart;
  /** Deshacer: valores que cambiaron después; si hay, no se tocó nada. */
  conflicts: string[];
  warning: string | null;
}

export interface JournalEntry {
  id: number;
  at: number;
  label: string;
  changes: { tweak_id: string }[];
  removed_apps: string[];
  undone: boolean;
  /** Cambio de plan de energía (GUIDs antes/después). */
  power?: { before: string; after: string; created: string | null } | null;
}

export interface BloatApp {
  id: string;
  title: string;
  tier: Risk;
  note: string | null;
  packages: { full_name: string; family_name: string; version: string }[];
}

export interface RemoveResult {
  id: string;
  ok: boolean;
  removed: string[];
  error: string | null;
}

/** Fuera de Tauri (vite en el navegador) usamos datos de ejemplo, sin tocar nada. */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri ? invoke<T>(cmd, args) : mock<T>(cmd, args);
}

/** Idioma del backend (catálogo y mensajes). */
export const i18nApi = {
  set: (lang: string) => call<void>("set_language", { lang }),
};

export const api = {
  overview: () => call<Overview>("get_overview"),
  plan: (changes: Desired[]) => call<PlannedChange[]>("plan_changes", { changes }),
  apply: (changes: Desired[]) => call<ApplyReport>("apply_changes", { changes }),
  history: () => call<JournalEntry[]>("history"),
  undo: (entryId: number, force = false) => call<ApplyReport>("undo", { entryId, force }),
  listBloat: () => call<BloatApp[]>("list_bloat"),
  removeBloat: (ids: string[], deprovision: boolean) =>
    call<RemoveResult[]>("remove_bloat", { ids, deprovision }),
};

export type WingetAction = "install" | "upgrade" | "uninstall";

export interface AppStatus {
  id: string;
  title: string;
  group: string;
  description: string;
  installed: string | null;
  available: string | null;
}

export interface AppsOverview {
  winget: string | null;
  error: string | null;
  groups: { id: string; title: string }[];
  apps: AppStatus[];
}

export interface ActionResult {
  id: string;
  ok: boolean;
  message: string | null;
}

/** Etapa que reporta winget mientras trabaja. */
export type WingetPhase = "download" | "hash" | "install" | "uninstall" | "done";

export interface WingetProgress {
  id: string;
  line: string;
  phase: WingetPhase | null;
  /** 0–1 cuando la línea trae avance. */
  fraction: number | null;
}

export interface InstallerInfo {
  url: string | null;
  sha256: string | null;
  version: string | null;
}

export interface SecurityCheck {
  installer: InstallerInfo | null;
  error: string | null;
}

export const wingetApi = {
  status: () => call<AppsOverview>("winget_status"),
  act: (id: string, action: WingetAction) => call<ActionResult>("winget_action", { id, action }),
  /** Íconos del catálogo (data URLs). Con `fetch` busca los que falten. */
  icons: (fetch: boolean) => call<Record<string, string>>("winget_icons", { fetch }),
  /** URL y SHA-256 del instalador según winget, para revisarlo antes de instalar. */
  check: (id: string) => call<SecurityCheck>("winget_check", { id }),
  /** Progreso en vivo de la operación en curso. */
  async onProgress(cb: (p: WingetProgress) => void): Promise<() => void> {
    if (!isTauri) return mockProgress(cb);
    const { listen } = await import("@tauri-apps/api/event");
    return listen<WingetProgress>("winget-progress", (e) => cb(e.payload));
  },
};

// ───────── Actualizaciones de P-Rune ─────────

export interface UpdateInfo {
  version: string;
  current: string;
  notes: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

export const appApi = {
  version: () => call<string>("app_version"),
};

export const updateApi = {
  /** Consulta GitHub Releases. null si ya está en la última versión. */
  check: () => call<UpdateInfo | null>("update_check"),
  /** Descarga, verifica la firma y lanza el instalador; la app se cierra. */
  install: () => call<void>("update_install"),
  async onProgress(cb: (p: UpdateProgress) => void): Promise<() => void> {
    if (!isTauri) return mockUpdateProgress(cb);
    const { listen } = await import("@tauri-apps/api/event");
    return listen<UpdateProgress>("update-progress", (e) => cb(e.payload));
  },
};

export const vtApi = {
  /** Abre en el navegador el informe de VirusTotal de ese hash. */
  open: (sha256: string) => call<void>("vt_open", { sha256 }),
};

export interface UserInfo {
  name: string;
  username: string;
  /** data URL de la foto de la cuenta, o null si no tiene. */
  picture: string | null;
}

export const userApi = {
  get: () => call<UserInfo>("get_user"),
};

// ───────── Disco (solo lectura) ─────────

export interface Drive {
  letter: string;
  label: string;
  fs: string;
  total: number;
  free: number;
  removable: boolean;
}

export interface ScanStatus {
  running: boolean;
  done: boolean;
  cancelled: boolean;
  root: string;
  files: number;
  dirs: number;
  bytes: number;
  denied: number;
  current: string;
  elapsed_ms: number;
  error: string | null;
}

export interface Hotspot {
  id: number;
  path: string;
  size: number;
  files: number;
}

export interface DiskSummary {
  root: string;
  size: number;
  files: number;
  dirs: number;
  cloud: number;
  denied_dirs: number;
  elapsed_ms: number;
  hotspots: Hotspot[];
}

export interface DirEntry {
  id: number;
  name: string;
  size: number;
  files: number;
  cloud: number;
  denied: boolean;
  link: boolean;
  has_children: boolean;
}

export interface DirView {
  id: number;
  path: string;
  size: number;
  files: number;
  own: number;
  own_files: number;
  cloud: number;
  crumbs: [number, string][];
  children: DirEntry[];
  hidden: number;
}

export const diskApi = {
  drives: () => call<Drive[]>("disk_drives"),
  scan: (letter: string) => call<void>("disk_scan", { letter }),
  status: () => call<ScanStatus>("disk_status"),
  cancel: () => call<void>("disk_cancel"),
  summary: () => call<DiskSummary>("disk_summary"),
  dir: (id: number) => call<DirView>("disk_dir", { id }),
  open: (id: number) => call<void>("disk_open", { id }),
};

// ───────── Limpieza de archivos descartables ─────────

export type CleanupId =
  | "temp_user"
  | "temp_system"
  | "crash"
  | "shaders"
  | "thumbnails"
  | "update_cache"
  | "spotify"
  | "prefetch"
  | "recycle_bin";

export interface CleanView {
  id: CleanupId;
  bytes: number;
  files: number;
  admin: boolean;
  per_user: boolean;
  warn: "slower_start" | "permanent" | "close_app" | null;
  /** Se puede limpiar ahora (permisos y cuenta correctos). */
  available: boolean;
}

export interface CleanResult {
  id: CleanupId;
  freed: number;
  deleted: number;
  skipped: number;
  error: string | null;
}

export const cleanupApi = {
  scan: () => call<CleanView[]>("cleanup_scan"),
  run: (ids: CleanupId[]) => call<CleanResult[]>("cleanup_run", { ids }),
};
