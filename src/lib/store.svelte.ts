import { api, userApi, type ApplyReport, type Desired, type Overview, type Restart, type Tweak, type UserInfo } from "./api";
import { t, tn, type Key } from "./i18n.svelte";

/** Pendiente: true/false para switches, id de opción para selectores. */
export type Pending = boolean | string;

export type View = "tweaks" | "apps" | "disk" | "bloat" | "history" | "versions" | "more" | "network";

export interface Toast {
  id: number;
  kind: "ok" | "error" | "info";
  text: string;
}

const RESTART_MSG: Record<Restart, Key | null> = {
  none: null,
  explorer: "toast.restart_explorer",
  logoff: "toast.restart_logoff",
  reboot: "toast.restart_reboot",
};

class AppStore {
  overview = $state<Overview | null>(null);
  user = $state<UserInfo | null>(null);
  /** true cuando terminó de leer la cuenta, haya foto o no (la bienvenida lo espera). */
  userLoaded = $state(false);
  loading = $state(true);
  loadError = $state<string | null>(null);
  view = $state<View>("tweaks");
  category = $state<string | null>(null);
  query = $state("");
  /** Cambios pendientes: id del tweak -> estado deseado. */
  pending = $state<Record<string, Pending>>({});
  busy = $state(false);
  toasts = $state<Toast[]>([]);
  paletteOpen = $state(false);
  planOpen = $state(false);

  tweaks = $derived(this.overview?.tweaks ?? []);
  pendingCount = $derived(Object.keys(this.pending).length);

  filtered = $derived.by(() => {
    const q = normalize(this.query);
    return this.tweaks.filter((t) => {
      if (this.category && !q && t.category !== this.category) return false;
      if (!q) return true;
      return normalize(`${t.title} ${t.description} ${t.tags.join(" ")} ${t.id}`).includes(q);
    });
  });

  async loadUser() {
    try {
      this.user = await userApi.get();
    } catch {
      this.user = null;
    } finally {
      this.userLoaded = true;
    }
  }

  async load() {
    this.loading = true;
    try {
      this.overview = await api.overview();
      this.loadError = null;
    } catch (e) {
      this.loadError = String(e);
    } finally {
      this.loading = false;
    }
  }

  isOn(t: Tweak): boolean {
    return t.state === "applied";
  }

  /** Estado que muestra el switch: el pendiente si existe, si no el real. */
  shown(t: Tweak): boolean {
    const p = this.pending[t.id];
    return typeof p === "boolean" ? p : this.isOn(t);
  }

  /** Opción que muestra el selector: la pendiente si existe, si no la real. */
  shownChoice(t: Tweak): string | null {
    const p = this.pending[t.id];
    return typeof p === "string" ? p : t.choice;
  }

  choiceLabel(tw: Tweak, id: string | null): string {
    return tw.choices.find((c) => c.id === id)?.label ?? t("tweak.unknown_value");
  }

  choose(t: Tweak, choice: string) {
    if (this.disabledReason(t)) return;
    const copy = { ...this.pending };
    if (choice === t.choice) delete copy[t.id];
    else copy[t.id] = choice;
    this.pending = copy;
  }

  /** Lo que se manda al backend. */
  desired(): Desired[] {
    return Object.entries(this.pending).map(([id, p]) =>
      typeof p === "string" ? { id, applied: true, choice: p } : { id, applied: p },
    );
  }

  disabledReason(tw: Tweak): string | null {
    if (tw.state === "unsupported") return t("disabled.unsupported");
    if (tw.ucpd && this.overview?.system.ucpd_active) return t("disabled.ucpd");
    if (tw.requires_admin && !this.overview?.system.elevated) return t("disabled.admin");
    const foreign = this.overview?.system.foreign_user;
    if (tw.per_user && foreign) return t("disabled.foreign", { user: foreign });
    return null;
  }

  toggle(t: Tweak) {
    if (this.disabledReason(t) || t.choices.length) return;
    const next = !this.shown(t);
    const copy = { ...this.pending };
    // Si vuelve al estado real, deja de estar pendiente. Parcial siempre queda pendiente.
    if (next === this.isOn(t) && t.state !== "partial") delete copy[t.id];
    else copy[t.id] = next;
    this.pending = copy;
  }

  /** Marca varios tweaks. Para selectores se usa "id:opcion" (p. ej. "search.taskbar_mode:icon"). */
  stageMany(ids: string[], applied: boolean) {
    const wanted = new Map(ids.map((s) => s.split(":") as [string, string | undefined]));
    const copy = { ...this.pending };
    for (const t of this.tweaks) {
      if (!wanted.has(t.id) || this.disabledReason(t)) continue;
      if (t.choices.length) {
        const c = wanted.get(t.id);
        if (!c || !t.choices.some((x) => x.id === c)) continue;
        if (c === t.choice) delete copy[t.id];
        else copy[t.id] = c;
      } else if (applied === this.isOn(t) && t.state !== "partial") delete copy[t.id];
      else copy[t.id] = applied;
    }
    this.pending = copy;
  }

  clearPending() {
    this.pending = {};
  }

  async applyPending() {
    const changes = this.desired();
    if (!changes.length) return;
    this.busy = true;
    try {
      const report = await api.apply(changes);
      this.report(report, "apply");
      // Lo que falló queda pendiente para corregirlo o reintentarlo; lo aplicado se limpia.
      const failed = new Set(report.results.filter((r) => !r.ok).map((r) => r.id));
      this.pending = Object.fromEntries(Object.entries(this.pending).filter(([id]) => failed.has(id)));
      this.planOpen = false;
      await this.load();
    } catch (e) {
      this.toast("error", String(e));
    } finally {
      this.busy = false;
    }
  }

  report(r: ApplyReport, kind: "apply" | "undo") {
    if (r.warning) this.toast("error", r.warning);
    const ok = r.results.filter((x) => x.ok).length;
    const failed = r.results.filter((x) => !x.ok);
    if (ok) this.toast("ok", tn(kind === "apply" ? "toast.applied" : "toast.undone", ok));
    for (const f of failed) {
      const tw = this.tweaks.find((x) => x.id === f.id);
      this.toast("error", `${tw?.title ?? f.id}: ${f.error}`);
    }
    const msg = RESTART_MSG[r.restart];
    if (msg && ok) this.toast("info", t(msg));
  }

  toast(kind: Toast["kind"], text: string) {
    const id = Math.random();
    this.toasts = [...this.toasts, { id, kind, text }];
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), kind === "error" ? 9000 : 5000);
  }
}

export function normalize(s: string) {
  return s.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase().trim();
}

export const store = new AppStore();

/** Perfiles: conjuntos de tweaks que se marcan como pendientes de una. `title` y `hint` son claves de i18n. */
export const PROFILES: { id: string; title: Key; hint: Key; ids: string[] }[] = [
  {
    id: "privacy",
    title: "profile.privacy",
    hint: "profile.privacy_hint",
    ids: [
      "privacy.advertising_id", "privacy.tailored_experiences", "privacy.online_speech",
      "privacy.inking_typing", "privacy.typing_insights", "privacy.track_app_launches",
      "privacy.feedback_frequency", "privacy.activity_history", "ai.recall", "services.diagtrack",
      "start.suggestions", "start.silent_installs", "start.tips", "start.settings_suggestions",
      "start.lockscreen_tips", "start.sync_provider_ads", "start.recommendations",
    ],
  },
  {
    id: "clean",
    title: "profile.clean",
    hint: "profile.clean_hint",
    ids: [
      "taskbar.hide_task_view", "taskbar.disable_widgets", "search.taskbar_mode:icon", "search.highlights",
      "start.recommendations", "start.account_notifications", "explorer.show_extensions",
      "explorer.open_this_pc", "start.finish_setup_nag", "start.welcome_experience",
    ],
  },
  {
    id: "gaming",
    title: "profile.gaming",
    hint: "profile.gaming_hint",
    ids: [
      "gaming.game_dvr", "gaming.mouse_acceleration", "gaming.sticky_keys_prompt",
      "taskbar.no_animations", "start.tips", "start.suggested_notifications",
      // Alto rendimiento (si el equipo tiene ese plan).
      "power.plan:8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c",
    ],
  },
  {
    id: "lite",
    title: "profile.lite",
    hint: "profile.lite_hint",
    ids: [
      "appearance.performance_visuals", "appearance.no_transparency", "taskbar.no_animations",
      "explorer.icons_not_thumbnails", "appearance.faster_menus",
    ],
  },
];
