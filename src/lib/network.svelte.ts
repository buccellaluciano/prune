import { networkApi, type AdapterDns, type DnsTestResult, type NetworkStatus } from "./api";
import { store } from "./store.svelte";
import { t } from "./i18n.svelte";

/**
 * Estado de la sección Red y del cambio de DNS esperando confirmación.
 *
 * La vuelta atrás la maneja el backend (si la interfaz se cuelga, igual vuelve); acá solo se
 * muestra la cuenta regresiva y se avisa cuando pasó.
 */
class Network {
  status = $state<NetworkStatus | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  busy = $state(false);
  /** Resultado de la última prueba, por servidor. */
  tests = $state<DnsTestResult[] | null>(null);
  /** Cambio aplicado que falta confirmar. */
  pending = $state<{ entryId: number; deadline: number } | null>(null);
  now = $state(Date.now());
  #tick: ReturnType<typeof setInterval> | null = null;
  #listening = false;

  secondsLeft = $derived(this.pending ? Math.max(0, Math.ceil((this.pending.deadline - this.now) / 1000)) : 0);

  async load() {
    this.listen();
    this.loading = true;
    try {
      this.status = await networkApi.status();
      this.error = null;
      const p = this.status.pending;
      if (p && !this.pending) this.#arm(p.entry_id, p.seconds_left);
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loading = false;
    }
  }

  async listen() {
    if (this.#listening) return;
    this.#listening = true;
    await networkApi.onReverted((r) => {
      this.#disarm();
      if (r.Err) store.toast("error", r.Err);
      else store.toast("info", t("dns.reverted_auto"));
      this.load();
    });
  }

  #arm(entryId: number, seconds: number) {
    this.pending = { entryId, deadline: Date.now() + seconds * 1000 };
    this.now = Date.now();
    this.#tick ??= setInterval(() => (this.now = Date.now()), 250);
  }

  #disarm() {
    this.pending = null;
    if (this.#tick) clearInterval(this.#tick);
    this.#tick = null;
  }

  async test(list: AdapterDns[]): Promise<DnsTestResult[] | null> {
    this.busy = true;
    try {
      this.tests = await networkApi.test(list);
      return this.tests;
    } catch (e) {
      store.toast("error", String(e));
      return null;
    } finally {
      this.busy = false;
    }
  }

  async apply(list: AdapterDns[]): Promise<boolean> {
    this.busy = true;
    try {
      const r = await networkApi.apply(list);
      this.#arm(r.entry_id, r.seconds);
      this.tests = null;
      await this.load();
      return true;
    } catch (e) {
      store.toast("error", String(e));
      return false;
    } finally {
      this.busy = false;
    }
  }

  async confirm() {
    if (!this.pending) return;
    const id = this.pending.entryId;
    this.#disarm();
    const kept = await networkApi.confirm(id).catch(() => false);
    store.toast(kept ? "ok" : "info", kept ? t("dns.kept") : t("dns.reverted_auto"));
    await this.load();
  }

  async revert() {
    if (!this.pending) return;
    const id = this.pending.entryId;
    this.#disarm();
    try {
      const r = await networkApi.revert(id);
      const failed = r.results.find((x) => !x.ok);
      store.toast(failed ? "error" : "ok", failed?.error ?? t("dns.reverted"));
    } catch (e) {
      store.toast("error", String(e));
    }
    await this.load();
  }
}

export const network = new Network();
