# Party Anywhere: roadmap

Cada fase termina com um critério verificável. Fase só começa quando a anterior cumpriu o critério
(ou quando decidimos conscientemente pular).

| Fase | Entrega | Pronto quando |
|---|---|---|
| **0. Fundação** ✅ | Docs (visão, UX, arquitetura, SDK, manifesto), protótipo navegável do Hub ([apps/hub/ui](../apps/hub/ui/index.html): lobby, adicionar jogador, Biblioteca 3D, gerenciar jogos), template de jogo. | Visão e fluxos aprovados pelo dono do produto. **UI e UX do protótipo aprovadas: são a referência visual da fase 2.** |
| **1. Spike de latência** ✅ ([spikes/latency](../spikes/latency/README.md)) | Servidor Rust mínimo + página do celular + controle virtual Xbox (ViGEm). Ferramenta que mede o tempo do toque até o estado chegar no PC. | Meta revista após medir: rede p50 < 10 ms; toque num botão p50 < 25 ms e p99 < 100 ms. iPhone: rede 4,7 ms, toque 23,6 / 81,6 ms (o resto é do Safari; WebRTC não ajudaria). Pendente, sem travar: medir um Android. |
| **2. Hub MVP (Modo Universal)** 🔧 feito, falta o teste com pessoas | App Tauri com lobby real: controles (gilrs: USB e já pareados; Xbox pelo XInput), celular por QR com token, teclados (Raw Input), vagas P1..P10 com reconexão (e "Caiu há N min" na carta de quem não voltou), controles virtuais (o instalador já oferece o componente), navegação por controle e celular, controle do celular com layouts (Simples, Avançado e o do jogo, editável), modos, nomes nos botões, "Quem vai jogar?" antes do jogo, instalador (NSIS) com atalho, aviso de atualização, janela sem moldura. | Teste de usabilidade: pessoa nova coloca 4 jogadores (2 celulares, 1 USB, 1 já pareado) em < 1 min sem ajuda, e joga um jogo com co-op local nativo **sem sentir atraso no celular**. |
| **3. Bluetooth** (provisório: a tela explica e abre o Bluetooth do Windows) | "Adicionar jogador → Controle Bluetooth": rádio, procura filtrada, instruções por marca, pareamento sem janela do Windows, esquecer. | Pareia DualSense, DualShock 4, Xbox (BLE), Switch Pro e um 8BitDo, cada um em < 30 s, sem abrir as Configurações. |
| **4. SDK v1** ✅ | `party_anywhere.h` (C/C++) e `PartyAnywhere.cs` (C#, Unity), jogo de exemplo para 10 jogadores, layouts e nomes nos botões, modos, tela do jogo no celular no formato do celular, câmera por arrastar o dedo. Testado em x64 e x86 (CI). | O exemplo roda com 10 jogadores sem nenhum código de controle próprio. Testes do SDK em x86 e x64. |
| **5. Biblioteca** ✅ | Manifesto v1 com validador, instalar por link ou pela pasta do projeto, capas (inclusive a da Steam), jogo-base pela Steam, pela Epic ou pela pasta escolhida (e instalar pela Steam/Epic), atualizações de jogos, controle do celular que vem com o jogo. CLI `pa` (`new` a partir de qualquer jogo instalado, `validate`, `pack`, `release`, `add`, `phone-sim`), servidor MCP, "Descobrir" pelo tópico `party-anywhere-game` (lista guardada 5 min), jogos da Steam e da Epic. | Colar o link do jogo de exemplo instala e abre em < 1 min; um jogo sem capa ou com hash errado é recusado com mensagem útil. |
| **6. Primeiro mod de verdade** 🔧 MicroWorks Co-op Local de 1 a 10, cada jogador com tudo o que o online tem (armas e veículos dele, painel próprio nos desafios de responder, quem é quem); falta testar no jogo, publicar e o teste com pessoas | Um mod co-op refeito do zero sobre o SDK e as skills de agente para criar jogos co-op. O caminho é **uma cópia do jogo, N jogadores e N visões** ([MOD-COOP-LOCAL.md](MOD-COOP-LOCAL.md), `pa docs coop`). | Publicado com manifesto, instalável por link, jogado de ponta a ponta com 4 ou mais pessoas. |
| **7. Depois** | Tela do jogo no celular **v2** (H.264/WebRTC, se o teste pedir; recorte da janela para jogos comuns), app de celular nativo e opcional, giroscópio (precisa de HTTPS na rede local, ver "Decisões"), Linux/Steam Deck, reemitir controles físicos na ordem do lobby (HidHide), binding Godot, relé pela internet, catálogo com selo "Verificado" (quando houver mods de terceiros para revisar). | Priorizado pelo que os usuários pedirem. |

## Para abrir ao público

O que falta, na ordem. Os itens com **(dono)** são decisões ou ações que só o dono do produto faz.

1. **Controles Xbox além de 4.** O Hub lê os controles Xbox pelo XInput, que tem 4 lugares, e no Modo
   Universal os controles virtuais dos celulares e teclados ocupam esses lugares também. Um Xbox de verdade sem
   lugar cai no Windows.Gaming.Input, que pode não entregar os botões a um app de PC. Caminho: ler os Xbox pela
   **GameInput** (a API de entrada atual da Microsoft, sem limite de 4) e deixar o XInput só como reserva.
   Precisa de 5 ou mais controles Xbox para testar.
2. **Testar o MicroWorks no jogo** com 2 a 4 jogadores: queimada, sapo e kart, os desafios de responder, o
   placar. **(dono)**
3. **Teste de usabilidade da fase 2** com pessoas de verdade e um jogo co-op local; medir no Diagnóstico do
   celular os quadros por segundo e o atraso da tela do jogo, num iPhone e num Android. **(dono)**
4. **Publicar o primeiro release** (`v0.1.0`): a tag roda `.github/workflows/release.yml` (instalador com hash,
   `pa-windows-x64.exe`). Sem release público, `pa hub install`, o aviso de atualização e o "baixe a `pa`" do
   `AGENTS.md` dos mods não têm de onde baixar. **(dono)**
5. **Tornar o repositório público** e publicar o MicroWorks num repositório público com o tópico
   `party-anywhere-game` e o release dele (`pa release --publish`). **(dono)**
6. **Assinatura do instalador** (Authenticode) para o SmartScreen não assustar. Até lá o README explica o aviso
   ("Mais informações → Executar assim mesmo"). Dá para lançar sem, como prévia; não dá para divulgar em massa
   sem. **(dono: o certificado é pago e ligado a uma pessoa ou empresa)**

## Limites conhecidos (dizer às pessoas)

- **Bluetooth:** um adaptador Bluetooth mantém no máximo 7 aparelhos ativos, e com controles o confortável é
  de 4 a 6. Para 10 jogadores, misture Bluetooth com USB e celulares (ou use dois adaptadores).
- **Jogos comuns (Modo Universal):** a maioria enxerga até 4 controles de Xbox (limite do Windows). De 5 a 10,
  só com mod.
- **Tela do jogo no celular:** v1 em JPEG, ~30 quadros por segundo com o Hub release (com `cargo run`, ~7). O
  atraso aparece no Diagnóstico do celular.

## Decisões do primeiro mod (MicroWorks)

- **Tela do jogo no celular desligada por padrão:** quem quer, liga no menu do celular.
- **Câmera por arrastar o dedo** (`pa_take_look`). **Giroscópio, não por enquanto:** os navegadores só dão os
  sensores de movimento em HTTPS, e o controle abre por `http://` na rede local. As saídas custam caro:
  certificado próprio assusta com um aviso (fere o UX), e um domínio com certificado por PC (como o Plex)
  exige um serviço nosso, que o produto não tem. Reavaliar com o app nativo do celular ou se as pessoas
  pedirem muito. Uma opção que nunca liga não aparece no menu.
- **Cada jogador tem o que é dele, como no online:** num jogo de rede, o que é de um jogador (arma, veículo,
  projéteis) precisa da autoridade dele no jogo de quem hospeda, e o que o jogo mostra a "o jogador local"
  (como o PDA de perguntas) precisa de um por jogador. O guia de mods (seção 3) traz isso como padrão.

## Fora do escopo (decidido)

- Contornar anti-cheat ou entrar em serviços online de terceiros.
- Distribuir arquivos de jogos.
- **Abrir várias cópias do jogo (uma por jogador).** Tentado no primeiro mod: multiplica memória, processador e
  som, depende de LAN e de o jogo aceitar rodar várias vezes. Todo mod é uma cópia do jogo com N visões
  (MOD-COOP-LOCAL.md). O Hub não abre cópias e o SDK aceita um jogo por vez.
- Contornar a proteção da loja (DRM, conta, propriedade).

## Próximos passos depois do lançamento

- **Template de mod Unity (BepInEx + Mirror)**, tirado do MicroWorks: troca de contexto, autoridade dos
  extras, tela dividida, HUD por jogador, painel de resposta, quem é quem. O segundo mod Unity começa dele.
- **Pareamento Bluetooth no app** (fase 3), depois dos controles Xbox pela GameInput.
- **Tela do jogo no celular v2**, se o teste mostrar atraso que incomoda.
