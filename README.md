# Party Anywhere

**Qualquer jogo. Qualquer controle. Até 10 pessoas no mesmo sofá.**

Party Anywhere é uma central de controles para Windows. Controles USB e Bluetooth, celulares (pelo QR
code, sem instalar app) e teclados viram jogadores **P1 a P10** num lobby só.

- **Com qualquer jogo:** celulares e teclados viram controles de Xbox (Modo Universal).
- **Com jogos feitos para ele:** o jogo lê os jogadores pelo SDK, sem escrever código de controle.
- **Na Biblioteca:** cole o link de um repositório e o jogo aparece com capa, pronto para jogar. Versões
  novas chegam sozinhas, e os jogos do seu PC (Steam, Epic ou qualquer .exe) entram com um clique.

## Jogar

1. Instale o Party Anywhere (`PartyAnywhere-X.Y.Z-setup.exe`, na página de Releases).
2. Ligue os controles, ou escaneie o QR com o celular (mesmo Wi-Fi do PC).
3. Em **Biblioteca**, escolha o jogo e aperte **Jogar**.

Para jogos comuns verem os celulares como controle, o Hub pede para instalar o componente de controles
virtuais (ViGEmBus) uma vez.

## Criar um jogo co-op

```bash
pa new meu-jogo --name "Meu Jogo"
```

O projeto já nasce pronto: manifesto, capa provisória, o SDK (`sdk/party_anywhere.h`) com um exemplo,
o workflow que publica cada versão e a configuração para agentes de IA.

| Ferramenta | Para quê |
|---|---|
| `pa validate` | Confere manifesto e capa, e diz o que fazer em cada problema. |
| `pa pack --from build --tag v0.1.0` | Gera o pacote e o `party-anywhere.release.json` com o hash. |
| `pa inspect dono/repo` | Lê o jogo publicado como o Hub lê. |
| `pa mcp` | As mesmas ferramentas para agentes de IA (Claude Code e outros com MCP). Roda no seu computador, sem servidor e sem custo. |

Publicar: suba a `version`, crie a tag `v0.1.0` e o workflow do projeto publica o release. Adicione o
tópico **`party-anywhere-game`** ao repositório para aparecer em "Descobrir" no Hub.

- [SDK](docs/SDK.md): como o jogo lê os jogadores (`party_anywhere.h`, C/C++, x86 e x64).
- [Manifesto](docs/GAME-MANIFEST.md): o `party-anywhere.json`, o anexo do release e a capa.

## Desenvolver

Requisitos: Rust estável, Node 22 (só para gerar o instalador) e Windows 10/11.

```bash
cargo test --workspace
```

```bash
cargo run -p party-anywhere-hub
```

A UI (`apps/hub/ui`) também abre no navegador, em modo demonstração, com o painel "Demo":

```bash
python -m http.server 5173 --directory apps/hub/ui
```

Gerar o instalador (`target/release/bundle/nsis/`):

```bash
npx --yes @tauri-apps/cli@2 build
```

(rode dentro de `apps/hub/src-tauri`). Uma tag `vX.Y.Z` publica o instalador e a `pa` pelo workflow
`.github/workflows/release.yml`.

| Pasta | O quê |
|---|---|
| `apps/hub` | O app (Tauri 2): `src-tauri` em Rust, `ui` em HTML/JS |
| `crates/` | Lobby, celular, controles virtuais, SDK, manifesto, biblioteca e a CLI `pa` |
| `sdk/c` | `party_anywhere.h` e o exemplo |
| `templates/game` | O projeto modelo que o `pa new` cria |
| `schema/` | Esquema JSON do manifesto |

Arquitetura e decisões: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Experiência: [docs/UX.md](docs/UX.md).

## Licença

[MIT](LICENSE).
