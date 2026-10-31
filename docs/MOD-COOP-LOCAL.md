# Transformar um jogo em co-op local: um jogo, N telas, N jogadores

Guia para quem faz mods (e para os agentes de IA que ajudam): como fazer um jogo do PC rodar com **várias
pessoas no mesmo PC**, inclusive jogos com multiplayer só online, e como **passar do limite de jogadores**
do jogo (1 → 4, 4 → 8).

> **O caminho é um só: uma cópia do jogo, com todos os jogadores dentro dela e uma visão por jogador.**
> O mod cria os jogadores extras como jogadores de verdade do jogo, e o próprio motor desenha a cena uma vez
> para cada um, cada visão no seu pedaço da tela. Não abra várias cópias do jogo.

## 1. Por que uma cópia só (e não uma cópia por jogador)

| | **Uma cópia, N visões** | Uma cópia por jogador (não use) |
|---|---|---|
| Simulação (física, IA, regras) | 1 vez | N vezes, e cada cópia é um mundo à parte sincronizado pela rede |
| Memória | o jogo uma vez + o que as visões a mais carregam | N jogos inteiros (texturas, modelos, áudio) |
| Desenho da cena | N vezes, mas cada visão é menor: o total de pixels é o da tela | N vezes, cada cópia com o próprio dispositivo de vídeo |
| Som | um mixer, sons do mesmo mundo | N jogos tocando ao mesmo tempo |
| Atraso entre jogadores | nenhum: é o mesmo mundo | a rede de cada cópia, mesmo no próprio PC |
| O que exige do jogo | um jeito de carregar o mod | LAN ou IP direto, rodar várias vezes no mesmo PC (lojas e jogos costumam bloquear), salvamento e configuração separados por cópia, cada janela lendo só o controle dela |

Abrir várias cópias parece mais fácil, mas quebra no que importa: cada cópia é um jogo inteiro na memória e
no processador (o primeiro mod que tentou esbarrou na memória logo nas primeiras cópias), o som vira bagunça e
cada janela luta pelo foco e pelo controle. Por isso o Party Anywhere não
abre cópias do jogo, e um mod que tenta isso está no caminho errado.

**E o multiplayer online do jogo?** Ele continua sendo útil, mas como código, não como rede: os jogadores
extras usam as mesmas entidades, armas, regras e progresso que o online usa para os jogadores remotos, só
que controlados no mesmo PC. A regra de ouro: **não reescreva o multiplayer, reaproveite o que o jogo já sabe
sobre "vários jogadores"**.

## 2. Funciona em qualquer jogo?

A **ideia** funciona em qualquer jogo: quase todo motor tem "o jogador local" e "a câmera" como algo único, e
desenha a cena a partir dessa câmera uma vez por quadro. O mod troca quem é "o jogador" e "a câmera" a cada
passo, e pede ao motor uma passada de desenho por jogador. O **código** é de cada jogo: o que muda é quanto
trabalho dá achar esses pontos.

| Tipo de jogo | Como costuma ser |
|---|---|
| **Unreal Engine (4 e 5)** | O motor já tem jogadores locais e tela dividida (um "jogador local" por pessoa, cada um com a sua câmera e o seu pedaço da tela). Muitos jogos só não usam isso. Com o UE4SS, o mod pode criar os jogadores locais extras e ligar a divisão. Quando o jogo não quebrou esse caminho, é o caso mais fácil. |
| **Unity** | Uma câmera por jogador com o retângulo dela na tela é nativo; o trabalho é criar o personagem extra e dar a cada um a sua câmera e a sua entrada. Mod por BepInEx ou MelonLoader. |
| **Motor próprio ou antigo** (DirectX 9/11, por exemplo) | Troca de contexto: por um instante, o motor enxerga o jogador extra como "o jogador" (foco, câmera, controle) e desenha a cena num pedaço da tela. Exige engenharia reversa; é o caso do mod de GTA San Andreas que serviu de referência para este guia (até 8 jogadores, numa cópia só). |

**Onde fica difícil (e o que fazer):**

- **Efeitos que usam o quadro anterior** (anti-serrilhado temporal, desfoque de movimento, reflexos em tela):
  cada visão precisa do próprio histórico, senão as imagens se misturam. Guarde o estado por visão ou
  desligue o efeito nas visões.
- **O mundo carregado em volta de um jogador só** (streaming de mapa, população, colisão, IA que dorme
  longe do jogador): estenda para o entorno de cada jogador. Sem isso, quem se afasta cai no vazio ou vê o
  mundo sumir.
- **Jogo pesado na placa de vídeo:** cada visão desenha a cena de novo (a quantidade de objetos multiplica,
  os pixels não). Veja "Desempenho" abaixo.

## 3. A receita (o que o mod de referência provou que funciona)

1. **Jogadores de verdade.** Os extras são o tipo de jogador do próprio jogo, não personagens comuns
   fingindo. Vida, armas, procurado, inventário e o que o jogo faz pelo jogador vêm de graça.
2. **Troca de contexto.** Em volta de tudo o que o motor faz "para o jogador" (processar o personagem e o
   veículo dele, a câmera dele, o desenho da visão dele), o mod põe aquele jogador no lugar do jogador
   principal e desfaz logo depois. Assim 2 jogadores e 8 jogadores passam pelo mesmo caminho.
3. **Uma passada de desenho por visão**, com o pipeline do próprio motor, num pedaço do quadro (viewport).
   O HUD de cada um sai sozinho, porque o motor desenha o HUD "do jogador em foco".
4. **Guarde e devolva o que uma visão suja.** Cada passada regrava valores globais que a próxima, ou a
   simulação do quadro seguinte, esperam ser do jogador principal: estado da câmera e do renderizador, luzes
   acumuladas, sombras, fade. Guarde antes, devolva depois.
5. **Cresça os limites com o número de visões.** Orçamento de streaming, listas de objetos instanciados,
   pools de entidades: com uma câmera cabem, com quatro em lugares diferentes uma visão apaga o que a outra
   carregou e o mundo pisca. Em jogo de 32 bits, a memória do processo (até 2 GB, ou 4 GB com "large address
   aware") é o teto real.
6. **Tela inteira quando faz sentido:** cenas de corte, menus, replays, cenas roteirizadas. Os jogadores
   extras continuam no mundo (a câmera deles segue andando), só não ganham visão naquele instante.
7. **Seguro por padrão.** Confira os bytes antes de alterar qualquer ponto do jogo; se não baterem (outra
   versão do jogo), desligue aquele recurso e registre no log, em vez de derrubar o jogo. Suporte uma versão
   do executável e declare-a na ficha (`acceptedSha256`). Log só de eventos raros.
8. **Lógica testável fora do jogo:** layout das visões, regras de entrada e saída, cálculo de controles. O
   que só existe dentro do jogo fica pequeno e conferido.

**O que o Party Anywhere já faz por você** (e que um mod sozinho teria de escrever): controles e celulares,
QR, lobby, quem é cada jogador, reconexão, vibração, o controle do celular com nomes e modos, e a imagem no
celular. O mod lê as vagas pelo SDK (`pa_get_pad`), e fica só com o que é do jogo: os jogadores extras, as
visões, o HUD, as regras de co-op e a escolha de personagem.

| Muda no mod | Como |
|---|---|
| **Entrada** | Cada jogador lê o próprio controle pelo SDK (`pa_get_pad(pa, vaga)`), não o "controle 1" do Windows. Desligue a leitura de controle e teclado do próprio jogo para os jogadores: o Windows continua entregando o teclado ao jogo, e um teclado que é o P3 no Party Anywhere moveria o P1. |
| **Imagem** | Uma visão por jogador (tela dividida na TV). Quem está no celular: a visão dele vai para o celular (`pa_view_wanted` / `pa_submit_view`), desenhada fora da tela e menor, e sai da TV. |
| **Quem é quem** | Nome e cor da vaga (`pa_get_seat`) no lugar do nome da conta online. |
| **Entrar e sair** | Os eventos do SDK (`PA_EVENT_SEAT_JOINED`, `RECONNECTING`, `LEFT`): o jogador entra no meio da partida e volta para o mesmo personagem. |
| **O controle do celular acompanha o jogo** | `pa_set_mode` ao entrar e sair do carro, `pa_set_label` com o nome de cada ação. |

**Checklist "não perdi nada"** (jogo com online): jogue a mesma partida no online original e no local e
confira todos os modos de jogo, entrar e sair no meio, morrer e voltar, salvar e carregar, trocar de mapa, o
HUD de cada jogador, o som (de quem é cada som) e a pausa (o local não pode travar os outros).

## 4. Desempenho

O custo de uma visão a mais é **desenhar a cena de novo**, não rodar o jogo de novo. Para manter o jogo liso:

- Cada visão no tamanho do pedaço dela, nunca a tela inteira.
- Nas visões extras, corte o que pesa e pouco se nota: distância de desenho e de detalhe (LOD) um pouco
  menores, sombras e reflexos mais simples, espelhos e passes caros só na visão principal.
- Quem joga pelo celular não ocupa a TV: a visão dele é desenhada pequena (640x360 basta) para o celular.
- Meça com o número máximo de jogadores, no PC mais fraco que você quer atender, e coloque na ficha só o que
  ficou liso.

## 5. Passar do limite de jogadores

É o ponto que mais quebra. O limite raramente está num lugar só: **ache todos antes de mudar um**.

### Onde o limite costuma estar

| Lugar | Como aparece | O que fazer |
|---|---|---|
| Constante de máximo | `MAX_PLAYERS = 4`, comparações `< 4`, laços `for (i = 0; i < 4; ...)` | Ache todas as comparações e laços com o número (procure pelo valor e pelos múltiplos dele). |
| Vetores de tamanho fixo | Arrays de jogadores, de câmeras, de placares, de inventários, com 4 posições | Realoque para o novo tamanho e redirecione **todas** as referências (senão o 5º jogador escreve na memória de outra coisa e o jogo trava longe dali). Uma alternativa que o mod de referência usou: manter os espaços do jogo e **trocar quem ocupa o espaço** a cada contexto, guardando os dados de cada jogador no mod. |
| Grupos e pools do motor | Cada jogador ocupa um grupo, uma equipe, uma entrada de pool | Confira o tamanho de cada pool que um jogador usa e aumente antes de o jogo criar os pools. |
| Interface | HUD, placar, lista de jogadores e seleção de personagem com 4 espaços | Desenhe os extras (ou uma versão compacta); a interface é o que a pessoa vê quebrar primeiro. |
| Pontos de nascer e câmeras | 4 pontos de nascer, 4 câmeras | Gere novos a partir dos existentes (ao lado de um ponto válido). |
| Controles do Windows | XInput vê no máximo **4** controles | Pelo SDK não há esse limite: até **10** jogadores com `pa_get_pad`. É mais um motivo para o mod ler o SDK, não o XInput. |
| Equilíbrio | Inimigos e recompensas escalados para 4 | Escale a dificuldade pelo número de jogadores (o próprio jogo costuma ter a fórmula para 1 a 4: estenda). |

### Método (do mais seguro para o mais arriscado)

1. **Meça o limite de verdade:** suba de 1 em 1 (5, depois 6...) e anote o primeiro sintoma (não entra,
   HUD some, trava). Cada sintoma aponta um dos lugares da tabela.
2. **Mude um lugar por vez** e teste entrar, sair, morrer e salvar com o novo número.
3. **Teste no limite:** com o máximo novo, todos ao mesmo tempo, uma partida inteira.
4. Na ficha do mod (`players.max`), coloque o máximo que funcionou de verdade, não o desejado.

## 6. Especial: jogo de 1 jogador para vários

Sem multiplayer nenhum para reaproveitar, os extras vêm do tipo de jogador do próprio jogo (ou do personagem
mais parecido). Comece pelo menor passo que prova a ideia: um segundo personagem que anda com o controle da
vaga 2, mesmo sem câmera própria. Depois a câmera e a visão, depois o HUD, depois o resto.

## 7. Ferramentas para a parte do jogo

Achar no jogo onde ficam os jogadores, a câmera e o desenho é engenharia reversa: um depurador e um
desmontador (x64dbg, Ghidra, Cheat Engine para achar valores na memória), as ferramentas da engine (Unity:
BepInEx, dnSpy; Unreal: UE4SS) e as comunidades de mod do jogo, que costumam já ter mapeado muito (para
jogos antigos há projetos de código reescrito e SDKs de plugin com os endereços de cada função). Alguns desses
programas têm servidor MCP, o que deixa o agente olhar o jogo junto com você. Só em jogos sem anti-cheat, e
sem distribuir nada do jogo original.

## 8. Ordem de trabalho sugerida (para o mod e para a IA)

1. Entrar no jogo: o mod carrega e manda um aviso para o celular (`pa_toast`).
2. Entrada: o jogador 1 do jogo passa a ler a vaga 1 do SDK.
3. Segundo jogador: um jogador de verdade do jogo, que entra, anda e interage com o controle da vaga 2.
4. Imagem: a troca de contexto, e então duas visões na tela; depois a visão de quem está no celular.
5. O que uma visão a mais quebra (passo 4 e 5 da receita): streaming, luzes, sombras, fade, HUD.
6. Até o limite original do jogo, com o checklist "não perdi nada".
7. Passar do limite, com o método da seção 5.
8. Ficha, capa e controle do celular do mod; publicar (opcional).
