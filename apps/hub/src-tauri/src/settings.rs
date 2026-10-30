//! `settings.json` (docs/ARCHITECTURE.md, "Dados salvos"): ajustes do Hub, layouts do controle do celular
//! (os prontos e os que a pessoa cria na aba Controles) e o layout que cada celular escolheu.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const SETTINGS_VERSION: u32 = 1;
/// Layout de quem ainda não escolheu.
pub const DEFAULT_LAYOUT: &str = "advanced";
/// Peças que um layout pode ter (a página do celular desenha cada uma na sua zona, sem sobrepor).
pub const CONTROLS: [&str; 15] = ["stickL", "stickR", "dpad", "A", "B", "X", "Y", "LB", "RB", "LT", "RT", "BACK", "START", "LS", "RS"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub id: String,
    pub name: String,
    pub controls: Vec<String>,
    /// Pronto (Simples, Avançado): não dá para editar nem apagar.
    #[serde(default)]
    pub builtin: bool,
}

pub fn builtins() -> Vec<Layout> {
    let l = |id: &str, name: &str, c: &[&str]| Layout {
        id: id.into(),
        name: name.into(),
        controls: c.iter().map(|s| s.to_string()).collect(),
        builtin: true,
    };
    vec![
        l("simple", "Simples", &["stickL", "A", "B", "START"]),
        l("advanced", "Avançado", &CONTROLS),
    ]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    /// "Entrar só ao apertar um botão".
    #[serde(default)]
    pub join_on_button: bool,
    /// Endereço do QR escolhido nos Ajustes (vazio: o Hub escolhe).
    #[serde(default)]
    pub preferred_ip: Option<String>,
    /// Layouts criados pela pessoa.
    #[serde(default)]
    pub layouts: Vec<Layout>,
    /// Chave do celular → layout escolhido.
    #[serde(default)]
    pub phone_layout: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { version: SETTINGS_VERSION, join_on_button: false, preferred_ip: None, layouts: vec![], phone_layout: HashMap::new() }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        let read = |p: &Path| fs::read(p).ok().and_then(|b| serde_json::from_slice::<Settings>(&b).ok());
        read(path).or_else(|| read(&path.with_extension("json.bak"))).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self).expect("Settings serializa"))?;
        if path.exists() {
            fs::copy(path, path.with_extension("json.bak"))?;
        }
        fs::rename(tmp, path)
    }

    pub fn all_layouts(&self) -> Vec<Layout> {
        builtins().into_iter().chain(self.layouts.iter().cloned()).collect()
    }

    pub fn layout_for(&self, device: &str) -> String {
        let id = self.phone_layout.get(device).cloned().unwrap_or_else(|| DEFAULT_LAYOUT.into());
        if self.all_layouts().iter().any(|l| l.id == id) {
            id
        } else {
            DEFAULT_LAYOUT.into()
        }
    }

    /// Cria ou atualiza um layout da pessoa. Só peças conhecidas, sem repetir; precisa de nome e de ao
    /// menos um controle de direção e um botão.
    pub fn upsert_layout(&mut self, id: Option<&str>, name: &str, controls: &[String]) -> Result<Layout, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 24 {
            return Err("dê um nome de até 24 letras ao layout.".into());
        }
        let mut picked: Vec<String> = CONTROLS.iter().filter(|c| controls.iter().any(|x| x == *c)).map(|c| c.to_string()).collect();
        picked.dedup();
        let has_dir = picked.iter().any(|c| c == "stickL" || c == "stickR" || c == "dpad");
        let has_button = picked.iter().any(|c| ["A", "B", "X", "Y"].contains(&c.as_str()));
        if !has_dir || !has_button {
            return Err("o layout precisa de um analógico ou direcional e de ao menos um botão (A, B, X ou Y).".into());
        }
        match id.and_then(|id| self.layouts.iter_mut().find(|l| l.id == id)) {
            Some(l) => {
                l.name = name.into();
                l.controls = picked;
                Ok(l.clone())
            }
            None => {
                let base: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
                let mut id = format!("custom-{}", base.trim_matches('-'));
                let mut n = 2;
                while self.all_layouts().iter().any(|l| l.id == id) {
                    id = format!("custom-{}-{n}", base.trim_matches('-'));
                    n += 1;
                }
                let l = Layout { id, name: name.into(), controls: picked, builtin: false };
                self.layouts.push(l.clone());
                Ok(l)
            }
        }
    }

    pub fn delete_layout(&mut self, id: &str) -> bool {
        let before = self.layouts.len();
        self.layouts.retain(|l| l.id != id);
        // quem usava volta para o padrão
        self.phone_layout.retain(|_, l| l != id);
        self.layouts.len() != before
    }
}

pub fn path(roaming: &Path) -> PathBuf {
    roaming.join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(c: &[&str]) -> Vec<String> {
        c.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn custom_layouts_are_validated_and_ordered() {
        let mut s = Settings::default();
        assert!(s.upsert_layout(None, "", &v(&["stickL", "A"])).is_err());
        assert!(s.upsert_layout(None, "Só botões", &v(&["A", "B"])).is_err(), "precisa de direção");
        assert!(s.upsert_layout(None, "Só direção", &v(&["dpad"])).is_err(), "precisa de botão");
        let l = s.upsert_layout(None, "Corrida!", &v(&["RT", "stickL", "A", "LT", "nada", "A"])).unwrap();
        assert_eq!(l.id, "custom-corrida");
        assert_eq!(l.controls, v(&["stickL", "A", "LT", "RT"]), "ordem canônica, sem repetir, sem peça desconhecida");
        let again = s.upsert_layout(None, "Corrida", &v(&["stickL", "A"])).unwrap();
        assert_eq!(again.id, "custom-corrida-2");
        let edited = s.upsert_layout(Some("custom-corrida"), "Corrida 2", &v(&["dpad", "B"])).unwrap();
        assert_eq!((edited.id.as_str(), edited.name.as_str()), ("custom-corrida", "Corrida 2"));
        assert_eq!(s.all_layouts().len(), 4);
    }

    #[test]
    fn deleted_layout_sends_its_phones_back_to_the_default() {
        let mut s = Settings::default();
        let l = s.upsert_layout(None, "Luta", &v(&["stickL", "A", "B", "X", "Y"])).unwrap();
        s.phone_layout.insert("phone:abc".into(), l.id.clone());
        assert_eq!(s.layout_for("phone:abc"), l.id);
        assert!(s.delete_layout(&l.id));
        assert_eq!(s.layout_for("phone:abc"), DEFAULT_LAYOUT);
        assert_eq!(s.layout_for("phone:novo"), DEFAULT_LAYOUT);
    }

    #[test]
    fn save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let p = path(dir.path());
        let mut s = Settings { join_on_button: true, ..Default::default() };
        s.upsert_layout(None, "Mira", &v(&["stickL", "stickR", "RT", "A"])).unwrap();
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p), s);
    }
}
