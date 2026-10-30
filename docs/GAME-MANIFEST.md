# `party-anywhere.json`: o arquivo que faz um jogo aparecer na Biblioteca

Todo jogo ou mod feito para o Party Anywhere tem **um arquivo `party-anywhere.json` na raiz do
repositório** e **uma imagem de capa**. Com isso, qualquer pessoa cola o link do repositório no Hub
e o jogo aparece na Biblioteca, pronto para instalar.

O modelo está em [`templates/game/`](../templates/game/). Os agentes de IA (skills em `.claude/skills/`)
devem criar este arquivo e **pedir a capa ao criador** sempre que começarem um jogo novo.

## Exemplo completo

```jsonc
{
  "$schema": "https://party-anywhere.dev/schema/game-manifest.v1.json",
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
      "exe": "gta_sa.exe",
      "acceptedSha256": ["…"],            // versões aceitas do executável
      "help": "docs/versao-do-jogo.md"    // o que fazer se a versão não bater
    }
  },

  "install": {
    "source": "github-release",           // v1: só releases do GitHub
    "asset": "sa-party-{version}.zip",    // pacote do release
    "sha256": "…",                        // hash do pacote; o Hub recusa se não bater
    "target": "baseGame",                 // "baseGame" (copia para a pasta do jogo) ou "self"
    "files": [ { "from": "SAParty.asi", "to": "SAParty.asi" } ]
  },

  "launch": {
    "exe": "gta_sa.exe",                  // relativo ao target
    "args": [],
    "workingDir": "."
  }
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
| `requires.baseGame` | não | Use quando o mod precisa do jogo original. O Hub procura na Steam e confere o hash. |
| `install` | sim | v1 aceita só `github-release` e cópia de arquivos. **Sem scripts de instalação.** |
| `launch` | sim | Como abrir o jogo depois de instalado. |

## A capa

A capa é a primeira impressão na Biblioteca, então vale caprichar:

- Retrato **3:4** (ex.: 900×1200), arte que funcione pequena e grande.
- O nome do jogo pode estar na arte, grande. A UI também escreve o nome embaixo.
- Nada de arte do jogo-base que você não tem direito de usar como "capa oficial". Prefira arte própria.
- Sem capa, o Hub recusa o manifesto e explica como criar uma (o template tem uma capa modelo editável).

## Publicar uma versão

1. Atualize `version` no manifesto.
2. Gere o pacote e calcule o SHA-256 (o template traz um script e um workflow do GitHub Actions que fazem
   isso e preenchem `install.sha256`).
3. Crie o release com a tag igual à versão e anexe o pacote.
4. Quem já instalou recebe a atualização no Hub.

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
