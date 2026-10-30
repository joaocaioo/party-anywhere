//! O Hub neste PC: está instalado? Em que versão? Está aberto? Há versão nova? Usado pela CLI (`pa hub`)
//! e pelo servidor MCP (`hub_status`, `install_hub`), para quem cria jogos começar com o Hub certo.

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;

use crate::http::Http;
use crate::updates::{latest_hub, HubUpdate};
use pa_manifest::sha256_hex;

/// Nome com que o instalador registra o Hub no Windows (productName do tauri.conf.json).
pub const PRODUCT_NAME: &str = "Party Anywhere";
/// Porta do servidor do celular: aberta = Hub rodando.
pub const PHONE_PORT: u16 = 27027;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledHub {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exe: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HubStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<InstalledHub>,
    pub running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<HubUpdate>,
    pub update_available: bool,
    pub summary: String,
}

/// Lê o registro de desinstalação do Windows (o instalador do Hub se registra lá).
#[cfg(windows)]
pub fn installed() -> Option<InstalledHub> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let Ok(uninstall) = RegKey::predef(root).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Uninstall") else {
            continue;
        };
        for name in uninstall.enum_keys().flatten() {
            let Ok(k) = uninstall.open_subkey(&name) else { continue };
            let display: String = k.get_value("DisplayName").unwrap_or_default();
            if display != PRODUCT_NAME {
                continue;
            }
            let version: String = k.get_value("DisplayVersion").unwrap_or_default();
            let icon: String = k.get_value("DisplayIcon").unwrap_or_default();
            let location: String = k.get_value("InstallLocation").unwrap_or_default();
            let exe = Some(icon.split(',').next().unwrap_or("").trim_matches('"').to_string())
                .filter(|p| p.to_lowercase().ends_with(".exe"))
                .map(PathBuf::from)
                .or_else(|| (!location.is_empty()).then(|| PathBuf::from(location.trim_matches('"')).join("party-anywhere.exe")));
            return Some(InstalledHub { version, exe });
        }
    }
    None
}

#[cfg(not(windows))]
pub fn installed() -> Option<InstalledHub> {
    None
}

pub fn running() -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], PHONE_PORT));
    std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

pub fn status(http: &dyn Http) -> HubStatus {
    let installed = installed();
    let running = running();
    let latest = latest_hub(http, None).ok().flatten();
    let newer = |inst: &InstalledHub, l: &HubUpdate| match (parse(&l.version), parse(&inst.version)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    };
    let update_available = matches!((&installed, &latest), (Some(i), Some(l)) if newer(i, l));
    let summary = match (&installed, &latest) {
        (None, Some(l)) => format!("O Party Anywhere não está instalado. A versão {} está disponível: `pa hub install`.", l.version),
        (None, None) if running => "Há um Hub aberto (versão de desenvolvimento), mas nenhum instalado; ainda não há versão publicada.".into(),
        (None, None) => "O Party Anywhere não está instalado, e ainda não há versão publicada para baixar.".into(),
        (Some(i), Some(l)) if update_available => {
            format!("Party Anywhere {} instalado; a versão {} está disponível: `pa hub install` atualiza.", i.version, l.version)
        }
        (Some(i), _) => format!("Party Anywhere {} instalado e atualizado{}.", i.version, if running { ", e aberto" } else { "" }),
    };
    HubStatus { installed, running, latest, update_available, summary }
}

/// Baixa o instalador e confere o `.sha256` publicado junto (quando o release tem). Devolve o caminho.
pub fn download_installer(http: &dyn Http, update: &HubUpdate) -> Result<PathBuf, String> {
    if !update.installer_url.starts_with("https://github.com/") {
        return Err("endereço de instalador inesperado".into());
    }
    let resp = http.get(&update.installer_url, None, 400 * 1024 * 1024)?;
    if !resp.ok() {
        return Err(format!("o download falhou ({}).", resp.status));
    }
    if let Some(url) = &update.sha256_url {
        let sum = http.get(url, None, 4096)?;
        if sum.ok() {
            let expected = String::from_utf8_lossy(&sum.body).split_whitespace().next().unwrap_or("").to_lowercase();
            if expected != sha256_hex(&resp.body) {
                return Err("o instalador baixado não confere com o publicado (hash diferente). Nada foi instalado.".into());
            }
        }
    }
    let path = std::env::temp_dir().join(format!("PartyAnywhere-{}-setup.exe", update.version));
    std::fs::write(&path, &resp.body).map_err(|e| e.to_string())?;
    Ok(path)
}

fn parse(v: &str) -> Option<semver::Version> {
    semver::Version::parse(v.trim().trim_start_matches('v')).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::fake::FakeHttp;

    fn update(installer: &str, sha: Option<&str>) -> HubUpdate {
        HubUpdate {
            version: "0.2.0".into(),
            installer_url: installer.into(),
            sha256_url: sha.map(String::from),
            notes: String::new(),
            etag: None,
        }
    }

    #[test]
    fn installer_hash_is_checked() {
        let url = "https://github.com/joaocaioo/party-anywhere/releases/download/v0.2.0/PartyAnywhere-0.2.0-setup.exe";
        let mut http = FakeHttp::default();
        http.on(url, 200, b"MZ instalador".to_vec());
        http.on(&format!("{url}.sha256"), 200, format!("{}  PartyAnywhere-0.2.0-setup.exe\n", sha256_hex(b"MZ instalador")));
        let path = download_installer(&http, &update(url, Some(&format!("{url}.sha256")))).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"MZ instalador");

        http.on(&format!("{url}.sha256"), 200, "0".repeat(64));
        assert!(download_installer(&http, &update(url, Some(&format!("{url}.sha256")))).unwrap_err().contains("não confere"));
        assert!(download_installer(&http, &update("https://exemplo.com/x.exe", None)).is_err(), "só do GitHub");
    }
}
