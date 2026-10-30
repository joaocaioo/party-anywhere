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

    fn add_to_library(&self, link: &str) -> Result<Value, String> {
        add_to_hub(link).map(|summary| json!({ "summary": summary }))
    }
}

/// Leva um link de mod (ou jogo) para a Biblioteca: confere que ele instala, e abre o Hub com
/// `--add <link>` (se o Hub já está aberto, a janela dele recebe o link). A pessoa confirma no Hub.
pub fn add_to_hub(link: &str) -> Result<String, String> {
    // pasta do projeto: o Hub roda em outra pasta, então vai o caminho completo
    let local = std::path::Path::new(link);
    let link = if local.is_dir() { local.canonicalize().map_err(|e| e.to_string())?.display().to_string() } else { link.to_string() };
    let link = link.trim_start_matches(r"\\?\");
    let report = pa_library::inspect(&Ureq::default(), link)?;
    if !report.installable {
        let why: Vec<String> = report.issues.iter().filter(|i| i.is_error()).map(|i| i.message.clone()).collect();
        return Err(format!("{} {}", report.summary, why.join(" ")));
    }
    let name = report.manifest.as_ref().map(|m| m.name.clone()).unwrap_or_else(|| link.to_string());
    // PA_HUB_EXE: para testar com um Hub que não foi instalado (compilado do código)
    let exe = std::env::var_os("PA_HUB_EXE")
        .map(std::path::PathBuf::from)
        .or_else(|| hubapp::installed().and_then(|h| h.exe))
        .or_else(running_hub_exe)
        .filter(|p| p.is_file())
        .ok_or("o Party Anywhere não está instalado neste PC. Rode `pa hub install` (ou a ferramenta install_hub) e tente de novo.")?;
    std::process::Command::new(&exe).args(["--add", link]).spawn().map_err(|e| format!("não consegui abrir o Party Anywhere: {e}"))?;
    Ok(format!(
        "Party Anywhere aberto com {name}. Lá, a pessoa vê a prévia, o Hub acha o jogo original e ela confirma em \"Instalar o mod\"."
    ))
}

/// O executável do Hub que está aberto agora (vale o compilado do código, que não aparece como instalado).
fn running_hub_exe() -> Option<std::path::PathBuf> {
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", "(Get-Process party-anywhere -ErrorAction SilentlyContinue | Select-Object -First 1).Path"])
        .output()
        .ok()?;
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!path.is_empty()).then(|| std::path::PathBuf::from(path))
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
