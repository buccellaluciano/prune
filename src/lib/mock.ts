// Backend simulado para desarrollar la UI en el navegador (npm run dev).
// Nunca se usa dentro de Tauri.
import type { AppsOverview, BloatApp, CleanView, JournalEntry, Overview, Tweak, WingetProgress } from "./api";

const op = (key: string, name: string, current: string, on: string, off = "(borra el valor)") => ({
  key, name, current, on, off,
});

const A = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced";
const CDM = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager";

type MockTweak = Omit<Tweak, "choices" | "choice" | "per_user" | "ucpd"> & Partial<Pick<Tweak, "choices" | "choice">>;

const SEARCH_VALUES: Record<string, string> = { hidden: "0", icon: "1", icon_label: "3", box: "2" };

const raw: MockTweak[] = [
  { id: "search.taskbar_mode", category: "search", title: "Búsqueda en la barra de tareas", description: "Cómo se muestra la búsqueda: oculta, solo la lupa, lupa con texto o el cuadro completo.", risk: "safe", restart: "none", tags: ["barra"], requires_admin: false, state: "applied", choice: "hidden",
    choices: [{ id: "hidden", label: "Oculta", default: false }, { id: "icon", label: "Ícono", default: false }, { id: "icon_label", label: "Ícono y texto", default: false }, { id: "box", label: "Cuadro", default: true }],
    ops: [op("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Search", "SearchboxTaskbarMode", "0", "0 Oculta · 1 Ícono · 3 Ícono y texto · 2 Cuadro", "")] },
  { id: "privacy.advertising_id", category: "privacy", title: "Desactivar ID de publicidad", description: "Evita que las apps usen tu identificador de publicidad para anuncios personalizados.", risk: "safe", restart: "none", tags: ["anuncios"], requires_admin: false, state: "default", ops: [op("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\AdvertisingInfo", "Enabled", "1", "0", "1")] },
  { id: "privacy.telemetry_policy", category: "privacy", title: "Telemetría al mínimo (directiva)", description: "Pone los datos de diagnóstico en el nivel más bajo que permite tu edición.", risk: "moderate", restart: "none", tags: ["telemetria"], requires_admin: true, state: "default", ops: [op("HKLM\\SOFTWARE\\Policies\\Microsoft\\Windows\\DataCollection", "AllowTelemetry", "(no existe)", "0")] },
  { id: "ai.recall", category: "ai", title: "Desactivar Recall (instantáneas)", description: "Directiva que impide que Recall guarde capturas de tu actividad.", risk: "safe", restart: "reboot", tags: ["recall"], requires_admin: true, state: "applied", ops: [op("HKLM\\SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsAI", "DisableAIDataAnalysis", "1", "1")] },
  { id: "start.recommendations", category: "start", title: "Ocultar recomendaciones en Inicio", description: "Saca las recomendaciones promocionales del menú Inicio.", risk: "safe", restart: "explorer", tags: ["anuncios"], requires_admin: false, state: "default", ops: [op(A, "Start_IrisRecommendations", "(no existe)", "0", "1")] },
  { id: "start.silent_installs", category: "start", title: "Bloquear instalación silenciosa de apps", description: "Evita que Windows instale juegos promocionados sin preguntar.", risk: "safe", restart: "none", tags: ["bloatware"], requires_admin: false, state: "partial", ops: [op(CDM, "SilentInstalledAppsEnabled", "1", "0", "1")] },
  { id: "search.web_results", category: "search", title: "Sin resultados web de Bing", description: "Las búsquedas de Inicio quedan solo locales.", risk: "moderate", restart: "explorer", tags: ["bing"], requires_admin: false, state: "default", ops: [op("HKCU\\Software\\Policies\\Microsoft\\Windows\\Explorer", "DisableSearchBoxSuggestions", "(no existe)", "1")] },
  { id: "taskbar.align_left", category: "taskbar", title: "Alinear íconos a la izquierda", description: "Botón de Inicio e íconos a la izquierda, como en Windows 10.", risk: "safe", restart: "none", tags: ["barra"], requires_admin: false, state: "default", ops: [op(A, "TaskbarAl", "(no existe)", "0", "1")] },
  { id: "taskbar.end_task", category: "taskbar", title: "'Finalizar tarea' con clic derecho", description: "Agrega 'Finalizar tarea' al menú contextual de la barra.", risk: "safe", restart: "none", tags: ["productividad"], requires_admin: false, state: "applied", ops: [op(A + "\\TaskbarDeveloperSettings", "TaskbarEndTask", "1", "1")] },
  { id: "taskbar.disable_widgets", category: "taskbar", title: "Desactivar Widgets", description: "Directiva que desactiva por completo el panel de Widgets (noticias, clima, MSN).", risk: "safe", restart: "explorer", tags: ["widgets"], requires_admin: true, state: "default", ops: [op("HKLM\\SOFTWARE\\Policies\\Microsoft\\Dsh", "AllowNewsAndInterests", "(no existe)", "0")] },
  { id: "explorer.show_extensions", category: "explorer", title: "Mostrar extensiones de archivo", description: "Así 'factura.pdf.exe' no se disfraza.", risk: "safe", restart: "none", tags: ["seguridad"], requires_admin: false, state: "default", ops: [op(A, "HideFileExt", "1", "0", "1")] },
  { id: "explorer.classic_context_menu", category: "explorer", title: "Menú contextual clásico", description: "El clic derecho completo, sin 'Mostrar más opciones'.", risk: "moderate", restart: "explorer", tags: ["menu"], requires_admin: false, state: "default", ops: [op("HKCU\\Software\\Classes\\CLSID\\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\\InprocServer32", "(Predeterminado)", "(no existe)", "\"\"", "(borra la clave)")] },
  { id: "appearance.dark_mode", category: "appearance", title: "Modo oscuro", description: "Modo oscuro para Windows y para las apps.", risk: "safe", restart: "none", tags: ["tema"], requires_admin: false, state: "applied", ops: [op("HKCU\\...\\Personalize", "AppsUseLightTheme", "0", "0", "1")] },
  { id: "appearance.performance_visuals", category: "appearance", title: "Efectos visuales al mínimo", description: "Como \"Ajustar para obtener el mejor rendimiento\": sin animaciones de ventanas y menús, sombras, desvanecimientos, vista previa del escritorio ni contenido al arrastrar. El texto sigue suavizado.", risk: "safe", restart: "logoff", tags: ["rendimiento", "basico"], requires_admin: false, state: "partial", ops: [
    op("HKCU\\Control Panel\\Desktop", "UserPreferencesMask", "92 12 03 80 12 00 00 00", "90 12 03 80 10 00 00 00", "9E 1E 07 80 12 00 00 00"),
    op("HKCU\\Control Panel\\Desktop\\WindowMetrics", "MinAnimate", "\"1\"", "\"0\"", "\"1\""),
    op("HKCU\\Control Panel\\Desktop", "DragFullWindows", "\"1\"", "\"0\"", "\"1\""),
    op("HKCU\\Software\\Microsoft\\Windows\\DWM", "EnableAeroPeek", "0", "0", "1"),
  ] },
  { id: "gaming.mouse_acceleration", category: "gaming", title: "Desactivar aceleración del mouse", description: "Movimiento 1:1, ideal para juegos.", risk: "safe", restart: "logoff", tags: ["mouse"], requires_admin: false, state: "default", ops: [op("HKCU\\Control Panel\\Mouse", "MouseSpeed", "\"1\"", "\"0\"", "\"1\"")] },
  { id: "power.plan", category: "power", title: "Plan de energía", description: "Alto y Máximo rendimiento no dejan que el procesador baje la frecuencia ni que entre en ahorro: responde más rápido pero consume y calienta más. En notebooks, conviene usarlos solo enchufado.", risk: "moderate", restart: "none", tags: ["energia"], requires_admin: true, state: "default",
    choice: "381b4222-f694-41f0-9685-ff5bb260df2e",
    choices: [
      { id: "a1841308-3541-4fab-bc81-f71556f20b4a", label: "Economizador", default: false },
      { id: "381b4222-f694-41f0-9685-ff5bb260df2e", label: "Equilibrado", default: true },
      { id: "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", label: "Alto rendimiento", default: false },
      { id: "ultimate", label: "Máximo rendimiento", default: false },
    ],
    ops: [op("Plan de energía (powrprof)", "activo", "Equilibrado", "Economizador · Equilibrado · Alto rendimiento · Máximo rendimiento", "")] },
];

const tweaks: Tweak[] = raw.map((t) => ({ choices: [], choice: null, per_user: t.ops.some((o) => o.key.startsWith("HKCU")), ucpd: t.id === "taskbar.disable_widgets", ...t }));

const overview: Overview = {
  system: { product: "Windows 11 Pro", display_version: "25H2", build: 26200, ubr: 7019, edition: "Professional", elevated: false, foreign_user: null, ucpd_active: true },
  categories: [
    { id: "privacy", title: "Privacidad", icon: "shield" },
    { id: "ai", title: "IA y Copilot", icon: "sparkles" },
    { id: "start", title: "Inicio y sugerencias", icon: "megaphone-off" },
    { id: "search", title: "Búsqueda", icon: "search" },
    { id: "taskbar", title: "Barra de tareas", icon: "panel-bottom" },
    { id: "explorer", title: "Explorador", icon: "folder" },
    { id: "appearance", title: "Apariencia", icon: "palette" },
    { id: "gaming", title: "Gaming y entrada", icon: "gamepad" },
    { id: "power", title: "Energía", icon: "zap" },
  ],
  tweaks,
};

const bloat: BloatApp[] = [
  { id: "Clipchamp.Clipchamp", title: "Clipchamp", tier: "safe", note: null, packages: [{ full_name: "Clipchamp.Clipchamp_4.2_x64", family_name: "Clipchamp.Clipchamp_yxz", version: "4.2.0.0" }] },
  { id: "Microsoft.BingNews", title: "Microsoft Noticias", tier: "safe", note: null, packages: [{ full_name: "Microsoft.BingNews_4.55_x64", family_name: "Microsoft.BingNews_8we", version: "4.55.0.0" }] },
  { id: "MicrosoftCorporationII.QuickAssist", title: "Asistencia rápida", tier: "moderate", note: "Es la herramienta de soporte remoto que más usan los estafadores.", packages: [{ full_name: "QuickAssist_2.0_x64", family_name: "QuickAssist_8we", version: "2.0.0.0" }] },
  { id: "Microsoft.GamingApp", title: "Xbox", tier: "advanced", note: "Necesaria para instalar juegos de PC Game Pass.", packages: [{ full_name: "Microsoft.GamingApp_2510_x64", family_name: "Microsoft.GamingApp_8we", version: "2510.0.0.0" }] },
];

const history: JournalEntry[] = [];

// Disco simulado: un árbol chico con tamaños en GB.
const GB = 1024 ** 3;
type MockDir = { id: number; name: string; parent: number; own: number; files: number; kids: number[] };
const dirs: MockDir[] = [];
function mkdir(name: string, parent: number, ownGb: number, files: number): number {
  const id = dirs.length;
  dirs.push({ id, name, parent, own: ownGb * GB, files, kids: [] });
  if (parent >= 0) dirs[parent].kids.push(id);
  return id;
}
{
  const c = mkdir("C:", -1, 12.4, 9);
  const win = mkdir("Windows", c, 0.4, 120);
  mkdir("WinSxS", win, 10.1, 61000);
  mkdir("System32", win, 5.6, 18000);
  const users = mkdir("Users", c, 0, 2);
  const me = mkdir("demo", users, 0.1, 40);
  const local = mkdir("AppData", me, 0, 0);
  const cache = mkdir("Local", local, 1.2, 9000);
  mkdir("StreamingApp\\cache", cache, 6.7, 2400);
  mkdir("Temp", cache, 2.3, 15000);
  mkdir("Downloads", me, 8.9, 312);
  mkdir("Program Files", c, 9.8, 41000);
}
const dsize = (id: number): number => dirs[id].own + dirs[id].kids.reduce((s, k) => s + dsize(k), 0);
const dfiles = (id: number): number => dirs[id].files + dirs[id].kids.reduce((s, k) => s + dfiles(k), 0);
const dpath = (id: number): string => (dirs[id].parent < 0 ? "C:\\" : `${dpath(dirs[id].parent).replace(/\\$/, "")}\\${dirs[id].name}`);
const scanState = { running: false, done: false, started: 0 };

const wingetApps: AppsOverview = {
  winget: "v1.29.380",
  error: null,
  groups: [
    { id: "browsers", title: "Navegadores" },
    { id: "utilities", title: "Utilidades" },
    { id: "dev", title: "Desarrollo" },
  ],
  apps: [
    { id: "Mozilla.Firefox", title: "Firefox", group: "browsers", description: "Navegador de Mozilla, independiente de Chromium.", installed: "143.0", available: "144.0.1" },
    { id: "Brave.Brave", title: "Brave", group: "browsers", description: "Basado en Chromium, con bloqueo de anuncios y rastreadores incluido.", installed: null, available: null },
    { id: "7zip.7zip", title: "7-Zip", group: "utilities", description: "Comprime y descomprime zip, 7z, rar y más.", installed: "25.01", available: null },
    { id: "Microsoft.PowerToys", title: "PowerToys", group: "utilities", description: "Utilidades oficiales de Microsoft.", installed: null, available: null },
    { id: "Git.Git", title: "Git", group: "dev", description: "Control de versiones.", installed: "2.56.0.4", available: null },
  ],
};
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

// Progreso simulado de winget (en Tauri llega por eventos).
type ProgressCb = (p: WingetProgress) => void;
const progressListeners = new Set<ProgressCb>();
export function mockProgress(cb: ProgressCb): () => void {
  progressListeners.add(cb);
  return () => progressListeners.delete(cb);
}
const emit = (p: WingetProgress) => progressListeners.forEach((cb) => cb(p));

// Actualización simulada: en la demo siempre hay una versión nueva para ver la interfaz.
type UpdateCb = (p: { downloaded: number; total: number | null }) => void;
const updateListeners = new Set<UpdateCb>();
export function mockUpdateProgress(cb: UpdateCb): () => void {
  updateListeners.add(cb);
  return () => updateListeners.delete(cb);
}


async function simulateWinget(id: string, action: string) {
  const send = (line: string, phase: WingetProgress["phase"], fraction: number | null = null) =>
    emit({ id, line, phase, fraction });
  if (action === "uninstall") {
    send("Iniciando la desinstalación del paquete...", "uninstall");
    await wait(1500);
    send("Desinstalado correctamente", "done");
    return;
  }
  send(`Descargando https://example.com/${id}-setup.exe`, "download", 0);
  for (let mb = 4; mb <= 48; mb += 4) {
    await wait(160);
    send(`  ${"█".repeat(mb / 4)}${"▒".repeat(12 - mb / 4)}  ${mb}.0 MB / 48.0 MB`, "download", mb / 48);
  }
  send("Se verificó correctamente el hash del instalador", "hash");
  await wait(500);
  send("Iniciando la instalación del paquete...", "install");
  await wait(1600);
  send("Instalado correctamente", "done");
}

// ───────── Idioma ─────────
// En la app real, el backend traduce el catálogo. En la demo se usan los mismos archivos
// (src-tauri/locales), cargados aparte para que no pesen en la app compilada.
const LOCALES = import.meta.glob("../../src-tauri/locales/*.json", { import: "default" }) as Record<string, () => Promise<any>>;
let L: any = {};

async function setLang(lang: string) {
  L = lang === "es" ? {} : ((await LOCALES[`../../src-tauri/locales/${lang}.json`]?.()) ?? {});
}

const POWER_KEYS: Record<string, string> = {
  "a1841308-3541-4fab-bc81-f71556f20b4a": "power_saver",
  "381b4222-f694-41f0-9685-ff5bb260df2e": "power_balanced",
  "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c": "power_high",
  ultimate: "power_ultimate",
};

function localized(o: Overview): Overview {
  const c = structuredClone(o);
  for (const cat of c.categories) cat.title = L.category?.[cat.id] ?? cat.title;
  for (const t of c.tweaks) {
    const x = t.id === "power.plan" ? (L.msg && { title: L.msg.power_title, description: L.msg.power_description }) : L.tweak?.[t.id];
    if (!x) continue;
    t.title = x.title;
    t.description = x.description;
    for (const ch of t.choices) ch.label = (t.id === "power.plan" ? L.msg?.[POWER_KEYS[ch.id]] : x.choices?.[ch.id]) ?? ch.label;
  }
  return c;
}

const MB = 1024 ** 2;
const cleanup: CleanView[] = [
  { id: "temp_user", bytes: 1.47 * GB, files: 3434, admin: false, per_user: true, warn: null, available: true },
  { id: "temp_system", bytes: 0, files: 0, admin: true, per_user: false, warn: null, available: false },
  { id: "crash", bytes: 0, files: 0, admin: true, per_user: true, warn: null, available: false },
  { id: "shaders", bytes: 1.4 * MB, files: 61, admin: false, per_user: true, warn: null, available: true },
  { id: "thumbnails", bytes: 5 * MB, files: 15, admin: false, per_user: true, warn: null, available: true },
  { id: "update_cache", bytes: 0, files: 0, admin: true, per_user: false, warn: null, available: false },
  { id: "spotify", bytes: 812 * MB, files: 2210, admin: false, per_user: true, warn: "close_app", available: true },
  { id: "prefetch", bytes: 0, files: 0, admin: true, per_user: false, warn: "slower_start", available: false },
  { id: "recycle_bin", bytes: 236 * MB, files: 41, admin: false, per_user: true, warn: "permanent", available: true },
];

export async function mock<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  await wait(120);
  const a = (args ?? {}) as any;
  switch (cmd) {
    case "app_version":
      return "0.3.1" as T;
    case "update_check":
      await wait(600);
      return { version: "0.4.0", current: "0.3.1", notes: "Servicios descartables como tweaks, limpieza de archivos en Disco y actualización desde la app." } as T;
    case "update_install": {
      const total = 2_400_000;
      for (let d = 0; d <= total; d += 240_000) {
        updateListeners.forEach((cb) => cb({ downloaded: d, total }));
        await wait(200);
      }
      throw "Demo: en el navegador no se instala nada.";
    }
    case "set_language":
      await setLang(a.lang);
      return null as T;
    case "get_overview":
      return localized(overview) as T;
    case "plan_changes":
      return a.changes.flatMap((d: any) => {
        const t = tweaks.find((t) => t.id === d.id)!;
        const to = (o: { on: string; off: string }) => (d.choice ? SEARCH_VALUES[d.choice] : d.applied ? o.on : o.off);
        return t.ops.map((o) => ({ tweak_id: t.id, key: o.key, name: o.name, from: o.current, to: to(o), noop: false }));
      }) as T;
    case "apply_changes": {
      for (const d of a.changes) {
        const t = tweaks.find((t) => t.id === d.id)!;
        if (t.requires_admin && !overview.system.elevated) continue;
        if (d.choice) {
          t.choice = d.choice;
          t.state = t.choices.find((c) => c.id === d.choice)?.default ? "default" : "applied";
          t.ops[0].current = SEARCH_VALUES[d.choice];
        } else t.state = d.applied ? "applied" : "default";
      }
      const results = a.changes.map((d: any) => {
        const t = tweaks.find((t) => t.id === d.id)!;
        const denied = t.requires_admin && !overview.system.elevated;
        return { id: d.id, ok: !denied, error: denied ? "Requiere ejecutar como administrador" : null };
      });
      const id = Date.now();
      history.unshift({ id, at: id / 1000, label: `${a.changes.length} tweaks`, changes: a.changes.map((d: any) => ({ tweak_id: d.id })), removed_apps: [], undone: false });
      return { entry_id: id, results, restart: "explorer", conflicts: [], warning: null } as T;
    }
    case "history":
      return structuredClone(history) as T;
    case "undo": {
      const e = history.find((h) => h.id === a.entryId)!;
      e.undone = true;
      return { entry_id: e.id, results: e.changes.map((c) => ({ id: c.tweak_id, ok: true, error: null })), restart: "none", conflicts: [], warning: null } as T;
    }
    case "winget_status":
      await wait(700);
      return {
        ...structuredClone(wingetApps),
        groups: wingetApps.groups.map((g) => ({ ...g, title: L.app_group?.[g.id] ?? g.title })),
        apps: wingetApps.apps.map((x) => ({ ...x, description: L.app?.[x.id] ?? x.description })),
      } as T;
    case "winget_icons":
      return {} as T;
    case "winget_check": {
      await wait(900);
      const sha = "3fe4ab99fa15629c652944309a2b2bc6e41fc933fc6b63f5df1ebcf3aea68263";
      const installer = { url: `https://example.com/${a.id}-setup.exe`, sha256: sha, version: "1.0.0" };
      return { installer, error: null } as T;
    }
    case "vt_open":
      return undefined as T;
    case "winget_action": {
      await simulateWinget(a.id, a.action);
      const app = wingetApps.apps.find((x) => x.id === a.id)!;
      if (a.action === "uninstall") app.installed = null;
      else app.installed = app.available ?? "1.0.0";
      app.available = null;
      return { id: a.id, ok: true, message: null } as T;
    }
    case "get_user":
      return { name: "Usuario de prueba", username: "demo", picture: null } as T;
    case "disk_drives":
      return [
        { letter: "C", label: "", fs: "NTFS", total: 237 * GB, free: 166 * GB, removable: false },
        { letter: "D", label: "Datos", fs: "NTFS", total: 931 * GB, free: 50 * GB, removable: false },
      ] as T;
    case "disk_scan":
      Object.assign(scanState, { running: true, done: false, started: Date.now() });
      setTimeout(() => Object.assign(scanState, { running: false, done: true }), 2500);
      return undefined as T;
    case "disk_cancel":
      Object.assign(scanState, { running: false, done: false });
      return undefined as T;
    case "disk_status": {
      const t = Math.min(1, (Date.now() - scanState.started) / 2500);
      return {
        running: scanState.running, done: scanState.done, cancelled: false, root: "C:\\",
        files: Math.round(160000 * t), dirs: Math.round(30000 * t), bytes: dsize(0) * t, denied: 3,
        current: "C:\\Users\\demo\\AppData\\Local\\StreamingApp\\cache", elapsed_ms: 1650, error: null,
      } as T;
    }
    case "disk_summary": {
      const hot = dirs.slice(1).filter((d) => {
        const s = dsize(d.id);
        return s > 1 * GB && !d.kids.some((k) => dsize(k) * 2 >= s);
      });
      return {
        root: "C:\\", size: dsize(0), files: dfiles(0), dirs: dirs.length, cloud: 1.4 * GB, denied_dirs: 3, elapsed_ms: 1650,
        hotspots: hot.map((d) => ({ id: d.id, path: dpath(d.id), size: dsize(d.id), files: dfiles(d.id) })).sort((x, y) => y.size - x.size),
      } as T;
    }
    case "disk_dir": {
      const d = dirs[a.id];
      const crumbs: [number, string][] = [];
      for (let x = d.id; x >= 0; x = dirs[x].parent) crumbs.unshift([x, dirs[x].name]);
      return {
        id: d.id, path: dpath(d.id), size: dsize(d.id), files: dfiles(d.id), own: d.own, own_files: d.files, cloud: 0, crumbs,
        children: d.kids
          .map((k) => ({ id: k, name: dirs[k].name, size: dsize(k), files: dfiles(k), cloud: 0, denied: false, link: false, has_children: dirs[k].kids.length > 0 }))
          .sort((x, y) => y.size - x.size),
        hidden: 0,
      } as T;
    }
    case "disk_open":
      return undefined as T;
    case "cleanup_scan":
      await wait(900);
      return structuredClone(cleanup) as T;
    case "cleanup_run": {
      await wait(1200);
      const res = (a.ids as string[]).map((id) => {
        const c = cleanup.find((x) => x.id === id)!;
        const freed = c.available ? c.bytes : 0;
        const skipped = id === "temp_user" ? 12 : 0;
        const out = { id: c.id, freed, deleted: c.available ? c.files - skipped : 0, skipped, error: c.available ? null : "Requiere ejecutar como administrador" };
        if (c.available) (c.bytes = 0), (c.files = skipped);
        return out;
      });
      return res as T;
    }
    case "list_bloat":
      return bloat.map((b) => ({ ...structuredClone(b), title: L.bloat?.[b.id]?.title ?? b.title, note: b.note && (L.bloat?.[b.id]?.note ?? b.note) })) as T;
    case "remove_bloat":
      return a.ids.map((id: string) => {
        const i = bloat.findIndex((b) => b.id === id);
        if (i >= 0) bloat.splice(i, 1);
        return { id, ok: true, removed: [id], error: null };
      }) as T;
  }
  throw new Error(`mock: comando desconocido ${cmd}`);
}
