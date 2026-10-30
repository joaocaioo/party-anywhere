# Party Anywhere

**Qualquer jogo. Qualquer controle. Até 10 pessoas no mesmo sofá.**

Party Anywhere é uma central de controles para Windows. Ela junta controles USB, controles Bluetooth
e celulares (via QR code, sem app) num lobby de até 10 jogadores.

- **Sozinho**, o Hub faz o celular virar um controle de Xbox para qualquer jogo.
- **Com o SDK**, qualquer jogo ou mod co-op lê os jogadores do Hub sem escrever código de controle.
- **Na Biblioteca**, você cola o link de um repositório e o jogo aparece com capa, pronto para jogar.

> Status: **fase 1 (spike de latência)**, com a UI e a UX do Hub aprovadas no protótipo. Ainda não há app
> para instalar. Veja o [roadmap](docs/ROADMAP.md).

## Medir a latência do celular

```bash
cargo run --release -p pa-latency-spike
```

Escaneie o QR do terminal com o celular (mesmo Wi-Fi). Detalhes em [spikes/latency](spikes/latency/README.md).

## Ver o protótipo

Abra [`prototype/hub/index.html`](prototype/hub/index.html) no navegador (Chrome ou Edge). Conecte um
controle e aperte qualquer botão: ele aparece no lobby. O painel "Demo" simula celulares, Bluetooth e
atualizações.

Para ver a Biblioteca com capas de jogos reais, sirva a pasta e abra http://localhost:5173:

```bash
python -m http.server 5173 --directory prototype/hub
```

As capas de exemplo ficam em `prototype/hub/sample-art/`, fora do git (arte com direitos autorais).
Sem elas, a Biblioteca usa capas geradas.

## Documentação

| Documento | Conteúdo |
|---|---|
| [Visão](docs/VISION.md) | Problema, público, casos de uso, princípios de UX |
| [UX](docs/UX.md) | Direção visual e todos os fluxos (lobby, celular, Bluetooth, biblioteca, atualização) |
| [Arquitetura](docs/ARCHITECTURE.md) | Decisões, módulos, protocolo do celular, Bluetooth, IPC, segurança |
| [SDK](docs/SDK.md) | Como um jogo co-op lê os jogadores do Hub |
| [Manifesto de jogo](docs/GAME-MANIFEST.md) | O `party-anywhere.json` que faz um jogo aparecer na Biblioteca |
| [Roadmap](docs/ROADMAP.md) | Fases e critérios de pronto |

## Criar um jogo para o Party Anywhere

Comece pelo [template](templates/game/): ele traz o `party-anywhere.json` e a capa modelo. O contrato
do SDK está em [docs/SDK.md](docs/SDK.md).

## Licença

[MIT](LICENSE).
