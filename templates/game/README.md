# Template de jogo Party Anywhere

Copie esta pasta para a raiz do repositório do seu jogo.

1. Edite `party-anywhere.json`: `id` (único e permanente), `name`, `tagline`, `players`, `install`, `launch`.
2. Crie `assets/cover.png` (retrato 3:4, mínimo 600×800): edite `assets/cover.svg` ou use a sua arte e exporte em PNG. A capa é obrigatória.
3. Use o SDK ([docs/SDK.md](../../docs/SDK.md)) para ler os jogadores do Hub.
4. Publique um release no GitHub com o pacote declarado em `install.asset`.
5. Cole o link do repositório na Biblioteca do Hub para testar a instalação.

Especificação completa: [docs/GAME-MANIFEST.md](../../docs/GAME-MANIFEST.md).
