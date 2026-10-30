//! "Jogos deste PC": Steam e Epic, lendo os arquivos que as próprias lojas mantêm (docs/ARCHITECTURE.md,
//! "Biblioteca: jogos do PC"). Nada é modificado; só guardamos como abrir e a capa.

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
    /// Capa como data URL (a UI mostra sem precisar de acesso ao disco).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
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

#[cfg(windows)]
fn steam_root() -> Option<PathBuf> {
    let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam").ok()?;
    let path: String = key.get_value("SteamPath").ok()?;
    Some(PathBuf::from(path))
}

#[cfg(not(windows))]
fn steam_root() -> Option<PathBuf> {
    None
}

pub fn scan_steam(steam: &Path) -> Vec<PcGame> {
    let mut libraries = vec![steam.to_path_buf()];
    if let Ok(vdf) = fs::read_to_string(steam.join("steamapps/libraryfolders.vdf")) {
        for path in vdf_values(&vdf, "path") {
            let p = PathBuf::from(path.replace("\\\\", "\\"));
            if !libraries.contains(&p) {
                libraries.push(p);
            }
        }
    }
    let mut out = Vec::new();
    for lib in libraries {
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
            out.push(PcGame {
                id: format!("steam-{appid}"),
                name: title.to_string(),
                store: "Steam".into(),
                players: "Modo Universal".into(),
                tagline: "Da sua Steam, com os celulares como controles.".into(),
                launch: format!("steam://rungameid/{appid}"),
                image: steam_cover(steam, appid),
            });
        }
    }
    out
}

/// Pasta de um jogo da Steam pelo appid (mods de jogo-base: o Hub acha o GTA SA sozinho).
pub fn steam_app_dir(appid: u32) -> Option<PathBuf> {
    let steam = steam_root()?;
    let mut libraries = vec![steam.clone()];
    if let Ok(vdf) = fs::read_to_string(steam.join("steamapps/libraryfolders.vdf")) {
        libraries.extend(vdf_values(&vdf, "path").map(|p| PathBuf::from(p.replace("\\\\", "\\"))));
    }
    libraries.into_iter().find_map(|lib| {
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

fn steam_cover(steam: &Path, appid: &str) -> Option<String> {
    let cache = steam.join("appcache/librarycache");
    // layout antigo (<appid>_library_600x900.jpg) e o novo (<appid>/library_600x900.jpg)
    let candidates = [cache.join(format!("{appid}_library_600x900.jpg")), cache.join(appid).join("library_600x900.jpg")];
    candidates.iter().find_map(|p| data_url(p))
}

fn data_url(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() > 1_500_000 {
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
        });
    }
    out
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
        let acf = |id: &str, name: &str| format!("\"AppState\"\n{{\n\t\"appid\"\t\t\"{id}\"\n\t\"name\"\t\t\"{name}\"\n}}\n");
        fs::write(apps.join("appmanifest_12120.acf"), acf("12120", "Grand Theft Auto: San Andreas")).unwrap();
        fs::write(apps.join("appmanifest_228980.acf"), acf("228980", "Steamworks Common Redistributables")).unwrap();
        fs::write(other.path().join("steamapps/appmanifest_728880.acf"), acf("728880", "Overcooked! 2")).unwrap();

        let mut games = scan_steam(steam.path());
        games.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<&str> = games.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["Grand Theft Auto: San Andreas", "Overcooked! 2"]);
        assert_eq!(games[0].launch, "steam://rungameid/12120");
    }

    #[test]
    fn reads_epic_manifests_without_dlc() {
        let dir = tempfile::tempdir().unwrap();
        let item = |name: &str, app: &str, main: &str| {
            serde_json::json!({ "DisplayName": name, "AppName": app, "CatalogNamespace": "ns", "CatalogItemId": "cid",
                "LaunchExecutable": "Game.exe", "bIsIncompleteInstall": false, "MainGameAppName": main })
            .to_string()
        };
        fs::write(dir.path().join("a.item"), item("Rocket League", "Sugar", "Sugar")).unwrap();
        fs::write(dir.path().join("b.item"), item("Pacote DLC", "SugarDLC", "Sugar")).unwrap();
        let games = scan_epic(dir.path());
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].launch, "com.epicgames.launcher://apps/ns%3Acid%3ASugar?action=launch&silent=true");
    }
}
