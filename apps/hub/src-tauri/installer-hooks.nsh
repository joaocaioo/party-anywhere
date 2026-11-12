; Fim da instalação do Hub.
;
; Idioma: o instalador pergunta (português ou inglês) e o Hub abre no idioma escolhido. Quem instala sem janela
; (um agente de IA, por exemplo) passa /LANG=en ou /LANG=pt-BR junto com /S. A escolha vai para "lang.pending"
; ao lado do Hub, que lê uma vez e guarda nos ajustes; atualização silenciosa sem /LANG não muda nada.
;
; Permissões: pede de uma vez, numa confirmação só do Windows, o que faz tudo funcionar:
; - o firewall deixar os celulares da mesma rede chegarem ao Hub (sem isso o Windows pergunta no meio do
;   primeiro lobby, e quem não é administrador nem consegue liberar);
; - o componente dos controles virtuais (ViGEmBus), se ainda não estiver no PC: sem ele, jogos comuns não veem
;   celulares e teclados.
; A regra do firewall vale só para o programa do Hub e só para aparelhos da rede local. Feita uma vez, fica
; (o "firewall.ok" na pasta do Hub marca); atualização silenciosa (o Hub se atualizando sozinho) não pergunta nada.
!include "FileFunc.nsh"

!macro NSIS_HOOK_POSTINSTALL
  ; ---- idioma ($6)
  ${GetParameters} $5
  ClearErrors
  ${GetOptions} $5 "/LANG=" $6
  IfErrors 0 lang_write
    IfSilent lang_done
    StrCpy $6 "pt-BR"
    StrCmp $LANGUAGE 1033 0 lang_write
    StrCpy $6 "en"
  lang_write:
    FileOpen $7 "$INSTDIR\lang.pending" w
    FileWrite $7 $6
    FileClose $7
  lang_done:

  ; ---- permissões
  IfSilent perms_done
  StrCpy $1 ""
  IfFileExists "$INSTDIR\firewall.ok" +2 0
    StrCpy $1 "firewall"
  StrCpy $2 ""
  ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Services\ViGEmBus" "ImagePath"
  StrCmp $0 "" 0 +2
    StrCpy $2 "vigem"
  StrCmp "$1$2" "" perms_done
  ; $3: os comandos; $4: o que a pessoa lê (no idioma do instalador: 1033 é inglês)
  StrCpy $3 ""
  StrCpy $4 ""
  StrCmp $1 "" perms_vigem
    StrCpy $3 "$3netsh advfirewall firewall delete rule name=$\"Party Anywhere$\" >nul & netsh advfirewall firewall add rule name=$\"Party Anywhere$\" dir=in action=allow program=$\"$INSTDIR\${MAINBINARYNAME}.exe$\" remoteip=localsubnet profile=any enable=yes && echo ok>$\"$INSTDIR\firewall.ok$\" & "
    StrCmp $LANGUAGE 1033 0 +3
      StrCpy $4 "$4$\r$\n• let phones on the same Wi-Fi connect to this PC;"
      Goto perms_vigem
    StrCpy $4 "$4$\r$\n• deixar os celulares do mesmo Wi-Fi se conectarem ao PC;"
  perms_vigem:
  StrCmp $2 "" perms_ask
    StrCpy $3 "$3start $\"$\" /wait $\"$INSTDIR\drivers\ViGEmBus_setup.exe$\" & "
    StrCmp $LANGUAGE 1033 0 +3
      StrCpy $4 "$4$\r$\n• install the free component that turns phones and keyboards into controllers in regular games (Steam, Epic);"
      Goto perms_ask
    StrCpy $4 "$4$\r$\n• instalar o componente gratuito que faz celulares e teclados virarem controles em jogos comuns (Steam, Epic);"
  perms_ask:
  StrCmp $LANGUAGE 1033 0 perms_ask_pt
    MessageBox MB_OKCANCEL|MB_ICONINFORMATION "For everything to work, Party Anywhere needs a Windows permission, just once, to:$\r$\n$4$\r$\n$\r$\nWindows will ask you to confirm. You can also do this later, from the Hub." IDCANCEL perms_done
    Goto perms_run
  perms_ask_pt:
    MessageBox MB_OKCANCEL|MB_ICONINFORMATION "Para tudo funcionar, o Party Anywhere precisa de uma permissão do Windows, uma vez só, para:$\r$\n$4$\r$\n$\r$\nO Windows vai pedir sua confirmação. Dá para fazer depois, pelo Hub." IDCANCEL perms_done
  perms_run:
  ; o cmd fica escondido; o "start" abre o instalador do componente numa janela normal, já com a permissão
  ExecShellWait "runas" "$SYSDIR\cmd.exe" "/c $3exit 0" SW_HIDE
  perms_done:
!macroend
