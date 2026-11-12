//! Abrir pela Steam um jogo que o Hub abre direto (o jogo-base de um mod).
//!
//! Um jogo da Steam aberto por outro programa chama `SteamAPI_RestartAppIfNecessary`: a Steam fecha o jogo e o
//! abre de novo por ela, e essa segunda janela não tem ligação com o Hub (sem jogadores, sem tela dividida). É o
//! que a Steam faz com quem não veio dela. O `steam_appid.txt` na pasta de onde o jogo roda diz à Steam qual é o
//! jogo, e ela deixa o jogo seguir aberto (o mesmo arquivo que os estúdios usam para testar). O jogo continua
//! sendo o da conta da pessoa e a Steam continua precisando estar aberta e logada.
//!
//! O arquivo só é criado quando não existe, e uma marca na pasta do mod lembra que foi o Hub: remover o mod o
//! apaga junto, e um `steam_appid.txt` que já estava lá nunca é tocado.

use std::fs;
use std::path::Path;

/// Nome do arquivo que a Steam procura na pasta de trabalho do jogo.
pub const APPID_FILE: &str = "steam_appid.txt";
/// Marca, na pasta do mod, de que o `steam_appid.txt` foi criado pelo Hub.
pub const CREATED_MARK: &str = "steam_appid.created";

/// O código da Steam do jogo nesta pasta, se ela é de uma biblioteca da Steam
/// (`...\steamapps\common\<pasta>` com o `appmanifest_<código>.acf` dizendo `installdir` = `<pasta>`).
pub fn app_id_for_dir(dir: &Path) -> Option<u32> {
    let mut cur = Some(dir);
    while let Some(d) = cur {
        let parent = d.parent()?;
        if parent.file_name().is_some_and(|n| n.eq_ignore_ascii_case("common"))
            && parent.parent().and_then(|p| p.file_name()).is_some_and(|n| n.eq_ignore_ascii_case("steamapps"))
        {
            let installdir = d.file_name()?.to_string_lossy().to_string();
            return app_id_in(parent.parent()?, &installdir);
        }
        cur = Some(parent);
    }
    None
}

/// Procura em `steamapps\appmanifest_*.acf` o jogo instalado em `common\<installdir>`.
fn app_id_in(steamapps: &Path, installdir: &str) -> Option<u32> {
    for e in fs::read_dir(steamapps).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(id) = name.strip_prefix("appmanifest_").and_then(|n| n.strip_suffix(".acf")) else { continue };
        let Ok(acf) = fs::read_to_string(e.path()) else { continue };
        let dir = acf.lines().find_map(|l| {
            let mut parts = l.split('"').filter(|p| !p.trim().is_empty());
            (parts.next()? == "installdir").then(|| parts.next().map(str::to_string)).flatten()
        });
        if dir.is_some_and(|d| d.eq_ignore_ascii_case(installdir)) {
            return id.parse().ok();
        }
    }
    None
}

/// Garante o `steam_appid.txt` em `work_dir`. Cria só se não existe e, nesse caso, deixa a marca em `mod_dir`.
pub fn ensure_app_id_file(work_dir: &Path, app_id: u32, mod_dir: &Path) -> Result<(), String> {
    let file = work_dir.join(APPID_FILE);
    if file.exists() {
        return Ok(());
    }
    fs::write(&file, app_id.to_string()).map_err(|e| format!("não consegui preparar o jogo para abrir pela Steam: {e}"))?;
    let _ = fs::create_dir_all(mod_dir);
    let _ = fs::write(mod_dir.join(CREATED_MARK), app_id.to_string());
    Ok(())
}

/// Remover o mod: apaga o `steam_appid.txt` que o Hub criou (e só ele).
pub fn remove_app_id_file(work_dir: &Path, mod_dir: &Path) {
    let mark = mod_dir.join(CREATED_MARK);
    if mark.exists() {
        let _ = fs::remove_file(work_dir.join(APPID_FILE));
        let _ = fs::remove_file(mark);
    }
}

/// A Steam está aberta e com alguém logado (sem isso, o jogo aberto com o `steam_appid.txt` não inicia).
#[cfg(windows)]
pub fn logged_in() -> bool {
    let Ok(key) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam\ActiveProcess") else {
        return false;
    };
    let pid: u32 = key.get_value("pid").unwrap_or(0);
    let user: u32 = key.get_value("ActiveUser").unwrap_or(0);
    pid != 0 && user != 0
}

#[cfg(not(windows))]
pub fn logged_in() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steam_lib(root: &Path) -> std::path::PathBuf {
        let apps = root.join("steamapps");
        fs::create_dir_all(apps.join("common/PEAK/PEAK_Data")).unwrap();
        fs::write(
            apps.join("appmanifest_3527290.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"3527290\"\n\t\"name\"\t\t\"PEAK\"\n\t\"installdir\"\t\t\"PEAK\"\n}\n",
        )
        .unwrap();
        fs::write(apps.join("appmanifest_10.acf"), "\"AppState\"\n{\n\t\"installdir\"\t\t\"Other\"\n}\n").unwrap();
        apps.join("common/PEAK")
    }

    #[test]
    fn finds_the_app_id_of_a_steam_folder() {
        let t = tempfile::tempdir().unwrap();
        let game = steam_lib(t.path());
        assert_eq!(app_id_for_dir(&game), Some(3527290));
        assert_eq!(app_id_for_dir(&game.join("PEAK_Data")), Some(3527290));
        assert_eq!(app_id_for_dir(t.path()), None);
        assert_eq!(app_id_for_dir(&t.path().join("GOG/PEAK")), None);
    }

    #[test]
    fn creates_once_and_removes_only_its_own_file() {
        let t = tempfile::tempdir().unwrap();
        let game = steam_lib(t.path());
        let mod_dir = t.path().join("mod/0.1.0");
        ensure_app_id_file(&game, 3527290, &mod_dir).unwrap();
        assert_eq!(fs::read_to_string(game.join(APPID_FILE)).unwrap(), "3527290");
        remove_app_id_file(&game, &mod_dir);
        assert!(!game.join(APPID_FILE).exists());

        // já existia: fica como estava, e remover o mod não apaga
        fs::write(game.join(APPID_FILE), "3527290\n").unwrap();
        ensure_app_id_file(&game, 3527290, &mod_dir).unwrap();
        remove_app_id_file(&game, &mod_dir);
        assert_eq!(fs::read_to_string(game.join(APPID_FILE)).unwrap(), "3527290\n");
    }
}
