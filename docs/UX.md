# Party Anywhere: UX

UX first: este documento manda nas decisões de produto. Quando a arquitetura atrapalha a
experiência, muda a arquitetura. Os princípios estão em [VISION.md](VISION.md#princípios-de-ux-valem-mais-que-qualquer-feature).

A UI está em [`apps/hub/ui/index.html`](../apps/hub/ui/index.html): no navegador ela roda em modo demonstração (painel "Demo"), no app usa os dados reais.

## Direção visual

Referência: o fliperama verde da Biblioteca (CRT verde-limão, luz difusa, tipografia condensada).

- **Fundo** grafite quase preto, num painel com bordas arredondadas e um brilho verde sutil.
- **No app, a janela é o painel**: sem a moldura do Windows, cantos arredondados de verdade, arrastada pelo
  topo, com minimizar, maximizar e fechar discretos no canto. Abre do tamanho da área útil da tela (nunca
  embaixo da barra de tarefas); mínimo 860×560.
- **Tipografia display**: Anton (condensada, caixa alta), a mesma da tela do fliperama. Texto corrido em Inter.
- **Um verde só** na interface (`#3cf060`, o mesmo dos botões). O verde-limão fica só na tela do fliperama.
  Sem degradês coloridos.
- **Logo só escrita**: "PARTY" em Anton creme e "ANYWHERE" em Bungee verde, como o letreiro de um fliperama;
  as duas palavras com a mesma altura de maiúscula. Fonte retrô só na logo, nunca em textos.
  A fonte pontilhada (Doto) fica restrita às siglas de marca e ao "P1" na tela de Bluetooth.
- **Botões**: o verde cheio fica para a ação que conclui um passo (Parear, Salvar, Instalar). Ações
  frequentes (Adicionar, Jogar) usam o verde suave, com texto, sem brilho forte.
- **Sem ícones genéricos de controle**: marcas aparecem como sigla pontilhada na cor da marca (PS, XB, NS);
  opções de um modal são numeradas (01, 02, 03).
- **Topo em pílulas**: logo | abas (numa pílula, no meio) | Adicionar + indicador de atualização (só um
  ícone). Na Biblioteca as pílulas ganham fundo escuro e flutuam sobre o 3D, sem faixa por cima.
- **Palco curvo** com piso em grade em perspectiva. As cartas dos jogadores ficam em arco, e a
  carta em foco fica maior e na frente.
- **Verde neon** como cor de ação principal (botão "Adicionar jogador", aba ativa).
- **Cartas de vidro** (blur e borda translúcida) para informações sobre a arte.
- **Movimento com propósito**: carta entra com escala e brilho quando um jogador conecta, e a carta
  desbota quando ele cai. Respeitar `prefers-reduced-motion`.

### Cores dos jogadores (fixas por número)

| P1 | P2 | P3 | P4 | P5 | P6 | P7 | P8 | P9 | P10 |
|---|---|---|---|---|---|---|---|---|---|
| `#3B82F6` azul | `#EF4444` vermelho | `#22C55E` verde | `#EAB308` amarelo | `#A855F7` roxo | `#F97316` laranja | `#06B6D4` ciano | `#EC4899` rosa | `#84CC16` lima | `#F5F5F4` branco |

P1 a P4 seguem a convenção dos consoles. A cor sempre aparece junto do número.

## Telas

| Tela | Para quê |
|---|---|
| **Lobby** (tela inicial) | Ver quem está conectado, adicionar jogadores, ir jogar. |
| **Adicionar jogador** (modal) | Escolher Celular ou Controle Bluetooth. |
| **Biblioteca** | Jogos instalados, catálogo, adicionar jogo por link. |
| **Controles** | Dispositivos conhecidos, renomear, esquecer, testar botões. |
| **Ajustes** | Idioma, rede, atualizações, componentes (driver de controle virtual). |

## Fluxo 1: primeira abertura

1. Instalador de um clique (sem perguntas além de "instalar").
2. O Hub abre direto no **Lobby**, com o QR já visível na carta "+".
3. Controles já conectados (USB ou já pareados) **já aparecem** como P1, P2...
4. Nenhum assistente de boas-vindas com 5 telas. Uma dica discreta, que some depois do primeiro jogador:
   "Conecte um controle ou escaneie o QR com o celular".

## Fluxo 2: Lobby

- Até **10 cartas** em arco. Depois da última carta de jogador fica sempre a carta **"+"**
  (até completar 10). Contador "3 / 10".
- **Carta de jogador**: número e cor, nome (editável) e um **desenho grande do aparelho**: controle,
  celular ou teclado. Embaixo, o aparelho ("Pixel 8", "Logitech K380", "Teclado do notebook") e o
  estado: latência para celulares, bateria quando houver, "Com fio" ou "Do notebook". Botão
  **Identificar**: o controle Xbox vibra; a tela do celular pisca na cor do jogador (e vibra no Android:
  o iPhone não deixa uma página vibrar sem um toque na tela); controles sem vibração no Windows
  (PlayStation, por enquanto) e teclados: a carta pisca, e o aviso diz isso em vez de prometer vibração.
- **Arco**: a carta em foco cresce e as vizinhas se afastam para abrir espaço; dos dois lados sempre
  aparece um pedaço de cada carta, inclusive a última.
- **Ordem**: P1 é quem entrou primeiro. Arrastar cartas (ou segurar LB/RB no controle) reordena.
- **Entrada**: controle conectado entra sozinho. Configuração opcional: "Entrar só ao apertar um botão",
  útil quando há controles parados na sala. **Teclado** (do notebook, USB ou Bluetooth) entra por
  **Adicionar jogador → Teclado**: aperta-se Enter no teclado que vai jogar (ou "Conectar" na lista de
  teclados encontrados). Cada teclado é um jogador diferente.
- **Queda**: a carta fica cinza com "Reconectando..." e **guarda a vaga**. Se o mesmo dispositivo
  voltar, volta para a mesma vaga. Remover é manual (botão na carta).
- **Ir jogar**: botão "Jogar" leva à Biblioteca. No Modo Universal, o lobby continua valendo enquanto
  qualquer jogo estiver aberto.

## Fluxo 3: adicionar jogador → Celular

1. Modal "Adicionar jogador" com duas opções grandes: **Celular** | **Controle Bluetooth**.
   Rodapé: "Controles USB e já pareados aparecem sozinhos."
2. Celular: QR grande, o endereço escrito embaixo (para digitar, se a câmera falhar) e 3 passos:
   "Mesma rede Wi-Fi → aponte a câmera → pronto".
3. O celular abre a página do controle e **já entra no lobby**. A carta aparece no Hub com animação,
   e o celular mostra "Você é o P4" na cor do jogador e vibra.
4. Sem app para instalar. A página pede para "Adicionar à tela inicial" só na segunda visita.
5. Erros com saída:
   - Celular não conecta em 10 s → "Seu celular está no mesmo Wi-Fi que o PC?" e um botão
     "Ver outros endereços" (para PCs com várias placas de rede).
   - Firewall bloqueou → botão "Permitir no firewall" (pede elevação uma vez).
   - Rede de convidados com isolamento de clientes → explicar em uma frase e sugerir a rede principal.

## Fluxo 4: adicionar jogador → Controle Bluetooth (melhor que o do Windows)

> Até a fase 3, a tela explica como colocar o controle em modo de pareamento (por marca) e abre o
> Bluetooth do Windows; depois de pareado, o controle entra sozinho no lobby.

O pareamento do Windows mostra tudo (fones, TVs, teclados), sem dizer como colocar cada controle
em modo de pareamento. O nosso:

1. **Bluetooth desligado?** Mostra o aviso e um botão "Ligar Bluetooth" ali mesmo.
2. **Antes de procurar, ensina**: carrossel com o controle desenhado e a instrução certa:
   - PlayStation (DualSense/DualShock 4): segure **PS + Create/Share** até a luz piscar rápido.
   - Xbox: segure o **botão de parear** (em cima, perto do USB) até o logo piscar rápido.
   - Switch Pro / Joy-Con: segure o **botão de sincronização** até as luzes correrem.
   - 8BitDo e genéricos: modo indicado no manual (link) e uma dica geral.
3. **Procura só controles** (filtro por tipo de dispositivo Bluetooth e nome conhecido), com uma
   animação de radar. Cada controle encontrado aparece com ícone da marca e nome amigável
   ("Controle PlayStation 5", e não "Wireless Controller").
4. Um toque em **Parear**: o Hub pareia sozinho, sem janela do Windows. Se o controle pedir PIN,
   o Hub responde sozinho.
5. Pareou → o controle **vira jogador na hora**, vibra e acende na cor da vaga.
6. **Nada encontrado em 20 s** → "Não achei. O controle está piscando rápido?" e as instruções de novo.
   Link "Controle não está na lista?" para os casos difíceis.

## Fluxo 5: Biblioteca

- **Um fliperama 3D** (Three.js). A cena segue a referência visual: gabinete verde-oliva em luz
  verde difusa, câmera próxima e de frente na altura da tela, **com as laterais encostando nas bordas**
  (o mesmo enquadramento da referência). Em janela baixa, o CRT e os manches sempre cabem inteiros.
- **Topo**: frente dos alto-falantes inclinada para dentro, sob um chapéu; cada alto-falante tem moldura
  arredondada e lisa, filete cromado e grade de metal perfurado levemente abaulada. Sem letreiro escrito.
- **Os jogos aparecem dentro da tela**: a imagem do jogo preenche a direita com um fade longo; embaixo, à
  esquerda e bem embaixo (os manches podem passar na frente), ficam selo, título, jogadores e descrição, sem
  botão e sem setas. A imagem passa por um **filtro verde da tela** (monocromática nas cores do tubo) e se
  dissolve no fundo; cada página tem o mesmo fundo do tubo, então a troca de jogo não mostra emenda. ← →, rolagem ou clique nas
  bordas trocam de jogo; A, Enter ou clique jogam. A luz que a tela joga no painel tem a cor do jogo exibido.
- Não existe card "Qualquer jogo": com o Hub aberto, o Modo Universal já vale para qualquer jogo.
- A legenda dos controles fica **escrita à mão (riscada na tinta) na borda do painel**, centralizada sob
  sob os botões, perto dos cantos: "←→ escolher" e "Ⓐ jogar". Na Biblioteca, o botão verde do topo vira **Adicionar jogo**.
- **Fluidez é requisito**: o Hub tem altura fixa (nada se redimensiona no meio de uma transição), a troca
  de jogo acontece no shader da tela e tudo roda a 60 fps sem quadro perdido.
- **As abas ficam lado a lado** (Lobby | Biblioteca | Controles | Ajustes). Trocar de aba é um
  movimento de câmera: indo para a direita, a vista atual sai pela esquerda e a nova entra pela direita.
  **Nunca duas camadas ao mesmo tempo**: a vista atual some rápido (0,3 s) e só então a nova aparece. Na
  Biblioteca, a câmera 3D **se aproxima como um zoom-in** (de trás, só ~12° de lado), com uma desaceleração
  longa e macia e um leve desfoque radial de movimento. Ao sair, um recuo curto e macio (0,5 s) com
  esmaecimento; a outra aba entra logo depois.
- **Jogar**: a câmera entra na tela do fliperama, a tela apaga (com um clarão de CRT) e o jogo abre.
  **Sair do jogo** faz o caminho contrário: a tela religa e a câmera volta para a posição normal.
- O enquadramento usa só a área livre entre o cabeçalho e os botões de baixo, em qualquer tamanho de janela.
- Sem WebGL ou sem internet para carregar o 3D, a Biblioteca cai para o **carrossel de capas** abaixo.
- **Carrossel de capas** (reserva, formato retrato 3:4) no mesmo palco do lobby; a capa em foco mostra
  nome, jogadores suportados (ex.: "2 a 8") e o botão **Jogar**.
- Primeira carta fixa: **"Qualquer jogo" (Modo Universal)**, que explica que, com o Hub aberto,
  qualquer jogo vê os celulares como controles.
- Selos: **Verificado** (do catálogo, revisado) ou **Da comunidade** (instalado por link).
- **Tudo fica salvo**: jogos adicionados, nomes, capas e descrições editados e a ordem. Fechar e abrir
  o Hub mostra a Biblioteca igual. Ninguém precisa clicar em "Salvar".
- Jogo com atualização: um ponto na capa; atualiza ao apertar Jogar (com barra de progresso) ou em segundo plano.

## Fluxo 6: adicionar jogo

Na Biblioteca, o botão do topo vira **Adicionar jogo** e abre a **Biblioteca**, com duas abas: *Adicionar
jogo* (os três caminhos abaixo) e *Organizar*, a lista dos jogos do fliperama, onde dá para **editar** (nome,
jogadores, descrição, imagem), **remover** (com Desfazer) e **mudar a ordem** arrastando (ou Alt + ↑ ↓). Qualquer jogo entra:
jogos que não usam o SDK rodam no **Modo Universal** (cada celular vira um controle de Xbox).

### 6a. Jogos deste PC (o caminho principal)

1. O Hub procura sozinho o que está instalado (Steam, Epic, GOG, Xbox) e mostra uma grade com capa, nome,
   loja e o tipo de co-op ("Co-op local · até 4", "Tela dividida", "1 jogador").
2. A pessoa marca um ou vários (busca no topo) e toca em **Adicionar N jogos**. Os que já estão na
   biblioteca aparecem marcados como "Na biblioteca".
3. Na tela do fliperama, o selo mostra a loja e "Modo Universal".

### 6b. Escolher pasta ou atalho

Para o jogo que não apareceu: escolher (ou arrastar) o executável, o atalho (.lnk, .url da Steam) ou a pasta.
O Hub lê nome e ícone do próprio executável, procura a capa pelo nome e, se não achar, gera uma.

### 6c. Jogo co-op por link (feito com o Party Anywhere)

1. Campo para colar a URL do repositório (GitHub no início).
2. O Hub lê o `party-anywhere.json` do repositório (ver [GAME-MANIFEST.md](GAME-MANIFEST.md)) e mostra
   uma **prévia**: capa, nome, autor, jogadores, tamanho do download, se exige um jogo-base.
3. Se não for do catálogo: aviso claro, em uma frase, sem drama: "Este jogo não foi verificado pela
   equipe Party Anywhere. Instale só se confiar no autor."
4. Exige jogo-base (ex.: GTA San Andreas)? O Hub **procura sozinho** nas bibliotecas da Steam; se não
   achar, pede a pasta. Avisa quando a versão do jogo não é a esperada.
5. **Instalar** → progresso → a capa aparece na Biblioteca com brilho de "novo".
6. Erros com saída: "Este repositório não tem `party-anywhere.json`" e um link para o guia do criador.

## Fluxo 7: atualização do próprio Hub

- O Hub verifica ao abrir e a cada 6 horas. Baixa em segundo plano.
- **Nunca** reinicia sozinho nem mostra janela modal. Um ícone discreto no canto direito do topo (depois de
  "Adicionar") mostra um **anel verde de progresso em volta** enquanto baixa e ganha um ponto verde quando
  a versão está pronta; ela aplica ao fechar o Hub ou num clique.
- Durante uma partida (jogo SDK conectado) nada aparece.
- Notas da versão em 3 linhas, em português, ao abrir a versão nova.

## Fluxo 8: dentro do jogo

- Com o jogo em tela cheia, ninguém vê o Hub. Por isso:
  - o **celular** sempre mostra quem é (número, cor) e avisos curtos;
  - jogos SDK recebem o endereço do QR e eventos de entrada e saída para desenharem o lobby
    **dentro** do jogo, se quiserem;
  - o Hub mostra uma notificação discreta do Windows quando alguém entra ou sai (desligável).

## O controle no celular

- Tela em paisagem, tela cheia e tela sempre acesa (Wake Lock).
- **Layouts**: `gamepad` (completo, estilo Xbox), `simple` (direcional + 2 botões), `driving`,
  `menu` (setas + confirmar/voltar). O jogo SDK escolhe o layout e dá nome aos botões ("Correr", "Atirar").
- Analógico **flutuante** (nasce onde o dedo toca), botões grandes e vibração ao tocar.
- Topo: número e cor do jogador, latência (verde, amarelo ou vermelho) e o botão de menu.
- Perdeu a conexão: tela "Reconectando..." que tenta sozinha; ao voltar para a mesma vaga, vibra.

## Navegação por controle no Hub

- Direcional/analógico move o foco entre as cartas e os botões; **A** confirma, **B** volta,
  **Start** é "Jogar", **Y** é "Adicionar jogador".
- O foco é sempre visível: anel na cor do jogador que está navegando.
- Qualquer jogador pode navegar; ações destrutivas (remover jogador) pedem confirmação.

## Textos (tom de voz)

Curtos, diretos, no "você", sem exclamação em excesso.

| Situação | Texto |
|---|---|
| Lobby vazio | "Conecte um controle ou escaneie o QR com o celular." |
| Jogador entrou | "P3 entrou" |
| Jogador caiu | "P2 caiu. Guardando a vaga..." |
| Lobby cheio | "Lobby cheio (10 de 10)." |
| Bluetooth sem resultado | "Não achei nenhum controle. Ele está piscando rápido?" |
| Atualização | "Atualização pronta. Instala quando você fechar o Hub." |
