# Transformar o multiplayer online de um jogo em co-op local

Guia para quem faz mods (e para os agentes de IA que ajudam): como pegar um jogo que tem **multiplayer
online** e fazê-lo rodar com **todo mundo no mesmo PC**, sem perder nada do que o online tem, e como **passar
do limite de jogadores** do jogo (1 → 4, 4 → 8).

> **A regra de ouro: não reescreva o multiplayer, reaproveite.** O jogo já sabe ter vários jogadores: os
> personagens, a sincronia, os modos, o progresso de cada um. O mod troca só o que muda no local: de onde
> vem o controle de cada jogador, onde a imagem de cada um aparece e quem é o "servidor".

## 1. Escolha o caminho

| | **A. Um jogo, vários jogadores dentro dele** | **B. Várias cópias do jogo, uma por jogador** |
|---|---|---|
| Como | O mod cria os jogadores extras dentro de uma única cópia do jogo, usando o código de multiplayer que já existe (os "jogadores remotos" passam a ser locais), e desenha uma visão por jogador (tela dividida). | Uma cópia do jogo por jogador, todas no mesmo PC: uma hospeda a partida, as outras entram pelo multiplayer do próprio jogo (LAN ou conexão direta em `127.0.0.1`). Cada janela ocupa a sua parte da tela. |
| Jogabilidade preservada | Alta, se o mod usa as entidades e a lógica de rede do jogo (não "imita" um jogador). | **Total**: é o multiplayer do próprio jogo, sem mudança de regra. |
| Custo no PC | Uma cópia do jogo e N visões. Leve. | N cópias do jogo inteiras (memória e processador × N). Pesado acima de 4. |
| Dificuldade | Alta: engenharia reversa do jogo (onde ficam os jogadores, a câmera, o HUD). | Média: o difícil é isolar as cópias (salvamento, configuração, controle de cada uma) e posicionar as janelas. |
| Bom para | Jogos em que um mod já consegue criar personagens (ex.: mundo aberto com scripts) e PCs modestos. | Jogos com LAN, conexão direta ou servidor dedicado local, e quando "igual ao online" é a prioridade. |
| Limites | Precisa achar e chamar o código certo do jogo; cada jogo é um projeto. | Jogos que exigem uma conta online por jogador (Steam, Epic) para entrar na partida precisam de **uma conta com o jogo por pessoa**: o mod não contorna isso. |

**Regra de decisão:** o jogo aceita partida por LAN, IP direto ou servidor local **sem** exigir uma conta
por jogador? → comece pelo **B** (preserva tudo, sai mais rápido). Senão, ou se o PC não aguenta N cópias →
**A**.

Nos dois caminhos: **nada de anti-cheat** (jogos com anti-cheat ficam de fora) e **nada de contornar a
proteção da loja** (DRM, conta, propriedade do jogo). Cada pessoa usa a sua cópia.

## 2. O que fica igual e o que muda

| Fica igual (é do jogo) | Muda no mod |
|---|---|
| Regras, modos, inimigos, física, progresso, itens, chat de eventos | **Entrada**: cada jogador lê o próprio controle pelo SDK (`pa_get_pad(pa, vaga)`), não o "controle 1" do Windows |
| A sincronia entre jogadores (no A: as entidades de rede do jogo; no B: a rede de verdade, em `localhost`) | **Imagem**: uma visão por jogador (tela dividida na TV) e, para quem está no celular, a visão dele vai para o celular (`pa_view_wanted` / `pa_submit_view`) e sai da TV |
| Salvamento de cada jogador, quando o jogo já separa por jogador | **Quem é quem**: nome e cor da vaga (`pa_get_seat`) no lugar do nome da conta online |
| | **Entrar e sair**: os eventos do SDK (`PA_EVENT_SEAT_JOINED`, `RECONNECTING`, `LEFT`) no lugar do "entrar na sala" online |
| | **O controle do celular acompanha o jogo**: `pa_set_mode` ao entrar e sair do carro, `pa_set_label` com o nome de cada ação |

**Desligue a leitura de controle e teclado do próprio jogo** para os jogadores do mod. O Windows continua
entregando o teclado (e o controle 1) ao jogo: sem isso, um teclado que é o P3 no Party Anywhere mexe no P1
do jogo, e o P1 recebe a mesma entrada duas vezes.

**Checklist "não perdi nada":** antes de publicar, jogue a mesma partida no online original e no local e
confira: todos os modos de jogo, entrar e sair no meio, morrer e voltar, salvar e carregar, trocar de mapa,
o HUD de cada jogador, o som (de quem é cada som), a pausa (no online o jogo nem pausa: o local também não
deve travar os outros).

## 3. Passar do limite de jogadores

É o ponto que mais quebra. O limite raramente está num lugar só: **ache todos antes de mudar um**.

### Onde o limite costuma estar

| Lugar | Como aparece | O que fazer |
|---|---|---|
| Constante de máximo | `MAX_PLAYERS = 4`, comparações `< 4`, laços `for (i = 0; i < 4; ...)` | Ache todas as comparações e laços com o número (procure pelo valor e pelos múltiplos dele). |
| Vetores de tamanho fixo | Arrays de jogadores, de câmeras, de placares, de inventários, com 4 posições | Realoque para o novo tamanho e redirecione **todas** as referências (senão o 5º jogador escreve na memória de outra coisa e o jogo trava longe dali). |
| Sessão de rede | "Sala cheia" com 4; o servidor recusa o 5º | No caminho B, quase sempre é configuração do servidor (`maxplayers`); no A, é o mesmo vetor/constante. |
| Interface | HUD, placar, lista de jogadores e seleção de personagem com 4 espaços | Desenhe os extras (ou uma versão compacta); a interface é o que a pessoa vê quebrar primeiro. |
| Pontos de nascer e câmeras | 4 pontos de nascer, 4 câmeras | Gere novos a partir dos existentes (ao lado de um ponto válido). |
| Controles do Windows | XInput vê no máximo **4** controles | Pelo SDK não há esse limite: até **10** jogadores com `pa_get_pad`. É mais um motivo para o mod ler o SDK, não o XInput. |
| Equilíbrio | Inimigos e recompensas escalados para 4 | Escale a dificuldade pelo número de jogadores (o próprio jogo costuma ter a fórmula para 1 a 4: estenda). |
| Desempenho | Cada visão a mais é a cena desenhada de novo | Veja abaixo. |

### Método (do mais seguro para o mais arriscado)

1. **Meça o limite de verdade:** suba de 1 em 1 (5, depois 6...) e anote o primeiro sintoma (não entra,
   HUD some, trava). Cada sintoma aponta um dos lugares da tabela.
2. **Mude um lugar por vez** e teste entrar, sair, morrer e salvar com o novo número.
3. **Teste no limite:** com o máximo novo, todos ao mesmo tempo, uma partida inteira.
4. Na ficha do mod (`players.max`), coloque o máximo que funcionou de verdade, não o desejado.

### Desempenho com mais jogadores

- **Tela dividida:** cada visão menor custa menos; use a resolução da parte da tela, não a da TV inteira.
- **Celular:** quem joga pela tela do celular não precisa de visão na TV. A visão dele pode ser menor
  (640x360) e com menos detalhe.
- **Caminho B:** cada cópia é um jogo inteiro. Acima de 4, prefira o A, ou gráficos no mínimo nas cópias.
- Meça com o número máximo de jogadores, no PC mais fraco que você quer atender.

## 4. Especial: jogo de 1 jogador para vários

Sem multiplayer nenhum para reaproveitar, só o caminho A, criando os personagens extras. Comece pelo menor
passo que prova a ideia: um segundo personagem que anda com o controle da vaga 2, mesmo sem câmera própria.
Depois a câmera, depois o HUD, depois o resto.

## 5. Ferramentas para a parte do jogo

Achar no jogo onde ficam os jogadores, a câmera e o HUD é engenharia reversa: um depurador e um
desmontador (x64dbg, Ghidra, Cheat Engine para achar valores na memória) ou as ferramentas da engine
(Unity: BepInEx, dnSpy; Unreal: UE4SS), e as comunidades de mod do jogo, que costumam já ter mapeado
muito. Alguns desses programas têm servidor MCP, o que deixa o agente olhar o jogo junto com você. Só em
jogos sem anti-cheat, e sem distribuir nada do jogo original.

## 6. Ordem de trabalho sugerida (para o mod e para a IA)

1. Entrar no jogo: o mod carrega e manda um aviso para o celular (`pa_toast`).
2. Entrada: o jogador 1 do jogo passa a ler a vaga 1 do SDK.
3. Segundo jogador: entra, anda, interage (pelo caminho escolhido).
4. Imagem: tela dividida para 2; a tela no celular de quem está no celular.
5. Até o limite original do jogo, com o checklist "não perdi nada".
6. Passar do limite, com o método da seção 3.
7. Ficha, capa e controle do celular do mod; publicar (opcional).
