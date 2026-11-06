# Debloater

Debloater para **Windows 11** hecho con Tauri 2 (Rust) + Svelte 5 + Tailwind 4.

Nació como reescritura de `buccellaluciano/debloater-alpha`. Del repo original se rescataron solo los
**datos** (las claves de registro y la lista de bloatware), ya corregidos. Se descartaron el código
PowerShell, los binarios de terceros, los "trolls" y todo componente inseguro.

## Principios

- **Sin PowerShell ni shell**: el registro se toca con `winreg` y las apps Appx con la API WinRT `PackageManager`.
- **Catálogo declarativo** (`src-tauri/catalog/*.toml`), embebido en el binario y validado al compilar con los tests.
- **Estado real**: cada switch refleja lo que hay en el registro, no un toggle a ciegas.
- **Plan antes de aplicar**: antes de escribir nada se ve el diff exacto.
- **Deshacer**: el historial guarda el valor anterior exacto (`%APPDATA%\com.debloater.app\journal.json`).
- **Superficie mínima**: los comandos solo aceptan IDs del catálogo. Sin plugin `shell` ni acceso a fs.

## Desarrollo

```bash
npm install
npm run tauri dev      # app real (sin admin en debug: los tweaks HKLM quedan deshabilitados)
npm run dev            # solo la UI en el navegador, con datos simulados (no toca el sistema)
cd src-tauri && cargo test
npm run tauri build    # instalador NSIS; el exe de release pide administrador (UAC)
```

## Estructura

| Ruta | Qué hay |
|---|---|
| `src-tauri/catalog/tweaks.toml` | Tweaks: categoría, riesgo, build mínima, operaciones on/off |
| `src-tauri/catalog/bloat.toml` | Apps removibles por nivel (seguro / moderado / avanzado) |
| `src-tauri/src/engine.rs` | Estado, plan, aplicar todo-o-nada por tweak, journal y deshacer |
| `src-tauri/src/registry.rs` | Lectura/escritura cruda del registro (vista 64 bits) |
| `src-tauri/src/appx.rs` | Listar y quitar paquetes del catálogo |
| `src/lib/store.svelte.ts` | Estado de la UI, cambios pendientes, perfiles |
| `src/lib/components/` | Sidebar, tarjetas, plan, paleta de comandos (Ctrl+K), bloatware, historial |
