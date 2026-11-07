; Hooks del instalador NSIS (bundle > windows > nsis > installerHooks).
; La plantilla de Tauri no carga el editor en las propiedades del instalador;
; se agrega acá para que figure "CompanyName" como en el exe y en el MSI.
; Este archivo se incluye antes de que Tauri defina ${MANUFACTURER}, por eso el valor va
; escrito: si cambia "publisher" en tauri.conf.json, cambiarlo también acá.
VIAddVersionKey "CompanyName" "buccellaluciano"

; La 0.1.0 se llamaba "Prune" y quedó registrada con otro nombre, así que la plantilla no
; la ve como instalación previa. Si sigue instalada, se desinstala en silencio antes de
; instalar P-Rune. Su desinstalador solo borra sus propios archivos: data\journal.json
; (el historial) se conserva y P-Rune lo migra a su carpeta al abrirse como admin.
!macro NSIS_HOOK_PREINSTALL
  Push $0
  ReadRegStr $0 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Prune" "UninstallString"
  ${If} $0 != ""
  ${AndIf} ${FileExists} "$PROGRAMFILES64\Prune\uninstall.exe"
    DetailPrint "Quitando Prune 0.1.0..."
    ExecWait '"$PROGRAMFILES64\Prune\uninstall.exe" /S _?=$PROGRAMFILES64\Prune'
    Delete "$PROGRAMFILES64\Prune\uninstall.exe"
    RMDir "$PROGRAMFILES64\Prune"
  ${EndIf}
  Pop $0
!macroend
