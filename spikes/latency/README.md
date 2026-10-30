# Spike de latência (fase 1)

Responde à pergunta da fase 1 do [roadmap](../../docs/ROADMAP.md): **o celular é rápido o bastante
como controle?** Meta: do toque até o PC, **p50 < 20 ms e p99 < 50 ms**, em Wi-Fi doméstico de 5 GHz,
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
| | | | | | | |

Se a meta não for batida em Wi-Fi bom, o próximo passo do roadmap é testar WebRTC DataChannel sem
ordenação antes de seguir para a fase 2.
