# Party Anywhere SDK (ABI 1)

Para quem cria jogos e mods co-op. Com o SDK, o seu jogo **não tem código de controle**: o Hub
descobre controles e celulares, organiza o lobby (P1..P10) e reconecta quem cai. O jogo só lê.

> O cabeçalho é [`sdk/c/party_anywhere.h`](../sdk/c/party_anywhere.h) (o `pa new` já coloca ele no projeto).
> Exemplo completo: [`sdk/c/examples/arena_console.c`](../sdk/c/examples/arena_console.c), uma arena no
> console para até 10 jogadores. Ele compila a cada build do Hub, e os testes de `crates/pa-sdk` conversam
> com o cabeçalho compilado pela mesma memória do Hub.

## Por que um cabeçalho C

- Um arquivo, `party_anywhere.h`, no estilo stb: inclua em qualquer lugar e, em **um** arquivo `.c/.cpp`,
  defina `PARTY_ANYWHERE_IMPLEMENTATION` antes do include.
- ABI C: funciona em C e C++ e, por FFI, em C#, Lua, Rust, Godot (GDExtension) e Unity.
- Compila em **x86 e x64** (mods de jogos antigos são 32 bits). Só depende da API do Windows.
- Sem DLL para distribuir, sem thread escondida, sem alocação depois do `pa_connect`.

## Exemplo mínimo

```c
#define PARTY_ANYWHERE_IMPLEMENTATION
#include "party_anywhere.h"

pa_client* pa = NULL;

void game_start(void) {
    pa_game_info info = {
        .struct_size = sizeof info,
        .game_id     = "com.exemplo.arena-blast",   // o mesmo id do party-anywhere.json
        .game_name   = "Arena Blast",
        .min_players = 1,
        .max_players = 10,
    };
    if (pa_connect(&info, &pa) != PA_OK) {
        // Hub fechado ou versão incompatível: avise o jogador ou use controles próprios. As outras funções
        // aceitam pa nulo e não fazem nada. Hub fechado: tente pa_connect de novo a cada segundo ou dois.
    }
    pa_set_accepting_players(pa, true);        // lobby aberto: celulares novos podem entrar
}

void game_frame(void) {
    pa_poll(pa);                               // uma vez por frame, na thread do jogo

    pa_event ev;
    while (pa_next_event(pa, &ev)) {
        switch (ev.type) {
        case PA_EVENT_SEAT_JOINED:       spawn_player(ev.seat);      break;
        case PA_EVENT_SEAT_RECONNECTING: pause_player(ev.seat);      break;
        case PA_EVENT_SEAT_RESUMED:      resume_player(ev.seat);     break;
        case PA_EVENT_SEAT_LEFT:         despawn_player(ev.seat);    break;
        case PA_EVENT_SEAT_RENAMED:      update_name(ev.seat);       break;
        case PA_EVENT_HUB_LOST:          show_hub_closed_message();  break;
        default: break;
        }
    }

    for (int s = 0; s < PA_MAX_SEATS; ++s) {
        pa_pad pad;
        if (pa_get_pad(pa, s, &pad)) {
            move_player(s, pad.lx, pad.ly);
            if (pad.pressed & PA_BTN_A) jump(s);  // "pressed" = apertou neste frame
        }
    }
}

void game_end(void) { pa_disconnect(pa); }
```

## API

### Conexão

```c
pa_result pa_connect(const pa_game_info* info, pa_client** out);
void      pa_disconnect(pa_client* pa);
void      pa_poll(pa_client* pa);              // lê eventos do pipe e atualiza "pressed/released"
bool      pa_is_connected(const pa_client* pa);
const char* pa_join_url(const pa_client* pa);  // URL do QR, para o jogo desenhar o próprio lobby
```

`pa_result`: `PA_OK`, `PA_ERR_HUB_NOT_RUNNING`, `PA_ERR_VERSION`, `PA_ERR_BUSY` (outro jogo já está
conectado), `PA_ERR_INVALID_ARG`.

### Vagas (jogadores)

```c
#define PA_MAX_SEATS 10

typedef struct pa_seat_info {
    uint8_t     index;          // 0..9 → P1..P10
    pa_seat_state state;        // PA_SEAT_EMPTY | PA_SEAT_ACTIVE | PA_SEAT_RECONNECTING
    pa_device_kind device;      // PA_DEVICE_GAMEPAD | PA_DEVICE_PHONE | PA_DEVICE_KEYBOARD
    uint8_t     r, g, b;        // cor do jogador (fixa por índice)
    char        name[32];       // UTF-8, ex.: "João"
} pa_seat_info;

bool pa_get_seat(const pa_client* pa, int seat, pa_seat_info* out);
int  pa_active_seat_count(const pa_client* pa);
```

### Estado do controle

```c
enum {
    PA_BTN_A = 1u << 0,  PA_BTN_B = 1u << 1,  PA_BTN_X = 1u << 2,  PA_BTN_Y = 1u << 3,
    PA_BTN_LB = 1u << 4, PA_BTN_RB = 1u << 5, PA_BTN_BACK = 1u << 6, PA_BTN_START = 1u << 7,
    PA_BTN_LS = 1u << 8, PA_BTN_RS = 1u << 9,
    PA_BTN_UP = 1u << 10, PA_BTN_DOWN = 1u << 11, PA_BTN_LEFT = 1u << 12, PA_BTN_RIGHT = 1u << 13,
    PA_BTN_GUIDE = 1u << 14,
};

typedef struct pa_pad {
    uint32_t buttons;           // seguros agora
    uint32_t pressed;           // apertados desde o pa_poll anterior
    uint32_t released;          // soltos desde o pa_poll anterior
    float lx, ly, rx, ry;       // -1..1, direita e cima positivos, zona morta aplicada
    float lt, rt;               // 0..1
} pa_pad;

bool pa_get_pad(const pa_client* pa, int seat, pa_pad* out);  // false (e zerado) se a vaga não está ativa
```

Regras:
- Sempre o formato de controle de Xbox, venha de um DualSense, de um Switch Pro ou de um celular.
- Vaga em `RECONNECTING` devolve `false`: o jogo recebe controle neutro, nunca o último estado preso.

### Do jogo para o jogador

```c
void pa_set_accepting_players(pa_client* pa, bool accepting);
void pa_set_layout(pa_client* pa, int seat, pa_layout layout);                 // GAMEPAD | SIMPLE | DRIVING | MENU
void pa_set_label(pa_client* pa, int seat, uint32_t button, const char* label); // "Correr", "Atirar"; "" tira
void pa_set_mode(pa_client* pa, int seat, int mode);                           // modo do controle: 0 = "A pé"
bool pa_take_look(pa_client* pa, int seat, float* dx_deg, float* dy_deg);     // câmera por deslocamento (abaixo)
void pa_toast(pa_client* pa, int seat, const char* text);                      // aviso curto no celular
void pa_rumble(pa_client* pa, int seat, float low, float high, int duration_ms);
void pa_set_light(pa_client* pa, int seat, uint8_t r, uint8_t g, uint8_t b);   // luz do controle, se houver
```

`seat = PA_ALL_SEATS` (-1) aplica a todos. `pa_set_accepting_players` repetido com o mesmo valor não gera
tráfego.

| Chamada | O que o Hub faz hoje |
|---|---|
| `pa_rumble` | Vibra o celular e o controle da vaga. |
| `pa_toast` | Mostra o aviso no topo do celular. |
| `pa_set_mode` | Troca o modo do controle do celular (os modos do controle do mod, na ordem da ficha: 0 = "A pé", 1 = "Veículo"). Use quando o jogo muda: entrou no carro, saiu do carro. O jogador ainda pode trocar pelo topo do celular. |
| `pa_set_label` | Põe o nome da ação no botão do celular ("Drift"); `""` tira. `button` é um `PA_BTN_*` (os direcionais dividem um nome só) ou, para gatilhos e analógicos, `PA_LABEL_LT`, `PA_LABEL_RT`, `PA_LABEL_STICK_L`, `PA_LABEL_STICK_R`. Até 12 letras. |
| `pa_set_layout` | `PA_LAYOUT_GAMEPAD` (completo) ou `PA_LAYOUT_SIMPLE` no celular da vaga, no lugar do "Padrão do jogo". `DRIVING` e `MENU` estão reservados: para "dirigindo", use os modos do controle do mod e `pa_set_mode`. Se o jogador escolhe outro layout no menu do celular, vale o dele. |
| `pa_set_accepting_players`, `pa_set_light` | Chegam ao Hub, mas ainda não fazem nada. |

O que o jogo pede para o celular vale enquanto ele está conectado, inclusive para quem cai e volta; quando o
jogo desconecta, cada celular volta ao layout de antes. `pa_set_mode` e `PA_LABEL_*` entraram depois na ABI
1, sem mudar nada do que existia: um Hub mais velho só ignora.

### Câmera pelo celular (arrastar o dedo)

Nos jogos de celular, a câmera gira arrastando o dedo na metade direita da tela: é deslocamento (como o
mouse), não velocidade (como o analógico). O Party Anywhere entrega assim:

```c
float dx, dy;
if (pa_take_look(pa, s, &dx, &dy)) {   // graus desde a última chamada, direita e para cima positivos
    yaw += dx; pitch += dy;            // como o mouse; o analógico direito continua valendo para o controle
}
```

- Chamar `pa_take_look` já avisa o Hub: a partir daí, a metade direita do celular vira área de câmera (e
  segurar um botão da direita e arrastar também gira, para atirar e mirar ao mesmo tempo). Sem chamar, o
  celular mostra o analógico direito, como antes.
- A sensibilidade é do jogador (menu do celular: Devagar, Normal, Rápida); o jogo só soma os graus.
- Aditivo na ABI 1: com um Hub mais velho, `pa_take_look` devolve sempre `false`.
- Giroscópio: ainda não. Os navegadores só dão os sensores de movimento em HTTPS, e o controle abre por
  `http://` na rede local.

### Tela do jogo no celular (todo jogo oferece; o jogador liga)

Todo jogo e mod do Party Anywhere deve oferecer: **quem liga "Tela do jogo no celular" no menu do celular vê
a própria parte do jogo nele, e ela sai da TV.** Vem **desligada**: só quem quer liga, e a escolha fica
guardada naquele celular.

```c
bool pa_view_wanted(pa_client* pa, int seat);
void pa_view_size(pa_client* pa, int seat, int* width, int* height);
bool pa_submit_view(pa_client* pa, int seat, const void* bgra, int width, int height, int stride_bytes);
```

A cada frame, para cada vaga:

```c
if (pa_view_wanted(pa, s)) {
    int w, h;
    pa_view_size(pa, s, &w, &h);       // o formato da tela do celular: 540 de altura (ex.: 1170x540)
    // 1. não desenhe a parte deste jogador na TV (a tela dividida fica para os outros)
    // 2. desenhe a visão dele num alvo à parte de w x h, leia os pixels (BGRA) e entregue
    pa_submit_view(pa, s, pixels, w, h, w * 4);
}
```

- `pa_view_wanted` é `false` sem o Hub, sem celular ou com a tela desligada: o jogo segue como sempre.
- **Desenhe no tamanho de `pa_view_size`:** é o formato da tela daquele celular deitado, e a imagem enche a
  tela sem faixas (16:9 se o Hub não souber). Muda se o celular girar: confira a cada quadro.
- `stride_bytes` negativo vale: imagem de baixo para cima (como a leitura da placa de vídeo costuma devolver),
  com `bgra` apontando a última linha.
- Até 1280x720. Tamanhos menores chegam mais rápido.
- Entregue a cada frame ou a cada 2: o Hub manda para o celular no ritmo que a rede aguenta (até 30 por
  segundo) e sempre o quadro mais novo, sem fila.
- Ler os pixels da placa de vídeo tem custo: use um alvo pequeno e leitura assíncrona (PBO no OpenGL,
  staging texture no Direct3D, `AsyncGPUReadback` no Unity: `sdk/csharp/Unity/PhoneViews.cs` já faz).
- O exemplo [`arena_console.c`](../sdk/c/examples/arena_console.c) faz tudo isso: desenha a arena em volta
  de quem está no celular e tira essa pessoa da tela principal.

## C# (Unity, .NET, Godot C#)

`sdk/csharp/PartyAnywhere.cs`: o mesmo SDK, um arquivo só, sem DLL nativa (lê a mesma memória). Copie para o
projeto e permita código `unsafe`. `Client.Connect`, `Poll`, `NextEvent`, `GetPad`, `TakeLook`, `ViewWanted`,
`ViewSize`, `SubmitView` e o resto, com os mesmos nomes do C. Compila em C# 7.3 / .NET Standard 2.0 (o Unity
antigo). Para a tela no celular num jogo Unity, `sdk/csharp/Unity/PhoneViews.cs`. O `pa new` já traz os dois.

## Como funciona por dentro

A tela do jogo no celular usa uma memória por vaga, `Local\PartyAnywhere.v1.view.<vaga>` (cabeçalho
`pa_view_header` de 64 bytes + os pixels), que o Hub cria quando o celular liga a tela e marca `wanted`; o Hub
também escreve ali o formato da tela do celular (`aspect_milli`). O jogo escreve com seqlock e o instante do
quadro (QueryPerformanceCounter); é aditivo, a ABI continua 1.

A câmera por deslocamento fica no fim da memória principal (`look`, por vaga, em milésimos de grau): o Hub
soma, o jogo troca por zero (`InterlockedExchange`). O jogo marca `game_caps` ao chamar `pa_take_look`, e o
Hub avisa os celulares. Um Hub mais velho tem a memória menor, sem `look`: o SDK confere o tamanho.

Memória compartilhada `Local\PartyAnywhere.v1`, criada pelo Hub (layout `pa_shm` no cabeçalho). Cada vaga
tem um seqlock: o jogo nunca lê um estado pela metade e nunca trava. Os eventos saem da comparação de um
`pa_poll` com o anterior. O layout exato está no próprio `party_anywhere.h`.

## Garantias do Hub

- `pa_get_pad` não faz chamada de sistema: lê memória compartilhada (custo de nanossegundos).
- A ordem dos eventos é a ordem real; `SEAT_JOINED` sempre vem antes do primeiro `pa_get_pad` válido daquela vaga.
- O índice da vaga é estável durante a sessão. P3 continua P3 depois de cair e voltar.
- Enquanto o jogo está conectado, o Hub desliga os controles virtuais do Modo Universal (sem entrada dupla).
- Um jogo por vez (`PA_ERR_BUSY` para o segundo). Se o jogo fechar sem `pa_disconnect`, o Hub libera sozinho.
- Hub fechado: `PA_EVENT_HUB_LOST` em até 1,5 s, e os pads ficam neutros.
- ABI versionada: um Hub novo aceita jogos com SDK da mesma versão maior.

## Integração por tipo de projeto

| Projeto | Como usar |
|---|---|
| Jogo em C/C++ | Inclua o cabeçalho. |
| Mod por DLL/ASI (ex.: GTA SA com plugin-sdk) | Inclua no plugin e chame `pa_poll` no mesmo ponto em que o jogo lê os pads. |
| Unity / BepInEx | Binding C# (`PartyAnywhere.cs`, previsto) por `DllImport` de uma DLL fina gerada do cabeçalho. |
| Godot | GDExtension fina (prevista). |
| Lua (REFramework, UE4SS) | FFI sobre a DLL fina. |

## Publicar

Junto do código, o repositório precisa de um `party-anywhere.json` e de uma capa. Ver
[GAME-MANIFEST.md](GAME-MANIFEST.md).
