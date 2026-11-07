; Hooks del instalador NSIS (bundle > windows > nsis > installerHooks).
; La plantilla de Tauri no carga el editor en las propiedades del instalador;
; se agrega acá para que figure "CompanyName" como en el exe y en el MSI.
; Este archivo se incluye antes de que Tauri defina ${MANUFACTURER}, por eso el valor va
; escrito: si cambia "publisher" en tauri.conf.json, cambiarlo también acá.
VIAddVersionKey "CompanyName" "buccellaluciano"
