import { isTauri } from "./api";

export type ThemeId = "modern" | "aero";

export const THEMES: { id: ThemeId; title: string; hint: string }[] = [
  { id: "modern", title: "Moderno", hint: "Oscuro, el tema original" },
  { id: "aero", title: "Aero 7", hint: "Vidrio y brillos al estilo Windows 7" },
];

function stored(): ThemeId {
  try {
    const v = localStorage.getItem("theme");
    if (v === "modern" || v === "aero") return v;
  } catch {}
  return "modern";
}

/**
 * Tema de la app. Además del CSS, el tema Aero cambia la ventana nativa:
 * sin marco de Windows (dibujamos el nuestro) y con efecto acrílico de DWM detrás.
 */
class ThemeStore {
  current = $state<ThemeId>(stored());
  maximized = $state(false);
  #watching = false;

  constructor() {
    if (typeof document !== "undefined") this.#applyDom();
  }

  #applyDom() {
    const el = document.documentElement;
    el.dataset.theme = this.current;
    el.dataset.native = String(isTauri);
  }

  async apply() {
    this.#applyDom();
    try {
      localStorage.setItem("theme", this.current);
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
      if (this.current === "aero") {
        await w.setTheme("light");
        await w.setDecorations(false);
        await w.setEffects({ effects: [Effect.Acrylic, Effect.Blur] });
      } else {
        await w.clearEffects();
        await w.setDecorations(true);
        await w.setTheme("dark");
      }
    } catch (e) {
      console.error("No se pudo aplicar el efecto de ventana", e);
    }
  }

  set(id: ThemeId) {
    if (id === this.current) return;
    this.current = id;
    this.apply();
  }

  async window(action: "minimize" | "toggleMaximize" | "close") {
    if (!isTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow()[action]();
  }
}

export const theme = new ThemeStore();
