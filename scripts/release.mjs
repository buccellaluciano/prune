// Arma una versión de P-Rune lista para la auto-actualización:
//   1. compila los instaladores firmados (MSI y NSIS) con la clave de actualizaciones,
//   2. genera latest.json (lo que consulta la app) con las firmas,
//   3. con --publish, crea el release en GitHub y sube todo.
//
// La clave NUNCA está en el repo. Antes de correrlo, en la misma terminal:
//   $env:TAURI_SIGNING_PRIVATE_KEY = "$env:USERPROFILE\.tauri\p-rune.key"
//   $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<la contraseña>"   (o dejá que lo pregunte)
//
// Uso:
//   node scripts/release.mjs --notes notas.md            # solo compila y arma latest.json
//   node scripts/release.mjs --notes notas.md --publish  # además publica el release
// Para pruebas locales: --base-url http://127.0.0.1:8765 --config archivo.json (se pasa a tauri build).
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, copyFileSync, writeFileSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { homedir } from "node:os";

const ROOT = resolve(import.meta.dirname, "..");
const REPO = "buccellaluciano/prune";

const args = process.argv.slice(2);
const opt = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
const flag = (name) => args.includes(name);
const fail = (msg) => {
  console.error(`\n✗ ${msg}`);
  process.exit(1);
};

const notesFile = opt("--notes");
const extraConfig = opt("--config");
const publish = flag("--publish");
if (!notesFile || !existsSync(notesFile)) fail("Falta --notes <archivo.md> con las notas de la versión.");

const conf = JSON.parse(readFileSync(join(ROOT, "src-tauri/tauri.conf.json"), "utf8"));
const overrides = extraConfig ? JSON.parse(readFileSync(extraConfig, "utf8")) : {};
const version = overrides.version ?? conf.version;
const product = conf.productName;
const pubkey = overrides.plugins?.updater?.pubkey ?? conf.plugins?.updater?.pubkey;
if (!pubkey) fail("plugins.updater.pubkey está vacío en tauri.conf.json: poné la clave pública (.pub).");
if (!process.env.TAURI_SIGNING_PRIVATE_KEY) fail("Falta TAURI_SIGNING_PRIVATE_KEY (ruta o contenido de la clave privada).");
// La sección Versiones de la app muestra el historial que viene adentro: sin entrada, la
// versión nueva aparecería sin resumen.
if (!readFileSync(join(ROOT, "src/lib/changelog.ts"), "utf8").includes(`version: "${version}"`)) {
  fail(`Falta la entrada de ${version} en src/lib/changelog.ts (con el resumen en los 9 idiomas).`);
}
const baseUrl = opt("--base-url") ?? `https://github.com/${REPO}/releases/download/v${version}`;
if (publish && opt("--base-url")) fail("--publish no se combina con --base-url.");

// 1. Compilar con artefactos de actualización (firmas .sig).
const buildConfig = { ...overrides, bundle: { ...(overrides.bundle ?? {}), createUpdaterArtifacts: true } };
const cfgPath = join(ROOT, "src-tauri/target/release-config.json");
mkdirSync(join(ROOT, "src-tauri/target"), { recursive: true });
writeFileSync(cfgPath, JSON.stringify(buildConfig));
// Rust guarda en el ejecutable rutas absolutas de la máquina que compila (los mensajes de
// error de cada dependencia: C:\Users\<usuario>\.cargo\registry\...). Se reemplazan por
// rutas neutras para no publicar el nombre de usuario ni la estructura de carpetas.
const remap = [[homedir(), "~"], [ROOT, "."]];
if (process.env.CARGO_HOME) remap.splice(1, 0, [process.env.CARGO_HOME, "~/.cargo"]);
const rustflags = [process.env.CARGO_ENCODED_RUSTFLAGS, ...remap.map(([from, to]) => `--remap-path-prefix=${from}=${to}`)]
  .filter(Boolean)
  .join("\x1f");
console.log(`Compilando ${product} ${version}…`);
execFileSync(process.execPath, [join(ROOT, "node_modules/@tauri-apps/cli/tauri.js"), "build", "--config", cfgPath], {
  cwd: ROOT,
  stdio: "inherit",
  env: { ...process.env, CARGO_ENCODED_RUSTFLAGS: rustflags },
});

// Comprobación: el ejecutable no puede llevar la carpeta del usuario.
const exe = readFileSync(join(ROOT, "src-tauri/target/release", `${conf.mainBinaryName ?? product}.exe`)).toString("latin1");
const leaked = [homedir(), basename(homedir())].filter((s) => s.length > 2 && exe.toLowerCase().includes(`\\${s.toLowerCase()}\\`));
if (leaked.length) fail(`El ejecutable todavía contiene rutas locales (${leaked.join(", ")}). No se publica.`);

// 2. Artefactos y firmas.
const bundle = join(ROOT, "src-tauri/target/release/bundle");
const files = {
  nsis: join(bundle, "nsis", `${product}_${version}_x64-setup.exe`),
  msi: join(bundle, "msi", `${product}_${version}_x64_es-ES.msi`),
};
const platforms = {};
const out = join(ROOT, "src-tauri/target/release/update", version);
mkdirSync(out, { recursive: true });
const sha = {};
for (const [kind, file] of Object.entries(files)) {
  const sig = `${file}.sig`;
  if (!existsSync(file) || !existsSync(sig)) fail(`No se encontró ${file} o su firma .sig.`);
  const name = file.split(/[\\/]/).pop();
  copyFileSync(file, join(out, name));
  sha[name] = createHash("sha256").update(readFileSync(file)).digest("hex");
  // La firma tiene que cubrir esta versión (requireSignedVersion en la app).
  const trusted = Buffer.from(readFileSync(sig, "utf8").trim(), "base64").toString("utf8");
  if (!trusted.includes(version)) fail(`La firma de ${name} no incluye la versión ${version}; actualizá @tauri-apps/cli.`);
  platforms[`windows-x86_64-${kind}`] = { signature: readFileSync(sig, "utf8").trim(), url: `${baseUrl}/${encodeURIComponent(name)}` };
}
// Instalaciones sin tipo registrado: el NSIS.
platforms["windows-x86_64"] = platforms["windows-x86_64-nsis"];

const notes = readFileSync(notesFile, "utf8").trim();
const latest = { version, notes, pub_date: new Date().toISOString(), platforms };
writeFileSync(join(out, "latest.json"), JSON.stringify(latest, null, 2) + "\n");
console.log(`\n✓ Listo en ${out}`);
for (const [name, h] of Object.entries(sha)) console.log(`  ${h}  ${name}`);

// 3. Publicar. El texto del release en GitHub suma descargas, aviso y hashes a las notas
// (en latest.json, que muestra la app, van solo las notas).
const [nsisName, msiName] = [files.nsis, files.msi].map((f) => f.split(/[\\/]/).pop());
const body = `${notes}

## Descargas

| Archivo | Para qué |
|---|---|
| **\`${msiName}\`** | Instalador recomendado. Instala para todo el equipo. |
| \`${nsisName}\` | Alternativa con NSIS. Mismo programa. |

Si ya tenés P-Rune 0.3.0 o posterior, no hace falta bajar nada: la app avisa y se actualiza sola.

> [!WARNING]
> Los ejecutables no están firmados con un certificado de código: Windows SmartScreen puede mostrar
> "editor desconocido". Las actualizaciones automáticas sí van firmadas con la clave del proyecto.

## Verificar la descarga (SHA-256)

\`\`\`
${Object.entries(sha).map(([n, h]) => `${h}  ${n}`).join("\n")}
\`\`\`
`;
const bodyFile = join(out, "release-body.md");
writeFileSync(bodyFile, body);

if (publish) {
  const assets = [...Object.keys(sha).map((n) => join(out, n)), join(out, "latest.json")];
  console.log(`\nPublicando v${version} en ${REPO}…`);
  execFileSync("gh", ["release", "create", `v${version}`, ...assets, "-R", REPO, "--target", "master", "--title", `${product} ${version}`, "--notes-file", bodyFile, "--latest"], {
    stdio: "inherit",
  });
}
