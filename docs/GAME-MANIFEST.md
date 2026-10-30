# `party-anywhere.json`: o arquivo que faz um jogo aparecer na Biblioteca

Todo jogo ou mod feito para o Party Anywhere tem **um arquivo `party-anywhere.json` na raiz do
repositório** e **uma imagem de capa**. Com isso, qualquer pessoa cola o link do repositório no Hub
e o jogo aparece na Biblioteca, pronto para instalar.

Cada **release** do jogo carrega um segundo arquivo, `party-anywhere.release.json`, gerado pela ferramenta
`pa`: é dele que o Hub tira o pacote, o hash e o aviso de versão nova.

O jeito fácil de começar é `pa new` (ou a ferramenta `create_game` do servidor MCP, para agentes de IA):
o projeto já nasce válido, com capa provisória, workflow de release e a skill para agentes. O modelo cru
está em [`templates/game/`](../templates/game/), e o esquema JSON em
[`schema/game-manifest.v1.json`](../schema/game-manifest.v1.json) (autocompletar no editor).

## Exemplo completo

```jsonc
{
  "$schema": "https://raw.githubusercontent.com/joaocaioo/party-anywhere/main/schema/game-manifest.v1.json",
  "manifestVersion": 1,

  "id": "com.exemplo.sa-party",          // único, minúsculas, pontos e hífens; nunca muda
  "name": "San Andreas Party",
  "tagline": "Splitscreen local para até 8 jogadores em GTA San Andreas.",
  "description": "README.md",              // texto longo vem do README do repositório
  "authors": ["seu-usuario"],
  "license": "MIT",
  "version": "0.3.0",                     // semver; igual à tag do release

  "art": {
    "cover": "assets/cover.png",          // OBRIGATÓRIA: retrato 3:4, 600x800 ou mais
    "hero":  "assets/hero.jpg",           // opcional: 16:9, 1920x1080, fundo da tela do jogo
    "logo":  "assets/logo.png"            // opcional: PNG transparente
  },

  "players": { "min": 1, "max": 8, "recommended": 4 },
  "sdk": { "abi": 1 },                    // versão do SDK usada (o Hub confere compatibilidade)
  "tags": ["acao", "mundo-aberto", "splitscreen"],

  "requires": {                           // opcional: jogo-base que o usuário precisa ter
    "baseGame": {
      "name": "Grand Theft Auto: San Andreas",
      "steamAppId": 12120,
      "epicAppName": "...",              // opcional: o AppName na Epic (o Hub acha e sabe pedir para instalar)
      "storeUrl": "https://...",          // opcional: onde comprar fora da Steam (GOG, site do jogo)
      "exe": "gta_sa.exe",
      "acceptedSha256": ["…"],            // versões aceitas do executável
      "help": "docs/versao-do-jogo.md"    // o que fazer se a versão não bater
    }
  },

  "install": {
    "source": "github-release",           // v1: só releases do GitHub
    "asset": "sa-party-{version}.zip",    // pacote do release (.zip)
    "target": "baseGame",                 // "baseGame" (copia para a pasta do jogo) ou "self"
    "files": [ { "from": "SAParty.asi", "to": "SAParty.asi" } ]
  },

  "launch": {
    "exe": "gta_sa.exe",                  // relativo ao target
    "args": [],
    "workingDir": "."
  },

  "minHubVersion": "0.2.0"                // opcional: Hub mínimo
}
```

## Campos

| Campo | Obrigatório | Regras |
|---|---|---|
| `manifestVersion` | sim | `1`. |
| `id` | sim | `^[a-z0-9]+(\.[a-z0-9-]+)+$`. Identidade do jogo para sempre (instalação, saves, catálogo). |
| `name` | sim | Até 40 caracteres. |
| `tagline` | sim | Até 90 caracteres. Aparece embaixo da capa. |
| `version` | sim | Semver, igual à tag do release (`v0.3.0` ou `0.3.0`). |
| `art.cover` | sim | PNG/JPG/WebP, **3:4**, mínimo 600×800, máximo 2 MB. Sem texto pequeno (vista de longe na TV). |
| `players` | sim | `1 ≤ min ≤ max ≤ 10`. |
| `sdk.abi` | sim | Versão maior do SDK. |
| `requires.baseGame` | não | Use quando o mod precisa do jogo original. O Hub procura na Steam (`steamAppId`), na Epic (`epicAppName`) e nos jogos do PC pelo `exe`; se não achar, oferece instalar pela Steam ou Epic, a loja (`storeUrl`) ou escolher o `.exe`/a pasta (cópia de qualquer lugar). Com `acceptedSha256`, confere a versão e mostra o `help` se não bater. |
| `install` | sim | v1 aceita só `github-release` e cópia de arquivos. **Sem scripts de instalação.** `asset` é um `.zip`; `target: "baseGame"` exige `requires.baseGame` e `files`. Caminhos sempre relativos, sem `..`. |
| `minHubVersion` | não | Hub mínimo, se o jogo usa algo recente do SDK. |
| `launch` | sim | Como abrir o jogo depois de instalado. |
| `controls` | não | O controle do celular feito para o jogo ("Padrão do jogo"). Ver "Controle no celular". |

## Controle no celular (`controls`, opcional)

O jogo pode trazer o controle do celular feito para ele. No Hub ele vira o **"Padrão do jogo"**: quando o
jogo abre, os celulares trocam sozinhos para ele; quando a pessoa volta para a biblioteca, cada um volta para o
seu. Quem preferir outro troca no menu do celular durante a partida.

O jeito fácil: monte no editor do Hub (**Controles → Criar layout**), com os modos e o nome de cada botão, e
copie as peças. Cada peça tem o centro (`x`, `y`, de 0 a 1 numa tela de celular deitado), o tamanho `s`
(0,6 a 1,6) e, opcional, o `label`: o nome da ação, que aparece no botão.

```jsonc
"controls": {
  "modes": [
    { "name": "A pé", "pieces": [
      { "kind": "stickL", "x": 0.14, "y": 0.5 },
      { "kind": "A", "x": 0.74, "y": 0.78, "s": 1.25, "label": "Pular" },
      { "kind": "X", "x": 0.62, "y": 0.55, "s": 1.25, "label": "Item" },
      { "kind": "RT", "x": 0.9, "y": 0.1, "s": 1.15, "label": "Impulso" }
    ] },
    { "name": "Veículo", "pieces": [ /* ... */ ] }
  ]
}
```

Regras (o `pa validate` confere e o Hub também): de 1 a 6 modos; em cada um, peças sem repetir, dentro da
tela, fora do meio do topo (onde fica o Menu), sem uma em cima da outra, com um analógico ou as setas e ao
menos um de A, B, X ou Y; nomes de até 12 letras. Peças: `stickL`, `stickR`, `dpad`, `A`, `B`, `X`, `Y`,
`LB`, `RB`, `LT`, `RT`, `BACK`, `START`, `LS`, `RS`. Os analógicos são livres: no celular, nascem onde o dedo
tocar na metade da tela em que estão.

Jogos comuns (sem manifesto, como os da Steam) também ganham um padrão: na biblioteca, **Organizar → editar o
jogo → Controle no celular**.

## A capa

A capa é a primeira impressão na Biblioteca, então vale caprichar:

- Retrato **3:4** (ex.: 900×1200), arte que funcione pequena e grande.
- O nome do jogo pode estar na arte, grande. A UI também escreve o nome embaixo.
- Nada de arte do jogo-base que você não tem direito de usar como "capa oficial". Prefira arte própria.
- Sem capa, o Hub recusa o manifesto e explica como criar uma (o template tem uma capa modelo editável).

## `party-anywhere.release.json` (anexo de cada release)

Gerado pelo `pa pack` a partir da pasta de build. O hash não fica no repositório porque só existe depois
do pacote pronto.

```json
{
  "manifestVersion": 1,
  "id": "com.exemplo.sa-party",
  "version": "0.3.0",
  "asset": "sa-party-0.3.0.zip",
  "sha256": "9f2c…",
  "size": 4815162,
  "sdkAbi": 1
}
```

O Hub confere: mesmo `id` do repositório, `version` igual à tag, o pacote anexado com o mesmo tamanho e,
depois de baixar, o mesmo SHA-256. Qualquer diferença: nada é instalado e a mensagem diz o porquê.

## Publicar uma versão

1. Suba `version` no manifesto.
2. Crie a tag igual (`v0.3.0`). O workflow do template (`.github/workflows/release.yml`) compila, roda
   `pa validate` e `pa pack` e publica o release com o pacote e o `party-anywhere.release.json`.
3. Quem já instalou vê o aviso de atualização no Hub (o Hub consulta o último release com ETag: sem
   novidade, a consulta nem conta no limite do GitHub).

Sem o workflow: `pa pack --from build --tag v0.3.0` e anexe os dois arquivos de `release/` ao release.

## Ferramentas para quem cria

| Ferramenta | O que faz |
|---|---|
| `pa hub` / `pa hub install` | Confere se o Party Anywhere está instalado e atualizado; instala ou atualiza (instalador conferido pelo hash). |
| `pa games [busca]` | Jogos instalados (Steam e Epic) com pasta, executável e aviso de anti-cheat. |
| `pa new <pasta> --name "Jogo"` | Projeto pronto: manifesto, capa provisória 900×1200, SDK com exemplo, workflow, `.mcp.json` e skill para agentes. |
| `pa new <pasta> --name "Mod" --base-game "<nome ou appid>"` | O mod co-op de um jogo instalado: `requires.baseGame` (nome, appid, executável) e `install.target: "baseGame"` já preenchidos. Recusa jogos com anti-cheat. |
| `pa validate` | Confere manifesto e capa, com o que fazer em cada problema. |
| `pa pack --from build --tag vX.Y.Z` | Pacote `.zip` reproduzível + `party-anywhere.release.json`. |
| `pa inspect dono/repo` | Lê o jogo publicado como o Hub lê e diz se instala. |
| `pa mcp` | As mesmas ferramentas para agentes de IA (Model Context Protocol, local, por stdio). Também serve a documentação do SDK e do manifesto. |

## Ser encontrado ("Descobrir")

Adicione o tópico **`party-anywhere-game`** ao repositório no GitHub. A aba "Descobrir" da Biblioteca
lista os repositórios públicos com esse tópico (os mais estrelados primeiro, sem arquivados). O selo
**Verificado** continua vindo do catálogo revisado.

## Do link à Biblioteca (o que o Hub faz)

```
URL colada → lê party-anywhere.json (branch padrão) → valida o esquema → baixa a capa → prévia
          → [Instalar] → baixa o pacote do release da versão → confere o sha256
          → acha o jogo-base (Steam) e confere o exe → copia com backup → aparece na Biblioteca
```

## Entrar no catálogo (selo "Verificado")

1. Abra um PR no repositório `party-anywhere/catalog` adicionando o seu jogo ao `index.json`
   (id, URL do repositório, versão, sha256 do pacote).
2. Mantenedores instalam, testam e revisam o pacote.
3. Aprovado: o jogo aparece para todos na Biblioteca com o selo **Verificado**. Versões novas passam
   por revisão leve antes de chegarem como atualização "verificada".
