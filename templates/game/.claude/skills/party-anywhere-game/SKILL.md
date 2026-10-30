---
name: party-anywhere-game
description: Criar, validar e publicar um jogo ou mod co-op local para o Party Anywhere (até 10 jogadores com controles, celulares e teclados), inclusive transformar um jogo instalado no PC em co-op. Use ao começar um jogo co-op, ao ler jogadores/controles, ao preparar a capa ou ao publicar uma versão.
---

# Jogo co-op para o Party Anywhere

O Party Anywhere é uma central de controles para Windows: controles USB e Bluetooth, celulares (QR, sem
app) e teclados viram jogadores P1..P10. O jogo **não escreve código de controle**: lê as vagas pelo SDK.
Um jogo publicado direito entra na Biblioteca de qualquer pessoa pelo link do GitHub, e as versões novas
chegam sozinhas.

## Ferramentas (servidor MCP `party-anywhere`, ou a CLI `pa`)

| Tarefa | MCP | CLI |
|---|---|---|
| O Hub está instalado e atualizado? | `hub_status` | `pa hub` |
| Instalar ou atualizar o Hub | `install_hub` (peça permissão: abre o instalador) | `pa hub install` |
| Jogos instalados que podem virar co-op | `find_installed_games` | `pa games [busca]` |
| Começar um jogo novo | `create_game` | `pa new <pasta> --name "..."` |
| Começar o mod co-op de um jogo do PC | `create_game` com `base_game` | `pa new <pasta> --name "..." --base-game "<nome ou appid>"` |
| Ler o contrato do SDK | `get_sdk_docs` | — |
| Ver o formato do manifesto | `get_manifest_spec` | — |
| Conferir manifesto e capa | `validate_game` | `pa validate` |
| Gerar o pacote do release | `pack_release` | `pa pack --from build` |
| Conferir o jogo publicado (como o Hub lê) | `inspect_published_game` | `pa inspect <dono/repo>` |
| Levar um mod publicado para a Biblioteca (para jogar) | `add_to_library` | `pa add <dono/repo>` |

## Fluxo

1. `hub_status`. Sem o Hub, ou com versão velha: explique e ofereça `install_hub` (só com o "sim" da pessoa).
2. Jogo novo: `create_game`. Mod de um jogo instalado: `find_installed_games`, confirme com a pessoa qual é,
   e `create_game` com `base_game`. Jogo com anti-cheat é recusado: explique e sugira outro.
3. `get_sdk_docs` e implemente: `pa_connect`, `pa_poll` uma vez por frame, eventos de vaga, `pa_get_pad`.
   Em mods, chame `pa_poll` no ponto em que o jogo lê os controles e ajuste `install.files` para o nome
   da DLL que o jogo carrega (proxy como `version.dll`/`dinput8.dll` ou ASI loader).
4. Peça a capa de verdade (retrato 3:4, mínimo 600×800, até 2 MB, sem texto pequeno). `validate_game`.
5. Ajuste o passo "Build" de `.github/workflows/release.yml`; suba `version` e crie a tag igual (`v0.1.0`).
6. Peça para a pessoa adicionar o tópico `party-anywhere-game` ao repositório (aparece em "Descobrir").
7. `inspect_published_game` no repositório: se diz "pronto para instalar", qualquer um cola o link no Hub.

## Regras

- **Controles só pelo SDK** (`sdk/party_anywhere.h`). Nunca leia XInput, DirectInput ou teclado direto:
  o Hub já entrega tudo como controle de Xbox, inclusive celulares e teclados.
- **Vaga em RECONNECTING**: pause ou proteja o personagem; o mesmo jogador volta para a mesma vaga.
- **`id` nunca muda** depois de publicado (`com.usuario.jogo`).
- **Nada de anti-cheat, jogos online de terceiros ou arquivos do jogo original** no pacote. O mod leva só
  os arquivos dele; o jogo-base a pessoa já tem.
- Em mods, preencha `requires.baseGame.acceptedSha256` com o hash do executável da versão suportada.
- Textos para o jogador em português do Brasil, curtos e sem jargão.
