import { updateApi, type UpdateInfo } from "./api";
import { store } from "./store.svelte";
import { t } from "./i18n.svelte";

/**
 * Actualizaciones de P-Rune. Al abrir se consulta en silencio (un pedido a GitHub; si falla,
 * no se muestra nada) y también se puede buscar a mano. Instalar siempre lo decide la persona.
 */
class Updates {
  info = $state<UpdateInfo | null>(null);
  checking = $state(false);
  /** Aviso cerrado con "Más tarde": vuelve a aparecer en la próxima apertura. */
  dismissed = $state(false);
  installing = $state(false);
  downloaded = $state(0);
  total = $state<number | null>(null);
  /** Esperando que la persona confirme su identidad en el diálogo de Windows. */
  verifying = $state(false);
  /** Descarga terminada y firma verificada: se está lanzando el instalador. */
  launching = $state(false);
  error = $state<string | null>(null);

  async check(silent: boolean) {
    if (this.checking || this.installing) return;
    this.checking = true;
    try {
      this.info = await updateApi.check();
      this.dismissed = false;
      if (!silent && !this.info) store.toast("info", t("update.latest"));
    } catch (e) {
      if (!silent) store.toast("error", String(e));
    } finally {
      this.checking = false;
    }
  }

  async install() {
    if (!this.info || this.installing) return;
    this.installing = true;
    this.verifying = true;
    this.launching = false;
    this.error = null;
    this.downloaded = 0;
    this.total = null;
    const stop = await updateApi.onProgress((p) => {
      this.verifying = false;
      this.downloaded = p.downloaded;
      this.total = p.total;
      if (p.total && p.downloaded >= p.total) this.launching = true;
    });
    try {
      // Si sale bien, la app se cierra y el instalador la vuelve a abrir.
      await updateApi.install();
    } catch (e) {
      this.error = String(e);
    } finally {
      this.verifying = false;
      stop();
    }
  }

  close() {
    this.installing = false;
    this.error = null;
  }
}

export const updates = new Updates();
