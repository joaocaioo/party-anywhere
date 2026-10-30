//! A Biblioteca do Hub ligada ao GitHub: colar um link e ver a prévia, descobrir jogos pelo tópico,
//! instalar com o hash conferido, guardar tudo em `library.json` e avisar quando sai versão nova
//! (dos jogos e do próprio Hub). A rede fica atrás de [`http::Http`]: tudo aqui é testável sem internet.

pub mod github;
pub mod http;
pub mod hubapp;
pub mod install;
pub mod pcgames;
pub mod store;
pub mod updates;
pub mod vigem;

pub use github::{discover, fetch_latest, fetch_manifest, inspect, Discovered, InspectReport, LatestRelease};
pub use store::{Installed, Library, LibraryEntry, LoadNote, Source};
pub use updates::{check_games, check_hub, latest_hub, GameUpdate, HubUpdate};

/// Entrada da Biblioteca para um jogo publicado (o que a capa e a tela do fliperama mostram).
pub fn entry_for(report: &InspectReport) -> Option<LibraryEntry> {
    let m = report.manifest.as_ref()?;
    let players = if m.players.min == m.players.max {
        format!("{} jogadores", m.players.max)
    } else {
        format!("{} a {} jogadores", m.players.min, m.players.max)
    };
    Some(LibraryEntry {
        id: m.id.clone(),
        name: m.name.clone(),
        tagline: m.tagline.clone(),
        players,
        source: Source::Repo { repo: report.repo.clone() },
        cover: report.cover_url.clone(),
        installed: None,
        update: None,
        etag: None,
        edited: false,
    })
}
