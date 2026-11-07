/**
 * Idiomas de la interfaz.
 *
 * Los textos de la interfaz viven en `locales/<idioma>.ts` (el español es la fuente y define
 * las claves; TypeScript exige que cada idioma las tenga todas). Los textos del catálogo y
 * los mensajes del backend los traduce Rust (`src-tauri/locales`): por eso, al cambiar de
 * idioma, se le avisa al backend y se vuelve a pedir lo que trae textos del catálogo.
 *
 * Plurales: las claves terminan en `.one`, `.other` (y `.few`/`.many` donde el idioma los
 * usa, como el ruso); `tn()` elige con `Intl.PluralRules`.
 */
import { i18nApi } from "./api";
import { es } from "./locales/es";
import { en } from "./locales/en";
import { ptBR } from "./locales/pt-BR";
import { fr } from "./locales/fr";
import { de } from "./locales/de";
import { it } from "./locales/it";
import { zhCN } from "./locales/zh-CN";
import { ja } from "./locales/ja";
import { ru } from "./locales/ru";

export type Key = keyof typeof es;
/** Claves de plural sin el sufijo (`"pending.count"` por `"pending.count.one"`/`".other"`). */
export type PluralKey = Key extends infer K ? (K extends `${infer B}.other` ? B : never) : never;
/** Un idioma: todas las claves del español, más formas de plural extra si las necesita. */
export type Dict = Record<Key, string> & { [k: string]: string };

export const LANGS = [
  { code: "es", name: "Español", english: "Spanish", intl: "es-AR" },
  { code: "en", name: "English", english: "English", intl: "en-US" },
  { code: "pt-BR", name: "Português (Brasil)", english: "Portuguese (Brazil)", intl: "pt-BR" },
  { code: "fr", name: "Français", english: "French", intl: "fr-FR" },
  { code: "de", name: "Deutsch", english: "German", intl: "de-DE" },
  { code: "it", name: "Italiano", english: "Italian", intl: "it-IT" },
  { code: "zh-CN", name: "简体中文", english: "Chinese (Simplified)", intl: "zh-CN" },
  { code: "ja", name: "日本語", english: "Japanese", intl: "ja-JP" },
  { code: "ru", name: "Русский", english: "Russian", intl: "ru-RU" },
] as const;

export type Lang = (typeof LANGS)[number]["code"];

const DICTS: Record<Lang, Dict> = { es, en, "pt-BR": ptBR, fr, de, it, "zh-CN": zhCN, ja, ru };

const STORAGE_KEY = "lang";

function isLang(v: unknown): v is Lang {
  return LANGS.some((l) => l.code === v);
}

/** Idioma guardado o, la primera vez, el de Windows (WebView2 usa el idioma de la interfaz). */
function detect(): Lang {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (isLang(saved)) return saved;
  } catch {}
  const wanted = typeof navigator === "undefined" ? [] : (navigator.languages ?? [navigator.language]);
  for (const raw of wanted) {
    const l = raw.toLowerCase();
    if (l.startsWith("pt")) return "pt-BR";
    if (l.startsWith("zh")) return "zh-CN";
    const base = l.split("-")[0];
    const hit = LANGS.find((x) => x.code === base);
    if (hit) return hit.code;
  }
  return "en";
}

function fill(s: string, vars?: Record<string, string | number>): string {
  if (!vars) return s;
  return s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
}

class I18n {
  lang = $state<Lang>(detect());
  /** Locale para Intl (números, fechas). */
  locale = $derived(LANGS.find((l) => l.code === this.lang)?.intl ?? "es-AR");
  #listeners: (() => void)[] = [];

  constructor() {
    if (typeof document !== "undefined") document.documentElement.lang = this.lang;
  }

  /** Le pasa el idioma al backend. Se llama antes de pedir el estado del sistema. */
  async init() {
    await this.#tellBackend(this.lang);
  }

  async #tellBackend(lang: Lang) {
    try {
      await i18nApi.set(lang);
    } catch (e) {
      console.error("No se pudo cambiar el idioma del backend", e);
    }
  }

  onChange(fn: () => void) {
    this.#listeners.push(fn);
  }

  /**
   * Primero el backend y después la interfaz: las vistas que se vuelven a montar con el
   * idioma nuevo piden sus datos y ya los reciben traducidos.
   */
  async set(lang: Lang) {
    if (lang === this.lang || !isLang(lang)) return;
    await this.#tellBackend(lang);
    this.lang = lang;
    try {
      localStorage.setItem(STORAGE_KEY, lang);
    } catch {}
    document.documentElement.lang = lang;
    for (const fn of this.#listeners) fn();
  }

  t(key: Key, vars?: Record<string, string | number>): string {
    return fill(DICTS[this.lang][key] ?? es[key] ?? key, vars);
  }

  tn(key: PluralKey, n: number, vars?: Record<string, string | number>): string {
    const d = DICTS[this.lang];
    const form = new Intl.PluralRules(this.locale).select(n);
    const s = d[`${key}.${form}`] ?? d[`${key}.other`] ?? (es as Dict)[`${key}.other`] ?? key;
    return fill(s, { n, ...vars });
  }
}

export const i18n = new I18n();

/** Texto traducido. Se puede usar en plantillas: se actualiza solo al cambiar de idioma. */
export const t = (key: Key, vars?: Record<string, string | number>) => i18n.t(key, vars);
/** Texto con plural según `n` (disponible como `{n}` en el texto). */
export const tn = (key: PluralKey, n: number, vars?: Record<string, string | number>) => i18n.tn(key, n, vars);
