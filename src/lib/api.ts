import { invoke } from "@tauri-apps/api/core";
import { mock } from "./mock";

export type Risk = "safe" | "moderate" | "advanced";
export type Restart = "none" | "explorer" | "logoff" | "reboot";
export type TweakState = "applied" | "default" | "partial" | "unsupported" | "error";

export interface SystemInfo {
  product: string;
  display_version: string;
  build: number;
  ubr: number;
  edition: string;
  elevated: boolean;
}

export interface Category {
  id: string;
  title: string;
  icon: string;
}

export interface OpView {
  key: string;
  name: string;
  current: string;
  on: string;
  off: string;
}

export interface Tweak {
  id: string;
  category: string;
  title: string;
  description: string;
  risk: Risk;
  restart: Restart;
  tags: string[];
  requires_admin: boolean;
  state: TweakState;
  ops: OpView[];
  /** Vacío para tweaks de switch; con opciones se muestra un selector. */
  choices: ChoiceView[];
  /** Opción vigente según el registro (null si no coincide con ninguna). */
  choice: string | null;
}

export interface ChoiceView {
  id: string;
  label: string;
  default: boolean;
}

export interface Overview {
  system: SystemInfo;
  categories: Category[];
  tweaks: Tweak[];
}

export interface Desired {
  id: string;
  applied: boolean;
  choice?: string;
}

export interface PlannedChange {
  tweak_id: string;
  key: string;
  name: string;
  from: string;
  to: string;
  noop: boolean;
}

export interface ItemResult {
  id: string;
  ok: boolean;
  error: string | null;
}

export interface ApplyReport {
  entry_id: number | null;
  results: ItemResult[];
  restart: Restart;
}

export interface JournalEntry {
  id: number;
  at: number;
  label: string;
  changes: { tweak_id: string }[];
  removed_apps: string[];
  undone: boolean;
}

export interface BloatApp {
  id: string;
  title: string;
  tier: Risk;
  note: string | null;
  packages: { full_name: string; family_name: string; version: string }[];
}

export interface RemoveResult {
  id: string;
  ok: boolean;
  removed: string[];
  error: string | null;
}

/** Fuera de Tauri (vite en el navegador) usamos datos de ejemplo, sin tocar nada. */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri ? invoke<T>(cmd, args) : mock<T>(cmd, args);
}

export const api = {
  overview: () => call<Overview>("get_overview"),
  plan: (changes: Desired[]) => call<PlannedChange[]>("plan_changes", { changes }),
  apply: (changes: Desired[]) => call<ApplyReport>("apply_changes", { changes }),
  history: () => call<JournalEntry[]>("history"),
  undo: (entryId: number) => call<ApplyReport>("undo", { entryId }),
  listBloat: () => call<BloatApp[]>("list_bloat"),
  removeBloat: (ids: string[], deprovision: boolean) =>
    call<RemoveResult[]>("remove_bloat", { ids, deprovision }),
};
