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
- **Referência, não cópia:** o mod SACoop do dono (`C:\Games\SACoop`, GTA San Andreas, até 8 jogadores numa
  cópia só do jogo) é a referência de produto e da técnica de todo mod: troca de contexto e uma passada de
  desenho por jogador (docs/MOD-COOP-LOCAL.md). Não copie código de lá; reimplemente.
- **Nada de** contornar anti-cheat, distribuir arquivos de jogos ou dados pessoais em commits.
- Pergunte antes de commitar, publicar releases ou instalar drivers.
- **Interface aprovada:** [apps/hub/ui](apps/hub/ui/index.html) é a UI do Hub (aprovada pelo dono). É a mesma
  no navegador (modo demonstração, com o painel Demo) e no app (dados reais, via `bridge.js`). Não redesenhe
  sem pedido do dono; mudou a UI, confira nos dois modos.

## Repositório e publicação

- `joaocaioo/party-anywhere` é **um repositório só**, que vai ser público: tudo o que está commitado é para
  todo mundo ver (código, docs, spikes, este arquivo). Licença: MIT.
- Nunca commite: dados pessoais (e-mail, caminhos com o nome de usuário do PC), tokens, arquivos de jogos e as
  capas de jogos reais (`apps/hub/ui/sample-art/`, ignorada pelo git).
- Commits usam o e-mail noreply do GitHub (já configurado neste repositório). Nunca use o e-mail pessoal.
