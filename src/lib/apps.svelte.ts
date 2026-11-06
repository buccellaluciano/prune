import { wingetApi, type AppsOverview, type SecurityCheck, type WingetAction, type WingetPhase } from "./api";

const REVIEW_KEY = "reviewInstalls";
function storedReview(): boolean {
  try {
    return localStorage.getItem(REVIEW_KEY) !== "false";
  } catch {
    return true;
  }
}
import { store } from "./store.svelte";

interface Job {
  id: string;
  action: WingetAction;
}

/** Etapas que muestra la pantalla de instalación, en orden. */
export type Step = "verify" | WingetPhase;

export const STEPS: Record<WingetAction, Step[]> = {
  install: ["verify", "download", "hash", "install", "done"],
  upgrade: ["verify", "download", "hash", "install", "done"],
  uninstall: ["uninstall", "done"],
};

/** Operación en curso (o la última, hasta que se cierra la pantalla). */
export interface Running extends Job {
  step: Step;
  fraction: number | null;
  /** Texto de la línea de avance (por ejemplo "12.0 MB / 48.0 MB"). */
  detail: string;
  log: string[];
  check: SecurityCheck | null;
  /** Esperando que la persona decida si sigue (VirusTotal marcó algo o falló). */
  review: string | null;
  result: { ok: boolean; message: string } | null;
}

const VERB: Record<WingetAction, [string, string]> = {
  install: ["Instalando", "instalado"],
  upgrade: ["Actualizando", "actualizado"],
  uninstall: ["Desinstalando", "desinstalado"],
};

/** Estado de la vista Programas. Vive fuera del componente para que la cola siga al cambiar de vista. */
class AppsStore {
  overview = $state<AppsOverview | null>(null);
  loading = $state(false);
  queue = $state<Job[]>([]);
  running = $state<Running | null>(null);
  /** La pantalla de instalación está a la vista (se puede ocultar mientras sigue). */
  sheetOpen = $state(false);
  icons = $state<Record<string, string>>({});
  /** Frenar cada instalación en la revisión (hash y URL) hasta que la persona confirme. */
  reviewFirst = $state(storedReview());

  upgradable = $derived(this.overview?.apps.filter((a) => a.installed && a.available) ?? []);
  busy = $derived(!!this.running && !this.running.result);

  #listening = false;
  #decide: ((go: boolean) => void) | null = null;

  async load() {
    if (this.loading) return;
    this.loading = true;
    if (!this.#listening) {
      this.#listening = true;
      await wingetApi.onProgress((p) => {
        const r = this.running;
        if (!r || r.id !== p.id || r.result) return;
        r.log = [...r.log.slice(-199), p.line];
        if (p.phase) r.step = p.phase;
        if (p.fraction !== null) {
          r.fraction = p.fraction;
          r.detail = p.line.replace(/[█▒]/g, "").trim();
        }
      });
      this.loadIcons();
    }
    try {
      this.overview = await wingetApi.status();
    } catch (e) {
      store.toast("error", String(e));
    } finally {
      this.loading = false;
    }
  }

  /** Primero los que ya están en caché; después busca los que falten. */
  async loadIcons() {
    try {
      this.icons = await wingetApi.icons(false);
      const all = await wingetApi.icons(true);
      if (Object.keys(all).length !== Object.keys(this.icons).length) this.icons = all;
    } catch {}
  }

  title(id: string) {
    return this.overview?.apps.find((a) => a.id === id)?.title ?? id;
  }

  jobFor(id: string): "running" | "queued" | null {
    if (this.busy && this.running?.id === id) return "running";
    return this.queue.some((j) => j.id === id) ? "queued" : null;
  }

  enqueue(id: string, action: WingetAction) {
    if (this.jobFor(id)) return;
    this.queue = [...this.queue, { id, action }];
    if (!this.busy) this.#drain();
  }

  cancelQueued(id: string) {
    this.queue = this.queue.filter((j) => j.id !== id);
  }

  /** Respuesta a la pantalla de revisión: seguir con la instalación o cancelarla. */
  decide(go: boolean) {
    this.#decide?.(go);
    this.#decide = null;
  }

  closeSheet() {
    this.sheetOpen = false;
    if (this.running?.result) this.running = null;
  }

  setReviewFirst(on: boolean) {
    this.reviewFirst = on;
    try {
      localStorage.setItem(REVIEW_KEY, String(on));
    } catch {}
  }

  /** Revisión previa: trae URL y SHA-256 del instalador. Si está activada (o no se pudo
   *  leer el manifiesto), espera la confirmación. Devuelve false si se canceló. */
  async #verify(r: Running): Promise<boolean> {
    r.step = "verify";
    const check = await wingetApi.check(r.id).catch((e) => ({ installer: null, error: String(e) }) as SecurityCheck);
    r.check = check;
    let reason: string | null = null;
    if (check.error || !check.installer?.sha256) reason = `No se pudo leer el instalador del manifiesto: ${check.error ?? "sin hash"}`;
    else if (this.reviewFirst) reason = "review";
    if (!reason) return true;
    r.review = reason;
    this.sheetOpen = true;
    const go = await new Promise<boolean>((res) => (this.#decide = res));
    r.review = null;
    return go;
  }

  async #drain() {
    let changed = false;
    while (this.queue.length) {
      const [job, ...rest] = this.queue;
      this.queue = rest;
      const r: Running = {
        ...job, step: STEPS[job.action][0], fraction: null, detail: "", log: [], check: null, review: null, result: null,
      };
      this.running = r;
      // Se muestra la pantalla al instalar o actualizar; las desinstalaciones van en la fila.
      if (job.action !== "uninstall") this.sheetOpen = true;
      const title = this.title(job.id);
      try {
        if (job.action !== "uninstall" && !(await this.#verify(this.running!))) {
          this.running!.result = { ok: false, message: "Instalación cancelada." };
          continue;
        }
        const res = await wingetApi.act(job.id, job.action);
        const msg = res.message ?? `${title} ${VERB[job.action][1]}.`;
        this.running!.result = { ok: res.ok, message: res.ok ? msg : (res.message ?? "Error") };
        if (res.ok) {
          this.running!.step = "done";
          this.running!.fraction = null;
          changed = true;
          if (!this.sheetOpen) store.toast("ok", res.message ? `${title}: ${res.message}` : msg);
        } else if (!this.sheetOpen) store.toast("error", `${title}: ${res.message}`);
      } catch (e) {
        this.running!.result = { ok: false, message: String(e) };
        if (!this.sheetOpen) store.toast("error", `${title}: ${e}`);
      }
      // Con más trabajos en cola, se muestra el resultado un momento y se sigue.
      if (this.queue.length) await new Promise((res) => setTimeout(res, 1200));
    }
    if (!this.sheetOpen) this.running = null;
    if (changed) await this.load();
  }

  verb(action: WingetAction) {
    return VERB[action][0];
  }
}

export const apps = new AppsStore();
