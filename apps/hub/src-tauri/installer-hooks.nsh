; Fim da instalação do Hub: pede de uma vez, numa confirmação só do Windows, o que faz tudo funcionar:
; - o firewall deixar os celulares da mesma rede chegarem ao Hub (sem isso o Windows pergunta no meio do
;   primeiro lobby, e quem não é administrador nem consegue liberar);
; - o componente dos controles virtuais (ViGEmBus), se ainda não estiver no PC: sem ele, jogos comuns não veem
;   celulares e teclados.
; A regra do firewall vale só para o programa do Hub e só para aparelhos da rede local. Feita uma vez, fica
; (o "firewall.ok" na pasta do Hub marca); atualização silenciosa (o Hub se atualizando sozinho) não pergunta nada.
!macro NSIS_HOOK_POSTINSTALL
  IfSilent perms_done
  StrCpy $1 ""
  IfFileExists "$INSTDIR\firewall.ok" +2 0
    StrCpy $1 "firewall"
  StrCpy $2 ""
  ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Services\ViGEmBus" "ImagePath"
  StrCmp $0 "" 0 +2
    StrCpy $2 "vigem"
  StrCmp "$1$2" "" perms_done
  ; $3: os comandos; $4: o que a pessoa lê
  StrCpy $3 ""
  StrCpy $4 ""
  StrCmp $1 "" +3
    StrCpy $3 "$3netsh advfirewall firewall delete rule name=$\"Party Anywhere$\" >nul & netsh advfirewall firewall add rule name=$\"Party Anywhere$\" dir=in action=allow program=$\"$INSTDIR\${MAINBINARYNAME}.exe$\" remoteip=localsubnet profile=any enable=yes && echo ok>$\"$INSTDIR\firewall.ok$\" & "
    StrCpy $4 "$4$\r$\n• deixar os celulares do mesmo Wi-Fi se conectarem ao PC;"
  StrCmp $2 "" +3
    StrCpy $3 "$3start $\"$\" /wait $\"$INSTDIR\drivers\ViGEmBus_setup.exe$\" & "
    StrCpy $4 "$4$\r$\n• instalar o componente gratuito que faz celulares e teclados virarem controles em jogos comuns (Steam, Epic);"
  MessageBox MB_OKCANCEL|MB_ICONINFORMATION "Para tudo funcionar, o Party Anywhere precisa de uma permissão do Windows, uma vez só, para:$\r$\n$4$\r$\n$\r$\nO Windows vai pedir sua confirmação. Dá para fazer depois, pelo Hub." IDCANCEL perms_done
  ; o cmd fica escondido; o "start" abre o instalador do componente numa janela normal, já com a permissão
  ExecShellWait "runas" "$SYSDIR\cmd.exe" "/c $3exit 0" SW_HIDE
  perms_done:
!macroend
