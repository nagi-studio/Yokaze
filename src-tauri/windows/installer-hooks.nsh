; Yokaze shipped as YesPlayMusic up to v0.12 (manufacturer "electron"), and as
; Yokaze with manufacturer "electron" in v0.13.x before v0.14.0 set publisher
; to "Nagi Studio". Preserve any custom install directory across those key
; changes, and remove a side-by-side legacy YesPlayMusic copy after install.
; The old uninstaller keeps app data unless the user ticks a checkbox that
; silent mode never shows; the app moves that data to the new identifier on
; first launch (src-tauri/src/identity_migration.rs).
!define YOKAZE_CURRENT_REGKEY "Software\Nagi Studio\Yokaze"
!define YOKAZE_V013_REGKEY "Software\electron\Yokaze"
!define YOKAZE_LEGACY_REGKEY "Software\electron\YesPlayMusic"
!define YOKAZE_LEGACY_UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\YesPlayMusic"

Var YokazeInstallDirInitialized

!macro YOKAZE_NORMALIZE_PATH PathVar TempVar
  StrCpy ${TempVar} ${PathVar} 1
  ${If} ${TempVar} == '"'
    StrCpy ${PathVar} ${PathVar} -1 1
  ${EndIf}
  StrLen ${TempVar} ${PathVar}
  ${If} ${TempVar} > 3
    StrCpy ${TempVar} ${PathVar} 1 -1
    ${If} ${TempVar} == '\'
      StrCpy ${PathVar} ${PathVar} -1
    ${EndIf}
  ${EndIf}
!macroend

!macro YOKAZE_RESTORE_LEGACY_INSTDIR
  Push $R0
  Push $R1
  ; Only fall back when the current publisher key has never been written and
  ; `.onInit` stayed on the default per-user path (no `/D=` override).
  ReadRegStr $R0 HKCU "${YOKAZE_CURRENT_REGKEY}" ""
  ${If} $R0 == ""
  ${AndIf} $INSTDIR == "$LOCALAPPDATA\Yokaze"
    ReadRegStr $R0 HKCU "${YOKAZE_V013_REGKEY}" ""
    !insertmacro YOKAZE_NORMALIZE_PATH $R0 $R1
    ; A deleted or offline legacy folder has no uninstaller; keep the default.
    ${If} $R0 != ""
    ${AndIf} ${FileExists} "$R0\uninstall.exe"
      StrCpy $INSTDIR $R0
      ; The template's maintenance page passes `_?=<current key>` to the old
      ; uninstaller, which would be empty for v0.13; point it at that folder.
      WriteRegStr HKCU "${YOKAZE_CURRENT_REGKEY}" "" $R0
    ${Else}
      ReadRegStr $R0 HKCU "${YOKAZE_LEGACY_REGKEY}" ""
      ${If} $R0 == ""
        ReadRegStr $R0 HKCU "${YOKAZE_LEGACY_UNINSTKEY}" "InstallLocation"
      ${EndIf}
      !insertmacro YOKAZE_NORMALIZE_PATH $R0 $R1
      ${If} $R0 != ""
      ${AndIf} ${FileExists} "$R0\uninstall.exe"
        StrCpy $R1 $R0 "" -13
        ${If} $R1 == "\YesPlayMusic"
          StrCpy $R0 $R0 -13
          StrLen $R1 $R0
          ${If} $R1 >= 2
            StrCpy $INSTDIR "$R0\Yokaze"
          ${EndIf}
        ${Else}
          StrCpy $INSTDIR $R0
        ${EndIf}
      ${EndIf}
    ${EndIf}
  ${EndIf}
  Pop $R1
  Pop $R0
!macroend

Function YokazeOnGuiInit
  !insertmacro YOKAZE_RESTORE_LEGACY_INSTDIR
  StrCpy $YokazeInstallDirInitialized 1
FunctionEnd

!define MUI_CUSTOMFUNCTION_GUIINIT YokazeOnGuiInit

!macro NSIS_HOOK_PREINSTALL
  ; Silent (`/S`) installs skip `.onGUIInit`; restore here and update `$OUTDIR`
  ; without overriding any choice made on the interactive directory page.
  ${If} $YokazeInstallDirInitialized != 1
    Push $R0
    StrCpy $R0 $INSTDIR
    !insertmacro YOKAZE_RESTORE_LEGACY_INSTDIR
    StrCpy $YokazeInstallDirInitialized 1
    ${If} $R0 != $INSTDIR
      SetOutPath $INSTDIR
      ; Section Install already created the default folder; drop it if empty.
      RMDir $R0
    ${EndIf}
    Pop $R0
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Push $R0
  Push $R1
  ; Drop the v0.13 record only once this install took that folder over. Users
  ; that v0.14.0 already moved to the default folder keep it: it is the only
  ; trace of where they originally installed.
  ReadRegStr $R0 HKCU "${YOKAZE_V013_REGKEY}" ""
  !insertmacro YOKAZE_NORMALIZE_PATH $R0 $R1
  ${If} $R0 == $INSTDIR
    DeleteRegKey HKCU "${YOKAZE_V013_REGKEY}"
  ${EndIf}
  ReadRegStr $R0 HKCU "${YOKAZE_LEGACY_UNINSTKEY}" "InstallLocation"
  !insertmacro YOKAZE_NORMALIZE_PATH $R0 $R1
  ${If} $R0 != ""
    ${If} $R0 == $INSTDIR
      DeleteRegKey HKCU "${YOKAZE_LEGACY_UNINSTKEY}"
      DeleteRegKey HKCU "${YOKAZE_LEGACY_REGKEY}"
    ${ElseIf} ${FileExists} "$R0\uninstall.exe"
      ClearErrors
      ExecWait '"$R0\uninstall.exe" /S _?=$R0' $R1
      ; `_?=` runs the uninstaller in place, so it cannot delete itself; keep
      ; it when the uninstall failed so the old copy can still be removed.
      ${IfNot} ${Errors}
      ${AndIf} $R1 == 0
        Delete "$R0\uninstall.exe"
        RMDir "$R0"
      ${EndIf}
    ${EndIf}
  ${EndIf}
  Pop $R1
  Pop $R0
!macroend
