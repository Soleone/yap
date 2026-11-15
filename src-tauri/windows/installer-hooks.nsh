; The installer also ships the `yap` command and puts its folder on the user's PATH, so Pluck and terminals find it.

!include WinMessages.nsh

; yap.exe is built beside yap-app.exe by the same build. Declaring it as an external binary instead would need it to
; exist before that build starts.
!macro NSIS_HOOK_POSTINSTALL
  File "/oname=$INSTDIR\yap.exe" "${MAINBINARYSRCPATH}\..\yap.exe"
  !insertmacro YAP_PATH 1
!macroend

; After the app-running check, so a cancelled uninstall leaves everything in place. Yap keeps its data in its own
; folders rather than the ones named after the bundle id, so the "Delete the application data" box removes those.
!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$INSTDIR\yap.exe"
  RMDir "$INSTDIR"
  !insertmacro YAP_PATH 0
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    RMDir /r "$LOCALAPPDATA\yap"
    RMDir /r "$APPDATA\yap"
  ${EndIf}
!macroend

; Adds the install folder to the user's PATH when ADD is 1, or removes it. PowerShell edits it because NSIS strings
; stop at 1024 characters and would cut a longer PATH short. The folder goes through an environment variable so a
; quote in it can't break the command.
!macro YAP_PATH ADD
  System::Call 'Kernel32::SetEnvironmentVariable(t "YAP_DIR", t "$INSTDIR")'
  nsExec::Exec `powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment'); $$dirs = @($$key.GetValue('Path', '', 'DoNotExpandEnvironmentNames') -split ';' | Where-Object { $$_ -and $$_ -ne $$env:YAP_DIR }); if (${ADD}) { $$dirs += $$env:YAP_DIR }; $$key.SetValue('Path', ($$dirs -join ';'), 'ExpandString')"`
  Pop $0
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend
