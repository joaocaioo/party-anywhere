//! `library.json`: a biblioteca da pessoa (o que adicionou, a ordem, o que editou, o que está instalado).
//! Gravação atômica com `.bak` do último arquivo bom.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const LIBRARY_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Library {
    pub version: u32,
    /// Na ordem da Biblioteca.
    pub games: Vec<LibraryEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub tagline: String,
    #[serde(default)]
    pub players: String,
    pub source: Source,
    /// Caminho local (cópia em `covers\`) ou URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<Installed>,
    /// Versão nova já vista e ainda não instalada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<String>,
    /// ETag do último `releases/latest`: a checagem seguinte responde 304 se nada mudou.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// A pessoa editou nome, capa ou descrição: atualizações do manifesto não sobrescrevem.
    #[serde(default)]
    pub edited: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Source {
    /// Jogo do Party Anywhere instalado por link (`dono/repo`).
    Repo { repo: String },
    /// Jogo do PC (Steam, Epic, GOG, Xbox, exe): o Hub só guarda como abrir.
    Pc { store: String, launch: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub version: String,
    pub dir: PathBuf,
    pub exe: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "dot")]
    pub working_dir: String,
    /// Mods de jogo-base: onde os arquivos foram copiados e onde ficou o backup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_dir: Option<PathBuf>,
}

fn dot() -> String {
    ".".into()
}

impl Library {
    pub fn new() -> Self {
        Self { version: LIBRARY_VERSION, games: Vec::new() }
    }

    pub fn get(&self, id: &str) -> Option<&LibraryEntry> {
        self.games.iter().find(|g| g.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut LibraryEntry> {
        self.games.iter_mut().find(|g| g.id == id)
    }

    /// Adiciona no fim, ou atualiza o que já existe com o mesmo id (sem perder a posição nem as edições).
    pub fn upsert(&mut self, entry: LibraryEntry) {
        match self.get_mut(&entry.id) {
            Some(existing) => {
                let keep = existing.edited.then(|| (existing.name.clone(), existing.tagline.clone(), existing.cover.clone()));
                *existing = LibraryEntry { edited: existing.edited, ..entry };
                if let Some((name, tagline, cover)) = keep {
                    existing.name = name;
                    existing.tagline = tagline;
                    existing.cover = cover;
                }
            }
            None => self.games.push(entry),
        }
    }

    pub fn remove(&mut self, id: &str) -> Option<LibraryEntry> {
        let i = self.games.iter().position(|g| g.id == id)?;
        Some(self.games.remove(i))
    }

    pub fn move_game(&mut self, from: usize, to: usize) -> bool {
        if from >= self.games.len() || to >= self.games.len() || from == to {
            return false;
        }
        let g = self.games.remove(from);
        self.games.insert(to, g);
        true
    }

    /// Lê o arquivo; se estiver ilegível, tenta o `.bak`. Sem nenhum dos dois: biblioteca vazia.
    pub fn load(path: &Path) -> (Self, LoadNote) {
        match read(path) {
            Ok(Some(lib)) => (lib, LoadNote::Ok),
            Ok(None) => (Self::new(), LoadNote::Fresh),
            Err(_) => match read(&bak(path)) {
                Ok(Some(lib)) => (lib, LoadNote::RestoredFromBackup),
                _ => (Self::new(), LoadNote::Unreadable),
            },
        }
    }

    /// Escreve num temporário e troca; o arquivo anterior vira `.bak`.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(self).expect("Library serializa");
        {
            use std::io::Write;
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&json)?;
            f.sync_all()?;
        }
        if path.exists() {
            fs::copy(path, bak(path))?;
        }
        fs::rename(&tmp, path)
    }
}

/// O que aconteceu ao abrir: o Hub avisa a pessoa só quando houve perda ou recuperação.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadNote {
    Ok,
    Fresh,
    RestoredFromBackup,
    Unreadable,
}

fn read(path: &Path) -> Result<Option<Library>, ()> {
    match fs::read(path) {
        Ok(bytes) => {
            let lib: Library = serde_json::from_slice(&bytes).map_err(|_| ())?;
            (lib.version == LIBRARY_VERSION).then_some(Some(lib)).ok_or(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
    }
}

fn bak(path: &Path) -> PathBuf {
    path.with_extension("json.bak")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn entry(id: &str) -> LibraryEntry {
        LibraryEntry {
            id: id.into(),
            name: id.into(),
            tagline: String::new(),
            players: "1 a 4 jogadores".into(),
            source: Source::Repo { repo: format!("joao/{id}") },
            cover: None,
            installed: None,
            update: None,
            etag: None,
            edited: false,
        }
    }

    #[test]
    fn save_load_round_trip_and_backup_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        assert_eq!(Library::load(&path).1, LoadNote::Fresh);

        let mut lib = Library::new();
        lib.upsert(entry("com.a.kart"));
        lib.save(&path).unwrap();
        lib.upsert(entry("com.a.arena"));
        lib.save(&path).unwrap();
        assert_eq!(Library::load(&path), (lib.clone(), LoadNote::Ok));

        // arquivo corrompido: volta o último bom
        fs::write(&path, b"{ quebrado").unwrap();
        let (restored, note) = Library::load(&path);
        assert_eq!(note, LoadNote::RestoredFromBackup);
        assert_eq!(restored.games.len(), 1);
    }

    #[test]
    fn upsert_keeps_position_and_user_edits() {
        let mut lib = Library::new();
        lib.upsert(entry("a.x"));
        lib.upsert(entry("a.y"));
        lib.get_mut("a.x").unwrap().name = "Meu nome".into();
        lib.get_mut("a.x").unwrap().edited = true;
        lib.upsert(LibraryEntry { name: "Nome do autor".into(), update: Some("2.0.0".into()), ..entry("a.x") });
        assert_eq!(lib.games[0].id, "a.x");
        assert_eq!(lib.games[0].name, "Meu nome");
        assert_eq!(lib.games[0].update.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn reorder_and_remove() {
        let mut lib = Library::new();
        for id in ["a.1", "a.2", "a.3"] {
            lib.upsert(entry(id));
        }
        assert!(lib.move_game(0, 2));
        assert_eq!(lib.games.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), ["a.2", "a.3", "a.1"]);
        assert!(lib.remove("a.3").is_some());
        assert!(!lib.move_game(0, 9));
    }
}
