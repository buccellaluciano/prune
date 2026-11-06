import { wingetApi, type AppsOverview, type WingetAction } from "./api";
import { store } from "./store.svelte";

interface Job {
  id: string;
  action: WingetAction;
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
  running = $state<(Job & { line: string }) | null>(null);

  upgradable = $derived(this.overview?.apps.filter((a) => a.installed && a.available) ?? []);

  #listening = false;

  async load() {
    if (this.loading) return;
    this.loading = true;
    if (!this.#listening) {
      this.#listening = true;
      await wingetApi.onProgress((p) => {
        if (this.running?.id === p.id) this.running.line = p.line;
      });
    }
    try {
      this.overview = await wingetApi.status();
    } catch (e) {
      store.toast("error", String(e));
    } finally {
      this.loading = false;
    }
  }

  jobFor(id: string): "running" | "queued" | null {
    if (this.running?.id === id) return "running";
    return this.queue.some((j) => j.id === id) ? "queued" : null;
  }

  enqueue(id: string, action: WingetAction) {
    if (this.jobFor(id)) return;
    this.queue = [...this.queue, { id, action }];
    if (!this.running) this.#drain();
  }

  cancelQueued(id: string) {
    this.queue = this.queue.filter((j) => j.id !== id);
  }

  async #drain() {
    let changed = false;
    while (this.queue.length) {
      const [job, ...rest] = this.queue;
      this.queue = rest;
      this.running = { ...job, line: "" };
      const title = this.overview?.apps.find((a) => a.id === job.id)?.title ?? job.id;
      try {
        const r = await wingetApi.act(job.id, job.action);
        if (r.ok) {
          changed = true;
          store.toast("ok", r.message ? `${title}: ${r.message}` : `${title} ${VERB[job.action][1]}.`);
        } else store.toast("error", `${title}: ${r.message}`);
      } catch (e) {
        store.toast("error", `${title}: ${e}`);
      }
    }
    this.running = null;
    if (changed) await this.load();
  }

  verb(action: WingetAction) {
    return VERB[action][0];
  }
}

export const apps = new AppsStore();
