# Instruções para agentes de IA

Este repositório é um **mod co-op local para o Party Anywhere** (ou um jogo feito para ele). O Party Anywhere é
um app para Windows que junta controles, celulares e teclados num lobby (até 10 jogadores) e abre o jogo para
todos jogarem juntos no mesmo PC. A ficha do mod é o `party-anywhere.json` na raiz.

Descubra primeiro o que a pessoa quer: **jogar** este mod ou **mexer** nele.

## A pessoa quer JOGAR este mod

O objetivo é terminar com o mod na Biblioteca do Party Anywhere, pronto para apertar Jogar.

1. **Tenha a ferramenta `pa`.** Se o servidor MCP `party-anywhere` está disponível, use as ferramentas dele.
   Senão, baixe o `pa-windows-x64.exe` do release mais novo de
   <https://github.com/joaocaioo/party-anywhere/releases/latest> e rode como `pa`.
2. **Confira o app:** `pa hub` (MCP: `hub_status`). Não instalado ou com versão nova: explique e, **com o
   "sim" da pessoa**, rode `pa hub install` (MCP: `install_hub`). O instalador abre na tela; ela confirma.
3. **Leve o mod para a Biblioteca:** `pa add <link deste repositório>` (MCP: `add_to_library`). O app abre
   com a prévia do mod. Ele procura sozinho o jogo original (Steam, Epic ou a pasta que a pessoa escolher).
   A pessoa confirma em **Instalar o mod**.
4. **Se o app disser que falta o jogo original:** ele mostra os botões (instalar pela Steam ou pela Epic, ver
   na loja, ou "já tenho": escolher o `.exe` ou a pasta). Explique isso à pessoa.

Regras: **nunca** baixe, copie ou procure o jogo original fora da conta da própria pessoa. Este repositório só
tem o mod. Jogos com anti-cheat ficam de fora.

## A pessoa quer MEXER no mod (ou criar o dela a partir deste)

Use a skill `.claude/skills/party-anywhere-game` e as ferramentas `pa`/MCP: `get_sdk_docs` antes de mexer em
controles (o mod lê os jogadores só pelo SDK), `validate_game` antes de publicar. Publicar é opcional: um
release no GitHub (tag igual à `version` da ficha) deixa qualquer pessoa jogar só com o link.
