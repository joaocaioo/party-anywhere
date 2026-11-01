//! Ler um jogo publicado como o Hub lê: manifesto na branch padrão, último release e o anexo dele.

use pa_manifest::{GameManifest, Issue, ReleaseManifest, RepoRef, DISCOVERY_TOPIC, MANIFEST_FILE, RELEASE_FILE};
use serde::{Deserialize, Serialize};

use crate::http::{Http, MAX_JSON_BYTES};

/// O que o GitHub diz de um release (só o que usamos).
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct GhRelease {
    pub tag_name: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<GhAsset>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Versão publicada pronta para instalar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestRelease {
    pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    pub release: ReleaseManifest,
    pub package_url: String,
}

/// Resultado de "colar o link": o que a prévia da Biblioteca mostra.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectReport {
    pub repo: String,
    pub url: String,
    pub installable: bool,
    pub summary: String,
    pub issues: Vec<Issue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<GameManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<LatestRelease>,
}

pub fn fetch_manifest(http: &dyn Http, repo: &RepoRef) -> Result<GameManifest, Vec<Issue>> {
    let resp = http.get(&repo.raw(MANIFEST_FILE), None, MAX_JSON_BYTES).map_err(|e| vec![Issue::error("rede", e)])?;
    if resp.status == 404 {
        return Err(vec![Issue::error(
            MANIFEST_FILE,
            format!("{repo} não tem {MANIFEST_FILE} na raiz (ou é privado). Quem fez o jogo precisa adicioná-lo."),
        )]);
    }
    if !resp.ok() {
        return Err(vec![Issue::error("rede", format!("o GitHub respondeu {} ao ler o manifesto.", resp.status))]);
    }
    let text = String::from_utf8_lossy(&resp.body);
    GameManifest::parse(&text).map(|(m, _)| m)
}

/// Último release estável com o anexo `party-anywhere.release.json` conferido contra o manifesto.
/// `Ok(None)` = repositório ainda sem release.
pub fn fetch_latest(http: &dyn Http, repo: &RepoRef, game: &GameManifest) -> Result<Option<LatestRelease>, Vec<Issue>> {
    let resp = http.get(&repo.api("releases/latest"), None, MAX_JSON_BYTES).map_err(|e| vec![Issue::error("rede", e)])?;
    if resp.status == 404 {
        return Ok(None);
    }
    if !resp.ok() {
        return Err(vec![api_issue(resp.status)]);
    }
    let gh: GhRelease = serde_json::from_slice(&resp.body)
        .map_err(|_| vec![Issue::error("release", "resposta inesperada do GitHub.")])?;
    resolve_release(http, game, gh).map(Some)
}

pub(crate) fn resolve_release(http: &dyn Http, game: &GameManifest, gh: GhRelease) -> Result<LatestRelease, Vec<Issue>> {
    let Some(asset) = gh.assets.iter().find(|a| a.name == RELEASE_FILE) else {
        return Err(vec![Issue::error(
            "release",
            format!("o release {} não tem o anexo {RELEASE_FILE}. Publique com `pa pack` (ou o workflow do template).", gh.tag_name),
        )]);
    };
    let resp = http.get(&asset.browser_download_url, None, MAX_JSON_BYTES).map_err(|e| vec![Issue::error("rede", e)])?;
    if !resp.ok() {
        return Err(vec![Issue::error("release", format!("não consegui baixar {RELEASE_FILE} ({}).", resp.status))]);
    }
    let release: ReleaseManifest = serde_json::from_slice(&resp.body)
        .map_err(|_| vec![Issue::error(RELEASE_FILE, "o anexo não é um JSON válido. Gere de novo com `pa pack`.")])?;
    let issues = release.validate(game, Some(&gh.tag_name));
    if issues.iter().any(Issue::is_error) {
        return Err(issues);
    }
    let Some(package) = gh.assets.iter().find(|a| a.name == release.asset) else {
        return Err(vec![Issue::error("release", format!("o pacote {} não está anexado ao release {}.", release.asset, gh.tag_name))]);
    };
    if package.size != release.size {
        return Err(vec![Issue::error("release", format!("{} tem tamanho diferente do declarado.", release.asset))]);
    }
    Ok(LatestRelease { tag: gh.tag_name, published_at: gh.published_at, package_url: package.browser_download_url.clone(), release })
}

/// Colou o link: lê tudo e diz, em português, se instala.
pub fn inspect(http: &dyn Http, input: &str) -> Result<InspectReport, String> {
    // a pasta de um projeto no PC (o criador jogando o próprio mod, sem publicar)
    let input = input.trim();
    let path = std::path::Path::new(input.strip_prefix(LOCAL_PREFIX).unwrap_or(input));
    if !input.contains("github.com") && path.is_dir() {
        return inspect_local(path);
    }
    let repo = RepoRef::parse(input).ok_or_else(|| {
        format!("\"{}\" não parece um link do GitHub. Use algo como https://github.com/dono/jogo.", input.trim())
    })?;
    let mut report = InspectReport {
        repo: repo.to_string(),
        url: repo.url(),
        installable: false,
        summary: String::new(),
        issues: vec![],
        manifest: None,
        cover_url: None,
        latest: None,
    };
    let game = match fetch_manifest(http, &repo) {
        Ok(g) => g,
        Err(issues) => {
            report.summary = format!("{repo} ainda não pode entrar na Biblioteca.");
            report.issues = issues;
            return Ok(report);
        }
    };
    report.cover_url = Some(match game.steam_cover() {
        Some(appid) => format!("steam:{appid}"),
        None => repo.raw(&game.art.cover),
    });
    report.issues.extend(game.validate().into_iter().filter(|i| !i.is_error()));
    match fetch_latest(http, &repo, &game) {
        Ok(Some(latest)) => {
            report.installable = true;
            report.summary = format!(
                "{} {} pronto para instalar ({} jogadores, {} MB).",
                game.name,
                latest.release.version,
                players_text(&game),
                (latest.release.size as f64 / 1_048_576.0).max(0.1).round()
            );
            report.latest = Some(latest);
        }
        Ok(None) => {
            report.summary = format!("{} existe, mas ainda não tem versão publicada (release).", game.name);
            report.issues.push(Issue::error("release", "crie a tag vX.Y.Z; o workflow do template publica o release."));
        }
        Err(issues) => {
            report.summary = format!("{}: a última versão publicada tem problemas.", game.name);
            report.issues.extend(issues);
        }
    }
    report.manifest = Some(game);
    Ok(report)
}

/// Prefixo do "repositório" de um mod instalado da pasta do projeto (sem publicar).
pub const LOCAL_PREFIX: &str = "local:";

/// O mesmo "colar o link", para a pasta de um projeto no PC: o criador joga o próprio mod sem publicar.
/// Precisa do `party-anywhere.json` e do pacote que o `pa pack` gera em `release/`.
pub fn inspect_local(dir: &std::path::Path) -> Result<InspectReport, String> {
    use pa_manifest::{ReleaseManifest, MANIFEST_FILE, RELEASE_FILE};
    let dir = dir.canonicalize().map_err(|_| format!("a pasta {} não existe.", dir.display()))?;
    let shown = dir.display().to_string().trim_start_matches(r"\\?\").to_string();
    let mut report = InspectReport {
        repo: format!("{LOCAL_PREFIX}{shown}"),
        url: shown.clone(),
        installable: false,
        summary: String::new(),
        issues: vec![],
        manifest: None,
        cover_url: None,
        latest: None,
    };
    let text = std::fs::read_to_string(dir.join(MANIFEST_FILE)).map_err(|_| format!("{shown} não tem {MANIFEST_FILE}: é a pasta do projeto do mod?"))?;
    let game = match GameManifest::parse(&text) {
        Ok((g, _)) => g,
        Err(issues) => {
            report.summary = "O party-anywhere.json tem problemas.".into();
            report.issues = issues;
            return Ok(report);
        }
    };
    report.cover_url = match game.steam_cover() {
        Some(appid) => Some(format!("steam:{appid}")),
        None => crate::pcgames::data_url(&dir.join(&game.art.cover)),
    };
    report.issues.extend(game.validate());
    let release = std::fs::read(dir.join("release").join(RELEASE_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<ReleaseManifest>(&b).ok());
    match release {
        None => report.issues.push(Issue::error("release", "falta o pacote: rode `pa pack --from build` na pasta do projeto.")),
        Some(rel) => {
            report.issues.extend(rel.validate(&game, None));
            let package = dir.join("release").join(&rel.asset);
            if !package.is_file() {
                report.issues.push(Issue::error("release", format!("falta {}: rode `pa pack` de novo.", rel.asset)));
            } else if !report.issues.iter().any(Issue::is_error) {
                report.installable = true;
                report.summary = format!("{} {} (do seu PC) pronto para instalar.", game.name, rel.version);
                report.latest = Some(LatestRelease {
                    tag: format!("v{}", rel.version),
                    published_at: None,
                    package_url: format!("file:///{}", package.display().to_string().trim_start_matches(r"\\?\").replace('\\', "/")),
                    release: rel,
                });
            }
        }
    }
    if !report.installable && report.summary.is_empty() {
        report.summary = format!("{} ainda não pode entrar na Biblioteca.", game.name);
    }
    report.manifest = Some(game);
    Ok(report)
}

fn players_text(g: &GameManifest) -> String {
    if g.players.min == g.players.max {
        g.players.max.to_string()
    } else {
        format!("{} a {}", g.players.min, g.players.max)
    }
}

/// Um repositório marcado com o tópico `party-anywhere-game`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovered {
    pub repo: String,
    pub description: Option<String>,
    pub stars: u64,
    pub updated_at: Option<String>,
}

/// "Descobrir": repositórios públicos com o tópico, os mais estrelados primeiro.
pub fn discover(http: &dyn Http) -> Result<Vec<Discovered>, String> {
    #[derive(Deserialize)]
    struct Search {
        items: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        full_name: String,
        description: Option<String>,
        stargazers_count: u64,
        pushed_at: Option<String>,
        #[serde(default)]
        archived: bool,
    }
    let url = format!("https://api.github.com/search/repositories?q=topic:{DISCOVERY_TOPIC}+fork:false&sort=stars&order=desc&per_page=50");
    let resp = http.get(&url, None, MAX_JSON_BYTES)?;
    if !resp.ok() {
        return Err(api_issue(resp.status).message);
    }
    let search: Search = serde_json::from_slice(&resp.body).map_err(|_| "resposta inesperada do GitHub.".to_string())?;
    Ok(search
        .items
        .into_iter()
        .filter(|i| !i.archived)
        .map(|i| Discovered { repo: i.full_name, description: i.description, stars: i.stargazers_count, updated_at: i.pushed_at })
        .collect())
}

/// Quanto tempo a lista do "Descobrir" vale: a busca do GitHub sem login aceita poucas consultas por minuto.
pub const DISCOVER_FRESH: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// A última lista do "Descobrir" e quando chegou.
#[derive(Default)]
pub struct DiscoverCache {
    last: Option<(std::time::Instant, Vec<Discovered>)>,
}

impl DiscoverCache {
    /// A lista guardada se ainda vale; senão busca de novo. Se o GitHub recusar (limite, sem internet), a última
    /// lista que chegou serve melhor que um erro.
    pub fn get(&mut self, now: std::time::Instant, http: &dyn Http) -> Result<Vec<Discovered>, String> {
        if let Some((at, items)) = &self.last {
            if now.duration_since(*at) < DISCOVER_FRESH {
                return Ok(items.clone());
            }
        }
        match discover(http) {
            Ok(items) => {
                self.last = Some((now, items.clone()));
                Ok(items)
            }
            Err(e) => self.last.as_ref().map(|(_, items)| items.clone()).ok_or(e),
        }
    }
}

fn api_issue(status: u16) -> Issue {
    match status {
        403 | 429 => Issue::error("rede", "o GitHub pediu uma pausa (limite de consultas). Tente de novo em alguns minutos."),
        _ => Issue::error("rede", format!("o GitHub respondeu {status}.")),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    #[test]
    fn a_project_folder_installs_like_a_published_repo() {
        use pa_manifest::ReleaseManifest;
        let dir = tempfile::tempdir().unwrap();
        let m = include_str!("../../../templates/game/party-anywhere.json");
        std::fs::write(dir.path().join("party-anywhere.json"), m).unwrap();
        let r = super::inspect(&crate::http::Ureq::default(), &dir.path().display().to_string()).unwrap();
        assert!(!r.installable, "sem pacote");
        assert!(r.issues.iter().any(|i| i.message.contains("pa pack")));
        let (game, _) = GameManifest::parse(m).unwrap();
        std::fs::create_dir_all(dir.path().join("release")).unwrap();
        let rel = ReleaseManifest::for_package(&game, b"PK zip");
        std::fs::write(dir.path().join("release").join(&rel.asset), b"PK zip").unwrap();
        std::fs::write(dir.path().join("release/party-anywhere.release.json"), serde_json::to_vec(&rel).unwrap()).unwrap();
        let r = super::inspect(&crate::http::Ureq::default(), &dir.path().display().to_string()).unwrap();
        assert!(r.installable, "{:?}", r.issues);
        assert!(r.repo.starts_with(super::LOCAL_PREFIX));
        let latest = r.latest.unwrap();
        let got = crate::http::Http::get(&crate::http::Ureq::default(), &latest.package_url, None, 1024).unwrap();
        assert!(latest.release.verify(&got.body), "o pacote lido do disco confere");
    }

    use super::*;
    use crate::http::fake::FakeHttp;

    pub const MANIFEST: &str = include_str!("../../../templates/game/party-anywhere.json");

    /// Um repositório publicado direitinho, com o pacote `package`.
    pub fn published(http: &mut FakeHttp, repo: &str, version: &str, package: &[u8]) -> ReleaseManifest {
        let r = RepoRef::parse(repo).unwrap();
        let manifest = MANIFEST.replace("\"version\": \"0.1.0\"", &format!("\"version\": \"{version}\""));
        let game = GameManifest::parse(&manifest).unwrap().0;
        let release = ReleaseManifest::for_package(&game, package);
        let base = format!("https://github.com/{repo}/releases/download/v{version}");
        let gh = serde_json::json!({
            "tag_name": format!("v{version}"),
            "published_at": "2026-10-29T12:00:00Z",
            "assets": [
                { "name": RELEASE_FILE, "browser_download_url": format!("{base}/{RELEASE_FILE}"), "size": 300 },
                { "name": release.asset, "browser_download_url": format!("{base}/{}", release.asset), "size": package.len() }
            ]
        });
        http.on(&r.raw(MANIFEST_FILE), 200, manifest)
            .on(&r.api("releases/latest"), 200, gh.to_string())
            .on(&format!("{base}/{RELEASE_FILE}"), 200, serde_json::to_string(&release).unwrap())
            .on(&format!("{base}/{}", release.asset), 200, package.to_vec());
        release
    }

    #[test]
    fn inspect_a_published_game() {
        let mut http = FakeHttp::default();
        published(&mut http, "joao/meu-jogo", "0.3.0", b"zip");
        let r = inspect(&http, "https://github.com/joao/meu-jogo").unwrap();
        assert!(r.installable, "{:?}", r.issues);
        assert_eq!(r.latest.unwrap().tag, "v0.3.0");
        assert!(r.summary.contains("Meu Jogo 0.3.0"));
        assert_eq!(r.cover_url.unwrap(), "https://raw.githubusercontent.com/joao/meu-jogo/HEAD/assets/cover.png");
    }

    #[test]
    fn inspect_explains_each_missing_piece() {
        let http = FakeHttp::default();
        assert!(inspect(&http, "gitlab.com/x/y").is_err());

        let r = inspect(&http, "joao/vazio").unwrap();
        assert!(!r.installable);
        assert!(r.issues[0].message.contains("party-anywhere.json"));

        let mut http = FakeHttp::default();
        http.on(&RepoRef::parse("joao/novo").unwrap().raw(MANIFEST_FILE), 200, MANIFEST);
        let r = inspect(&http, "joao/novo").unwrap();
        assert!(!r.installable);
        assert!(r.summary.contains("ainda não tem versão"));
    }

    #[test]
    fn release_without_attachment_or_with_wrong_size_is_refused() {
        let mut http = FakeHttp::default();
        published(&mut http, "joao/x", "1.0.0", b"pacote");
        let repo = RepoRef::parse("joao/x").unwrap();
        // tira o anexo do release
        let gh = serde_json::json!({ "tag_name": "v1.0.0", "assets": [] });
        http.on(&repo.api("releases/latest"), 200, gh.to_string());
        let r = inspect(&http, "joao/x").unwrap();
        assert!(!r.installable);
        assert!(r.issues.iter().any(|i| i.message.contains(RELEASE_FILE)));
    }

    #[test]
    fn discover_lists_topic_repos_without_archived() {
        let mut http = FakeHttp::default();
        let body = serde_json::json!({ "items": [
            { "full_name": "a/kart", "description": "Kart", "stargazers_count": 10, "pushed_at": null, "archived": false },
            { "full_name": "b/velho", "description": null, "stargazers_count": 99, "pushed_at": null, "archived": true }
        ]});
        http.on(
            "https://api.github.com/search/repositories?q=topic:party-anywhere-game+fork:false&sort=stars&order=desc&per_page=50",
            200,
            body.to_string(),
        );
        let found = discover(&http).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].repo, "a/kart");
    }

    #[test]
    fn discover_is_cached_and_survives_the_rate_limit() {
        let url = "https://api.github.com/search/repositories?q=topic:party-anywhere-game+fork:false&sort=stars&order=desc&per_page=50";
        let body = serde_json::json!({ "items": [{ "full_name": "a/kart", "description": null, "stargazers_count": 1, "pushed_at": null }] });
        let mut http = FakeHttp::default();
        http.on(url, 200, body.to_string());
        let mut cache = DiscoverCache::default();
        let t0 = std::time::Instant::now();
        assert_eq!(cache.get(t0, &http).unwrap().len(), 1);
        // dentro do prazo: nem pergunta ao GitHub (que agora recusaria)
        let mut limited = FakeHttp::default();
        limited.on(url, 403, "{}");
        assert_eq!(cache.get(t0 + std::time::Duration::from_secs(60), &limited).unwrap().len(), 1);
        // venceu e o GitHub recusa: a última lista serve
        assert_eq!(cache.get(t0 + DISCOVER_FRESH * 2, &limited).unwrap()[0].repo, "a/kart");
        // sem nada guardado, o erro diz o que fazer
        assert!(DiscoverCache::default().get(t0, &limited).unwrap_err().contains("alguns minutos"));
    }
}
