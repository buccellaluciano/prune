import { diskApi, type DirView, type DiskSummary, type Drive, type ScanStatus } from "./api";
import { store } from "./store.svelte";

/** Estado de la vista Disco. Vive fuera del componente: el análisis sigue al cambiar de vista. */
class DiskStore {
  drives = $state<Drive[] | null>(null);
  status = $state<ScanStatus | null>(null);
  summary = $state<DiskSummary | null>(null);
  dir = $state<DirView | null>(null);
  tab = $state<"hotspots" | "tree">("hotspots");
  #timer: ReturnType<typeof setInterval> | null = null;

  async init() {
    if (!this.drives) this.drives = await diskApi.drives().catch(() => []);
    const s = await diskApi.status();
    this.status = s;
    if (s.running) this.#poll();
    else if (s.done && !this.summary) await this.#loadResult();
  }

  async scan(letter: string) {
    try {
      this.summary = null;
      this.dir = null;
      await diskApi.scan(letter);
      this.status = await diskApi.status();
      this.#poll();
    } catch (e) {
      store.toast("error", String(e));
    }
  }

  async cancel() {
    await diskApi.cancel();
  }

  #poll() {
    if (this.#timer) return;
    this.#timer = setInterval(async () => {
      const s = await diskApi.status();
      this.status = s;
      if (!s.running) {
        clearInterval(this.#timer!);
        this.#timer = null;
        if (s.done) await this.#loadResult();
        else if (s.cancelled) store.toast("info", "Análisis cancelado.");
        this.drives = await diskApi.drives().catch(() => this.drives);
      }
    }, 250);
  }

  async #loadResult() {
    this.summary = await diskApi.summary();
    this.dir = await diskApi.dir(0);
    this.tab = "hotspots";
  }

  async go(id: number) {
    try {
      this.dir = await diskApi.dir(id);
      this.tab = "tree";
    } catch (e) {
      store.toast("error", String(e));
    }
  }

  async open(id: number) {
    await diskApi.open(id).catch((e) => store.toast("error", String(e)));
  }
}

export const disk = new DiskStore();
