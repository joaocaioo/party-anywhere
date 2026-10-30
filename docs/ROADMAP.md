# Party Anywhere: roadmap

Cada fase termina com um critério verificável. Fase só começa quando a anterior cumpriu o critério
(ou quando decidimos conscientemente pular).

| Fase | Entrega | Pronto quando |
|---|---|---|
| **0. Fundação** ✅ | Docs (visão, UX, arquitetura, SDK, manifesto), protótipo navegável do Hub ([prototype/hub](../prototype/hub/index.html): lobby, adicionar jogador, Biblioteca 3D, gerenciar jogos), template de jogo. | Visão e fluxos aprovados pelo dono do produto. **UI e UX do protótipo aprovadas: são a referência visual da fase 2.** |
| **1. Spike de latência** 🔧 ferramenta pronta, falta medir nos celulares ([spikes/latency](../spikes/latency/README.md)) | Servidor Rust mínimo + página do celular + controle virtual Xbox (ViGEm). Ferramenta que mede o tempo do toque até o estado chegar no PC. | p50 < 20 ms e p99 < 50 ms em Wi-Fi 5 GHz doméstico, medidos em pelo menos 2 celulares (Android e iPhone). Senão: testar WebRTC antes de seguir. |
| **2. Hub MVP (Modo Universal)** | App Tauri com lobby real: SDL3 (USB e já pareados), celular por QR, vagas P1..P10 com reconexão, controles virtuais, navegação por controle, instalador, atualização automática. | Teste de usabilidade: pessoa nova coloca 4 jogadores (2 celulares, 1 USB, 1 já pareado) em < 1 min sem ajuda, e joga um jogo com co-op local nativo. |
| **3. Bluetooth** | "Adicionar jogador → Controle Bluetooth": rádio, procura filtrada, instruções por marca, pareamento sem janela do Windows, esquecer. | Pareia DualSense, DualShock 4, Xbox (BLE), Switch Pro e um 8BitDo, cada um em < 30 s, sem abrir as Configurações. |
| **4. SDK v1** | `party_anywhere.h`, servidor IPC, jogo de exemplo pequeno e de código aberto para 10 jogadores, layouts e rótulos no celular. | O exemplo roda com 10 jogadores (misturando celulares e controles) sem nenhum código de controle próprio. Testes do SDK em x86 e x64. |
| **5. Biblioteca** | Manifesto v1 com validador, instalar por link, capas, jogo-base pela Steam, atualizações de jogos, catálogo com selo Verificado, template e workflow de release. | Colar o link do jogo de exemplo instala e abre em < 1 min; um jogo sem capa ou com hash errado é recusado com mensagem útil. |
| **6. Primeiro mod de verdade** | Um mod co-op refeito do zero sobre o SDK (candidato: GTA San Andreas, splitscreen até 8) e skills de agente para criar jogos co-op com o SDK. | Publicado com manifesto, instalável por link, jogado de ponta a ponta com 4 ou mais pessoas. |
| **7. Depois** | WebRTC, giroscópio (HTTPS local), Linux/Steam Deck, reemitir controles físicos na ordem do lobby (HidHide), binding C#/Unity e Godot, relé pela internet. | Priorizado pelo que os usuários pedirem. |

## Fora do escopo (decidido)

- Contornar anti-cheat ou entrar em serviços online de terceiros.
- Distribuir arquivos de jogos.
- Launcher de múltiplas instâncias estilo Nucleus (reavaliar depois da fase 6).
