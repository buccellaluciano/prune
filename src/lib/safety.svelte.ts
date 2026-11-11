// Red de seguridad: puntos de restauración y cambios que Windows volvió atrás.
import { api, driftApi, profileApi, restoreApi, type Drift, type ImportPreview, type RestoreStatus } from "./api";
import { store } from "./store.svelte";
import { t } from "./i18n.svelte";

const DAY = 24 * 3600;

class RestoreStore {
  status = $state<RestoreStatus | null>(null);
  creating = $state(false);

  /** Se puede crear uno ahora (admin, activado y sin directiva que lo apague). */
  available = $derived(!!this.status && this.status.elevated && this.status.enabled && !this.status.policy);
  /** Hay uno de las últimas 24 horas (Windows no crea otro). */
  recent = $derived(!!this.status?.last && Date.now() / 1000 - this.status.last.at < DAY);

  async load() {
    try {
      this.status = await restoreApi.status();
    } catch {
      this.status = null;
    }
  }

  /** Crea un punto (o usa el de hoy). false si falló: quien llama no sigue. */
  async create(): Promise<boolean> {
    this.creating = true;
    try {
      const r = await restoreApi.create();
      store.toast(r.reused ? "info" : "ok", t(r.reused ? "restore.reused" : "restore.created"));
      await this.load();
      return true;
    } catch (e) {
      store.toast("error", String(e));
      return false;
    } finally {
      this.creating = false;
    }
  }
}

export const restore = new RestoreStore();

/** Marca tweaks ("id" o "id:opcion") y abre el plan. Si ninguno se pudo marcar (por ejemplo,
 *  piden admin), avisa en vez de abrir un plan vacío. */
export function stageAndReview(ids: string[]) {
  store.stageMany(ids, true);
  store.view = "tweaks";
  if (store.pendingCount) store.planOpen = true;
  else store.toast("info", t("config.nothing_staged"));
}

const DISMISSED_KEY = "driftDismissed";

function readDismissed(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(DISMISSED_KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x === "string") : [];
  } catch {
    return [];
  }
}

const driftKey = (d: Drift) => `${d.kind}:${d.id}@${d.entry_id}`;

class DriftStore {
  items = $state<Drift[]>([]);
  open = $state(false);
  dismissed = $state<string[]>(readDismissed());

  /** Lo que todavía no se ignoró. Si se vuelve a aplicar, la entrada cambia y se avisa de nuevo. */
  visible = $derived(this.items.filter((d) => !this.dismissed.includes(driftKey(d))));

  async check() {
    try {
      this.items = await driftApi.check();
    } catch {
      this.items = [];
    }
  }

  ignore(list: Drift[]) {
    const keys = new Set([...this.dismissed, ...list.map(driftKey)]);
    // Solo se guardan las que siguen existiendo, para que no crezca para siempre.
    const live = new Set(this.items.map(driftKey));
    this.dismissed = [...keys].filter((k) => live.has(k));
    try {
      localStorage.setItem(DISMISSED_KEY, JSON.stringify(this.dismissed));
    } catch {
      /* sin almacenamiento: se vuelve a avisar la próxima vez */
    }
  }

  /** Tweaks al plan (para revisarlos) y apps quitadas de nuevo. */
  async reapply(list: Drift[]) {
    const tweaks = list.filter((d) => d.kind === "tweak").map((d) => (d.choice ? `${d.id}:${d.choice}` : d.id));
    const bloat = list.filter((d) => d.kind === "bloat").map((d) => d.id);
    if (bloat.length) {
      // Quitar apps no se deshace desde P-Rune: punto de restauración antes, si no hay uno de hoy.
      await restore.load();
      if (restore.available && !restore.recent && !(await restore.create())) return;
      try {
        const res = await api.removeBloat(bloat, false);
        for (const r of res.filter((r) => !r.ok)) store.toast("error", `${r.id}: ${r.error}`);
      } catch (e) {
        store.toast("error", String(e));
      }
    }
    this.open = false;
    if (tweaks.length) stageAndReview(tweaks);
    await this.check();
  }
}

export const drift = new DriftStore();

class ProfileStore {
  preview = $state<ImportPreview | null>(null);
  busy = $state<"export" | "import" | null>(null);

  async export() {
    this.busy = "export";
    try {
      const file = await profileApi.export();
      if (file) store.toast("ok", t("config.exported", { file }));
    } catch (e) {
      store.toast("error", String(e));
    } finally {
      this.busy = null;
    }
  }

  async import() {
    this.busy = "import";
    try {
      this.preview = await profileApi.import();
    } catch (e) {
      store.toast("error", String(e));
    } finally {
      this.busy = null;
    }
  }
}

export const profile = new ProfileStore();
