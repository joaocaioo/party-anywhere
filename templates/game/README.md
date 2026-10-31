# Template de jogo Party Anywhere

O jeito mais fácil de começar é pela ferramenta `pa`, que cria tudo isto já preenchido:

```bash
pa new meu-jogo --id com.seu-usuario.meu-jogo --name "Meu Jogo"
```

Com agentes de IA (Claude Code, Cursor, Codex e outros): abra o agente nesta pasta. Ele lê o `AGENTS.md`
(e a skill em `.claude/skills/party-anywhere-game`), usa a `pa` no terminal e lê a documentação com
`pa docs`. Não precisa configurar nada.

## Passo a passo

1. Edite `party-anywhere.json`: `id` (único e permanente), `name`, `tagline`, `players`, `install`, `launch`.
2. Troque `assets/cover.png` pela capa de verdade (retrato 3:4, mínimo 600×800, até 2 MB).
3. Use o SDK (`party_anywhere.h`) para ler os jogadores do Hub.
4. Rode `pa validate`.
5. Ajuste o passo "Build" de `.github/workflows/release.yml`, suba a versão e crie a tag (`v0.1.0`):
   o workflow publica o release com o pacote e o `party-anywhere.release.json`.
6. Adicione o tópico **`party-anywhere-game`** ao repositório: o jogo aparece em "Descobrir" no Hub.
7. Cole o link do repositório na Biblioteca do Hub para instalar. Versões novas chegam como atualização.

Especificação completa: [GAME-MANIFEST.md](https://github.com/joaocaioo/party-anywhere/blob/main/docs/GAME-MANIFEST.md).
