; Kill running AuraAlpha processes before install
!macro NSIS_HOOK_PREINSTALL
  nsExec::ExecToLog 'taskkill /F /IM AuraAlpha.exe /T'
  nsExec::ExecToLog 'taskkill /F /IM aura-grid-worker.exe /T'
  Sleep 2000
!macroend

; Clean up user data on uninstall
!macro NSIS_HOOK_POSTUNINSTALL
  ; Remove app data directory: %LOCALAPPDATA%\cc.auraalpha.desktop\
  RMDir /r "$LOCALAPPDATA\cc.auraalpha.desktop"

  ; Remove grid worker data: %USERPROFILE%\.aura-worker\
  RMDir /r "$PROFILE\.aura-worker"

  ; Remove app lock file: %APPDATA%\AuraAlpha\.app.lock
  Delete "$APPDATA\AuraAlpha\.app.lock"
  Delete "$APPDATA\AuraAlpha\.running.lock"
  RMDir "$APPDATA\AuraAlpha"
!macroend
