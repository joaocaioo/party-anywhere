# Party Anywhere: notas para agentes

Central de controles (Hub) + SDK + Biblioteca para jogos co-op locais de até 10 jogadores.
Comece por [docs/VISION.md](docs/VISION.md), [docs/UX.md](docs/UX.md) e [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Regras

- **UX first.** Toda mudança visível segue os princípios de [docs/UX.md](docs/UX.md): zero configuração,
  sem jargão, nenhum erro sem saída, navegável por controle, nunca interromper uma partida.
- **Os docs são o contrato.** Mudou comportamento, protocolo, ABI do SDK ou manifesto? Atualize o doc no
  mesmo PR. Protocolo do celular, ABI do SDK e manifesto são versionados: mudança incompatível sobe a versão.
- **Lógica testável sem hardware.** Regras de lobby, protocolo e manifesto ficam em código puro com testes;
  hardware (SDL3, WinRT, ViGEm) fica atrás de interfaces com implementações falsas.
- **Nada no caminho da entrada aloca ou escreve em disco.** Log só de eventos raros.
- **Idioma:** docs e textos da UI em português do Brasil; código, identificadores e commits em inglês.
- **Referência, não cópia:** o mod SACoop do dono (`C:\Games\SACoop`) é referência de produto. Não copie
  código de lá; reimplemente.
- **Nada de** contornar anti-cheat, distribuir arquivos de jogos ou dados pessoais em commits.
- Pergunte antes de commitar, publicar releases ou instalar drivers.
- **Interface aprovada:** [prototype/hub](prototype/hub/index.html) é a referência visual e de comportamento do
  Hub. O app reproduz o protótipo fielmente; não redesenhe sem pedido do dono.

## Repositório e publicação

- `joaocaioo/party-anywhere` é **privado** e guarda tudo, inclusive o processo (protótipo, spikes, docs internos).
- Commits usam o e-mail noreply do GitHub (já configurado neste repositório). Nunca use o e-mail pessoal.
- **Nunca torne este repositório público.** O histórico inteiro iria junto. Na primeira versão, o público
  recebe um histórico novo e limpo só com o produto: Hub, SDK, template e docs para usuários e criadores.
  Ficam de fora o protótipo, os spikes, VISION, ROADMAP, UX, ARCHITECTURE e este arquivo. Licença: MIT.
