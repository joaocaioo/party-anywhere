//! O jogo original que um mod precisa (`requires.baseGame`): onde ele está neste PC, se é a versão que o
//! mod usa e, se não está, como conseguir (instalar pela Steam ou pela Epic, ou a página da loja).
//!
//! Vale qualquer cópia que a pessoa tenha: Steam, Epic ou uma pasta escolhida (GOG, CD, cópia antiga).
//! O Hub nunca baixa o jogo original: só o mod, que vai para a pasta do jogo.

use std::fs;
use std::path::{Path, PathBuf};

use pa_manifest::{sha256_hex, BaseGame};
use serde::Serialize;

use crate::pcgames;

/// Onde o jogo foi achado.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundBase {
    /// A pasta do jogo (a que tem o executável): é para onde os arquivos do mod vão.
    pub dir: PathBuf,
    /// "Steam", "Epic", "Seu PC" ou "Pasta escolhida".
    pub source: String,
}

/// O resultado da procura, com tudo que a tela precisa para guiar a pessoa.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseCheck {
    pub name: String,
    pub exe: String,
    pub found: Option<FoundBase>,
    /// Achou algo, mas não serve (pasta sem o executável, versão diferente): o que dizer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
    /// O que fazer se a versão não bater (texto do autor do mod).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steam_install: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epic_install: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_url: Option<String>,
}

/// Procura o jogo. `chosen`: uma pasta ou um arquivo que a pessoa escolheu (tem prioridade).
pub fn check(base: &BaseGame, chosen: Option<&Path>) -> BaseCheck {
    let mut out = BaseCheck {
        name: base.name.clone(),
        exe: base.exe.clone(),
        found: None,
        problem: None,
        help: base.help.clone(),
        steam_install: base.steam_app_id.map(|id| format!("steam://install/{id}")),
        epic_install: base.epic_app_name.as_ref().map(|a| format!("com.epicgames.launcher://apps/{a}?action=install&silent=true")),
        store_url: base.store_url.clone().or_else(|| base.steam_app_id.map(|id| format!("https://store.steampowered.com/app/{id}"))),
    };
    let candidates: Vec<(PathBuf, &str)> = match chosen {
        Some(p) => vec![(p.to_path_buf(), "Pasta escolhida")],
        None => {
            let mut c = Vec::new();
            if let Some(dir) = base.steam_app_id.and_then(pcgames::steam_app_dir) {
                c.push((dir, "Steam"));
            }
            let games = pcgames::scan();
            if let Some(app) = &base.epic_app_name {
                let epic = games.iter().filter(|g| g.epic_app_name.as_ref().is_some_and(|a| a.eq_ignore_ascii_case(app)));
                c.extend(epic.filter_map(|g| g.install_dir.clone()).map(|d| (d, "Epic")));
            }
            // qualquer jogo do PC (Steam ou Epic) que tenha o executável
            c.extend(games.iter().filter_map(|g| g.install_dir.clone().map(|d| (d, if g.store == "Epic" { "Epic" } else { "Steam" }))));
            c
        }
    };
    for (path, source) in candidates {
        let Some(dir) = locate(&path, &base.exe) else {
            if chosen.is_some() {
                out.problem = Some(format!("Nessa pasta não tem o {}. Escolha a pasta onde o {} está instalado (ou o próprio {}).", base.exe, base.name, base.exe));
            }
            continue;
        };
        if !base.accepted_sha256.is_empty() {
            let hash = fs::read(dir.join(&base.exe)).map(|b| sha256_hex(&b)).unwrap_or_default();
            if !base.accepted_sha256.iter().any(|h| h.eq_ignore_ascii_case(&hash)) {
                out.problem = Some(format!("Achamos o {} em {}, mas não na versão que este mod usa.", base.name, dir.display()));
                continue;
            }
        }
        out.problem = None;
        out.found = Some(FoundBase { dir, source: source.into() });
        break;
    }
    out
}

/// A pasta do jogo a partir do que a pessoa escolheu: o próprio executável, a pasta dele ou uma pasta acima
/// (até dois níveis: "Jogos\GTA San Andreas\" ou o .exe dentro de "bin\").
pub fn locate(path: &Path, exe: &str) -> Option<PathBuf> {
    let exe_name = Path::new(exe).file_name()?.to_string_lossy().to_lowercase();
    let start = if path.is_file() { path.parent()?.to_path_buf() } else { path.to_path_buf() };
    // o executável do manifesto pode ter pasta ("bin/jogo.exe"): a raiz é onde esse caminho existe
    let fits = |dir: &Path| dir.join(exe).is_file();
    if fits(&start) {
        return Some(start);
    }
    if path.is_file() && path.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == exe_name) {
        // escolheu o .exe certo, mas o manifesto espera uma subpasta: sobe até a raiz
        let mut dir = start.clone();
        for _ in 0..3 {
            if fits(&dir) {
                return Some(dir);
            }
            dir = dir.parent()?.to_path_buf();
        }
    }
    let children = |dir: &Path| -> Vec<PathBuf> {
        fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default()
    };
    for child in children(&start) {
        if fits(&child) {
            return Some(child);
        }
        for grand in children(&child) {
            if fits(&grand) {
                return Some(grand);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(exe: &str, hashes: Vec<String>) -> BaseGame {
        BaseGame {
            name: "GTA San Andreas".into(),
            steam_app_id: None,
            epic_app_name: None,
            store_url: Some("https://exemplo.com/loja".into()),
            exe: exe.into(),
            accepted_sha256: hashes,
            help: None,
        }
    }

    #[test]
    fn finds_the_game_from_a_folder_above_or_the_exe_itself() {
        let root = tempfile::tempdir().unwrap();
        let game = root.path().join("Jogos/GTA San Andreas");
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("gta_sa.exe"), b"MZ jogo").unwrap();
        let b = base("gta_sa.exe", vec![]);
        for picked in [game.clone(), game.join("gta_sa.exe"), root.path().join("Jogos"), root.path().to_path_buf()] {
            let c = check(&b, Some(&picked));
            assert_eq!(c.found.as_ref().map(|f| f.dir.clone()), Some(game.clone()), "{}", picked.display());
            assert_eq!(c.found.unwrap().source, "Pasta escolhida");
        }
    }

    #[test]
    fn explains_a_wrong_folder_and_a_wrong_version() {
        let dir = tempfile::tempdir().unwrap();
        let c = check(&base("gta_sa.exe", vec![]), Some(dir.path()));
        assert!(c.found.is_none());
        assert!(c.problem.unwrap().contains("não tem o gta_sa.exe"));
        fs::write(dir.path().join("gta_sa.exe"), b"MZ outra versao").unwrap();
        let c = check(&base("gta_sa.exe", vec![sha256_hex(b"MZ versao certa")]), Some(dir.path()));
        assert!(c.found.is_none());
        assert!(c.problem.unwrap().contains("não na versão"));
        let ok = check(&base("gta_sa.exe", vec![sha256_hex(b"MZ outra versao")]), Some(dir.path()));
        assert!(ok.found.is_some() && ok.problem.is_none());
    }

    #[test]
    fn says_how_to_get_the_game() {
        let mut b = base("x.exe", vec![]);
        b.steam_app_id = Some(12120);
        b.epic_app_name = Some("Sugar".into());
        b.store_url = None;
        let c = check(&b, Some(Path::new("Z:/nao/existe")));
        assert_eq!(c.steam_install.as_deref(), Some("steam://install/12120"));
        assert!(c.epic_install.unwrap().starts_with("com.epicgames.launcher://apps/Sugar"));
        assert_eq!(c.store_url.as_deref(), Some("https://store.steampowered.com/app/12120"));
    }
}
