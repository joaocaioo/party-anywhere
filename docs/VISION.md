# Party Anywhere: visão do produto

> **Qualquer jogo. Qualquer controle. Até 10 pessoas no mesmo sofá.**

## O problema

Juntar gente para jogar no PC é chato:

- Falta controle para todo mundo, e celular todo mundo tem.
- Parear controle Bluetooth no Windows exige abrir as Configurações, procurar numa lista
  cheia de fones e teclados, adivinhar qual é o "Wireless Controller" e torcer.
- Cada jogo co-op caseiro (mod de splitscreen, jogo indie) reinventa a parte de controles:
  detectar dispositivos, decidir quem é P1 e P2, reconectar, celular como controle.
- Quem cria um mod co-op não tem onde publicar de um jeito que outra pessoa instale em um clique.

## A resposta

Party Anywhere tem três peças, nesta ordem de importância:

1. **O Hub**: um app para Windows, sempre igual (mesmo visual, mesmo fluxo), que reúne
   todos os controles num lobby de até 10 jogadores. Funciona sozinho.
2. **O SDK**: uma biblioteca pequena que qualquer jogo co-op (mod, jogo próprio) usa para
   ler os jogadores do Hub. Quem cria um jogo não escreve uma linha de código de controle.
3. **A Biblioteca**: a aba de jogos do Hub. Cola o link de um repositório, o jogo aparece
   com capa e fica pronto para jogar. Os melhores entram no catálogo oficial.

```
   Controle USB / Bluetooth ─┐
   Celular (QR code) ────────┼──►  HUB  ──►  Modo Universal: qualquer jogo vê controles de Xbox
   Teclado e mouse ──────────┘   lobby P1..P10
                                     └────►  Modo SDK: jogos co-op leem os jogadores direto do Hub
```

## Para quem

| Pessoa | O que quer | Peça principal |
|---|---|---|
| **Quem joga** | Abrir, conectar todo mundo e jogar. Não quer ler nada. | Hub |
| **Quem cria** | Fazer um jogo ou mod co-op sem cuidar de controles. Publicar fácil. | SDK + manifesto + skills de agente |
| **Quem cuida do catálogo** | Revisar e destacar os jogos bons, com segurança. | Catálogo |

## Casos de uso (e qual modo resolve)

| Situação | Como o Party Anywhere resolve |
|---|---|
| "Quero jogar com controle, mas não tenho." | Escaneia o QR: o celular vira um controle de Xbox para **qualquer** jogo (Modo Universal). |
| "O jogo já tem co-op local, mas só tenho um controle." | Os amigos escaneiam o QR e viram P2, P3, P4 (Modo Universal). |
| "Comprei um controle Bluetooth." | "Adicionar jogador" → "Controle Bluetooth": o Hub acha só controles, diz como colocar aquele modelo em modo de pareamento e pareia sozinho. |
| "Quero jogar o mod de splitscreen que alguém fez, com 8 pessoas." | Cola o link do repositório na Biblioteca, instala, joga. O mod lê os 8 jogadores pelo SDK. |
| "Quero fazer co-op no meu jogo favorito." | Clona o repo, pede ao seu agente de IA (as skills ensinam o SDK), publica com um `party-anywhere.json` e uma capa. |

## Princípios de UX (valem mais que qualquer feature)

1. **Zero configuração.** Conectou, apareceu. Nada de "mapear botões" para começar a jogar.
2. **De pegar o celular a estar no lobby: menos de 10 segundos.** Para controle já pareado: menos de 1 segundo.
3. **Todo dispositivo responde.** Ao entrar, o controle vibra e a luz dele assume a cor do jogador
   (quando o controle tem luz); o celular mostra o número e a cor. Ninguém pergunta "qual sou eu?".
4. **Cor nunca sozinha.** Sempre cor **e** número (P1..P10), por causa de daltonismo.
5. **Feito para a TV.** O Hub inteiro se navega pelo controle, lido a 3 metros de distância. Mouse é opcional.
6. **Sem jargão.** Nada de "XInput", "HID", "slot", "porta". Escreva "controle", "celular", "jogador".
7. **Nenhum erro sem saída.** Toda mensagem de erro diz o que fazer, com um botão para fazer.
8. **Não interromper.** Atualização, aviso, download: nunca no meio de uma partida.
9. **Seguro por padrão.** Jogo instalado por link mostra claramente se foi verificado ou não.

## O que NÃO é (por enquanto)

- Não é um launcher genérico de múltiplas instâncias. Quando um mod precisa de várias cópias do jogo (um
  jogador por cópia, pelo multiplayer do próprio jogo), o Hub dá esse suporte para aquele mod (ver
  MOD-COOP-LOCAL.md), sem contornar DRM nem anti-cheat.
- Não contorna anti-cheat nem entra em serviços online de terceiros. GTA Online e afins estão fora.
- Não distribui arquivos de jogos. Mods exigem a cópia do jogo que o usuário já tem.
- Não é multiplataforma no início: Windows primeiro; Linux/Steam Deck depois.

## Como saber que deu certo

- Uma pessoa que nunca viu o Hub coloca 4 amigos (2 celulares, 1 controle Bluetooth novo,
  1 controle USB) no lobby em **menos de 1 minuto**, sem ajuda.
- Latência do celular numa rede Wi-Fi doméstica de 5 GHz: rede **p50 abaixo de 10 ms**; do toque num botão
  ao PC, **p50 abaixo de 25 ms e p99 abaixo de 100 ms** (o resto é do navegador do celular). E, no fim,
  ninguém sentir atraso jogando.
- Quem cria um jogo vai de "clonei o repo" a "meu jogo lê 10 jogadores" em **uma tarde**.

Os detalhes estão em [UX.md](UX.md), [ARCHITECTURE.md](ARCHITECTURE.md), [SDK.md](SDK.md),
[GAME-MANIFEST.md](GAME-MANIFEST.md) e [ROADMAP.md](ROADMAP.md).
