const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** Tamaño legible en base 1024, como lo muestra Windows ("12,4 GB"). */
export function bytes(n: number): string {
  let i = 0;
  while (n >= 1024 && i < UNITS.length - 1) {
    n /= 1024;
    i++;
  }
  const digits = i === 0 || n >= 100 ? 0 : 1;
  return `${n.toLocaleString("es-AR", { maximumFractionDigits: digits, minimumFractionDigits: digits })} ${UNITS[i]}`;
}

export function count(n: number): string {
  return n.toLocaleString("es-AR");
}

export function seconds(ms: number): string {
  return `${(ms / 1000).toLocaleString("es-AR", { maximumFractionDigits: 1 })} s`;
}
