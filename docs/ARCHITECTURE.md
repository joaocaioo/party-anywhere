# Party Anywhere: arquitetura

Este documento é o contrato técnico. Quando um código contradiz este documento, um dos dois está
errado: corrija um deles no mesmo PR. A experiência manda ([UX.md](UX.md)); a arquitetura serve a ela.

## Visão geral

```
┌──────────────────────────── Party Anywhere Hub (um processo, Windows) ────────────────────────────┐
│                                                                                                   │
│  ENTRADAS                         NÚCLEO                              SAÍDAS                      │
│  pa-input (SDL3) ──────┐                                        ┌──► pa-virtualpad (ViGEm)        │
│  controles USB/BT      │      ┌─────────────────────────┐       │    Modo Universal: até 4 Xbox   │
│                        ├────► │ pa-core                 │ ──────┤                                 │
│  pa-phone (HTTP+WS) ───┤      │ dispositivos → vagas    │       └──► pa-ipc (memória + pipe)      │
│  celulares             │      │ P1..P10, cor, nome      │            Modo SDK: jogos co-op        │
│                        │      │ eventos, persistência   │                                         │
│  pa-bluetooth (WinRT) ─┘      └────────────┬────────────┘                                         │
│  procurar / parear                         │                                                      │
│                                            ▼                                                      │
│                          UI (Tauri + React)   pa-library   pa-updater                             │
└───────────────────────────────────────────────────────────────────────────────────────────────────┘
        ▲ Wi-Fi local                                   ▲ memória compartilhada + named pipe
   página do controle no celular                   jogo/mod com party_anywhere.h (x86 ou x64)
```

## Decisões (e por quê)

| # | Decisão | Por quê | Alternativas descartadas |
|---|---|---|---|
| D1 | **Hub em Rust** (workspace com crates por módulo) | Desempenho previsível no caminho da entrada, sem coletor de lixo; boa interoperabilidade com Win32 e WinRT (crate `windows`); um binário só. | C++ (mais risco de memória na parte de rede); C#/.NET (runtime, GC no caminho quente) |
| D2 | **UI em Tauri 2 com HTML/JS sem build** (`apps/hub/ui`) | A UI aprovada já existia em HTML; uma base só roda no navegador (modo demo) e no app (`bridge.js` troca a simulação pelo núcleo). Sem bundler, sem risco de o visual derivar. Janela sem moldura do Windows, do tamanho da área útil da tela. | React/TS (reescrever a UI aprovada e manter duas); Electron (pesado) |
| D3 | **Controles físicos via gilrs** (Windows.Gaming.Input + XInput), atrás do lobby | Rust puro, sem compilar C: hotplug, bateria, vibração, mais de 4 controles. Um filtro ignora o eco dos nossos controles virtuais. SDL3 continua sendo a troca se precisarmos de luz do DualSense ou giroscópio. | SDL3 agora (build nativo pesado no MVP); XInput puro (4 controles, só Xbox) |
| D4 | **Celular via página web servida pelo Hub** (HTTP + WebSocket) | Sem app para instalar: o QR abre o navegador. Medido no iPhone: rede ~5 ms; do toque ao PC ~24 ms (p50), quase todo dentro do Safari. | App nativo (fricção de instalação); nuvem (latência, privacidade) |
| D5 | **Protocolo do celular: binário para estado, JSON para controle** | O estado vai 60 vezes por segundo e precisa ser mínimo; o resto (layout, avisos) muda pouco e precisa ser fácil de evoluir. | Tudo JSON (desperdício no caminho quente); tudo binário (difícil evoluir layouts) |
| D6 | **Sempre o estado completo do controle, nunca eventos** | Perder ou repetir uma mensagem não deixa botão preso. Base para migrar para WebRTC sem ordenação. | Eventos "apertou/soltou" |
| D7 | **Bluetooth via WinRT (`Windows.Devices.Enumeration`)** dentro do Hub | Pareamento personalizado sem a janela do Windows, com filtro de controles e PIN automático. | Mandar para as Configurações do Windows (a experiência que queremos superar) |
| D8 | **Modo Universal via ViGEmBus** atrás de uma interface `VirtualPadSink` | É o único driver de controle virtual assinado e amplamente instalado. Está **descontinuado** (2023): isolado atrás de uma interface para trocar depois. | Driver próprio agora (exige certificado EV e assinatura da Microsoft; caro e lento) |
| D9 | **SDK = cabeçalho C único** (`party_anywhere.h`, estilo stb) | ABI C funciona em C, C++, C#, Lua, Godot, Unity; um arquivo só, compila em x86 e x64 (mods de jogos antigos são 32 bits); nenhuma DLL para distribuir. | DLL (arquiteturas e versões para distribuir); SDK por linguagem |
| D10 | **IPC do SDK: só memória compartilhada** (vagas com seqlock + anel de comandos do jogo) | Estado com custo de nanossegundos e sem chamada de sistema por frame. Os eventos (entrou, caiu, voltou, saiu) o SDK gera comparando uma leitura com a anterior: não se perdem nem saem de ordem e não precisam de pipe. Menos partes móveis. | Memória + named pipe (mais código e estado para sincronizar); socket local (latência e cópias) |
| D11 | **Manifesto `party-anywhere.json` na raiz do repositório do jogo** | Um arquivo previsível permite "colar o link e instalar". Ver [GAME-MANIFEST.md](GAME-MANIFEST.md). | Convenções implícitas; registro central obrigatório |
| D12 | **Atualizações pelo GitHub Releases**, com ETag | Sem servidor nosso. Jogos: `party-anywhere.release.json` com hash. Hub: o instalador do release mais novo. Antes da versão pública: assinatura do instalador (ver Riscos). | Servidor próprio de atualização |
| D13 | **Windows primeiro** | É onde estão os jogos e os mods. Linux/Steam Deck depois (uinput torna o Modo Universal trivial lá). | — |

## Módulos

Um crate por módulo, cada um com testes que rodam sem hardware (dispositivos falsos).

| Crate | Responsabilidade | Depende de |
|---|---|---|
| `pa-core` | `Pad`, vagas e **regras do lobby** (`Lobby`), puro e testado. | — |
| `pa-phone` | Servidor HTTP + WebSocket (axum/tokio), página embutida, protocolo, escolha do IP, tokens; as vagas vêm do `Lobby` compartilhado. | `pa-core` |
| `pa-virtualpad` | `VirtualPads`: controles virtuais de Xbox 360 no ViGEm, numa thread própria. | `pa-core` |
| `pa-sdk` | Lado do Hub do SDK: a memória compartilhada (espelho do `party_anywhere.h`), testada contra o cabeçalho C compilado. | `pa-core` |
| `pa-manifest` | `party-anywhere.json` e `party-anywhere.release.json`: tipos, validação com mensagens que dizem o que fazer, capa, links. | — |
| `pa-library` | GitHub: prévia por link, Descobrir (tópico), instalação com hash e backup de jogo-base, `library.json`, atualizações (jogos e Hub). | `pa-manifest` |
| `pa-cli` | `pa` para criadores (`new`, `validate`, `pack`, `inspect`) e o servidor MCP (`pa mcp`). | `pa-manifest`, `pa-library` |
| `apps/hub/src-tauri` | O app: lobby compartilhado, controles (gilrs), teclados (Raw Input), celulares, Modo Universal, SDK, Biblioteca, jogos do PC (Steam/Epic), atualizações. | todos |
| `apps/hub/ui` | A UI (HTML/JS). | — |
| `sdk/c` | `party_anywhere.h` e o exemplo `arena_console.c`. | — |

O Bluetooth próprio (D7) é a fase 3; até lá, "Adicionar jogador → Controle Bluetooth" explica o pareamento
e abre o Bluetooth do Windows, e o controle pareado entra sozinho pelo gilrs.

## Modelo de dados

```rust
// pa-core (forma conceitual)
struct Device {
    id: DeviceId,              // estável entre sessões quando possível
    kind: DeviceKind,          // Gamepad | Phone | Keyboard
    identity: DeviceIdentity,  // serial/caminho HID/endereço BT, ou token do celular
    display_name: String,      // "Controle PlayStation 5", "iPhone do João"
    brand: Brand,              // PlayStation | Xbox | Nintendo | Generic | Phone
    caps: Caps,                // vibração, luz, bateria, giroscópio
    battery: Option<u8>,
    latency_ms: Option<u16>,   // celulares
    connected: bool,
}

struct Seat {                  // vaga no lobby: 0..9 → P1..P10
    index: u8,
    color: Rgb,                // fixa por índice (UX.md)
    name: String,
    device: Option<DeviceId>,
    state: SeatState,          // Active | Reconnecting { since } | Empty
}

struct Pad {                   // formato comum de estado, em termos de controle Xbox
    buttons: u32,              // bits: A B X Y LB RB Back Start LS RS Up Down Left Right Guide
    lx: f32, ly: f32,          // -1..1, direita e cima positivos, zona morta aplicada
    rx: f32, ry: f32,
    lt: f32, rt: f32,          // 0..1
}
```

O número de sequência não faz parte do `Pad`: ele é do transporte (protocolo do celular, memória
compartilhada do SDK), que o usa para descartar amostras velhas.

Regras do lobby (`pa-core::lobby`, testadas sem hardware):

- No máximo **10 vagas**. Dispositivo novo ocupa a menor vaga vazia. Com as 10 ocupadas, ele fica na
  espera e entra (ao apertar um botão) quando uma vaga abrir.
- Dispositivo conhecido que volta retoma **a vaga dele**, mesmo se o Hub ainda não percebeu a queda (o
  celular reabriu a página, por exemplo): a conexão antiga é fechada. A chave do celular é o token.
- Vaga em `Reconnecting` não é dada a outro dispositivo até a pessoa remover (UX: guardar a vaga), e conta
  como ocupada para "lobby cheio".
- Reordenar troca as vagas (inclusive com uma vazia); o nome vai junto e **a cor fica com o número** (P1 é
  sempre azul).
- Celular sempre entra ao conectar (a pessoa escaneou o QR de propósito). Controle entra sozinho, ou, com
  "Entrar só ao apertar um botão", fica em espera até apertar algo.
- Teclado vira jogador só por ação explícita: apertar **Enter nele** na tela Adicionar jogador → Teclado
  ou "Conectar" (senão todo notebook teria um "P1 fantasma"). Tecla qualquer não basta. Ver
  [Teclados como jogadores](#teclados-como-jogadores).
- **Remover** um jogador cujo aparelho continua ligado não o devolve ao lobby sozinho: ele vai para a
  espera e volta só apertando um botão. Remover uma vaga em `Reconnecting` esquece o aparelho.
- **Desfazer** a remoção devolve a mesma vaga, com o nome, se ela continua livre; se o aparelho saiu
  nesse meio-tempo, volta como `Reconnecting`.
- Nome: o lembrado do aparelho, senão "Jogador N". Renomear corta espaços e passa de 20 caracteres;
  vazio volta para "Jogador N".

## Threads e tempo

- **Thread de entrada** (SDL3, 1000 Hz de leitura, eventos de hotplug) → publica `Pad` no núcleo.
- **Runtime tokio** para o celular, o IPC e a rede. O WebSocket de cada celular grava direto o `Pad` da vaga.
- **Publicador** a cada 1 ms: escreve os estados na memória compartilhada (Modo SDK) e/ou no ViGEm (Modo Universal).
- **UI** recebe eventos agregados no máximo 30 vezes por segundo (a UI nunca fica no caminho da entrada).
- Nada no caminho quente aloca memória ou escreve em disco. Log de eventos raros, nunca por frame.

## Celular: protocolo v1

Transporte: `ws://<ip>:<porta>/ws`. A página é servida em `http://<ip>:<porta>/`. Porta padrão `27027`
(se ocupada, a próxima livre). O QR aponta para `http://<ip>:<porta>/?h=<id curto do Hub>`.

**Escolha do IP**: listar interfaces ativas, descartar loopback, link-local, Hyper-V/WSL/VPN/Docker
(por tipo e nome), preferir a interface com gateway padrão. A UI oferece "Ver outros endereços".

### Binário (caminho quente)

| Dir | Tipo | Formato | Notas |
|---|---|---|---|
| C→S | `0x02 STATE` | `[02][seq u16][botões u16][lx i8][ly i8][rx i8][ry i8][lt u8][rt u8]` (11 bytes) | Enviado quando muda e, parado, a cada 100 ms. `seq` descarta amostras velhas (preparado para WebRTC). |
| C→S | `0x03 PING` | `[03][t u16]` | A cada 1 s; mede latência. |
| S→C | `0x83 PONG` | `[83][t u16]` | |
| S→C | `0x84 RUMBLE` | `[84][ms u16][força u8]` | `navigator.vibrate` onde existir. |

**Medição** (a página do Hub usa sempre; uma página mínima pode usar só STATE):

| Dir | Tipo | Formato | Notas |
|---|---|---|---|
| C→S | `0x05 STATE_T` | STATE + `[tipo u8][t f64]` (20 bytes) | `t` = instante no relógio do celular (`performance.now()`): do toque num botão ou de soltar o analógico (tipo 1), do movimento do analógico (3), do envio do heartbeat (0) ou da rajada de teste (2). |
| C→S | `0x06 SYNC` | `[06][id u16][t0 f64]` | Ao conectar, 12 seguidas; depois, uma a cada 2 s. |
| S→C | `0x85 SYNC_REPLY` | `[85][id u16]` | Respondida na hora. |
| C→S | `0x07 SYNC_DONE` | `[07][id u16][t1 f64]` | O Hub estima `offset = chegada do SYNC − (t0 + t1) / 2` e usa a troca de menor ida e volta (erro ≤ rtt / 2). |

Com isso o Hub sabe, por celular e em tempo real, quanto um toque leva para chegar. Na fase 2 esse
número aparece na carta do jogador.

Implementação: `crates/pa-phone/src/protocol.rs` (codificação, com testes) e
`crates/pa-phone/web/controller.html` (página).

### JSON (frames de texto, baixa frequência)

```jsonc
// C→S, primeira mensagem
{ "t": "hello", "v": 1, "token": "base64…" /* ausente na primeira vez */, "device": "iPhone" }
// S→C
{ "t": "welcome", "token": "base64…", "seat": 3, "color": "#22C55E", "name": "Jogador 3" }
{ "t": "reject", "reason": "full" | "closed" | "version" | "kicked" | "duplicate" | "shutdown" }
{ "t": "seat", "seat": 3, "color": "#22C55E", "name": "João", "state": "active" }
{ "t": "layout", "id": "gamepad" | "simple" | "driving" | "menu", "labels": { "A": "Correr", "RT": "Atirar" } }
{ "t": "toast", "text": "Você está no time azul" }
```

### Vida da conexão

- 500 ms sem mensagem → o Hub entrega o controle **neutro** (nada de carro acelerando sozinho).
- 5 s → desconecta; a vaga vai para `Reconnecting`; o token continua válido.
- A página fecha o socket quando vai para segundo plano e reconecta quando volta.
- Limites: mensagem binária > 64 bytes ou texto > 4 KB fecha a conexão; mais de 1000 mensagens/s fecha.
- Celulares novos só entram com o lobby aberto no Hub (ou com um jogo SDK aceitando jogadores).

### Evolução prevista

- **WebRTC DataChannel** sem ordenação e sem reenvio, sinalizado pelo próprio WebSocket: elimina o
  bloqueio de fila do TCP em Wi-Fi ruim. Só entra se as medições mostrarem p99 ruim com WebSocket.
- **HTTPS local** (exigido pelo iOS para giroscópio): em aberto, ver Riscos.

## Bluetooth

Implementação em `pa-bluetooth` com a crate `windows` (WinRT), sem janela do Windows:

1. **Rádio**: `Windows.Devices.Radios.Radio` para saber se o Bluetooth existe e está ligado, e para
   ligá-lo a pedido do usuário.
2. **Procurar**: `DeviceInformation.CreateWatcher` com dois seletores, um para Bluetooth clássico não
   pareado e um para BLE não pareado.
3. **Filtrar só controles**:
   - clássico: Class of Device com major `Peripheral (0x05)` e minor `Joystick`/`Gamepad`;
   - BLE: Appearance `0x03C4` (Gamepad) ou serviço HID com nome conhecido;
   - tabela de nomes conhecidos (`Wireless Controller` → "Controle PlayStation", `Xbox Wireless Controller`,
     `Pro Controller`, `Joy-Con (L/R)`, `8BitDo …`) para nome amigável e ícone da marca.
4. **Parear**: `DeviceInformation.Pairing.Custom.PairAsync(ConfirmOnly | ProvidePin | DisplayPin)`;
   no evento `PairingRequested`, aceitar e, se pedir PIN, responder `0000`.
5. **Ligar ao lobby**: depois de parear, o SDL3 vê o controle chegar (hotplug). O Hub casa os dois pelo
   endereço Bluetooth presente no caminho HID e põe o controle **na vaga que a pessoa estava preenchendo**.
6. **Esquecer**: `UnpairAsync` na tela Controles.

Controles com particularidades (Joy-Con em par, Xbox que precisa de firmware novo para BLE) ficam numa
tabela de quirks com instruções específicas na UI.

## Teclados como jogadores

Dá para ter vários teclados jogando ao mesmo tempo, cada um como um jogador, e saber qual é qual.

- **Separar os teclados**: Raw Input (`RegisterRawInputDevices`, página 0x01 / uso 0x06, `RIDEV_INPUTSINK`).
  Cada `WM_INPUT` traz o `hDevice` do teclado que apertou, então dois teclados nunca se confundem.
- **Saber o tipo**: o caminho do aparelho (`GetRawInputDeviceInfo(RIDI_DEVICENAME)`) e a árvore do
  SetupAPI dizem por onde ele está ligado:

  | Caminho / barramento | Na carta |
  |---|---|
  | `ACPI`, `*PNP0303` (i8042) ou HID sobre I²C | Teclado · Do notebook |
  | `HID#VID_xxxx&PID_xxxx` com pai USB | Teclado · Com fio |
  | `BTHENUM` (Bluetooth clássico) ou `BTHLEDevice` (BLE) | Teclado · Bluetooth (bateria pelo GATT, quando houver) |

  O nome vem de `DEVPKEY_Device_BusReportedDeviceDesc` ("Logitech K380").
- **Um teclado, várias coleções**: muitos teclados aparecem como 2 ou 3 dispositivos HID (teclas +
  multimídia). O Hub junta tudo pelo `DEVPKEY_Device_ContainerId`; a mesma chave identifica o teclado
  entre sessões.
- **Entrar**: teclado não entra sozinho. Em Adicionar jogador → Teclado, o Hub lista os teclados
  encontrados e espera um Enter: o teclado que apertou entra (ou a pessoa clica "Conectar" na linha dele).
- **Virar controle**: mapa padrão (WASD = analógico esquerdo, setas = direito, Espaço = A, Shift = B,
  E = X, Q = Y, Enter = Start), editável em Controles. No Modo SDK o jogo recebe esse `Pad` como de
  qualquer controle.
- **Limite do Windows**: toda tecla de todo teclado também vai para a janela do jogo, e Raw Input não
  consegue bloquear. No **Modo SDK** isso não importa (o jogo lê só o Hub). No **Modo Universal**, sem
  driver: um teclado pode jogar "nativo" como P1 (o jogo lê o teclado direto); teclados extras viram
  controle virtual, mas as teclas deles também chegariam ao jogo. Para teclados extras de verdade é
  preciso um filtro de teclado em modo kernel (ex.: Interception, que tem licença própria, ou um
  filtro KMDF nosso). Fica opcional, depois do MVP, instalado só com permissão da pessoa.

## Modo Universal (controles virtuais)

- Cada **celular** e cada **teclado** numa vaga ganha um controle virtual de Xbox 360 no ViGEm, no número
  da vaga. Controles físicos continuam sendo vistos pelo jogo diretamente (sem duplicar).
- Os controles virtuais também aparecem para o gilrs: um Xbox 360 que surge até 3 s depois de o Hub ligar
  um virtual é o eco dele e é ignorado (`gamepads::EchoFilter`).
- Sem o ViGEmBus instalado, o lobby funciona igual (e os jogos do SDK também); só os jogos comuns não veem
  os celulares. A UI explica e oferece instalar o componente (pede administrador uma vez).
- Limite de 4 controles de Xbox no XInput: do 5º em diante, jogos XInput não veem. Planejado: DualShock 4
  virtual acima de 4 e o aviso "alguns jogos só aceitam 4 jogadores".
- Enquanto um jogo do SDK está conectado, o Hub **desliga todos os controles virtuais** (senão o jogo leria
  a mesma pessoa duas vezes) e liga de novo quando o jogo sai.
- **Futuro opcional**: esconder controles físicos (HidHide) e reemitir tudo como virtual, na ordem do
  lobby. Isso garante que P1 do lobby seja P1 no jogo e que um DualSense funcione em jogo "só Xbox".

## Modo SDK (IPC)

Contrato completo em [SDK.md](SDK.md). O mecanismo é **só memória compartilhada** `Local\PartyAnywhere.v1`
(4976 bytes, layout `pa_shm` no cabeçalho, espelhado em `crates/pa-sdk`):

- cabeçalho: `magic` ("PAH1"), `abi` (1), tamanho, `hub_heartbeat` (+1 a cada 100 ms), `game_pid` (o jogo
  conectado), endereço do QR;
- 10 vagas, cada uma com `seqlock` (ímpar = o Hub escrevendo), estado, aparelho, cor, nome e o `Pad`.
  O Hub escreve (atrás de uma trava, um escritor por vez); o jogo lê sem trava e repete se pegou o meio;
- anel de 64 comandos do jogo para o Hub (`rumble`, `toast`, `accepting`, `label`, `layout`, `light`): o jogo
  escreve e avança `cmd_write`, o Hub lê a cada 100 ms e avança `cmd_read`.

Regras:
- **Um jogo por vez**: o jogo reserva com `InterlockedCompareExchange(game_pid, 0 → pid)`; outro recebe
  `PA_ERR_BUSY`. Se o processo do jogo morrer sem desconectar, o Hub percebe e libera.
- Os eventos (`SEAT_JOINED`, `RECONNECTING`, `RESUMED`, `LEFT`, `RENAMED`) o SDK gera comparando cada
  `pa_poll` com o anterior; `HUB_LOST` quando o heartbeat para por 1,5 s (os pads ficam neutros).
- O SDK funciona **sem o Hub aberto**: `pa_connect` falha com um código claro, e o jogo decide.
- Aplicados hoje pelo Hub: `rumble` (controle Xbox pelo XInput; celular Android) e `toast` (celular). `label`, `layout` e `light`
  já fazem parte da ABI e chegam ao Hub; o protocolo do celular ganha os layouts na próxima versão.

## Navegar o Hub com controle ou celular

- Qualquer jogador ativo navega (menos teclados: o teclado do PC é para escrever). O mouse continua valendo.
- O núcleo lê o estado das vagas a 60 Hz (`nav.rs`), acha as bordas (apertou agora) e a direção do
  analógico/direcional com repetição (380 ms, depois 170 ms) e manda ações à UI (`nav`): cima, baixo,
  esquerda, direita, A, B, Y, LB, RB, Start.
- Só com a janela do Hub em foco e sem jogo do SDK conectado: enquanto alguém joga, os botões são do jogo.
  "Em foco" é a janela em primeiro plano do Windows (`GetForegroundWindow`): o foco do Tauri some quando o
  teclado está dentro da página (WebView2), isto é, depois de qualquer clique no Hub.
- O componente dos controles virtuais (ViGEmBus 1.22.0, BSD-3, versão fixa conferida pelo hash em
  `pa_library::vigem`): o instalador do Hub leva o arquivo e, no fim, oferece instalar (o Windows pede
  administrador; atualização silenciosa não pergunta). No app, "Instalar" baixa a mesma versão, abre com o
  aviso de administrador (`ShellExecuteEx` "runas": `CreateProcess` falha calado com instalador que exige
  admin), espera fechar, confere e avisa cada passo (`virtual-install`: baixando %, instalador aberto,
  conferindo, pronto ou o erro). Na tela de antes do jogo, pronto já segue para o jogo.
- Controles Xbox são lidos direto pelo XInput (`XInputGetState`, vaga achada como na vibração); os outros, pelos
  eventos do gilrs. `PA_DEBUG_INPUT=1` mostra no terminal o que chega de cada controle e o que a navegação vê.
- A UI (`navAction`): nas telas de cartas, esquerda/direita andam nas cartas, cima vai para as abas, baixo
  para o rodapé; nas janelas e telas de opções, o elemento mais próximo dentro de um cone na direção. A
  confirma, B volta, LB/RB trocam de aba. No navegador (modo demonstração) a mesma função recebe a Gamepad API.

## Controle do celular: layouts e menu

- Um layout tem de 1 a 6 **modos** (ex.: "A pé" e "Veículo", para jogos que mudam os comandos) e cada modo,
  **peças posicionadas** (`stickL`, `stickR`, `dpad`, `A`, `B`, `X`, `Y`, `LB`, `RB`, `LT`, `RT`, `BACK`,
  `START`, `LS`, `RS`): `{ kind, x, y, s }`, com o centro em 0..1 numa **tela de referência** deitada de
  proporção 2,17 (`CANVAS`) e `s` multiplicando o tamanho base da peça (medido na altura dessa tela).
- A tela de referência é a tela inteira do celular deitado. No meio do topo fica a **pílula** (jogador, modos,
  conexão, Menu): nenhuma peça vai ali (`TOP_PILL`, meia largura 0,22 e altura 0,14), os cantos ficam para os
  ombros e gatilhos.
- O núcleo (`settings.rs`) confere cada modo ao salvar: peça dentro da tela e fora da pílula, sem repetir, sem
  sobrepor, com um analógico ou as setas e ao menos um de A/B/X/Y. O editor mostra os mesmos erros ao vivo.
- O celular encaixa a tela de referência na sua: a altura encolhe junto (`u = min(altura, largura / 2,17)`) e
  as posições horizontais esticam na largura toda (só afastam as peças). O que não se sobrepõe no editor não se
  sobrepõe em nenhum celular. Com mais de um modo, os nomes ficam na pílula e um toque troca (solta tudo antes).
- **Analógicos livres**: o esquerdo nasce onde o dedo tocar na metade da tela em que ele está, fora das outras
  peças (o direito, na outra metade). Se os dois estão na mesma metade, cada um fica no seu retângulo. A posição
  no editor vira só a dica (um círculo fraco), que some no primeiro uso.
- iPhone: o Safari ignora `user-scalable=no`; a página bloqueia o toque duplo e a pinça e, se mesmo assim a tela
  ficar com zoom, volta ao normal sozinha.
- Prontos: **Simples** (analógico, A, B, X, Y grandes e Start) e **Avançado** (tudo). Personalizados: criados
  na aba Controles. Guardados em `settings.json`, com o layout que cada celular escolheu (pela chave do
  token). Layouts no formato antigo (só a lista `controls`) viram um modo com as posições do Avançado.
- Protocolo (aditivo, v1): `layouts` (Hub → celular: lista com os modos e o atual), `layout` e `rename`
  (celular → Hub). O modo em uso fica só no celular.
- O menu abre **segurando** o botão do topo por 0,5 s (um anel enche): sem abrir sem querer no meio do jogo.
  Abrir o menu ou trocar de layout solta todos os botões (nada fica preso no jogo).

## Jogo original de um mod (`requires.baseGame`)

O Hub nunca baixa o jogo original: o mod roda sobre a cópia da pessoa. Ao instalar um mod pelo link
(`pa_library::basegame`):

1. Procura: Steam (`steamAppId` → `appmanifest`), Epic (`epicAppName` → manifestos do launcher) e qualquer
   jogo do PC cuja pasta tenha o `exe`.
2. Não achou: a tela oferece `steam://install/<appid>`, `com.epicgames.launcher://apps/<AppName>?action=install`,
   a loja (`storeUrl` ou a página da Steam) e "já tenho": escolher o `.exe` ou a pasta (vale a pasta do jogo, o
   próprio `.exe` ou uma pasta até dois níveis acima). Mandou instalar: a tela consulta de novo a cada 3 s e
   segue sozinha quando o jogo aparece.
3. Com `acceptedSha256`, confere o executável; versão diferente mostra o `help` do autor.
4. Instala o mod na pasta achada, com backup dos originais. Ao jogar, se o jogo sumiu da pasta, a mensagem
   diz para instalar de novo pelo link.

## Fliperama da Biblioteca: desempenho

O visual é fixo; o desempenho vem de não fazer trabalho à toa (medido com 200 jogos numa Radeon integrada):

- O 3D só desenha quando alguém vê: aba Biblioteca na tela e nenhum motivo para pausar (`arcade.hold`):
  jogo aberto (a tela do jogo cobre tudo), janela do Hub por cima, Hub em segundo plano ou minimizado.
  Pausado, a placa de vídeo fica livre para o jogo; o último quadro fica na tela.
- Páginas (a tela de cada jogo) só do jogo em foco e dos vizinhos (±2); as de longe saem da placa e as capas
  de mais de 4 de distância saem da memória. Os vizinhos são montados em tempo ocioso, um por vez, primeiro na
  direção em que a pessoa anda. Memória e texturas ficam planas com qualquer número de jogos.
- A página é desenhada num canvas em memória comum (`willReadFrequently`): enviar para a placa é uma cópia e
  medir a cor da luz não para a GPU (num canvas na placa, cada página nova parava a GPU ~120 a 150 ms).
- Capas decodificadas fora da thread principal (`img.decode()`). As da Steam chegam por `pa-art://`
  (lidas do disco quando aparecem, com cache), não convertidas todas em base64 ao abrir.
- `arcade.stats()`: texturas, páginas e capas guardadas, quadros desenhados e os motivos de pausa.

## Antes do jogo abrir: quem joga

- Depois do zoom na tela do fliperama e antes de abrir o jogo (`preGame` na UI):
  1. O jogo não diz quantos aceita (jogos do PC)? O Hub pergunta (1 a 10) e guarda em `players` ("Até 4
     jogadores"). Também pergunta ao adicionar o jogo (dá para deixar para depois).
  2. Jogo comum, com celular ou teclado no lobby e sem o controle virtual (ViGEmBus)? Avisa e oferece instalar,
     conferir ou jogar só com os controles.
  3. Mais gente do que o jogo aceita? "Quem vai jogar?": cada um aperta **A** no seu controle ou celular e a
     bolinha acende; **B** apaga; clique também vale; **Start** começa.
- **Padrão do jogo**: `settings.game_layout` liga um jogo a um layout (qualquer um, até os prontos); um jogo
  que traz `controls` no manifesto ganha um layout "do jogo" ao instalar (não editável, duplicável). Ao abrir
  o jogo, `Hub::set_session_layout` manda esse layout a todos os celulares, menos a quem trocou no menu
  durante a partida; `game_stopped` devolve cada um ao seu. As regras das peças ficam em
  `pa_manifest::controls` (Hub, `pa validate` e MCP usam as mesmas).
- `game_play(id, players)`: as vagas de fora ficam no banco (`Hub::set_players`): sem controle virtual e, no
  SDK, como vaga vazia. `game_stopped` (ao voltar para a biblioteca) devolve todo mundo.

## Tela do jogo no celular (desenho; fase 7)

Ideia: o jogador vê a própria parte da tela (ou o jogo inteiro) no celular, com os controles por cima, como
um jogo de celular. Serve para jogos em que um não pode ver a tela do outro (prop hunt, esconde-esconde) ou
para quem prefere olhar para o celular em vez da TV.

**Latência realista**: não existe "zero". Capturar, codificar em vídeo (H.264 na placa de vídeo), mandar
pela rede local e decodificar no celular soma **50 a 100 ms** num Wi-Fi 5 GHz bom (Steam Link e Moonlight
ficam nessa faixa). Ótimo para festa e esconde-esconde, perceptível em jogo de reflexo. A primeira etapa é
um spike medindo exatamente isso, como fizemos com o toque.

**Como, por tipo de jogo**:

| | Jogos do SDK (os nossos) | Jogos comuns (Steam, Epic, .exe) |
|---|---|---|
| O que vai para o celular | A **visão daquele jogador**, que o jogo desenha separada (`pa_set_private_view`) | Um recorte da janela do jogo (a tela dividida daquele jogador) ou o jogo inteiro |
| Some da TV? | **Sim**: o jogo deixa de desenhar aquela parte na tela principal (privacidade de verdade) | **Não**: o jogo continua desenhando tudo na TV; o Hub só copia |
| Serve para esconde-esconde | Sim | Não (o outro vê na TV) |
| Como o Hub captura | Textura compartilhada que o jogo entrega (sem cópia extra) | Windows.Graphics.Capture da janela + recorte |

**Pipeline**: captura (textura do jogo ou WGC) → codificador de hardware (Media Foundation: NVENC, AMF ou
Quick Sync), H.264 baixa latência, sem quadros B, 720p a 60 fps → **WebRTC** para o navegador do celular (o
navegador já decodifica H.264 no hardware; sem app) → `<video>` em tela cheia com os controles do layout por
cima. O controle continua indo pelo WebSocket de hoje.

**SDK (proposta para a ABI 2, não implementada)**: `pa_set_private_view(pa, seat, texture, w, h)` para o jogo
entregar a visão do jogador; `PA_EVENT_SCREEN_ON/OFF` quando o jogador liga ou desliga a tela no menu do
celular, para o jogo parar de desenhar aquela parte na TV.

**Ordem**: (1) spike de latência WGC + Media Foundation + WebRTC em 1 celular; (2) jogos comuns com o recorte
escolhido no menu do celular; (3) a visão privada do SDK.

## Biblioteca: jogos do PC

Qualquer jogo instalado entra na Biblioteca e roda no **Modo Universal** (controles virtuais). Nada é
copiado nem modificado: o Hub só guarda onde está o jogo, como abrir e a arte.

| Fonte | Como encontrar | Como abrir | Arte e informações |
|---|---|---|---|
| Steam | `libraryfolders.vdf` → `steamapps/appmanifest_*.acf` (appid, nome, pasta) | `steam://rungameid/<appid>` | arte local em `appcache/librarycache/<appid>_library_600x900.jpg` (+ hero e logo); senão o CDN público da Steam. Nome, descrição e categorias ("Shared/Split Screen", "Local Co-op") pela API pública `store.steampowered.com/api/appdetails` |
| Epic | `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON) | `com.epicgames.launcher://apps/<app>?action=launch` | imagens do próprio manifesto/catálogo quando houver; senão busca por nome |
| GOG | registro `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\*` | executável do registro | busca por nome |
| Xbox / Microsoft Store | pacotes instalados (`PackageManager`) com a pasta `XboxGames` | `shell:AppsFolder\<AUMID>` | logos do pacote |
| Executável ou atalho | escolhido pela pessoa (.exe, .lnk, .url, ou a pasta) | o próprio arquivo | nome e ícone do recurso de versão do .exe (`ProductName`, `FileDescription`); capa pela busca por nome (SteamGridDB, com chave da pessoa, opcional); senão uma capa gerada |

- A varredura roda em segundo plano ao abrir "Jogos deste PC" e fica em cache; nada é enviado para fora,
  exceto as consultas de arte e informações (que podem ser desligadas nos Ajustes).
- Capas e informações baixadas ficam em `%LOCALAPPDATA%\PartyAnywhere\cache\` e são exibidas na tela do fliperama
  com o filtro verde da tela.

## Dados salvos

Tudo o que a pessoa monta fica salvo: fechar e abrir o Hub mostra a biblioteca, os nomes e os ajustes
exatamente como estavam.

| Arquivo (`%APPDATA%\PartyAnywhere\`) | Conteúdo |
|---|---|
| `library.json` | jogos na ordem da Biblioteca: id, origem (Steam, Epic, GOG, Xbox, exe, link), como abrir, nome, jogadores, descrição, capa e o que a pessoa editou |
| `covers\<id>.<ext>` | capas escolhidas pela pessoa, **copiadas** para cá (apagar o arquivo original não quebra nada) |
| `devices.json` | aparelhos conhecidos: nome dado a cada um ("Júlio" volta como Júlio), controles pareados |
| `settings.json` | Ajustes (entrar ao apertar um botão, endereço do QR), layouts do celular criados pela pessoa e o layout que cada celular escolheu |

- Cada arquivo tem `version`; versão antiga é migrada ao abrir, nunca descartada.
- Gravação atômica: escreve num temporário, força para o disco e troca (`ReplaceFile`), mantendo o último
  arquivo bom como `.bak`. Arquivo ilegível → carrega o `.bak` e avisa, sem perder a biblioteca.
- Grava ao mudar algo (com 300 ms de folga para juntar mudanças seguidas), numa thread de E/S. Nunca no
  caminho da entrada.
- Caches (artes baixadas, varredura de jogos) ficam à parte, em `%LOCALAPPDATA%\PartyAnywhere\cache\`:
  podem ser apagados sem perder nada que a pessoa fez.
- O protótipo usa `localStorage` (`pa.library.v1`) no lugar do `library.json`.

## Biblioteca: instalação por link

- Fonte: um repositório GitHub com `party-anywhere.json` na raiz e, para instalar, um **release**
  com o pacote declarado no manifesto (nunca "clonar o repositório e rodar").
- Passos: ler o manifesto pela API → validar o esquema → mostrar a prévia → baixar o pacote do release →
  conferir o **SHA-256** declarado → extrair em `%LOCALAPPDATA%\PartyAnywhere\games\<id>\<versão>\` →
  aplicar no jogo-base, quando houver (cópia com backup e desinstalação limpa).
- Jogo-base: detectar pelas bibliotecas da Steam (`libraryfolders.vdf`) e pelos caminhos comuns; conferir
  a versão pelo hash do executável declarado no manifesto.
- **Catálogo** `party-anywhere/catalog`: repositório com `index.json` dos jogos verificados (id, URL do
  repositório, versão revisada, hash). Entrar no catálogo = PR revisado por mantenedores.
- Atualizações de jogos: o Hub compara a versão instalada com o último release (e, para jogos
  verificados, só oferece versões revisadas).

## Atualização do Hub

- O Hub consulta o último release de `joaocaioo/party-anywhere` ao abrir e a cada 6 h, com ETag (sem
  novidade, a consulta nem conta no limite do GitHub). Versão nova: o ícone de atualização avisa; um clique
  baixa `PartyAnywhere-X.Y.Z-setup.exe` (só de `https://github.com/`), abre o instalador e fecha o Hub.
  Nunca durante uma partida, nunca sozinho.
- Os jogos instalados por link são conferidos no mesmo ciclo; a novidade aparece como aviso com "Atualizar".
- O release sai do workflow `.github/workflows/release.yml` a cada tag `vX.Y.Z` (instalador do Hub e a
  ferramenta `pa`).
- **Antes da versão pública**: assinar o instalador (Authenticode) ou migrar para o updater do Tauri com
  assinatura minisign. Hoje a confiança é só o HTTPS do GitHub (ver Riscos).
- O protocolo do celular e a ABI do SDK são **versionados**: um Hub novo continua falando com jogos de
  SDK antigo (compatibilidade para trás por pelo menos uma versão maior).

## Segurança

- Instalar jogo por link é **executar código de terceiros** (mods injetam DLL). Mitigações: selo
  Verificado/Da comunidade, hash obrigatório, nada roda durante a instalação (sem scripts de instalação
  no início), pasta isolada por jogo, desinstalação limpa.
- O servidor do celular escuta só em redes privadas (perfil de rede do Windows) e só aceita celulares
  novos com o lobby aberto. Tokens aleatórios de 128 bits; nada sai da rede local.
- IPC: o pipe aceita só processos do mesmo usuário (ACL padrão do pipe local).
- Nenhuma telemetria sem consentimento explícito.

## Estrutura do repositório

```
party-anywhere/
  apps/hub/
    ui/               UI do Hub (HTML/JS, sem build): no navegador roda em modo demo; no app, bridge.js
                      liga a UI ao núcleo Rust. vendor/ = Three.js, QR e fontes locais (sem internet)
    src-tauri/        app desktop (Tauri 2): lobby, celulares, controles, biblioteca, atualizações
    tools/            vendor_ui.py (atualiza vendor/)
  crates/
    pa-core/          Pad, vagas e regras do lobby (puro, testado)
    pa-phone/         servidor do celular (HTTP + WebSocket) e página do controle
    pa-virtualpad/    controles virtuais (ViGEmBus) atrás de uma interface
    pa-manifest/      party-anywhere.json e party-anywhere.release.json (puro, testado)
    pa-library/       GitHub: prévia por link, Descobrir, instalação com hash, library.json, atualizações
    pa-cli/           `pa` para criadores (new, validate, pack, inspect) e servidor MCP (`pa mcp`)
  sdk/c/              party_anywhere.h (SDK, stb-style) e exemplo
  schema/             game-manifest.v1.json (esquema publicado)
  templates/game/     projeto modelo: manifesto, capa, workflow de release, .mcp.json, skill para agentes
  spikes/latency/     medidor de latência da fase 1
  docs/
```

## Riscos e questões em aberto

| Risco | Impacto | Plano |
|---|---|---|
| ViGEmBus descontinuado | Modo Universal pode quebrar numa atualização do Windows | Interface `VirtualPadSink`; acompanhar forks; avaliar driver próprio quando houver tração. |
| Wi-Fi ruim (economia de energia do celular, 2,4 GHz lotado) | Picos de latência | Medir no spike; WebRTC sem ordenação; mostrar latência na carta e dar dica de 5 GHz. |
| Isolamento de clientes (redes de convidados) | Celular não acha o PC | Detectar falha e explicar; futuro: relé via internet (opcional). |
| iOS exige HTTPS para giroscópio | Sem controle por movimento no iPhone | Em aberto: certificado curinga com DNS que resolve para IP local (exige serviço nosso). Fora do MVP. |
| Steam Input reemite controles | Controle duplicado ou ordem trocada | Detectar Steam rodando e orientar; testar com e sem. |
| Código de terceiros na Biblioteca | Malware | Selos, hash, catálogo revisado, sem scripts; avaliar assinatura de autores. |
| Teclas de teclados extras chegam ao jogo (Modo Universal) | Dois jogadores mexendo no mesmo personagem | MVP: teclados extras só no Modo SDK; no Universal, um teclado nativo. Depois: filtro de teclado opcional (avaliar licença e anti-cheat). |
| Instalador do Hub sem assinatura | Atualização adulterada se a conta do GitHub for comprometida; SmartScreen avisa "editor desconhecido" | Antes da versão pública: certificado de assinatura de código (Authenticode) e/ou updater do Tauri com minisign; a chave só no CI. |
| Pareamento BT varia por fabricante | "Não pareia" é o pior momento da UX | Tabela de quirks alimentada por testes reais com cada controle. |
