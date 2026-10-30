//! "Jogos deste PC": Steam e Epic, lendo os arquivos que as próprias lojas mantêm (docs/ARCHITECTURE.md,
//! "Biblioteca: jogos do PC"). Nada é modificado. Serve o Hub (Biblioteca), a CLI (`pa games`, `pa new
//! --base-game`) e o servidor MCP (`find_installed_games`).

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcGame {
    pub id: String,
    pub name: String,
    pub store: String,
    pub players: String,
    pub tagline: String,
    /// URI ou caminho para abrir.
    pub launch: String,
    /// Capa: data URL, ou uma referência `steam:<appid>` que [`art_data_url`] resolve sob demanda (as capas
    /// da Steam têm centenas de KB: não vão para o library.json).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_app_id: Option<u32>,
    /// O `AppName` da Epic (com maiúsculas: a Epic diferencia).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic_app_name: Option<String>,
}

pub fn scan() -> Vec<PcGame> {
    let mut out = Vec::new();
    if let Some(steam) = steam_root() {
        out.extend(scan_steam(&steam));
    }
    out.extend(scan_epic(Path::new(r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests")));
    out.sort_by_key(|g| g.name.to_lowercase());
    out.dedup_by(|a, b| a.id == b.id);
    out
}

/// Busca por nome (sem acento e sem caixa) ou pelo appid da Steam.
pub fn find(query: &str) -> Vec<PcGame> {
    let q = fold(query.trim());
    scan()
        .into_iter()
        .filter(|g| q.is_empty() || fold(&g.name).contains(&q) || g.steam_app_id.is_some_and(|id| id.to_string() == q))
        .collect()
}

fn fold(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

#[cfg(windows)]
pub fn steam_root() -> Option<PathBuf> {
    let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam").ok()?;
    let path: String = key.get_value("SteamPath").ok()?;
    Some(PathBuf::from(path))
}

#[cfg(not(windows))]
pub fn steam_root() -> Option<PathBuf> {
    None
}

fn steam_libraries(steam: &Path) -> Vec<PathBuf> {
    let mut libraries = vec![steam.to_path_buf()];
    if let Ok(vdf) = fs::read_to_string(steam.join("steamapps/libraryfolders.vdf")) {
        for path in vdf_values(&vdf, "path") {
            let p = PathBuf::from(path.replace("\\\\", "\\"));
            if !libraries.contains(&p) {
                libraries.push(p);
            }
        }
    }
    libraries
}

pub fn scan_steam(steam: &Path) -> Vec<PcGame> {
    let mut out = Vec::new();
    for lib in steam_libraries(steam) {
        let Ok(entries) = fs::read_dir(lib.join("steamapps")) else { continue };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Ok(acf) = fs::read_to_string(e.path()) else { continue };
            let (Some(appid), Some(title)) = (vdf_values(&acf, "appid").next(), vdf_values(&acf, "name").next()) else { continue };
            if is_steam_tool(appid, title) {
                continue;
            }
            let install_dir = vdf_values(&acf, "installdir").next().map(|d| lib.join("steamapps/common").join(d));
            out.push(PcGame {
                id: format!("steam-{appid}"),
                name: title.to_string(),
                store: "Steam".into(),
                players: "Modo Universal".into(),
                tagline: "Da sua Steam, com os celulares como controles.".into(),
                launch: format!("steam://rungameid/{appid}"),
                image: steam_art_path(steam, appid, Art::Cover).map(|_| format!("steam:{appid}")),
                install_dir,
                steam_app_id: appid.parse().ok(),
                epic_app_name: None,
            });
        }
    }
    out
}

/// Pasta de um jogo da Steam pelo appid (mods de jogo-base: o Hub acha o jogo sozinho).
pub fn steam_app_dir(appid: u32) -> Option<PathBuf> {
    let steam = steam_root()?;
    steam_libraries(&steam).into_iter().find_map(|lib| {
        let acf = fs::read_to_string(lib.join(format!("steamapps/appmanifest_{appid}.acf"))).ok()?;
        let dir = vdf_values(&acf, "installdir").next()?;
        let path = lib.join("steamapps/common").join(dir);
        path.is_dir().then_some(path)
    })
}

fn is_steam_tool(appid: &str, name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    appid == "228980"
        || n.contains("redistributable")
        || n.starts_with("proton")
        || n.contains("steam linux runtime")
        || n.contains("steamworks")
        || n == "steamvr"
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Art {
    /// Capa em pé (600×900).
    Cover,
    /// Arte larga sem logo (fundo da tela do fliperama).
    Hero,
}

fn steam_art_path(steam: &Path, appid: &str, art: Art) -> Option<PathBuf> {
    let cache = steam.join("appcache/librarycache");
    // layout antigo (<appid>_library_600x900.jpg) e o novo (<appid>/library_600x900.jpg)
    let file = match art {
        Art::Cover => "library_600x900.jpg",
        Art::Hero => "library_hero.jpg",
    };
    [cache.join(format!("{appid}_{file}")), cache.join(appid).join(file)].into_iter().find(|p| p.is_file())
}

/// O arquivo de arte da Steam de um appid (só dígitos: nada de caminho vindo de fora).
pub fn steam_art_file(appid: &str, art: Art) -> Option<PathBuf> {
    if appid.is_empty() || !appid.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    steam_art_path(&steam_root()?, appid, art)
}

/// Resolve uma referência de arte (`steam:<appid>`) em data URL. Data URLs e links passam direto.
pub fn art_data_url(reference: &str, art: Art) -> Option<String> {
    if let Some(appid) = reference.strip_prefix("steam:") {
        let path = steam_art_path(&steam_root()?, appid, art)?;
        return data_url(&path);
    }
    (art == Art::Cover).then(|| reference.to_string())
}

fn data_url(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() > 4_000_000 {
        return None;
    }
    let mime = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) { "image/png" } else { "image/jpeg" };
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// Valores de uma chave num arquivo VDF/ACF da Steam (`"chave"  "valor"`).
fn vdf_values<'a>(text: &'a str, key: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    text.lines().filter_map(move |line| {
        let mut parts = line.trim().splitn(2, '\t');
        let k = parts.next()?.trim().trim_matches('"');
        if !k.eq_ignore_ascii_case(key) {
            return None;
        }
        let v = parts.next()?.trim().trim_matches('"');
        (!v.is_empty()).then_some(v)
    })
}

pub fn scan_epic(manifests: &Path) -> Vec<PcGame> {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Item {
        display_name: String,
        app_name: String,
        catalog_namespace: String,
        catalog_item_id: String,
        #[serde(default)]
        launch_executable: String,
        #[serde(default)]
        install_location: String,
        #[serde(default)]
        b_is_incomplete_install: bool,
        #[serde(default)]
        main_game_app_name: String,
    }
    let Ok(entries) = fs::read_dir(manifests) else { return vec![] };
    let mut out = Vec::new();
    for e in entries.flatten() {
        if !e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("item")) {
            continue;
        }
        let Ok(item) = fs::read(e.path()).map_err(|_| ()).and_then(|b| serde_json::from_slice::<Item>(&b).map_err(|_| ())) else {
            continue;
        };
        // DLC e instalação pela metade não entram
        let is_dlc = !item.main_game_app_name.is_empty() && item.main_game_app_name != item.app_name;
        if item.b_is_incomplete_install || item.launch_executable.is_empty() || is_dlc {
            continue;
        }
        out.push(PcGame {
            id: format!("epic-{}", item.app_name.to_lowercase()),
            name: item.display_name,
            store: "Epic".into(),
            players: "Modo Universal".into(),
            tagline: "Da sua Epic, com os celulares como controles.".into(),
            launch: format!(
                "com.epicgames.launcher://apps/{}%3A{}%3A{}?action=launch&silent=true",
                item.catalog_namespace, item.catalog_item_id, item.app_name
            ),
            image: None,
            install_dir: (!item.install_location.is_empty()).then(|| PathBuf::from(item.install_location)),
            steam_app_id: None,
            epic_app_name: Some(item.app_name),
        });
    }
    out
}

/// Executáveis prováveis do jogo (na pasta e um nível abaixo), do maior para o menor, sem instaladores,
/// desinstaladores e afins. Relativos à pasta do jogo.
pub fn exe_candidates(dir: &Path) -> Vec<String> {
    const SKIP: [&str; 12] =
        ["unins", "uninstall", "setup", "install", "crash", "report", "redist", "vcredist", "dxsetup", "dotnet", "launcherhelper", "easyanticheat"];
    let mut found: Vec<(u64, String)> = Vec::new();
    let mut visit = |d: &Path, prefix: &str| {
        let Ok(entries) = fs::read_dir(d) else { return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if !lower.ends_with(".exe") || SKIP.iter().any(|s| lower.contains(s)) {
                continue;
            }
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            found.push((size, format!("{prefix}{name}")));
        }
    };
    visit(dir, "");
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten().filter(|e| e.path().is_dir()) {
            let sub = e.file_name().to_string_lossy().to_string();
            visit(&e.path(), &format!("{sub}/"));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    found.into_iter().map(|(_, p)| p).collect()
}

/// Anti-cheat na pasta do jogo? O Party Anywhere não faz mods para esses jogos: contornar anti-cheat está
/// fora do escopo, e o jogador poderia ser banido.
pub fn anti_cheat(dir: &Path) -> Option<&'static str> {
    const KNOWN: [(&str, &str); 7] = [
        ("easyanticheat", "EasyAntiCheat"),
        ("battleye", "BattlEye"),
        ("beservice", "BattlEye"),
        ("pnkbstr", "PunkBuster"),
        ("gameguard", "nProtect GameGuard"),
        ("xigncode", "XIGNCODE3"),
        ("vgk", "Vanguard"),
    ];
    let mut names = Vec::new();
    for d in std::iter::once(dir.to_path_buf()).chain(fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir())) {
        if let Ok(entries) = fs::read_dir(&d) {
            names.extend(entries.flatten().map(|e| e.file_name().to_string_lossy().to_lowercase()));
        }
    }
    KNOWN.iter().find(|(needle, _)| names.iter().any(|n| n.starts_with(needle))).map(|(_, label)| *label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_steam_libraries_and_skips_tools() {
        let steam = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let apps = steam.path().join("steamapps");
        fs::create_dir_all(&apps).unwrap();
        fs::create_dir_all(other.path().join("steamapps")).unwrap();
        let vdf = format!(
            "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}\n",
            steam.path().display().to_string().replace('\\', "\\\\"),
            other.path().display().to_string().replace('\\', "\\\\")
        );
        fs::write(apps.join("libraryfolders.vdf"), vdf).unwrap();
        let acf = |id: &str, name: &str, dir: &str| {
            format!("\"AppState\"\n{{\n\t\"appid\"\t\t\"{id}\"\n\t\"name\"\t\t\"{name}\"\n\t\"installdir\"\t\t\"{dir}\"\n}}\n")
        };
        fs::write(apps.join("appmanifest_12120.acf"), acf("12120", "Grand Theft Auto: San Andreas", "GTA San Andreas")).unwrap();
        fs::write(apps.join("appmanifest_228980.acf"), acf("228980", "Steamworks Common Redistributables", "x")).unwrap();
        fs::write(other.path().join("steamapps/appmanifest_728880.acf"), acf("728880", "Overcooked! 2", "Overcooked! 2")).unwrap();
        fs::create_dir_all(apps.join("../appcache/librarycache/12120")).unwrap();
        fs::write(apps.join("../appcache/librarycache/12120/library_600x900.jpg"), b"jpg").unwrap();

        let mut games = scan_steam(steam.path());
        games.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<&str> = games.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["Grand Theft Auto: San Andreas", "Overcooked! 2"]);
        assert_eq!(games[0].launch, "steam://rungameid/12120");
        assert_eq!(games[0].image.as_deref(), Some("steam:12120"), "referência, não a imagem inteira");
        assert_eq!(games[1].image, None);
        assert_eq!(games[0].steam_app_id, Some(12120));
        assert!(games[0].install_dir.as_ref().unwrap().ends_with("steamapps/common/GTA San Andreas"));
    }

    #[test]
    fn reads_epic_manifests_without_dlc() {
        let dir = tempfile::tempdir().unwrap();
        let item = |name: &str, app: &str, main: &str| {
            serde_json::json!({ "DisplayName": name, "AppName": app, "CatalogNamespace": "ns", "CatalogItemId": "cid",
                "LaunchExecutable": "Game.exe", "InstallLocation": "D:/Epic/Game", "bIsIncompleteInstall": false, "MainGameAppName": main })
            .to_string()
        };
        fs::write(dir.path().join("a.item"), item("Rocket League", "Sugar", "Sugar")).unwrap();
        fs::write(dir.path().join("b.item"), item("Pacote DLC", "SugarDLC", "Sugar")).unwrap();
        let games = scan_epic(dir.path());
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].launch, "com.epicgames.launcher://apps/ns%3Acid%3ASugar?action=launch&silent=true");
        assert_eq!(games[0].install_dir.as_deref(), Some(Path::new("D:/Epic/Game")));
    }

    #[test]
    fn picks_the_game_exe_and_skips_installers() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("gta_sa.exe"), vec![0u8; 5000]).unwrap();
        fs::write(dir.path().join("unins000.exe"), vec![0u8; 9000]).unwrap();
        fs::create_dir_all(dir.path().join("bin")).unwrap();
        fs::write(dir.path().join("bin/tool.exe"), vec![0u8; 100]).unwrap();
        fs::write(dir.path().join("bin/CrashReporter.exe"), vec![0u8; 99999]).unwrap();
        assert_eq!(exe_candidates(dir.path()), ["gta_sa.exe", "bin/tool.exe"]);
    }

    #[test]
    fn detects_anti_cheat() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(anti_cheat(dir.path()), None);
        fs::create_dir_all(dir.path().join("EasyAntiCheat")).unwrap();
        assert_eq!(anti_cheat(dir.path()), Some("EasyAntiCheat"));
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("Binaries")).unwrap();
        fs::write(dir.path().join("Binaries/BEService_x64.exe"), b"x").unwrap();
        assert_eq!(anti_cheat(dir.path()), Some("BattlEye"));
    }

    #[test]
    fn search_ignores_case_and_accents() {
        assert_eq!(fold("ONE PIECE: Pirate Warriors 3"), "onepiecepiratewarriors3");
    }
}
