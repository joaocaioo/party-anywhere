//! Avisos de atualização: dos jogos instalados por link e do próprio Hub. Usa ETag: quando nada mudou o
//! GitHub responde 304, que não gasta a cota de consultas.

use pa_manifest::RepoRef;
use serde::Serialize;

use crate::github::GhRelease;
use crate::http::{Http, MAX_JSON_BYTES};
use crate::store::{Library, Source};

/// Repositório onde o Hub é publicado (releases públicos a partir da primeira versão).
pub const HUB_REPO: &str = "joaocaioo/party-anywhere";
/// Nome do instalador do Hub nos releases.
pub const HUB_INSTALLER_SUFFIX: &str = "-setup.exe";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameUpdate {
    pub id: String,
    pub name: String,
    pub from: String,
    pub to: String,
}

/// Confere cada jogo instalado por link. Marca `update` nas entradas e devolve as novidades.
/// Falha de rede num jogo não impede os outros.
pub fn check_games(http: &dyn Http, library: &mut Library) -> Vec<GameUpdate> {
    let mut found = Vec::new();
    for entry in &mut library.games {
        let (Source::Repo { repo }, Some(installed)) = (&entry.source, &entry.installed) else { continue };
        // instalado da pasta do projeto: a versão nova vem de um novo `pa add`, não do GitHub
        if repo.starts_with(crate::github::LOCAL_PREFIX) {
            continue;
        }
        let Some(repo) = RepoRef::parse(repo) else { continue };
        let Ok(resp) = http.get(&repo.api("releases/latest"), entry.etag.as_deref(), MAX_JSON_BYTES) else { continue };
        if resp.status == 304 || !resp.ok() {
            continue;
        }
        entry.etag = resp.etag.clone();
        let Ok(gh) = serde_json::from_slice::<GhRelease>(&resp.body) else { continue };
        let (Some(latest), Some(current)) = (parse(&gh.tag_name), parse(&installed.version)) else { continue };
        if latest > current {
            let to = latest.to_string();
            if entry.update.as_deref() != Some(&to) {
                found.push(GameUpdate { id: entry.id.clone(), name: entry.name.clone(), from: current.to_string(), to: to.clone() });
            }
            entry.update = Some(to);
        } else {
            entry.update = None;
        }
    }
    found
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubUpdate {
    pub version: String,
    pub installer_url: String,
    /// `<instalador>.sha256` publicado junto, quando o release tem.
    pub sha256_url: Option<String>,
    pub notes: String,
    pub etag: Option<String>,
}

/// Há um Hub mais novo que `current`? `Ok(None)` também quando nada mudou desde `etag`.
pub fn check_hub(http: &dyn Http, current: &str, etag: Option<&str>) -> Result<Option<HubUpdate>, String> {
    let Some(latest) = latest_hub(http, etag)? else { return Ok(None) };
    let (Some(l), Some(c)) = (parse(&latest.version), parse(current)) else { return Ok(None) };
    Ok((l > c).then_some(latest))
}

/// O último Hub publicado, com instalador. `Ok(None)`: nada publicado ainda, ou nada mudou desde `etag`.
pub fn latest_hub(http: &dyn Http, etag: Option<&str>) -> Result<Option<HubUpdate>, String> {
    #[derive(serde::Deserialize)]
    struct Rel {
        tag_name: String,
        #[serde(default)]
        body: Option<String>,
        #[serde(default)]
        assets: Vec<Asset>,
    }
    #[derive(serde::Deserialize)]
    struct Asset {
        name: String,
        browser_download_url: String,
    }
    let repo = RepoRef::parse(HUB_REPO).expect("repo do Hub");
    let resp = http.get(&repo.api("releases/latest"), etag, MAX_JSON_BYTES)?;
    if resp.status == 304 || resp.status == 404 {
        return Ok(None);
    }
    if !resp.ok() {
        return Err(format!("o GitHub respondeu {}.", resp.status));
    }
    let rel: Rel = serde_json::from_slice(&resp.body).map_err(|_| "resposta inesperada do GitHub.".to_string())?;
    let Some(version) = parse(&rel.tag_name) else { return Ok(None) };
    let Some(installer) = rel.assets.iter().find(|a| a.name.ends_with(HUB_INSTALLER_SUFFIX)) else { return Ok(None) };
    let sha = format!("{}.sha256", installer.name);
    let sha256_url = rel.assets.iter().find(|a| a.name == sha).map(|a| a.browser_download_url.clone());
    // notas curtas: as 3 primeiras linhas com conteúdo (UX: notas da versão em 3 linhas)
    let notes = rel.body.unwrap_or_default().lines().map(str::trim).filter(|l| !l.is_empty()).take(3).collect::<Vec<_>>().join("\n");
    Ok(Some(HubUpdate { version: version.to_string(), installer_url: installer.browser_download_url.clone(), sha256_url, notes, etag: resp.etag }))
}

fn parse(v: &str) -> Option<semver::Version> {
    semver::Version::parse(v.trim().trim_start_matches('v')).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::fake::FakeHttp;
    use crate::store::tests::entry;
    use crate::store::Installed;

    fn installed(v: &str) -> Option<Installed> {
        Some(Installed { version: v.into(), dir: "C:/x".into(), exe: "x.exe".into(), args: vec![], working_dir: ".".into(), base_dir: None })
    }

    #[test]
    fn finds_new_game_versions_once_and_uses_etag() {
        let mut lib = Library::new();
        lib.upsert(crate::store::LibraryEntry { installed: installed("0.1.0"), ..entry("com.a.kart") });
        lib.upsert(crate::store::LibraryEntry { installed: None, ..entry("com.a.naoinstalado") });
        let mut http = FakeHttp::default();
        http.on("https://api.github.com/repos/joao/com.a.kart/releases/latest", 200, r#"{"tag_name":"v0.2.0","assets":[]}"#);

        let found = check_games(&http, &mut lib);
        assert_eq!(found, vec![GameUpdate { id: "com.a.kart".into(), name: "com.a.kart".into(), from: "0.1.0".into(), to: "0.2.0".into() }]);
        assert_eq!(lib.games[0].update.as_deref(), Some("0.2.0"));

        // segunda checagem: ETag → 304, nada novo, nenhum aviso repetido
        assert!(check_games(&http, &mut lib).is_empty());
        let calls = http.calls.borrow();
        assert_eq!(calls.len(), 2, "só o jogo instalado é consultado");
        assert!(calls[1].1.is_some(), "a segunda vai com If-None-Match");
    }

    #[test]
    fn hub_update_needs_a_newer_tag_and_an_installer() {
        let url = "https://api.github.com/repos/joaocaioo/party-anywhere/releases/latest";
        let mut http = FakeHttp::default();
        http.on(url, 200, r#"{"tag_name":"v0.2.0","body":"Bluetooth mais rápido\n\nLobby novo\nCorreções\nOutra","assets":[{"name":"PartyAnywhere-0.2.0-setup.exe","browser_download_url":"https://x/setup.exe"}]}"#);
        let u = check_hub(&http, "0.1.0", None).unwrap().unwrap();
        assert_eq!((u.version.as_str(), u.installer_url.as_str()), ("0.2.0", "https://x/setup.exe"));
        assert_eq!(u.notes.lines().count(), 3);
        assert!(check_hub(&http, "0.2.0", None).unwrap().is_none());
        assert!(check_hub(&FakeHttp::default(), "0.1.0", None).unwrap().is_none(), "sem release ainda");
    }
}
