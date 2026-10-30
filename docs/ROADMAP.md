# Party Anywhere: roadmap

Cada fase termina com um critério verificável. Fase só começa quando a anterior cumpriu o critério
(ou quando decidimos conscientemente pular).

| Fase | Entrega | Pronto quando |
|---|---|---|
| **0. Fundação** ✅ | Docs (visão, UX, arquitetura, SDK, manifesto), protótipo navegável do Hub ([apps/hub/ui](../apps/hub/ui/index.html): lobby, adicionar jogador, Biblioteca 3D, gerenciar jogos), template de jogo. | Visão e fluxos aprovados pelo dono do produto. **UI e UX do protótipo aprovadas: são a referência visual da fase 2.** |
| **1. Spike de latência** ✅ ([spikes/latency](../spikes/latency/README.md)) | Servidor Rust mínimo + página do celular + controle virtual Xbox (ViGEm). Ferramenta que mede o tempo do toque até o estado chegar no PC. | Meta revista após medir: rede p50 < 10 ms; toque num botão p50 < 25 ms e p99 < 100 ms. iPhone: rede 4,7 ms, toque 23,6 / 81,6 ms (o resto é do Safari; WebRTC não ajudaria). Pendente, sem travar a fase 2: medir um Android e testar dentro de um jogo. |
| **2. Hub MVP (Modo Universal)** 🔧 feito, falta o teste com pessoas | App Tauri com lobby real: controles (gilrs: USB e já pareados; Xbox pelo XInput), celular por QR com token, teclados (Raw Input), vagas P1..P10 com reconexão, controles virtuais (o instalador já oferece o componente), navegação por controle e celular, controle do celular com layouts, modos, nomes nos botões e "Padrão do jogo", "Quem vai jogar?" antes do jogo, instalador (NSIS) com atalho, aviso de atualização, janela sem moldura. | Teste de usabilidade: pessoa nova coloca 4 jogadores (2 celulares, 1 USB, 1 já pareado) em < 1 min sem ajuda, e joga um jogo com co-op local nativo **sem sentir atraso no celular**. |
| **3. Bluetooth** (provisório: a tela explica e abre o Bluetooth do Windows) | "Adicionar jogador → Controle Bluetooth": rádio, procura filtrada, instruções por marca, pareamento sem janela do Windows, esquecer. | Pareia DualSense, DualShock 4, Xbox (BLE), Switch Pro e um 8BitDo, cada um em < 30 s, sem abrir as Configurações. |
| **4. SDK v1** 🔧 SDK, exemplo e a tela do jogo no celular prontos; falta o jogo escolher layout e rótulos pelo SDK e o teste em x86 | `party_anywhere.h`, servidor IPC, jogo de exemplo pequeno e de código aberto para 10 jogadores, layouts e rótulos no celular. | O exemplo roda com 10 jogadores (misturando celulares e controles) sem nenhum código de controle próprio. Testes do SDK em x86 e x64. |
| **5. Biblioteca** 🔧 feito, falta o catálogo Verificado | Manifesto v1 com validador, instalar por link ou pela pasta do projeto, capas, jogo-base pela Steam, pela Epic ou pela pasta escolhida (e instalar pela Steam/Epic), atualizações de jogos, catálogo com selo Verificado, template e workflow de release. Também: CLI `pa`, servidor MCP para agentes, "Descobrir" pelo tópico `party-anywhere-game`, jogos da Steam e da Epic. | Colar o link do jogo de exemplo instala e abre em < 1 min; um jogo sem capa ou com hash errado é recusado com mensagem útil. |
| **6. Primeiro mod de verdade** | Um mod co-op refeito do zero sobre o SDK (candidato: GTA San Andreas, splitscreen até 8) e skills de agente para criar jogos co-op com o SDK. | Publicado com manifesto, instalável por link, jogado de ponta a ponta com 4 ou mais pessoas. |
| **7. Depois** | Tela do jogo no celular **v2** (a v1 com JPEG já existe: H.264/WebRTC se o teste em celular pedir; recorte da janela para jogos comuns), app de celular nativo e opcional (se o teste em jogo pedir menos atraso que o navegador dá), giroscópio (HTTPS local), Linux/Steam Deck, reemitir controles físicos na ordem do lobby (HidHide), binding C#/Unity e Godot, relé pela internet. | Priorizado pelo que os usuários pedirem. |

## Fora do escopo (decidido)

- Contornar anti-cheat ou entrar em serviços online de terceiros.
- Distribuir arquivos de jogos.
- Launcher de múltiplas instâncias estilo Nucleus (reavaliar depois da fase 6).

## Antes da versão pública

- Assinar o instalador do Hub (Authenticode) ou migrar a atualização para o updater do Tauri com minisign.
- Publicar num repositório público com histórico novo, só com o produto (ver CLAUDE.md).
- Teste de usabilidade da fase 2 com pessoas de verdade e um jogo co-op local.
- Medir um Android (fase 1) e testar o celular dentro de um jogo.
