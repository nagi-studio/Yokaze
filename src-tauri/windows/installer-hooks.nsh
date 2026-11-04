; Yokaze shipped as YesPlayMusic up to v0.12. Tauri derives the install
; directory and the uninstall entry from the product name, so after the
; rename an update installs side by side with the old copy; remove it.
; The old uninstaller keeps app data unless the user ticks a checkbox that
; silent mode never shows; the app moves that data to the new identifier on
; first launch (src-tauri/src/identity_migration.rs).
!define YOKAZE_LEGACY_UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\YesPlayMusic"

!macro NSIS_HOOK_POSTINSTALL
  Push $R0
  Push $R1
  ReadRegStr $R0 HKCU "${YOKAZE_LEGACY_UNINSTKEY}" "InstallLocation"
  ; Tauri stores the location quoted.
  StrCpy $R1 $R0 1
  ${If} $R1 == '"'
    StrCpy $R0 $R0 -1 1
  ${EndIf}
  ${If} $R0 != ""
  ${AndIf} $R0 != $INSTDIR
  ${AndIf} ${FileExists} "$R0\uninstall.exe"
    ExecWait '"$R0\uninstall.exe" /S _?=$R0'
    ; `_?=` runs the uninstaller in place, so it cannot delete itself.
    Delete "$R0\uninstall.exe"
    RMDir "$R0"
  ${EndIf}
  Pop $R1
  Pop $R0
!macroend
