---
name: party-anywhere-game
description: Criar, validar e publicar um jogo ou mod co-op local para o Party Anywhere (até 10 jogadores com controles, celulares e teclados). Use ao começar um jogo co-op, ao ler jogadores/controles, ao preparar a capa ou ao publicar uma versão.
---

# Jogo co-op para o Party Anywhere

O Party Anywhere é uma central de controles para Windows: controles USB e Bluetooth, celulares (QR, sem
app) e teclados viram jogadores P1..P10. O jogo **não escreve código de controle**: lê as vagas pelo SDK.

## Ferramentas (servidor MCP `party-anywhere`, ou a CLI `pa`)

| Tarefa | MCP | CLI |
|---|---|---|
| Começar um projeto | `create_game` | `pa new <pasta>` |
| Ler o contrato do SDK | `get_sdk_docs` | — |
| Ver o formato do manifesto | `get_manifest_spec` | — |
| Conferir manifesto e capa | `validate_game` | `pa validate` |
| Gerar o pacote do release | `pack_release` | `pa pack --from build` |
| Conferir um jogo publicado | `inspect_published_game` | `pa inspect <dono/repo>` |

## Regras

1. **Controles só pelo SDK** (`party_anywhere.h`): `pa_connect`, `pa_poll` uma vez por frame, eventos de
   vaga e `pa_get_pad`. Nunca leia XInput, DirectInput ou teclado direto: o Hub já entrega tudo como
   controle de Xbox, inclusive celulares e teclados.
2. **Vaga em RECONNECTING**: pause ou proteja o personagem; o mesmo jogador volta para a mesma vaga.
3. **Capa obrigatória**: peça a arte ao criador (retrato 3:4, mínimo 600×800, até 2 MB, sem texto
   pequeno). A capa provisória do `pa new` serve só para testar.
4. **`id` nunca muda** depois de publicado (`com.usuario.jogo`).
5. **Publicar**: suba `version` no `party-anywhere.json`, crie a tag igual (`v0.2.0`) e o workflow
   `.github/workflows/release.yml` gera e publica o pacote. Ajuste o passo "Build" para o projeto.
6. **Ser encontrado**: adicione o tópico `party-anywhere-game` ao repositório no GitHub. Ele aparece em
   "Descobrir", na Biblioteca do Hub. Quem já instalou recebe as versões novas sozinho.
7. Textos para o jogador em português do Brasil, curtos e sem jargão.
