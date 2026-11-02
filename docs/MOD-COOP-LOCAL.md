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
| **Unity** | Uma câmera por jogador com o retângulo dela na tela é nativo; o trabalho é criar o personagem extra e dar a cada um a sua câmera e a sua entrada. Mod por BepInEx ou MelonLoader, com o SDK em C# (`sdk/csharp/PartyAnywhere.cs`, que o `pa new` já traz). Receita completa na seção 4 (é a do mod do MicroWorks: Unity + Mirror, 1 a 10 jogadores). |
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
| **Imagem** | Uma visão por jogador (tela dividida na TV, seção 3.1). Quem ligou a tela do jogo no celular: a visão dele vai para o celular (`pa_view_wanted`, `pa_view_size`, `pa_submit_view`), desenhada fora da tela no formato do celular, e sai da TV. |
| **Quem é quem** | Nome e cor da vaga (`pa_get_seat`) no lugar do nome da conta online. |
| **Entrar e sair** | Os eventos do SDK (`PA_EVENT_SEAT_JOINED`, `RECONNECTING`, `LEFT`): o jogador entra no meio da partida e volta para o mesmo personagem. |
| **O controle do celular acompanha o jogo** | `controls` na ficha (seção 3.4), `pa_set_mode` ao entrar e sair do carro, `pa_set_label` com o nome de cada ação. |
| **Câmera pelo celular** | `pa_take_look` (graus, como o mouse) somado ao ângulo da câmera, além do analógico direito para quem joga com controle. Com isso o celular troca o analógico direito por arrastar o dedo, como num jogo de celular. |

**Checklist "não perdi nada"** (jogo com online): jogue a mesma partida no online original e no local e
confira todos os modos de jogo, entrar e sair no meio, morrer e voltar, salvar e carregar, trocar de mapa, o
HUD de cada jogador, o som (de quem é cada som) e a pausa (o local não pode travar os outros).

### 3.1 Tela dividida: o padrão de todo mod

- **Grade sem cantos vazios.** O número de linhas segue 1; 2 lado a lado; 2x2; 3x2; 3x3; 4x3 (até 2
  jogadores: 1 linha; até 6: 2 linhas; até 10: 3 linhas), mas **os jogadores se dividem por igual entre as
  linhas e cada linha ocupa a largura toda**: 3 = 2+1, 5 = 3+2, 7 = 3+2+2, 8 = 3+3+2, 10 = 4+3+3. Nunca sobra
  canto preto. As linhas de cima ficam com o jogador a mais.
- **Ordem:** quem hospeda primeiro, depois por vaga (P1, P2...).
- **Reorganiza no mesmo quadro** quando alguém entra, sai ou passa para o celular. Se todos estão no celular,
  a TV fica preta com "Todos jogando pelo celular".
- **Ver o corpo dos outros em primeira pessoa:** o jogo esconde o corpo do "jogador local", e agora todos são
  locais. Ponha cada corpo numa camada só dele e faça a câmera de cada um esconder só a própria camada.
  Ligar e desligar o corpo antes de cada câmera não funciona em pipelines que preparam todas as câmeras antes
  de desenhar (HDRP).
- **Som:** só uma câmera ouve (a de quem hospeda). Duas escutando dobram o som.
- **Áreas sem visão** mostram restos de quadros antigos: pinte de preto o que nenhuma visão cobre.

### 3.2 HUD por jogador

- **HUD do jogador** (mira, vida, "usar"): o conteúdo dele vai para uma moldura do tamanho da parte dele.
- **HUD da partida** (rodada, placar, posição, "passou/falhou"): o jogo costuma ter um só. Dê a cada jogador
  extra **uma cópia só com as telas**, sem a lógica (pausa, espera pelos jogadores, rede: copiar a lógica
  repete rodadas). As chamadas da partida vão ao original e se repetem em cada cópia, no contexto do jogador
  dela; as do código de um extra vão só para a cópia dele.
- **Escala:** HUD montado a partir do centro da tela sai da parte quando a largura é espremida. Encolha o HUD
  inteiro sem mudar a proporção e encoste no alto da parte.
- Avisos gerais (alguém entrou, votação) ficam um só, na parte de quem hospeda.

### 3.3 Quem é quem

- Nome e cor da vaga do Party Anywhere (`pa_get_seat`) no lugar do nome e da foto da conta (que seriam os da
  mesma conta para todos): um avatar com a cor e "P2".
- Um `pa_toast` ao entrar ("Você é o P2").
- Com a tela dividida: a etiqueta "P2 · nome" na cor da vaga num canto de cada parte, e "P3" sobre a cabeça de
  cada jogador do sofá, desenhado no HUD de quem vê (uma placa no mundo não vira para cada câmera em motores que
  preparam todas as câmeras antes de desenhar, como o HDRP).

### 3.4 Controle do celular

- Todo mod declara `controls` na ficha, com os modos do jogo: o **meio livre**; andar na esquerda; câmera,
  tiro e ações principais na direita; ações raras num segundo modo. Monte no editor do Hub (Controles → Criar
  layout) e copie com "Copiar para a ficha do mod". O `pa new` já traz um inicial nesse padrão, e o `pa validate`
  avisa qual peça tampa o meio. Quem instala recebe esse controle junto e pode editá-lo no Hub.
- Nomes nos botões curtos (até ~8 letras), iguais no `controls` e no `pa_set_label`.
- Câmera: leia `pa_take_look` (arrastar o dedo) além do analógico direito.

### 3.5 Todo jogador tem tudo, como no online

O teste: cada jogador do sofá faz tudo o que um jogador da internet faz, e os pontos caem na conta dele. Dois
lugares escondem jogabilidade perdida:

- **O que é de um jogador e nasce depois dele** (a arma na mão, a bola, o veículo que ele controla, os
  projéteis). Num jogo de rede, cada PC tem autoridade sobre o que é seu. Aqui, o jogo de quem hospeda só dá
  autoridade ao que é da conexão de quem hospeda: o extra pega a bola e não consegue arremessar. Dê ao extra a
  autoridade sobre o que é dele (no jogo de quem hospeda), avise o objeto como a rede avisaria no jogo dele
  (no Mirror, `OnStartAuthority`), no contexto do extra, e ponha a troca de contexto no código desses objetos
  (a vaga sai do dono do objeto na rede).
- **O que o jogo mostra só "ao jogador local"** (um aparelho de perguntas, um campo de digitar, uma tela de
  escolha). Fica um só, de quem hospeda, às vezes cobrindo a tela inteira. Cada jogador ganha o seu, na parte
  dele (e no celular de quem joga por lá), usável no controle: teclado numérico no direcional, opções para
  escolher, e a resposta vai pelo mesmo evento de rede do jogo, no contexto do jogador. O original fica na
  parte de quem hospeda. Campo de texto livre não cabe num controle: troque por opções (a certa e outras do
  próprio jogo).
- **O que o jogo decide pela "camada do jogador local"**. Muitos jogos põem o próprio personagem numa camada só
  dele, e os dos outros PCs em outra. Aqui todos são locais, e isso esconde dois erros opostos: o que procura
  alvos (empurrar, socar, atirar) só olha a camada dos outros e não acha ninguém; o que reage ao jogador local
  (o chão que mata, a placa de pressão) guarda o primeiro que encostou e trata todo mundo como ele. Enquanto um
  jogador procura alvos, ponha os outros jogadores do sofá na camada "dos outros PCs" e devolva logo depois; e
  faça cada encostada valer para quem encostou.
- **A parte "de cliente" de cada desafio.** Num jogo de rede, cada PC roda o código do desafio para o seu jogador
  ("já se mexeu?", "pulou 3 vezes?", "pisou no ladrilho certo?"). Num jogo só, ele roda uma vez, para quem
  hospeda. Rode-o também para cada jogador do sofá, no contexto dele, com a parte de servidor desligada, o estado
  do desafio separado por jogador e o mesmo sorteio; e não registre de novo os ouvintes de evento.
- **Uma "câmera ativa" global.** A mira, o "usar", o arremesso e o tiro costumam sair dela: dê uma por jogador.
- **Comandos de objetos da fase** (que aceitam comando de qualquer um): o servidor tem de saber qual jogador do sofá
  mandou, senão tudo conta para quem hospeda.
- **O que o jogo entrega quando "todos chegaram"** (veículos): com uma pessoa só no lobby, isso pode acontecer antes
  de os jogadores do sofá nascerem.
- **Gestos do mouse** (chacoalhar uma lata movendo o mouse rápido de um lado ao outro). O analógico, usado como
  mouse, não chega lá a tempo: no controle, faça o analógico ser a posição do gesto (até o fim de um lado, até o
  fim do outro).

Antes de publicar, passe por **todos** os modos e desafios do jogo com 2 jogadores e confira cada um.

### 3.6 Regras de produto

- Nada de contornar pagamento, DRM, conta ou anti-cheat, nem "jogar offline" pulando a loja. Se um recurso
  é pago no jogo (sala privada, por exemplo), o mod não libera, e diz isso na página dele.
- Usar o código do próprio jogo para tudo (contexto), em vez de reimplementar: é o que não perde jogabilidade.

## 4. Receita Unity (e jogo online com Mirror)

É a do mod do MicroWorks (Unity 2019.3, Mirror, BepInEx, 1 a 10 jogadores numa cópia só). Adapte a outras
redes (Netcode, Photon, a do próprio jogo) com as mesmas ideias.

1. **Uma conexão de mentira por vaga, no servidor** (no Mirror, uma `NetworkConnectionToClient` com id
   negativo). O personagem extra nasce pelo caminho normal do jogo (`AddPlayerForConnection`), então entra no
   placar, nas fases e nas regras como qualquer jogador. A conexão engole as mensagens que mandaria pela rede,
   inclusive o envio em lote.
2. **Troca de contexto em volta de todo código do extra:** `Start`, `Update`, `LateUpdate`, `FixedUpdate`,
   `OnEnable`, `OnDisable` e as corrotinas de cada componente dele e do que ele controla. Dentro, ele é "o
   jogador local" (as referências estáticas do jogo e da rede, a entrada da vaga dele, a identidade dele).
   Câmera, HUD, movimento e ações passam a usar o código do próprio jogo.
3. **Mensagens de rede para o extra** (ClientRpc, TargetRpc) também rodam no contexto dele. Sem isso o jogo
   acha que o personagem é de outro PC e reenvia para sempre.
4. **Comandos do extra:** o servidor confere o dono; diga a ele qual é a conexão certa.
5. **Identidade:** uma id de conta falsa que nunca colide com uma real (no MicroWorks, o SteamID real com outro
   campo de "instância"). O jogo pede à loja os dados da conta e espera para sempre por uma id que não existe:
   para ids falsas, responda na hora.
6. **Pegadinha do Harmony:** os *finalizers* de vários patches no mesmo método rodam na ordem dos prefixes,
   não na inversa. Entrar no contexto de quem já está nele não pode fazer nada (só a entrada de fora troca e
   volta), senão o contexto fica preso.
7. **Câmeras:** `Camera.rect` por jogador na grade da seção 3.1.
8. **Tela no celular:** `sdk/csharp/Unity/PhoneViews.cs` pronto: a câmera de quem está no celular desenha
   num `RenderTexture` no formato do celular (`ViewSize`), lido com `AsyncGPUReadback` (até 3 leituras em
   andamento, uma pedida por quadro) e entregue com stride negativo (a imagem volta de baixo para cima). O HUD
   dele passa a `ScreenSpaceCamera` com aquela câmera e volta ao modo original quando ele volta à TV. O
   `CanvasScaler` mede a janela, não a imagem: compense, senão o HUD sai gigante no celular.
9. **Ferramentas de diagnóstico que valem o tempo:** um vigia de travamento (com o jogo parado por 8 s, uma
   thread interrompe a principal e o log mostra onde ela estava), um contador de mensagens de rede por quadro
   e "onde o jogador X aparece na câmera de Y". Desligue tudo antes de entregar.

## 5. Desempenho

O custo de uma visão a mais é **desenhar a cena de novo**, não rodar o jogo de novo. Para manter o jogo liso:

- Cada visão no tamanho do pedaço dela, nunca a tela inteira.
- Nas visões extras, corte o que pesa e pouco se nota: distância de desenho e de detalhe (LOD) um pouco
  menores, sombras e reflexos mais simples, espelhos e passes caros só na visão principal.
- **Com mais de um jogador na tela, use o preset de vídeo mais leve do próprio jogo, só na memória:** as
  configurações da pessoa voltam com um jogador, e gravar configurações grava as dela, nunca o preset. No
  MicroWorks (Ryzen 7 5700U, vídeo integrado, 1080p): 4 jogadores foram de 20 para 34 quadros por segundo; 6
  jogadores, 27; 10 jogadores, 19.
- Quem joga pelo celular não ocupa a TV: a visão dele é desenhada menor (540 de altura, no formato do
  celular: `pa_view_size`).
- Meça com o número máximo de jogadores, no PC mais fraco que você quer atender, com o **Hub release** (sem
  otimização, o Hub comprime a imagem do celular devagar), e coloque na ficha só o que ficou liso.

## 6. Passar do limite de jogadores

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

## 7. Especial: jogo de 1 jogador para vários

Sem multiplayer nenhum para reaproveitar, os extras vêm do tipo de jogador do próprio jogo (ou do personagem
mais parecido). Comece pelo menor passo que prova a ideia: um segundo personagem que anda com o controle da
vaga 2, mesmo sem câmera própria. Depois a câmera e a visão, depois o HUD, depois o resto.

## 8. Ferramentas para a parte do jogo

Achar no jogo onde ficam os jogadores, a câmera e o desenho é engenharia reversa: um depurador e um
desmontador (x64dbg, Ghidra, Cheat Engine para achar valores na memória), as ferramentas da engine (Unity:
BepInEx, dnSpy; Unreal: UE4SS) e as comunidades de mod do jogo, que costumam já ter mapeado muito (para
jogos antigos há projetos de código reescrito e SDKs de plugin com os endereços de cada função). Alguns desses
programas têm servidor MCP, o que deixa o agente olhar o jogo junto com você. Só em jogos sem anti-cheat, e
sem distribuir nada do jogo original.

## 9. Testar sozinho (para quem faz e para a IA)

1. O Hub **release** aberto.
2. Celulares de mentira: `pa phone-sim --players 4 --screen --save-view prints` (cada um entra numa vaga,
   pode ligar a tela do jogo e guarda o último quadro em `prints\P2.jpg`...). Para apertar botões e mexer a
   câmera, um roteiro: `--pad roteiro.json`, com passos como `{ "ms": 800, "buttons": ["A"], "ly": 1, "look":
   [90, 0] }` (look em graus por segundo).
3. Opções de desenvolvimento do mod (pular tutorial, criar a partida sozinho, medir quadros por segundo),
   **desligadas antes de entregar**.
4. Um script que fecha o jogo, instala a DLL nova, abre o jogo, espera "P{k} entrou" no log e captura a janela
   sem roubar o foco.
5. **Pronto quando:** zero exceções no log do jogo; a partida roda vários minutos sem travar; capturas
   conferidas com 1, 2, 3, 4, 6 e 10 jogadores; a tela do jogo no celular liga e desliga.

## 10. Ordem de trabalho sugerida (para o mod e para a IA)

1. Entrar no jogo: o mod carrega e manda um aviso para o celular (`pa_toast`).
2. Entrada: o jogador 1 do jogo passa a ler a vaga 1 do SDK.
3. Segundo jogador: um jogador de verdade do jogo, que entra, anda e interage com o controle da vaga 2.
4. Imagem: a troca de contexto, e então duas visões na tela; depois a visão de quem está no celular.
5. O que uma visão a mais quebra (passo 4 e 5 da receita): streaming, luzes, sombras, fade, HUD.
6. Até o limite original do jogo, com o checklist "não perdi nada".
7. Passar do limite, com o método da seção 6.
8. Ficha, capa e controle do celular do mod; publicar (opcional: `scripts\release.ps1 -Publicar`, que compila e chama
   `pa release --publish`, com o repositório público).
