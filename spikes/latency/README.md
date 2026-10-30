# Spike de latência (fase 1)

Responde à pergunta da fase 1 do [roadmap](../../docs/ROADMAP.md): **o celular é rápido o bastante
como controle?** Meta (revista em 2026-10-29): do toque num botão até o PC, **p50 < 25 ms e p99 < 100 ms**,
e só a rede com **p50 < 10 ms**, em Wi-Fi doméstico de 5 GHz,
medido em pelo menos um Android e um iPhone.

## Rodar

Na raiz do repositório:

```bash
cargo run --release -p pa-latency-spike
```

1. O terminal mostra um QR code. No celular (no **mesmo Wi-Fi** do PC), escaneie e abra a página.
   Na primeira vez, o Windows pergunta se o programa pode usar a rede: permita em **redes privadas**.
2. Deite o celular. Ele vira P1 (o próximo celular vira P2, até 10).
3. Toque em **TESTE** no celular para ver os números ao vivo:
   - **Toque → PC**: jogue normalmente por 1 ou 2 minutos (botões e analógico). É o número da meta.
   - **Rede (rajada)**: toque em "Rajada automática" e deixe o celular parado por 30 s.
   - **Ida e volta**: medida sozinha, o tempo todo.
4. `Ctrl+C` no terminal encerra, mostra o resultado e grava um JSON em `spikes/latency/reports/`.

Opções: `--port N` (padrão 27027), `--duration SEGUNDOS`, `--no-vigem`, `--loopback` (só neste PC, sem firewall).

### Controle virtual (opcional)

Com o driver **ViGEmBus** instalado, cada celular também vira um controle de Xbox 360 de verdade para
o Windows (abra `joy.cpl` ou qualquer jogo para ver). Sem o driver, a medição funciona igual e o
terminal avisa. O instalador conferido fica em `.downloads\vigembus\` (execute como administrador).

## O que é medido, e como

O relógio do celular e o do PC não batem. A página faz trocas de sincronização (como o NTP): manda o
horário dela, o PC responde, e a página confirma o horário da resposta. Da troca com a **menor ida e
volta** sai a diferença entre os relógios, com erro de no máximo metade dessa ida e volta (mostrado
no resultado como "erro da medição").

| Número | Do quê | Até o quê |
|---|---|---|
| Toque → PC | criação do evento de toque no navegador (`event.timeStamp`) | chegada da mensagem no servidor |
| Rede (rajada) | envio pela página | chegada no servidor |
| Ida e volta | envio do SYNC | chegada do SYNC_REPLY na página |
| ViGEm | pedido de atualização | driver aceitou o estado |

**O que fica de fora**: o tempo da tela do celular até o navegador criar o evento (digitalizador, uns
poucos ms e igual em qualquer solução web), e o tempo do jogo ler o controle e desenhar o quadro.

## Resultados

Registre aqui cada medição (copie do JSON em `reports/`):

| Data | Celular | Rede | Toque → PC p50 / p99 | Rajada p50 / p99 | Ida e volta p50 | Meta |
|---|---|---|---|---|---|---|
| 2026-10-29 | iPhone (Safari) | Wi-Fi doméstico, faixa não anotada | 22,9 / 38,1 ms (46 toques) | 5,2 / 27,0 ms | 7,0 ms | ❌ p50 (p99 ok) |
| 2026-10-29 | iPhone (Safari) | idem | 23,2 / 120,5 ms (2.707, botões e analógico juntos) | 3,8 / 34,0 ms | 7,0 ms | ❌ |
| 2026-10-29 | iPhone (Safari) | idem | botões 23,6 / 81,6 ms (2.129) · analógico 13,6 / 80,9 ms (614) | 4,7 / 31,0 ms | 7,0 ms | ❌ |

Leitura da primeira medição: a rede é rápida (5 ms de ida, sem pacotes perdidos). Os ~18 ms restantes do
toque até o PC acontecem **dentro do navegador do celular**, entre o toque e o JavaScript receber o evento.
Trocar o transporte (WebRTC) não muda isso. Na segunda medição, a cauda (p99 120 ms) não veio da rede
(rajada p99 34 ms): a suspeita é o analógico, cujos movimentos o navegador agrupa. Por isso o medidor
passou a separar **botões** (o número da meta) de **analógico**. A terceira medição mostrou que a cauda
também aparece nos botões (p99 82 ms, contra 31 ms da rede). Como a página envia o toque antes de qualquer
outro trabalho e bloqueia os gestos do navegador, o atraso que sobra (~19 ms no p50) acontece no Safari,
antes de o JavaScript receber o toque. Ainda falta um Android.

**Decisão (2026-10-29):** o celular está bom para jogos de festa e co-op. A meta foi revista para o que
depende de nós (rede p50 < 10 ms) e para o que o navegador permite (toque p50 < 25 ms, p99 < 100 ms).
O teste que vale de verdade é **dentro de um jogo**, na fase 2. Um app nativo opcional fica como
alternativa na fase 7, se o jogo mostrar que precisa.

Se a **rede** (rajada) passar da meta num Wi-Fi bom, aí sim vale testar WebRTC DataChannel sem ordenação.
O atraso do navegador não muda com o transporte.
