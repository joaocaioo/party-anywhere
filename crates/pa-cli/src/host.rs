//! O mundo real para a CLI e o MCP: GitHub, Steam e Epic deste PC, o Hub instalado.

use pa_library::http::Ureq;
use pa_library::{hubapp, pcgames};
use serde_json::{json, Value};

use crate::mcp::Host;
use crate::{resolve_base_game, BaseGameSpec};

pub struct RealHost;

impl Host for RealHost {
    fn inspect(&self, repo: &str) -> Result<Value, String> {
        let report = pa_library::inspect(&Ureq::default(), repo)?;
        serde_json::to_value(report).map_err(|e| e.to_string())
    }

    fn hub_status(&self) -> Value {
        serde_json::to_value(hubapp::status(&Ureq::default())).unwrap_or_default()
    }

    fn install_hub(&self) -> Result<Value, String> {
        install_hub().map(|summary| json!({ "summary": summary }))
    }

    fn find_games(&self, query: &str) -> Vec<Value> {
        pcgames::find(query).iter().map(game_json).collect()
    }

    fn resolve_base_game(&self, query: &str) -> Result<BaseGameSpec, String> {
        resolve_base_game(query)
    }
}

/// Baixa o Hub mais novo (conferindo o hash) e abre o instalador; a pessoa confirma na tela.
pub fn install_hub() -> Result<String, String> {
    let http = Ureq::default();
    let latest = pa_library::latest_hub(&http, None)?.ok_or("ainda não há versão do Party Anywhere publicada para baixar.")?;
    if let Some(inst) = hubapp::installed() {
        let newer = match (semver::Version::parse(&latest.version), semver::Version::parse(inst.version.trim_start_matches('v'))) {
            (Ok(l), Ok(i)) => l > i,
            _ => true,
        };
        if !newer {
            return Ok(format!("O Party Anywhere {} já é a versão mais nova.", inst.version));
        }
    }
    let path = hubapp::download_installer(&http, &latest)?;
    std::process::Command::new(&path).spawn().map_err(|e| format!("não consegui abrir o instalador: {e}"))?;
    Ok(format!("Instalador do Party Anywhere {} aberto. É só confirmar na tela.", latest.version))
}

/// Um jogo do PC para quem vai escolher qual vira co-op: onde está, o executável e se tem anti-cheat.
pub fn game_json(g: &pcgames::PcGame) -> Value {
    let dir = g.install_dir.as_deref();
    json!({
        "name": g.name,
        "store": g.store,
        "steamAppId": g.steam_app_id,
        "installDir": dir,
        "exe": dir.and_then(|d| pcgames::exe_candidates(d).into_iter().next()),
        "antiCheat": dir.and_then(pcgames::anti_cheat),
    })
}
