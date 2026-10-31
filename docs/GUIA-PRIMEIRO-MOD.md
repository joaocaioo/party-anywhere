# Guia: o primeiro mod co-op, numa conversa nova

Passo a passo para transformar um jogo seu em co-op local com o Party Anywhere, testar e (se quiser) publicar.
Feito para seguir numa conversa nova do Claude Code, com o contexto limpo.

> Enquanto não houver uma versão na página de Releases, o Hub e a `pa` vêm do código (passo 1), e a
> publicação do mod é feita pela linha de comando (passo 6). Com a versão publicada, é só baixar.

## 1. Preparar o PC (uma vez)

1. **O Hub aberto.** Na pasta do party-anywhere:

   ```bash
   cargo run -p party-anywhere-hub
   ```

   Deixe ele aberto enquanto trabalha no mod. (Se preferir instalado: gere o instalador com
   `npx --yes @tauri-apps/cli@2 build --config tauri.release.conf.json` em `apps/hub/src-tauri` e instale.)
2. **A `pa`** (já instalada neste PC; para atualizar depois de mudanças no código):

   ```bash
   cargo install --path crates/pa-cli
   ```

3. **Celulares nos jogos comuns:** no Hub, Ajustes → Controle virtual → **Instalar** (o ViGEmBus). O mod
   do SDK não precisa dele, mas é bom ter para testar o celular em qualquer jogo.

## 2. Escolher o jogo

- Sem anti-cheat (a `pa games` marca os que têm) e que você tenha instalado.
- Os melhores candidatos: jogos com **co-op só online** ou de um jogador, que roda bem em PC (tela dividida
  cabe), e que têm um jeito conhecido de carregar código (mods existentes, ASI loader, API de scripts).

```bash
pa games
```

## 3. Criar a pasta do mod e abrir a conversa

1. Crie uma pasta **fora** do repositório do party-anywhere (o mod é um repositório à parte), por exemplo
   `C:\Projetos\meu-mod-coop`, e abra o Claude Code nela.
2. Comece a conversa com algo assim (troque o que está entre `< >`):

   > Quero transformar o **<nome do jogo>** em co-op local para o Party Anywhere: **<quantos>** jogadores no
   > mesmo PC, **<tela dividida / mesma tela>**, cada um com o próprio personagem. Use a ferramenta `pa`:
   > confira o Hub (`pa hub`), crie o projeto nesta pasta com o jogo-base (`pa new . --base-game`), leia o
   > SDK (`pa docs sdk`) e o guia de co-op local (`pa docs coop`) e me proponha o caminho técnico antes de
   > escrever código: uma cópia do jogo, com todos os jogadores dentro dela e uma visão por jogador (nada de
   > abrir várias cópias do jogo). Quero tudo o que o online tem, sem perder jogabilidade, e **<N>** jogadores
   > (o jogo aceita **<M>**). Nada de anti-cheat e nada de copiar arquivos do jogo original para o repositório.

3. O que esperar: o agente cria o projeto (ficha, capa provisória, SDK, `AGENTS.md`), e então vem a parte
   difícil, que é específica de cada jogo: como o mod entra no jogo (uma DLL carregada por proxy, ASI loader
   ou a API de mods do jogo), como criar os jogadores extras, trocar o contexto entre eles e desenhar uma
   visão para cada um. É um trabalho de idas e
   voltas: peça uma coisa de cada vez ("primeiro: o jogo carrega a DLL e mostra um aviso no celular").

## 4. Testar sem publicar

A cada versão do mod que quiser testar:

```bash
pa pack --from build
```

```bash
pa add .
```

O Hub abre com a prévia do mod, acha o seu jogo (Steam, Epic ou a pasta que você escolher) e você confirma em
**Instalar o mod**. Ele aparece na Biblioteca com a marca "Seu mod (do PC)". Entre com controles ou
celulares e aperte **Jogar**. Para remover: Biblioteca → Organizar → o mod → Remover (os arquivos originais do
jogo voltam).

## 5. Deixar pronto para outras pessoas

```bash
pa validate
```

Ele avisa o que falta trocar: o `seu-usuario` (no `id` e em `authors`; o id não muda depois de publicado),
a frase da capa, a **capa** de verdade (retrato 3:4, arte sua), quantos jogadores (`players`). O controle do
celular do mod: monte no Hub (Controles → Criar layout, com nomes nos botões e modos) e use **Copiar para a
ficha do mod**; cole no `party-anywhere.json`.

## 6. Publicar (opcional, mas recomendado)

Publicar faz o mod chegar a qualquer pessoa só com o link (e aparecer em "Descobrir" no Hub). Na pasta do mod,
com o [GitHub CLI](https://cli.github.com/):

```bash
gh repo create meu-mod-coop --public --source . --push
```

```bash
gh repo edit --add-topic party-anywhere-game
```

```bash
pa pack --from build --tag v0.1.0
```

```bash
gh release create v0.1.0 release/*
```

Pronto: no Hub, **Adicionar → Mod co-op por link** com o link do repositório (ou `pa add dono/meu-mod-coop`).
Versão nova: suba a `version` no `party-anywhere.json`, `pa pack --tag vX.Y.Z` e `gh release create vX.Y.Z
release/*`; quem instalou recebe o aviso. (Quando o Party Anywhere for público, o workflow do mod faz o
pack e o release sozinho ao criar a tag.)

## Quem achar o seu mod

O repositório do mod já se explica: o README diz como jogar e o `AGENTS.md` ensina qualquer agente de IA
(Claude e outros) a conferir o Party Anywhere, instalar se faltar e levar o mod para a Biblioteca. Basta a
pessoa pedir: *"instale este mod no Party Anywhere"* com o link.
