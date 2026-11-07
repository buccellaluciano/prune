import { isTauri } from "./api";
import type { Key } from "./i18n.svelte";

export type ThemeId = "modern" | "aero";

/** `title` y `hint` son claves de i18n. */
export const THEMES: { id: ThemeId; title: Key; hint: Key }[] = [
  { id: "modern", title: "theme.modern", hint: "theme.modern_hint" },
  { id: "aero", title: "theme.aero", hint: "theme.aero_hint" },
];

function stored(): ThemeId {
  try {
    const v = localStorage.getItem("theme");
    if (v === "modern" || v === "aero") return v;
  } catch {}
  return "modern";
}

function storedGlass(): boolean {
  try {
    return localStorage.getItem("glass") === "true";
  } catch {
    return false;
  }
}

/**
 * Tema de la app y apariencia de la ventana nativa.
 * - Aero: sin marco de Windows (dibujamos el nuestro).
 * - `glass` (opcional, apagado por defecto): efecto acrílico de DWM detrás de la ventana,
 *   el desenfoque real del escritorio. Oscuro en Moderno, claro en Aero.
 */
class ThemeStore {
  current = $state<ThemeId>(stored());
  glass = $state(storedGlass());
  maximized = $state(false);
  #watching = false;

  constructor() {
    if (typeof document !== "undefined") this.#applyDom();
  }

  #applyDom() {
    const el = document.documentElement;
    el.dataset.theme = this.current;
    el.dataset.native = String(isTauri);
    el.dataset.glass = String(this.glass && isTauri);
  }

  async apply() {
    this.#applyDom();
    try {
      localStorage.setItem("theme", this.current);
      localStorage.setItem("glass", String(this.glass));
    } catch {}
    if (!isTauri) return;
    const { getCurrentWindow, Effect } = await import("@tauri-apps/api/window");
    const w = getCurrentWindow();
    if (!this.#watching) {
      this.#watching = true;
      this.maximized = await w.isMaximized();
      await w.onResized(async () => (this.maximized = await w.isMaximized()));
    }
    try {
      const aero = this.current === "aero";
      // El tono del acrílico sigue al tema de la ventana: claro en Aero, oscuro en Moderno.
      await w.setTheme(aero ? "light" : "dark");
      // Sin marco de Windows en Aero, y en Moderno con vidrio (la barra nativa no es transparente).
      await w.setDecorations(!aero && !this.glass);
      if (this.glass) await w.setEffects({ effects: [Effect.Acrylic, Effect.Blur] });
      else await w.clearEffects();
    } catch (e) {
      console.error("No se pudo aplicar el efecto de ventana", e);
    }
  }

  setGlass(on: boolean) {
    if (on === this.glass) return;
    this.glass = on;
    this.apply();
  }

  set(id: ThemeId) {
    if (id === this.current) return;
    this.current = id;
    this.apply();
    // Solo al elegirlo a mano (no al abrir la app con Aero ya activo): el "tada" de Windows.
    if (id === "aero" && isTauri) {
      import("@tauri-apps/api/core").then(({ invoke }) => invoke("play_theme_sound")).catch(() => {});
    }
  }

  async window(action: "minimize" | "toggleMaximize" | "close") {
    if (!isTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow()[action]();
  }
}

export const theme = new ThemeStore();
