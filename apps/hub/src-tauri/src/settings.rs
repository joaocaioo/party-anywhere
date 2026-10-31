//! `settings.json`: ajustes do Hub, layouts do controle do celular
//! (os prontos e os que a pessoa monta na aba Controles) e o layout que cada celular escolheu.
//!
//! Layout: um ou mais **modos** (ex.: "A pé" e "Veículo", para jogos que mudam os comandos), cada um com
//! peças posicionadas numa tela de referência de proporção fixa (`CANVAS`, a tela inteira de um celular
//! atual, deitado). O celular encaixa essa tela inteira na dele (encolhendo junto), então o que não se
//! sobrepõe no editor não se sobrepõe em nenhum celular. No meio do topo fica a pílula do jogador e do Menu
//! (`TOP_PILL`): nenhuma peça vai ali. Os analógicos são "livres": no celular, o esquerdo nasce onde o dedo
//! tocar na metade da tela em que ele está (o direito, na dele), fora das outras peças.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use pa_manifest::controls::{check_modes, Mode, Piece, CANVAS};

pub const SETTINGS_VERSION: u32 = 1;
/// Layout de quem ainda não escolheu.
pub const DEFAULT_LAYOUT: &str = "advanced";
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub modes: Vec<Mode>,
    /// Formato antigo (só a lista de peças): convertido em `modes` ao carregar.
    #[serde(default, skip_serializing)]
    pub controls: Vec<String>,
    /// Pronto (Simples, Avançado): não dá para editar nem apagar; dá para duplicar.
    #[serde(default)]
    pub builtin: bool,
    /// Jogo da biblioteca em que este layout entra sozinho ("Padrão do jogo"). Guardado em `game_layout`
    /// (ou no próprio layout, quando ele veio com o jogo); `all_layouts` preenche para a UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game: Option<String>,
    /// Veio com o jogo (`controls` no manifesto): não dá para editar nem apagar; dá para duplicar.
    #[serde(default)]
    pub from_game: bool,
}

fn p(kind: &str, x_units: f32, y: f32, s: f32) -> Piece {
    Piece { kind: kind.into(), x: x_units / CANVAS, y, s, label: None }
}

/// O Avançado, peça por peça (posições em unidades da altura; x é convertido para 0..1 da largura).
fn advanced_pieces() -> Vec<Piece> {
    vec![
        p("LT", 0.22, 0.1, 1.15),
        p("LB", 0.66, 0.1, 1.15),
        p("RB", 1.51, 0.1, 1.15),
        p("RT", 1.95, 0.1, 1.15),
        p("stickL", 0.3, 0.44, 1.0),
        p("dpad", 0.76, 0.78, 1.1),
        p("BACK", 1.085, 0.33, 1.1),
        p("START", 1.085, 0.47, 1.1),
        p("LS", 1.085, 0.66, 1.1),
        p("RS", 1.085, 0.8, 1.1),
        p("Y", 1.6, 0.35, 1.25),
        p("X", 1.35, 0.6, 1.25),
        p("B", 1.85, 0.6, 1.25),
        p("A", 1.6, 0.85, 1.25),
        p("stickR", 1.98, 0.87, 0.7),
    ]
}

pub fn builtins() -> Vec<Layout> {
    let one_mode = |pieces| vec![Mode { name: "Padrão".into(), pieces }];
    vec![
        Layout {
            id: "simple".into(),
            name: "Simples".into(),
            modes: one_mode(vec![
                p("stickL", 0.5, 0.52, 1.25),
                p("START", 1.085, 0.52, 1.0),
                p("Y", 1.72, 0.25, 1.5),
                p("X", 1.42, 0.55, 1.5),
                p("B", 2.02, 0.55, 1.5),
                p("A", 1.72, 0.85, 1.5),
            ]),
            controls: vec![],
            builtin: true,
            game: None,
            from_game: false,
        },
        Layout {
            id: "advanced".into(),
            name: "Avançado".into(),
            modes: one_mode(advanced_pieces()),
            controls: vec![],
            builtin: true,
            game: None,
            from_game: false,
        },
    ]
}

/// Layout no formato antigo (lista de peças) → um modo com as peças nas posições do Avançado.
fn from_controls(controls: &[String]) -> Vec<Mode> {
    let pieces = advanced_pieces().into_iter().filter(|p| controls.contains(&p.kind)).collect();
    vec![Mode { name: "Padrão".into(), pieces }]
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
    /// Layouts montados pela pessoa.
    #[serde(default)]
    pub layouts: Vec<Layout>,
    /// Chave do celular → layout escolhido.
    #[serde(default)]
    pub phone_layout: HashMap<String, String>,
    /// Jogo da biblioteca → layout que entra sozinho quando ele abre (qualquer layout, até os prontos).
    #[serde(default)]
    pub game_layout: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            join_on_button: false,
            preferred_ip: None,
            layouts: vec![],
            phone_layout: HashMap::new(),
            game_layout: HashMap::new(),
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        let read = |p: &Path| fs::read(p).ok().and_then(|b| serde_json::from_slice::<Settings>(&b).ok());
        let mut s = read(path).or_else(|| read(&path.with_extension("json.bak"))).unwrap_or_default();
        for l in &mut s.layouts {
            if l.modes.is_empty() && !l.controls.is_empty() {
                l.modes = from_controls(&l.controls);
            }
        }
        s
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
        let mut all: Vec<Layout> = builtins().into_iter().chain(self.layouts.iter().cloned()).collect();
        for l in all.iter_mut().filter(|l| !l.from_game) {
            let mut games: Vec<&String> = self.game_layout.iter().filter(|(_, id)| **id == l.id).map(|(g, _)| g).collect();
            games.sort();
            l.game = games.first().map(|g| g.to_string());
        }
        all
    }

    /// Liga um jogo a um layout (`None`: o jogo volta a não ter padrão próprio).
    pub fn set_game(&mut self, game: &str, layout: Option<&str>) -> Result<(), String> {
        match layout {
            Some(id) if self.all_layouts().iter().any(|l| l.id == id) => {
                self.game_layout.insert(game.into(), id.into());
                Ok(())
            }
            Some(_) => Err("esse layout não existe mais.".into()),
            None => {
                self.game_layout.remove(game);
                Ok(())
            }
        }
    }

    pub fn layout_for(&self, device: &str) -> String {
        let id = self.phone_layout.get(device).cloned().unwrap_or_else(|| DEFAULT_LAYOUT.into());
        if self.all_layouts().iter().any(|l| l.id == id) {
            id
        } else {
            DEFAULT_LAYOUT.into()
        }
    }

    /// Cria ou atualiza um layout da pessoa (todos os modos conferidos). `game`: o jogo em que ele entra sozinho.
    pub fn upsert_layout(&mut self, id: Option<&str>, name: &str, modes: Vec<Mode>, game: Option<String>) -> Result<Layout, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 24 {
            return Err("dê um nome de até 24 letras ao layout.".into());
        }
        check_modes(&modes)?;
        let modes: Vec<Mode> = modes.into_iter().map(clean_mode).collect();
        let game = game.filter(|g| !g.trim().is_empty());
        let saved = match id.and_then(|id| self.layouts.iter_mut().find(|l| l.id == id && !l.from_game)) {
            Some(l) => {
                l.name = name.into();
                l.modes = modes;
                l.clone()
            }
            None => {
                let base: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
                let base = base.trim_matches('-').to_string();
                let mut id = format!("custom-{base}");
                let mut n = 2;
                while self.all_layouts().iter().any(|l| l.id == id) {
                    id = format!("custom-{base}-{n}");
                    n += 1;
                }
                let l = Layout { id, name: name.into(), modes, controls: vec![], builtin: false, game: None, from_game: false };
                self.layouts.push(l.clone());
                l
            }
        };
        // o jogo escolhido no editor: este layout passa a ser o padrão dele (e deixa de ser o de outros)
        self.game_layout.retain(|_, l| *l != saved.id);
        if let Some(g) = &game {
            self.game_layout.insert(g.clone(), saved.id.clone());
        }
        Ok(Layout { game, ..saved })
    }

    /// O controle que veio com o jogo (manifesto): entra como "do jogo", no lugar do anterior.
    pub fn set_game_layout(&mut self, game: &str, name: &str, modes: Vec<Mode>) -> Result<Layout, String> {
        check_modes(&modes)?;
        let slug: String = game.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        let l = Layout {
            id: format!("game-{slug}"),
            name: name.chars().take(24).collect(),
            modes: modes.into_iter().map(clean_mode).collect(),
            controls: vec![],
            builtin: false,
            game: Some(game.into()),
            from_game: true,
        };
        self.layouts.retain(|x| x.id != l.id);
        self.layouts.push(l.clone());
        Ok(l)
    }

    /// O layout que entra sozinho quando o jogo abre: o que a pessoa escolheu para ele; senão, o que veio com ele.
    pub fn layout_for_game(&self, game: &str) -> Option<String> {
        let all = self.all_layouts();
        let mine = self.game_layout.get(game).filter(|id| all.iter().any(|l| &l.id == *id)).cloned();
        mine.or_else(|| self.layouts.iter().find(|l| l.from_game && l.game.as_deref() == Some(game)).map(|l| l.id.clone()))
    }

    pub fn delete_layout(&mut self, id: &str) -> bool {
        let before = self.layouts.len();
        self.layouts.retain(|l| l.id != id || l.from_game);
        self.game_layout.retain(|_, l| l != id);
        // quem usava volta para o padrão
        self.phone_layout.retain(|_, l| l != id);
        self.layouts.len() != before
    }
}

fn clean_mode(m: Mode) -> Mode {
    let pieces = m
        .pieces
        .into_iter()
        .map(|p| Piece { label: p.label.map(|l| l.trim().to_string()).filter(|l| !l.is_empty()), ..p })
        .collect();
    Mode { name: m.name.trim().into(), pieces }
}

pub fn path(roaming: &Path) -> PathBuf {
    roaming.join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pa_manifest::controls::check_mode;

    fn mode(name: &str, pieces: Vec<Piece>) -> Mode {
        Mode { name: name.into(), pieces }
    }

    #[test]
    fn builtins_fit_the_screen_without_overlaps() {
        for l in builtins() {
            for m in &l.modes {
                check_mode(m).unwrap_or_else(|e| panic!("{}: {e}", l.name));
            }
        }
    }

    #[test]
    fn modes_are_validated() {
        let mut s = Settings::default();
        assert!(s.upsert_layout(None, "X", vec![], None).is_err(), "precisa de modo");
        let no_button = mode("A pé", vec![p("stickL", 0.4, 0.5, 1.0)]);
        assert!(s.upsert_layout(None, "X", vec![no_button], None).unwrap_err().contains("botão"));
        let out = mode("A pé", vec![p("stickL", 0.1, 0.5, 1.0), p("A", 1.5, 0.5, 1.0)]);
        assert!(s.upsert_layout(None, "X", vec![out], None).unwrap_err().contains("borda"));
        let pill = mode("A pé", vec![p("stickL", 0.4, 0.5, 1.0), p("A", 1.085, 0.1, 1.0)]);
        assert!(s.upsert_layout(None, "X", vec![pill], None).unwrap_err().contains("Menu"));
        let overlap = mode("A pé", vec![p("stickL", 0.4, 0.5, 1.0), p("A", 0.45, 0.5, 1.0)]);
        assert!(s.upsert_layout(None, "X", vec![overlap], None).unwrap_err().contains("uma em cima da outra"));
        let walk = mode("A pé", vec![p("stickL", 0.4, 0.5, 1.0), p("A", 1.6, 0.6, 1.0), p("B", 1.85, 0.4, 1.0)]);
        let drive = mode("Veículo", vec![p("stickL", 0.4, 0.5, 1.0), p("RT", 1.8, 0.5, 1.0), p("A", 1.6, 0.8, 1.0)]);
        let l = s.upsert_layout(None, "GTA", vec![walk, drive], None).unwrap();
        assert_eq!((l.id.as_str(), l.modes.len()), ("custom-gta", 2));
    }

    #[test]
    fn a_game_uses_its_own_layout_first_then_the_one_that_came_with_it() {
        let mut s = Settings::default();
        let m = || vec![mode("Padrão", vec![p("stickL", 0.4, 0.5, 1.0), Piece { label: Some(" Impulso ".into()), ..p("RT", 1.8, 0.5, 1.0) }, p("A", 1.6, 0.8, 1.0)])];
        assert_eq!(s.layout_for_game("speedrunners"), None);
        let shipped = s.set_game_layout("speedrunners", "SpeedRunners", m()).unwrap();
        assert!(shipped.from_game);
        assert_eq!(s.layout_for_game("speedrunners").as_deref(), Some(shipped.id.as_str()));
        let mine = s.upsert_layout(None, "Meu SR", m(), Some("speedrunners".into())).unwrap();
        assert_eq!(mine.modes[0].pieces[1].label.as_deref(), Some("Impulso"));
        assert_eq!(s.layout_for_game("speedrunners").as_deref(), Some(mine.id.as_str()));
        assert!(!s.delete_layout(&shipped.id), "o que veio com o jogo não se apaga");
        s.set_game("speedrunners", Some("simple")).unwrap();
        assert_eq!(s.layout_for_game("speedrunners").as_deref(), Some("simple"), "até um pronto pode ser o padrão");
        assert_eq!(s.all_layouts().iter().find(|l| l.id == "simple").unwrap().game.as_deref(), Some("speedrunners"));
        s.set_game("speedrunners", None).unwrap();
        assert_eq!(s.layout_for_game("speedrunners").as_deref(), Some(shipped.id.as_str()));
        let long = vec![mode("Padrão", vec![p("stickL", 0.4, 0.5, 1.0), Piece { label: Some("Nome comprido demais".into()), ..p("A", 1.6, 0.8, 1.0) }])];
        assert!(s.upsert_layout(None, "X", long, None).is_err());
    }

    #[test]
    fn old_list_layouts_become_positioned() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(dir.path());
        fs::write(&path, r#"{"version":1,"layouts":[{"id":"custom-corrida","name":"Corrida","controls":["stickL","A","LT","RT"]}]}"#).unwrap();
        let s = Settings::load(&path);
        let m = &s.layouts[0].modes[0];
        assert_eq!(m.pieces.iter().map(|p| p.kind.as_str()).collect::<Vec<_>>(), ["LT", "RT", "stickL", "A"]);
        check_mode(m).unwrap();
    }

    #[test]
    fn deleted_layout_sends_its_phones_back_to_the_default() {
        let mut s = Settings::default();
        let m = mode("Padrão", vec![p("stickL", 0.4, 0.5, 1.0), p("A", 1.6, 0.6, 1.0)]);
        let l = s.upsert_layout(None, "Luta", vec![m], None).unwrap();
        s.phone_layout.insert("phone:abc".into(), l.id.clone());
        assert_eq!(s.layout_for("phone:abc"), l.id);
        assert!(s.delete_layout(&l.id));
        assert_eq!(s.layout_for("phone:abc"), DEFAULT_LAYOUT);
    }

    #[test]
    fn save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(dir.path());
        let mut s = Settings { join_on_button: true, ..Default::default() };
        s.upsert_layout(None, "Mira", vec![mode("Padrão", vec![p("stickL", 0.4, 0.5, 1.0), p("A", 1.6, 0.6, 1.0)])], None).unwrap();
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
    }
}
