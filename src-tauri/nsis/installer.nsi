; Kill running AuraAlpha processes before install
!macro NSIS_HOOK_PREINSTALL
  nsExec::ExecToLog 'taskkill /F /IM AuraAlpha.exe /T'
  nsExec::ExecToLog 'taskkill /F /IM aura-grid-worker.exe /T'
  Sleep 2000
!macroend
