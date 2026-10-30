/*
 * Arena no console: até 10 jogadores, cada um uma letra colorida que anda pela tela. Nenhuma linha de
 * código de controle: celulares, controles e teclados chegam pelo Hub.
 *
 *   cl /O2 arena_console.c          (ou: gcc -O2 arena_console.c -o arena_console.exe)
 *
 * Abra o Party Anywhere, rode o exemplo e entre com o celular pelo QR (o endereço aparece aqui também).
 */
#define PARTY_ANYWHERE_IMPLEMENTATION
#include "../party_anywhere.h"

#include <stdio.h>

#define W 60
#define H 18

typedef struct { float x, y; int alive; } player;

static player players[PA_MAX_SEATS];

static void spawn(int s) {
    players[s].x = (float)(5 + (s * 11) % (W - 10));
    players[s].y = (float)(3 + (s * 7) % (H - 6));
    players[s].alive = 1;
}

/* um passo do jogo; separado do main para o teste compilar a lógica */
void arena_step(pa_client* pa) {
    pa_event ev;
    int s;
    pa_poll(pa);
    while (pa_next_event(pa, &ev)) {
        if (ev.type == PA_EVENT_SEAT_JOINED || ev.type == PA_EVENT_SEAT_RESUMED) {
            if (ev.type == PA_EVENT_SEAT_JOINED) spawn(ev.seat);
            pa_toast(pa, ev.seat, "Você está na arena!");
            pa_rumble(pa, ev.seat, 0.3f, 0.3f, 150);
        } else if (ev.type == PA_EVENT_SEAT_LEFT) {
            players[ev.seat].alive = 0;
        }
    }
    for (s = 0; s < PA_MAX_SEATS; ++s) {
        pa_pad pad;
        if (!players[s].alive || !pa_get_pad(pa, s, &pad)) continue; /* reconectando: parado e protegido */
        players[s].x += pad.lx * 0.9f;
        players[s].y -= pad.ly * 0.5f;
        if (pad.pressed & PA_BTN_A) pa_rumble(pa, s, 0.8f, 0.8f, 80);
        if (players[s].x < 1) players[s].x = 1;
        if (players[s].x > W - 2) players[s].x = W - 2;
        if (players[s].y < 1) players[s].y = 1;
        if (players[s].y > H - 2) players[s].y = H - 2;
    }
}

#ifndef PA_EXAMPLE_NO_MAIN
static void draw(pa_client* pa) {
    char grid[H][W + 1];
    int x, y, s;
    for (y = 0; y < H; ++y) {
        for (x = 0; x < W; ++x) grid[y][x] = (y == 0 || y == H - 1 || x == 0 || x == W - 1) ? '#' : ' ';
        grid[y][W] = 0;
    }
    for (s = 0; s < PA_MAX_SEATS; ++s) {
        if (players[s].alive) grid[(int)players[s].y][(int)players[s].x] = (char)('1' + (s == 9 ? -1 : s));
    }
    printf("\x1b[H");
    for (y = 0; y < H; ++y) puts(grid[y]);
    printf("Jogadores: %d   Entre pelo celular: %s        \n", pa_active_seat_count(pa), pa_join_url(pa));
}

int main(void) {
    pa_client* pa = NULL;
    pa_game_info info = { sizeof(pa_game_info), "com.party-anywhere.arena-console", "Arena (exemplo)", 1, 10 };
    pa_result r = pa_connect(&info, &pa);
    if (r != PA_OK) {
        printf(r == PA_ERR_HUB_NOT_RUNNING ? "Abra o Party Anywhere e rode de novo.\n" : "Não deu para conectar ao Hub (%d).\n", r);
        return 1;
    }
    pa_set_accepting_players(pa, true);
    printf("\x1b[2J");
    for (;;) {
        arena_step(pa);
        draw(pa);
        if (!pa_is_connected(pa)) { printf("O Hub fechou.\n"); break; }
        Sleep(16);
    }
    pa_disconnect(pa);
    return 0;
}
#endif
