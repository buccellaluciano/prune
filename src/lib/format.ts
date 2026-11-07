import { i18n } from "./i18n.svelte";

const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** Tamaño legible en base 1024, como lo muestra Windows ("12,4 GB"). */
export function bytes(n: number): string {
  let i = 0;
  while (n >= 1024 && i < UNITS.length - 1) {
    n /= 1024;
    i++;
  }
  const digits = i === 0 || n >= 100 ? 0 : 1;
  return `${n.toLocaleString(i18n.locale, { maximumFractionDigits: digits, minimumFractionDigits: digits })} ${UNITS[i]}`;
}

export function count(n: number): string {
  return n.toLocaleString(i18n.locale);
}

export function seconds(ms: number): string {
  return `${(ms / 1000).toLocaleString(i18n.locale, { maximumFractionDigits: 1 })} s`;
}

/** Iniciales para el avatar cuando la cuenta no tiene foto ("Ana Pérez" → "AP"). */
export function initials(name: string): string {
  const parts = name.trim().split(/\s+/);
  return (parts.length > 1 ? parts[0][0] + parts[parts.length - 1][0] : name.slice(0, 1)) || "?";
}
