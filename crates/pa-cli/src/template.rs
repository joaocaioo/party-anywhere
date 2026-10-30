//! Arquivos do `pa new`, vindos de templates/game (a fonte única do template).

use pa_manifest::{BaseGame, GameManifest, InstallFile, InstallTarget, Requires, MANIFEST_FILE};

use crate::BaseGameSpec;

const MANIFEST: &str = include_str!("../../../templates/game/party-anywhere.json");
pub const COVER_SVG: &str = include_str!("../../../templates/game/assets/cover.svg");
pub const GITIGNORE: &str = include_str!("../../../templates/game/.gitignore");
pub const MCP_JSON: &str = include_str!("../../../templates/game/.mcp.json");
pub const RELEASE_WORKFLOW: &str = include_str!("../../../templates/game/.github/workflows/release.yml");
pub const AGENTS: &str = include_str!("../../../templates/game/AGENTS.md");
pub const CLAUDE: &str = include_str!("../../../templates/game/CLAUDE.md");
pub const SKILL: &str = include_str!("../../../templates/game/.claude/skills/party-anywhere-game/SKILL.md");
pub const SDK_HEADER: &str = include_str!("../../../sdk/c/party_anywhere.h");
pub const SDK_EXAMPLE: &str = include_str!("../../../sdk/c/examples/arena_console.c");

pub fn manifest(id: &str, name: &str, tagline: &str, slug: &str, base: Option<&BaseGameSpec>) -> Result<String, String> {
    let (mut m, _) = GameManifest::parse(MANIFEST).map_err(|_| "o template embutido está inválido".to_string())?;
    m.id = id.to_string();
    m.name = name.chars().take(pa_manifest::MAX_NAME_CHARS).collect();
    m.tagline = tagline.chars().take(pa_manifest::MAX_TAGLINE_CHARS).collect();
    m.authors = vec![id.split('.').nth(1).unwrap_or("seu-usuario").to_string()];
    m.install.asset = format!("{slug}-{{version}}.zip");
    m.install.files.clear();
    m.launch.exe = format!("{slug}.exe");
    if let Some(b) = base {
        // mod co-op: o pacote leva a DLL do mod para a pasta do jogo (com backup) e o Hub abre o próprio jogo
        m.requires = Some(Requires {
            base_game: Some(BaseGame {
                name: b.name.clone(),
                steam_app_id: b.steam_app_id,
                epic_app_name: b.epic_app_name.clone(),
                store_url: None,
                exe: b.exe.clone(),
                accepted_sha256: vec![],
                help: None,
            }),
        });
        m.install.target = InstallTarget::BaseGame;
        m.install.files = vec![InstallFile { from: format!("{slug}.dll"), to: format!("{slug}.dll") }];
        m.launch.exe = b.exe.clone();
        m.tags = vec!["mod".into(), "co-op".into()];
    }
    let issues: Vec<_> = m.validate().into_iter().filter(|i| i.is_error()).collect();
    if let Some(i) = issues.first() {
        return Err(format!("{MANIFEST_FILE} gerado ficou inválido: {i}"));
    }
    Ok(serde_json::to_string_pretty(&m).expect("manifesto serializa") + "\n")
}

pub fn readme(name: &str, tagline: &str, base: Option<&BaseGameSpec>) -> String {
    let mod_section = base
        .map(|b| {
            format!(
                "## Como este mod funciona\n\n\
                 Mod co-op local para **{}**. Quem instala precisa ter o jogo{}; o Hub acha a pasta sozinho, \
                 copia os arquivos do mod com backup e abre o jogo.\n\n\
                 - O mod é uma DLL carregada pelo jogo (por um proxy como `version.dll`/`dinput8.dll` ou um ASI \
                 loader). Ajuste `install.files` no `{MANIFEST_FILE}` para o nome que o jogo carrega.\n\
                 - Os jogadores chegam pelo `sdk/party_anywhere.h`: chame `pa_poll` no mesmo ponto em que o jogo \
                 lê os controles e use `pa_get_pad` para cada vaga.\n\
                 - Em `requires.baseGame.acceptedSha256`, coloque o hash do `{}` da versão que o mod suporta.\n\
                 - Só jogos sem anti-cheat, e nada de distribuir arquivos do jogo original.\n\n",
                b.name,
                if b.steam_app_id.is_some() { " na Steam" } else { "" },
                b.exe
            )
        })
        .unwrap_or_default();
    let needs = base
        .map(|b| format!("Precisa do **{}** (a sua cópia: Steam, Epic ou de onde for). O app acha sozinho ou deixa você escolher a pasta.\n\n", b.name))
        .unwrap_or_default();
    format!(
        "# {name}\n\n{tagline}\n\n\
         ## Jogar\n\n\
         {needs}\
         1. Instale o [Party Anywhere](https://github.com/joaocaioo/party-anywhere/releases/latest) (ele avisa quando há versão nova).\n\
         2. Na Biblioteca, em **Adicionar jogo → Por link**, cole o link deste repositório.\n\
         3. Conecte os controles ou escaneie o QR com o celular e aperte **Jogar**.\n\n\
         Com um agente de IA (Claude e outros), é só pedir *\"instale este mod no Party Anywhere\"* com o link deste \
         repositório: o `AGENTS.md` diz a ele o que fazer (conferir o app, instalar se faltar e levar o mod para a sua \
         Biblioteca).\n\n\
         {mod_section}## Desenvolver\n\n\
         - `pa validate` confere o manifesto e a capa.\n\
         - Crie a tag `vX.Y.Z` (igual ao `version` do `{MANIFEST_FILE}`) para publicar uma versão.\n"
    )
}

/// Capa provisória 900×1200 (3:4) para o projeto nascer válido. A de verdade vem do criador.
pub fn placeholder_cover() -> Result<Vec<u8>, String> {
    const W: u32 = 900;
    const H: u32 = 1200;
    let mut data = Vec::with_capacity((W * H * 3) as usize);
    for y in 0..H {
        let t = y as f32 / H as f32;
        for x in 0..W {
            // degradê verde do Hub com faixas diagonais discretas
            let stripe = if ((x + y) / 60) % 2 == 0 { 1.0 } else { 0.92 };
            let r = (8.0 + 22.0 * t) * stripe;
            let g = (20.0 + 120.0 * t) * stripe;
            let b = (12.0 + 40.0 * t) * stripe;
            data.extend_from_slice(&[r as u8, g as u8, b as u8]);
        }
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, W, H);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&data).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(out)
}
