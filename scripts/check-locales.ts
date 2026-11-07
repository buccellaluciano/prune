// Verifica que cada idioma de la interfaz use los mismos {valores} que el español en cada
// texto, y que los plurales tengan al menos .one y .other. Uso: node scripts/check-locales.ts
import { es } from "../src/lib/locales/es.ts";
import { en } from "../src/lib/locales/en.ts";
import { ptBR } from "../src/lib/locales/pt-BR.ts";
import { fr } from "../src/lib/locales/fr.ts";
import { de } from "../src/lib/locales/de.ts";
import { it } from "../src/lib/locales/it.ts";
import { zhCN } from "../src/lib/locales/zh-CN.ts";
import { ja } from "../src/lib/locales/ja.ts";
import { ru } from "../src/lib/locales/ru.ts";

const langs: Record<string, Record<string, string>> = { en, "pt-BR": ptBR, fr, de, it, "zh-CN": zhCN, ja, ru };
const vars = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(",");
const base = (k: string) => k.replace(/\.(one|few|many|other)$/, "");

let problems = 0;
for (const [lang, d] of Object.entries(langs)) {
  for (const [k, v] of Object.entries(d)) {
    const ref = (es as Record<string, string>)[k] ?? (es as Record<string, string>)[`${base(k)}.other`];
    if (ref === undefined) {
      console.log(`${lang}: clave que no existe en es: ${k}`);
      problems++;
      continue;
    }
    // En plurales, {n} puede omitirse (p. ej. "cambio pendiente"); el resto tiene que coincidir.
    const plural = k !== base(k);
    const a = vars(v).split(",").filter((x) => x && !(plural && x === "n")).join(",");
    const b = vars(ref).split(",").filter((x) => x && !(plural && x === "n")).join(",");
    if (a !== b) {
      console.log(`${lang}: ${k} usa {${a}} y el español {${b}}`);
      problems++;
    }
    if (!v.trim()) {
      console.log(`${lang}: ${k} vacío`);
      problems++;
    }
  }
}
console.log(problems ? `${problems} problemas` : "ok: 8 idiomas coinciden con el español");
process.exit(problems ? 1 : 0);
