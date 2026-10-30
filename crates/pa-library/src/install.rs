//! Instalar: baixa o pacote do release, confere tamanho e sha256 e extrai. Nada roda durante a
//! instalação (sem scripts): só arquivos copiados para pastas do Party Anywhere ou do jogo-base, com backup.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use pa_manifest::{is_safe_relative_path, GameManifest, InstallTarget};

use crate::github::LatestRelease;
use crate::http::{Http, MAX_PACKAGE_BYTES};
use crate::store::Installed;

/// Instala a versão `latest` em `games_root\<id>\<versão>\`. Mods de jogo-base também copiam os
/// arquivos para `base_dir` (guardando os originais em `<versão>\backup\`).
pub fn install(
    http: &dyn Http,
    game: &GameManifest,
    latest: &LatestRelease,
    games_root: &Path,
    base_dir: Option<&Path>,
) -> Result<Installed, String> {
    if game.install.target == InstallTarget::BaseGame && base_dir.is_none() {
        let base = game.requires.as_ref().and_then(|r| r.base_game.as_ref()).map(|b| b.name.as_str()).unwrap_or("o jogo original");
        return Err(format!("este mod precisa de {base}. Escolha a pasta do jogo para continuar."));
    }
    let resp = http.get(&latest.package_url, None, MAX_PACKAGE_BYTES)?;
    if !resp.ok() {
        return Err(format!("não consegui baixar o pacote ({}). Tente de novo em instantes.", resp.status));
    }
    if !latest.release.verify(&resp.body) {
        return Err("o pacote baixado não confere com o publicado (hash diferente). Nada foi instalado.".into());
    }

    let version = latest.release.version.clone();
    let dest = games_root.join(&game.id).join(&version);
    let staging = games_root.join(&game.id).join(format!(".{version}.partial"));
    let _ = fs::remove_dir_all(&staging);
    extract(&resp.body, &staging).inspect_err(|_| {
        let _ = fs::remove_dir_all(&staging);
    })?;
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| format!("não consegui substituir a versão anterior: {e}"))?;
    }
    fs::rename(&staging, &dest).map_err(|e| format!("não consegui concluir a instalação: {e}"))?;

    let mut installed = Installed {
        version,
        dir: dest.clone(),
        exe: game.launch.exe.clone(),
        args: game.launch.args.clone(),
        working_dir: game.launch.working_dir.clone(),
        base_dir: None,
    };
    if let (InstallTarget::BaseGame, Some(base)) = (game.install.target, base_dir) {
        copy_into_base(game, &dest, base)?;
        installed.base_dir = Some(base.to_path_buf());
        installed.dir = base.to_path_buf(); // abre o jogo-base, com o mod dentro
    }
    Ok(installed)
}

/// Extrai recusando qualquer caminho que saia da pasta (zip slip).
pub fn extract(zip_bytes: &[u8], dest: &Path) -> Result<usize, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(zip_bytes)).map_err(|_| "o pacote não é um .zip válido.".to_string())?;
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut count = 0;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = file.enclosed_name().filter(|p| is_safe_relative_path(&p.to_string_lossy())) else {
            return Err(format!("o pacote tem um caminho inseguro ({}). Nada foi instalado.", file.name()));
        };
        let out = dest.join(rel);
        if file.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        fs::write(&out, buf).map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok(count)
}

fn copy_into_base(game: &GameManifest, package_dir: &Path, base: &Path) -> Result<(), String> {
    let backup = package_dir.join("backup");
    for f in &game.install.files {
        let from = package_dir.join(&f.from);
        let to = base.join(&f.to);
        if !from.exists() {
            return Err(format!("{} não veio no pacote.", f.from));
        }
        if to.exists() {
            let saved = backup.join(&f.to);
            if let Some(p) = saved.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            if !saved.exists() {
                fs::copy(&to, &saved).map_err(|e| format!("não consegui guardar o backup de {}: {e}", f.to))?;
            }
        }
        if let Some(p) = to.parent() {
            fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        fs::copy(&from, &to).map_err(|e| format!("não consegui copiar {} para a pasta do jogo: {e}", f.to))?;
    }
    Ok(())
}

/// Desinstala um mod de jogo-base: devolve os originais do backup e apaga o que o mod trouxe.
pub fn uninstall_from_base(game: &GameManifest, installed: &Installed, package_dir: &Path) -> Result<(), String> {
    let Some(base) = &installed.base_dir else { return Ok(()) };
    let backup = package_dir.join("backup");
    for f in &game.install.files {
        let to = base.join(&f.to);
        let saved = backup.join(&f.to);
        if saved.exists() {
            fs::copy(&saved, &to).map_err(|e| e.to_string())?;
        } else if to.exists() {
            fs::remove_file(&to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Caminho completo do executável instalado.
pub fn exe_path(installed: &Installed) -> PathBuf {
    installed.dir.join(&installed.exe)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::github::tests::{published, MANIFEST};
    use crate::github::fetch_latest;
    use crate::http::fake::FakeHttp;
    use pa_manifest::{BaseGame, InstallFile, RepoRef, Requires};

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut out);
            for (name, data) in files {
                z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(data).unwrap();
            }
            z.finish().unwrap();
        }
        out.into_inner()
    }

    fn game() -> GameManifest {
        GameManifest::parse(MANIFEST).unwrap().0
    }

    #[test]
    fn installs_a_verified_package() {
        let pkg = zip_of(&[("MeuJogo.exe", b"MZ"), ("data/a.bin", b"a")]);
        let mut http = FakeHttp::default();
        published(&mut http, "joao/meu-jogo", "0.1.0", &pkg);
        let g = game();
        let latest = fetch_latest(&http, &RepoRef::parse("joao/meu-jogo").unwrap(), &g).unwrap().unwrap();

        let root = tempfile::tempdir().unwrap();
        let inst = install(&http, &g, &latest, root.path(), None).unwrap();
        assert_eq!(inst.version, "0.1.0");
        assert_eq!(fs::read(exe_path(&inst)).unwrap(), b"MZ");
        assert!(inst.dir.join("data/a.bin").exists());
        assert!(!root.path().join("com.seu-usuario.meu-jogo/.0.1.0.partial").exists());
    }

    #[test]
    fn tampered_package_installs_nothing() {
        let pkg = zip_of(&[("MeuJogo.exe", b"MZ")]);
        let mut http = FakeHttp::default();
        published(&mut http, "joao/meu-jogo", "0.1.0", &pkg);
        let g = game();
        let mut latest = fetch_latest(&http, &RepoRef::parse("joao/meu-jogo").unwrap(), &g).unwrap().unwrap();
        latest.release.sha256 = "0".repeat(64);
        let root = tempfile::tempdir().unwrap();
        let err = install(&http, &g, &latest, root.path(), None).unwrap_err();
        assert!(err.contains("não confere"));
        assert!(fs::read_dir(root.path()).unwrap().next().is_none());
    }

    #[test]
    fn zip_slip_is_refused() {
        let evil = zip_of(&[("../../fora.txt", b"x")]);
        let dir = tempfile::tempdir().unwrap();
        assert!(extract(&evil, &dir.path().join("x")).unwrap_err().contains("inseguro"));
        assert!(!dir.path().join("fora.txt").exists());
    }

    #[test]
    fn base_game_mod_copies_with_backup_and_uninstalls_cleanly() {
        let pkg = zip_of(&[("SAParty.asi", b"mod"), ("data/gta.dat", b"novo")]);
        let mut g = game();
        g.install.target = InstallTarget::BaseGame;
        g.requires = Some(Requires {
            base_game: Some(BaseGame { name: "GTA SA".into(), steam_app_id: None, exe: "gta_sa.exe".into(), accepted_sha256: vec![], help: None }),
        });
        g.install.files = vec![
            InstallFile { from: "SAParty.asi".into(), to: "SAParty.asi".into() },
            InstallFile { from: "data/gta.dat".into(), to: "data/gta.dat".into() },
        ];
        let mut http = FakeHttp::default();
        published(&mut http, "joao/meu-jogo", "0.1.0", &pkg);
        let latest = fetch_latest(&http, &RepoRef::parse("joao/meu-jogo").unwrap(), &game()).unwrap().unwrap();

        let root = tempfile::tempdir().unwrap();
        let base = tempfile::tempdir().unwrap();
        fs::create_dir_all(base.path().join("data")).unwrap();
        fs::write(base.path().join("data/gta.dat"), b"original").unwrap();

        assert!(install(&http, &g, &latest, root.path(), None).unwrap_err().contains("GTA SA"));
        let inst = install(&http, &g, &latest, root.path(), Some(base.path())).unwrap();
        assert_eq!(fs::read(base.path().join("data/gta.dat")).unwrap(), b"novo");
        assert_eq!(fs::read(base.path().join("SAParty.asi")).unwrap(), b"mod");

        let pkg_dir = root.path().join(&g.id).join("0.1.0");
        uninstall_from_base(&g, &inst, &pkg_dir).unwrap();
        assert_eq!(fs::read(base.path().join("data/gta.dat")).unwrap(), b"original");
        assert!(!base.path().join("SAParty.asi").exists());
    }
}
