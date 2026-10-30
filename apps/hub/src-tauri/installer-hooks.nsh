; Fim da instalação do Hub: sem o componente dos controles virtuais (ViGEmBus) no PC, oferece instalar
; agora. Sem ele, jogos comuns não veem celulares e teclados. O Windows pede a confirmação de administrador.
; Atualização silenciosa (o Hub se atualizando sozinho) não pergunta nada.
!macro NSIS_HOOK_POSTINSTALL
  IfSilent vigem_done
  ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Services\ViGEmBus" "ImagePath"
  StrCmp $0 "" 0 vigem_done
  MessageBox MB_YESNO|MB_ICONQUESTION "Instalar também o componente gratuito que faz celulares e teclados funcionarem como controles em jogos comuns (Steam, Epic)?$\r$\n$\r$\nRecomendado. O Windows vai pedir sua confirmação." IDNO vigem_done
  ExecShellWait "runas" "$INSTDIR\drivers\ViGEmBus_setup.exe"
  vigem_done:
!macroend
