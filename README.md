<p align="center">
  <img src="src-tauri/icons/prune.svg" width="96" alt="P-Rune" />
</p>

<h1 align="center">P-Rune</h1>

<p align="center">
  Debloater y ajustes para <b>Windows 11</b>, con vista previa de cada cambio y deshacer exacto.<br />
  Hecho con Tauri 2 (Rust) + Svelte 5 + Tailwind 4. En 9 idiomas.
</p>

![Tweaks](docs/screenshots/tweaks.png)

> [!WARNING]
> P-Rune modifica el registro de Windows, quita apps preinstaladas e instala programas, y corre como
> administrador. Revisá el plan antes de aplicar cada cambio. Se distribuye **sin garantía** (ver
> [LICENSE](LICENSE)): usalo bajo tu responsabilidad. Quitar apps no se puede deshacer desde P-Rune;
> se reinstalan desde Microsoft Store.

## Qué hace

- **Tweaks** de privacidad, IA/Copilot, Inicio, búsqueda, barra de tareas, Explorador, apariencia,
  gaming, servicios y plan de energía. Cada switch muestra el estado **real** del registro.
- **Servicios**: desactiva servicios de Windows descartables (telemetría, mapas, Xbox, cola de impresión…),
  cada uno marcado por riesgo. Si un servicio no existe en esa edición de Windows, no se crea nunca.
- **Plan antes de aplicar**: el diff exacto de cada valor que se va a escribir.
- **Historial con deshacer**: guarda el valor anterior exacto; si algo cambió después, avisa antes de pisarlo.
- **Perfiles** (Privacidad, Escritorio limpio, Gaming, Equipo liviano) que marcan varios tweaks de una.
- **Exportar e importar la configuración**: un archivo con los tweaks aplicados, las apps quitadas y los
  programas instalados, para llevarlos a otra PC. Solo guarda ids del catálogo; al importar se muestra
  qué cambiaría y los tweaks pasan por el plan de siempre.
- **Punto de restauración** antes de cambios grandes o de quitar apps (Restaurar sistema de Windows),
  y desde el Historial se crea uno o se abre Restaurar sistema.
- **Cambios que Windows deshizo**: si una actualización vuelve atrás un tweak o reinstala una app que
  quitaste, P-Rune avisa y deja reaplicarlo o ignorarlo.
- **Inicio de Windows**: lo que arranca al iniciar sesión (registro, carpeta Inicio y tareas programadas
  de terceros), con su firmante. Se desactiva como lo hace el Administrador de tareas, sin borrar nada.
- **Windows Update**: pausar hasta 5 semanas, horas activas y no instalar drivers por Windows Update.
- **Red**: DNS de cada conexión (IPv4 e IPv6) desde una lista o a mano, con DNS cifrado, y activar o
  desactivar el proxy ya configurado. Cada cambio de DNS vuelve atrás solo si no se confirma.
- **Bloatware**: quita apps Appx de una lista curada; lo esencial (Store, winget, Terminal,
  Calculadora, Seguridad de Windows) nunca aparece.
- **Programas**: instala y actualiza ~40 programas confiables con winget, con progreso por etapas y una
  revisión previa con el **SHA-256 del instalador** para verificarlo en VirusTotal.
- **Disco**: encuentra las carpetas que más ocupan (el análisis es solo lectura) y **limpia lo
  descartable** con vista previa: temporales, cachés de sombreadores y miniaturas, volcados e informes de
  errores, descargas de Windows Update, caché de Spotify, Prefetch y papelera. Respeta lo de las últimas
  24 horas y lo que está en uso.
- **Temas**: Moderno (oscuro) y Aero 7, con **transparencia real** opcional (acrílico de Windows).
- **Idiomas**: español, English, Português (Brasil), Français, Deutsch, Italiano, 简体中文, 日本語 y
  Русский. Usa el idioma de Windows la primera vez y se cambia en vivo desde la barra lateral o con Ctrl K.
  Se traduce todo: interfaz, catálogo de tweaks, bloatware, programas y mensajes de error.
- **Bienvenida**: al abrir, una pantalla breve con la foto y el usuario de Windows de quien la abrió.
- **Actualizaciones**: P-Rune avisa cuando hay una versión nueva y se actualiza sola. Antes de instalar,
  Windows pide confirmar que sos vos (Windows Hello o la contraseña, en el diálogo de Windows).
- **Versiones**: la versión instalada, la nueva si hay, y el historial con un resumen de cada una.
- **Más apps**: instala [WMO](https://github.com/buccellaluciano/win-media-overlay), otra app del
  mismo autor, solo para el usuario y sin admin. Baja de GitHub la versión fijada en
  `src-tauri/src/wmo.rs` y la instala solo si el SHA-256 coincide.

| Plan de cambios | Revisión antes de instalar |
|---|---|
| ![Plan](docs/screenshots/plan.png) | ![Instalación](docs/screenshots/instalacion.png) |

| Disco | Tema Aero 7 |
|---|---|
| ![Disco](docs/screenshots/disco.png) | ![Aero](docs/screenshots/aero.png) |

## Instalación

Descargá el instalador **`.msi`** desde *Releases* (también hay uno `.exe` hecho con NSIS). Se instala
para todo el equipo en `Program Files\P-Rune` y pide permisos de administrador al abrirse.

Si tenías la 0.1.0 (se llamaba Prune), el instalador nuevo la reemplaza y P-Rune trae su historial al
abrirse por primera vez.

Los ejecutables todavía no están firmados: Windows SmartScreen puede mostrar "editor desconocido".

## Seguridad

- **Sin PowerShell ni consola**: el registro se toca con `winreg`, las apps con la API WinRT
  `PackageManager` y la energía con `powrprof`.
- **Superficie mínima**: los comandos solo aceptan IDs del catálogo; la interfaz tiene solo los permisos
  que usa, CSP estricta y prototipos congelados.
- **winget verificado**: se ejecuta desde `Program Files\WindowsApps` y solo si la firma es de Microsoft.
- **Historial protegido**: instalada, guarda el historial en `Program Files\P-Rune\data` (solo admins
  escriben) y valida cada entrada contra el catálogo antes de deshacer.
- DLLs solo desde `System32` y carpetas temporales con permisos solo para administradores.
- **Actualizaciones firmadas**: solo se instalan si están firmadas con la clave del proyecto y la firma
  cubre la versión anunciada; el instalador se guarda en una carpeta que solo pueden tocar los
  administradores, y antes de instalar Windows confirma la identidad de quien está frente a la PC.
- **Limpieza a prueba de enlaces**: nunca entra en uniones ni enlaces, fija la carpeta mientras la vacía
  (nadie puede cambiarla por otra a mitad de camino) y las carpetas del perfil se limpian con permisos de
  usuario común, no como administrador.

Para reportar una vulnerabilidad, ver [SECURITY.md](SECURITY.md).

## Desarrollo

```bash
npm install
npm run tauri dev      # app real (sin admin en debug: los tweaks de sistema quedan deshabilitados)
npm run dev            # solo la interfaz en el navegador, con datos simulados (no toca el sistema)
cd src-tauri && cargo test
npm run tauri build    # instaladores MSI y NSIS; el exe de release pide administrador (UAC)
npm run check:locales  # verifica que las traducciones de la interfaz usen los mismos {valores}
```

### Publicar una versión

1. Subí la versión en `package.json`, `src-tauri/Cargo.toml` y `src-tauri/tauri.conf.json`, y agregá su
   entrada (con el resumen en los 9 idiomas) en `src/lib/changelog.ts`.
2. Escribí las notas en `docs/releases/<versión>.md`.
   Si salió una versión nueva de WMO, actualizá en `src-tauri/src/wmo.rs` la versión, la URL,
   el tamaño y el SHA-256 del zip (`gh release view -R buccellaluciano/win-media-overlay --json assets`),
   y corré `cargo test wmo -- --include-ignored` para comprobarlos contra la descarga real.
3. En PowerShell, cargá la clave de firma (nunca va al repo) y publicá:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = "<ruta a p-rune.key>"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Net.NetworkCredential]::new('', (Read-Host "Contraseña de la clave" -AsSecureString)).Password
node scripts/release.mjs --notes docs/releases/<versión>.md --publish
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

El script compila y firma el MSI y el NSIS, verifica que las firmas cubran la versión, arma
`latest.json` (lo que consultan las apps instaladas) y crea el release en GitHub.

## Estructura

| Ruta | Qué hay |
|---|---|
| `src-tauri/catalog/tweaks.toml` | Tweaks: categoría, riesgo, build mínima, operaciones, opciones |
| `src-tauri/catalog/bloat.toml` | Apps removibles por nivel (seguro / moderado / avanzado) |
| `src-tauri/catalog/apps.toml` | Programas de winget por grupo, con su sitio oficial |
| `src-tauri/src/engine.rs` | Estado, plan, aplicar todo-o-nada, historial y deshacer |
| `src-tauri/src/registry.rs` | Lectura y escritura del registro (vista 64 bits) |
| `src-tauri/src/appx.rs` | Listar y quitar paquetes del catálogo |
| `src-tauri/src/winget.rs` | winget: estado, instalar/actualizar/desinstalar, progreso |
| `src-tauri/src/disk.rs` | Análisis de uso de disco en paralelo |
| `src-tauri/src/power.rs` | Plan de energía |
| `src-tauri/src/cleanup.rs` | Limpieza de archivos descartables |
| `src-tauri/src/updater.rs` | Actualizaciones: consulta, descarga verificada e instalador |
| `src-tauri/src/verify.rs` | Confirmación de identidad con Windows antes de actualizar |
| `src-tauri/src/i18n.rs` | Idioma del backend: mensajes y textos del catálogo |
| `src-tauri/locales/` | Traducciones del catálogo y de los mensajes (el español vive en los `.toml`) |
| `src/lib/locales/` | Traducciones de la interfaz (`es.ts` define las claves) |
| `src/lib/changelog.ts` | Historial de versiones que muestra la sección Versiones |
| `scripts/release.mjs` | Compila, firma y publica una versión |
| `src/lib/` | Estado de la interfaz, temas, componentes |

## Origen

P-Rune nació como reescritura de [`debloater-alpha`](https://github.com/buccellaluciano/debloater-alpha):
de ahí se tomaron solo los datos (claves de registro y lista de bloatware), revisados y corregidos.

## Licencia

[MIT](LICENSE) © 2026 buccellaluciano
