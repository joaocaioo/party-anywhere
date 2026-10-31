//! Operações de quem cria jogos: criar o projeto, validar e empacotar um release. A CLI `pa` e o servidor
//! MCP (`pa mcp`) chamam as mesmas funções, então humano e agente veem as mesmas regras e mensagens.

pub mod docs;
pub mod host;
pub mod mcp;
mod template;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use pa_manifest::{check_cover, GameManifest, InstallTarget, Issue, ReleaseManifest, MANIFEST_FILE, RELEASE_FILE};
use serde::Serialize;

/// Pasta que o `pa pack` empacota, se nada for dito.
pub const DEFAULT_BUILD_DIR: &str = "build";
/// Onde o pacote e o anexo do release são gravados.
pub const RELEASE_DIR: &str = "release";

#[derive(Clone, Debug, Default)]
pub struct NewOptions {
    pub name: String,
    pub id: Option<String>,
    pub tagline: Option<String>,
    /// Mod co-op de um jogo instalado: o projeto já nasce apontando para ele.
    pub base_game: Option<BaseGameSpec>,
}

/// O jogo-base de um mod, achado na Steam ou na Epic deste PC.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseGameSpec {
    pub name: String,
    pub steam_app_id: Option<u32>,
    pub epic_app_name: Option<String>,
    /// Executável, relativo à pasta do jogo.
    pub exe: String,
}

/// Acha o jogo instalado (nome ou appid da Steam) e confere se dá para fazer um mod co-op dele.
pub fn resolve_base_game(query: &str) -> Result<BaseGameSpec, String> {
    use pa_library::pcgames;
    let found = pcgames::find(query);
    let game = match found.as_slice() {
        [] => return Err(format!("não achei \"{query}\" na Steam nem na Epic deste PC. `pa games` lista o que está instalado.")),
        [one] => one.clone(),
        many => match many.iter().find(|g| g.name.eq_ignore_ascii_case(query.trim())) {
            Some(exact) => exact.clone(),
            None => {
                let names: Vec<String> = many.iter().map(|g| format!("{} ({})", g.name, g.steam_app_id.map(|i| i.to_string()).unwrap_or(g.store.clone()))).collect();
                return Err(format!("\"{query}\" combina com vários jogos: {}. Seja mais específico ou use o appid.", names.join(", ")));
            }
        },
    };
    let dir = game.install_dir.clone().ok_or_else(|| format!("não sei onde {} está instalado.", game.name))?;
    if let Some(ac) = pcgames::anti_cheat(&dir) {
        return Err(format!(
            "{} usa anti-cheat ({ac}). O Party Anywhere não faz mods para esses jogos: contornar anti-cheat está fora do \
             escopo e pode banir a conta de quem joga. Escolha um jogo sem anti-cheat.",
            game.name
        ));
    }
    let exe = pcgames::exe_candidates(&dir).into_iter().next().ok_or_else(|| format!("não achei o executável de {} em {}.", game.name, dir.display()))?;
    Ok(BaseGameSpec { name: game.name, steam_app_id: game.steam_app_id, epic_app_name: game.epic_app_name, exe })
}

/// Cria um projeto de jogo já válido: manifesto, capa provisória, workflow de release, AGENTS.md e skill.
pub fn new_project(dir: &Path, opts: &NewOptions) -> Result<Vec<PathBuf>, String> {
    if dir.join(MANIFEST_FILE).exists() {
        return Err(format!("{} já tem um {MANIFEST_FILE}. Use `pa validate` para conferir.", dir.display()));
    }
    let name = opts.name.trim();
    if name.is_empty() {
        return Err("diga o nome do jogo: pa new <pasta> --name \"Meu Jogo\"".into());
    }
    let slug = slugify(name);
    if slug.is_empty() {
        return Err("o nome precisa ter ao menos uma letra ou número.".into());
    }
    let id = opts.id.clone().unwrap_or_else(|| format!("com.seu-usuario.{slug}"));
    if !pa_manifest::is_valid_id(&id) {
        return Err(format!("\"{id}\" não é um id válido. Use com.seu-usuario.{slug}."));
    }
    let tagline = opts.tagline.clone().unwrap_or_else(|| "Uma frase curta que vende o jogo.".into());
    let manifest = template::manifest(&id, name, &tagline, &slug, opts.base_game.as_ref())?;

    let files: Vec<(&str, Vec<u8>)> = vec![
        (MANIFEST_FILE, manifest.into_bytes()),
        ("README.md", template::readme(name, &tagline, opts.base_game.as_ref()).into_bytes()),
        ("assets/cover.png", template::placeholder_cover()?),
        ("assets/cover.svg", template::COVER_SVG.as_bytes().to_vec()),
        (".gitignore", template::GITIGNORE.as_bytes().to_vec()),
        ("AGENTS.md", template::AGENTS.as_bytes().to_vec()),
        ("CLAUDE.md", template::CLAUDE.as_bytes().to_vec()),
        (".github/workflows/release.yml", template::RELEASE_WORKFLOW.as_bytes().to_vec()),
        (".claude/skills/party-anywhere-game/SKILL.md", template::SKILL.as_bytes().to_vec()),
        ("sdk/party_anywhere.h", template::SDK_HEADER.as_bytes().to_vec()),
        ("sdk/examples/arena_console.c", template::SDK_EXAMPLE.as_bytes().to_vec()),
    ];
    let mut created = Vec::new();
    for (rel, bytes) in files {
        let path = dir.join(rel);
        if path.exists() {
            continue; // nunca sobrescreve o que a pessoa já tem
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("não consegui criar {}: {e}", parent.display()))?;
        }
        fs::write(&path, bytes).map_err(|e| format!("não consegui gravar {}: {e}", path.display()))?;
        created.push(path);
    }
    Ok(created)
}

#[derive(Debug, Serialize)]
pub struct Validation {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub issues: Vec<Issue>,
    #[serde(skip)]
    pub manifest: Option<GameManifest>,
}

/// Confere o manifesto e os arquivos que ele cita (capa e afins).
pub fn validate_dir(dir: &Path) -> Validation {
    let path = dir.join(MANIFEST_FILE);
    let json = match fs::read_to_string(&path) {
        Ok(j) => j,
        Err(_) => {
            return Validation {
                ok: false,
                id: None,
                version: None,
                issues: vec![Issue::error(
                    MANIFEST_FILE,
                    format!("não achei {}. Crie o projeto com `pa new` ou copie templates/game.", path.display()),
                )],
                manifest: None,
            }
        }
    };
    let (manifest, mut issues) = match GameManifest::parse(&json) {
        Ok(ok) => ok,
        Err(issues) => return Validation { ok: false, id: None, version: None, issues, manifest: None },
    };
    // o que o `pa new` deixou para a pessoa trocar: avisa (publicar assim confunde quem vê na Biblioteca)
    if manifest.id.contains("seu-usuario") || manifest.authors.iter().any(|a| a == "seu-usuario") {
        issues.push(Issue::warning("id", "troque \"seu-usuario\" pelo seu usuário do GitHub (no id e em authors). O id não muda depois de publicado."));
    }
    if manifest.tagline == "Uma frase curta que vende o jogo." {
        issues.push(Issue::warning("tagline", "escreva a frase que aparece embaixo da capa (ex.: \"Tela dividida para 4 no mundo inteiro\")."));
    }
    match fs::read(dir.join(&manifest.art.cover)) {
        Ok(bytes) => {
            if let Err(issue) = check_cover(&bytes) {
                issues.push(issue);
            }
            if template::placeholder_cover().is_ok_and(|p| p == bytes) {
                issues.push(Issue::warning("art.cover", "a capa ainda é a provisória: troque assets/cover.png pela arte do seu mod (retrato 3:4)."));
            }
        }
        Err(_) => issues.push(Issue::error(
            "art.cover",
            format!("não achei {}. A capa é obrigatória (retrato 3:4, mínimo 600×800).", manifest.art.cover),
        )),
    }
    for (field, p) in [("art.hero", &manifest.art.hero), ("art.logo", &manifest.art.logo)] {
        if let Some(p) = p {
            if !dir.join(p).exists() {
                issues.push(Issue::error(field, format!("não achei {p}.")));
            }
        }
    }
    if let Some(d) = manifest.description.as_deref().filter(|d| d.to_ascii_lowercase().ends_with(".md")) {
        if !dir.join(d).exists() {
            issues.push(Issue::warning("description", format!("não achei {d}; a Biblioteca vai mostrar só a tagline.")));
        }
    }
    let ok = !issues.iter().any(Issue::is_error);
    Validation { ok, id: Some(manifest.id.clone()), version: Some(manifest.version.clone()), issues, manifest: Some(manifest) }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Packed {
    pub package: PathBuf,
    pub release_manifest: PathBuf,
    pub release: ReleaseManifest,
    pub files: usize,
}

/// Gera `release/<pacote>.zip` e `release/party-anywhere.release.json` a partir da pasta de build.
pub fn pack_dir(dir: &Path, from: Option<&Path>, tag: Option<&str>) -> Result<Packed, Vec<Issue>> {
    let v = validate_dir(dir);
    let Some(manifest) = v.manifest.filter(|_| v.ok) else { return Err(v.issues) };
    if let Some(tag) = tag {
        if tag.trim_start_matches('v') != manifest.version.trim_start_matches('v') {
            return Err(vec![Issue::error(
                "version",
                format!("a tag é {tag}, mas o manifesto diz {}. Suba a versão no {MANIFEST_FILE} antes da tag.", manifest.version),
            )]);
        }
    }
    let src = dir.join(from.unwrap_or(Path::new(DEFAULT_BUILD_DIR)));
    let mut entries = Vec::new();
    collect_files(&src, &src, &mut entries).map_err(|_| {
        vec![Issue::error("build", format!("não achei {}. Compile o jogo com a saída nessa pasta (ou use --from).", src.display()))]
    })?;
    if entries.is_empty() {
        return Err(vec![Issue::error("build", format!("{} está vazia.", src.display()))]);
    }
    entries.sort();
    let has = |rel: &str| entries.iter().any(|e| e.eq_ignore_ascii_case(&rel.replace('\\', "/")));
    let mut missing = Vec::new();
    for (i, f) in manifest.install.files.iter().enumerate() {
        if !has(&f.from) {
            missing.push(Issue::error(&format!("install.files[{i}].from"), format!("{} não está no build.", f.from)));
        }
    }
    if manifest.install.target == InstallTarget::SelfDir && !has(&manifest.launch.exe) {
        missing.push(Issue::error("launch.exe", format!("{} não está no build: o Hub não teria o que abrir.", manifest.launch.exe)));
    }
    if !missing.is_empty() {
        return Err(missing);
    }

    let io_err = |e: std::io::Error| vec![Issue::error("release", format!("não consegui gravar o pacote: {e}"))];
    let zip_err = |e: zip::result::ZipError| vec![Issue::error("release", format!("não consegui gerar o zip: {e}"))];
    let out_dir = dir.join(RELEASE_DIR);
    fs::create_dir_all(&out_dir).map_err(io_err)?;
    let package = out_dir.join(manifest.asset_name());
    {
        let file = fs::File::create(&package).map_err(io_err)?;
        let mut zip = zip::ZipWriter::new(file);
        // sem data de modificação: o mesmo build gera o mesmo pacote (e o mesmo hash)
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for rel in &entries {
            zip.start_file(rel.as_str(), opts).map_err(zip_err)?;
            zip.write_all(&fs::read(src.join(rel)).map_err(io_err)?).map_err(io_err)?;
        }
        zip.finish().map_err(zip_err)?;
    }
    let bytes = fs::read(&package).map_err(io_err)?;
    let release = ReleaseManifest::for_package(&manifest, &bytes);
    let release_manifest = out_dir.join(RELEASE_FILE);
    let json = serde_json::to_string_pretty(&release).expect("ReleaseManifest serializa");
    fs::write(&release_manifest, json + "\n").map_err(io_err)?;
    Ok(Packed { package, release_manifest, release, files: entries.len() })
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(root, &path, out)?;
        } else {
            let rel = path.strip_prefix(root).expect("dentro da raiz");
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// "Arena Blast!" → "arena-blast".
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(fold_accent) {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

fn fold_accent(c: char) -> Option<char> {
    Some(match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
        'ç' | 'Ç' => 'c',
        'ñ' | 'Ñ' => 'n',
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let opts = NewOptions { name: "Arena Blast".into(), id: Some("com.joao.arena-blast".into()), ..Default::default() };
        let created = new_project(dir.path(), &opts).unwrap();
        assert!(created.len() >= 10);
        dir
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("Arena Blast!"), "arena-blast");
        assert_eq!(slugify("  São João: a Festa  "), "sao-joao-a-festa");
        assert_eq!(slugify("!!!"), "");
    }

    #[test]
    fn new_project_is_valid_out_of_the_box() {
        let dir = project();
        let v = validate_dir(dir.path());
        assert!(v.ok, "{:?}", v.issues);
        // só os lembretes do que trocar antes de publicar (a capa provisória), nenhum erro
        assert!(v.issues.iter().all(|i| !i.is_error()), "{:?}", v.issues);
        assert!(v.issues.iter().any(|i| i.field == "art.cover" && i.message.contains("provisória")));
        let m = v.manifest.unwrap();
        assert_eq!((m.id.as_str(), m.name.as_str(), m.launch.exe.as_str()), ("com.joao.arena-blast", "Arena Blast", "arena-blast.exe"));
        assert!(new_project(dir.path(), &NewOptions { name: "x".into(), ..Default::default() }).is_err(), "não sobrescreve");
    }

    #[test]
    fn missing_manifest_or_cover_says_what_to_do() {
        let empty = tempfile::tempdir().unwrap();
        assert!(validate_dir(empty.path()).issues[0].message.contains("pa new"));

        let dir = project();
        fs::remove_file(dir.path().join("assets/cover.png")).unwrap();
        let v = validate_dir(dir.path());
        assert!(!v.ok);
        assert!(v.issues.iter().any(|i| i.field == "art.cover" && i.is_error()));
    }

    #[test]
    fn pack_builds_a_verifiable_release() {
        let dir = project();
        let build = dir.path().join("build");
        fs::create_dir_all(build.join("data")).unwrap();
        fs::write(build.join("arena-blast.exe"), b"MZ fake exe").unwrap();
        fs::write(build.join("data/level1.bin"), b"level").unwrap();

        let p = pack_dir(dir.path(), None, Some("v0.1.0")).unwrap();
        assert_eq!(p.files, 2);
        assert_eq!(p.package.file_name().unwrap(), "arena-blast-0.1.0.zip");
        let bytes = fs::read(&p.package).unwrap();
        assert!(p.release.verify(&bytes));

        let written: ReleaseManifest = serde_json::from_str(&fs::read_to_string(&p.release_manifest).unwrap()).unwrap();
        assert_eq!(written, p.release);

        // mesmo build, mesmo hash
        let again = pack_dir(dir.path(), None, None).unwrap();
        assert_eq!(again.release.sha256, p.release.sha256);
    }

    #[test]
    fn coop_mod_of_an_installed_game_points_at_it() {
        let dir = tempfile::tempdir().unwrap();
        let base = BaseGameSpec { name: "Grand Theft Auto: San Andreas".into(), steam_app_id: Some(12120), epic_app_name: None, exe: "gta_sa.exe".into() };
        let opts = NewOptions { name: "San Andreas Party".into(), base_game: Some(base), ..Default::default() };
        new_project(dir.path(), &opts).unwrap();
        let v = validate_dir(dir.path());
        assert!(v.ok, "{:?}", v.issues);
        let m = v.manifest.unwrap();
        let b = m.requires.unwrap().base_game.unwrap();
        assert_eq!((b.steam_app_id, b.exe.as_str()), (Some(12120), "gta_sa.exe"));
        assert_eq!(m.install.target, InstallTarget::BaseGame);
        assert_eq!(m.install.files[0].from, "san-andreas-party.dll");
        assert_eq!(m.launch.exe, "gta_sa.exe");
        assert!(fs::read_to_string(dir.path().join("README.md")).unwrap().contains("Grand Theft Auto: San Andreas"));

        // o build do mod tem a DLL, não o jogo
        fs::create_dir_all(dir.path().join("build")).unwrap();
        fs::write(dir.path().join("build/san-andreas-party.dll"), b"MZ").unwrap();
        assert!(pack_dir(dir.path(), None, None).is_ok());
    }

    #[test]
    fn pack_refuses_wrong_tag_missing_build_and_missing_exe() {
        let dir = project();
        assert_eq!(pack_dir(dir.path(), None, None).unwrap_err()[0].field, "build");
        fs::create_dir_all(dir.path().join("build")).unwrap();
        fs::write(dir.path().join("build/outro.exe"), b"x").unwrap();
        assert_eq!(pack_dir(dir.path(), None, None).unwrap_err()[0].field, "launch.exe");
        assert_eq!(pack_dir(dir.path(), None, Some("v9.9.9")).unwrap_err()[0].field, "version");
    }
}
