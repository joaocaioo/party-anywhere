//! `settings.json` (docs/ARCHITECTURE.md, "Dados salvos"): ajustes do Hub, layouts do controle do celular
//! (os prontos e os que a pessoa monta na aba Controles) e o layout que cada celular escolheu.
//!
//! Layout: um ou mais **modos** (ex.: "A pé" e "Veículo", para jogos que mudam os comandos), cada um com
//! peças posicionadas numa tela de referência de proporção fixa (`CANVAS`, a dos celulares atuais). O
//! celular encaixa essa tela inteira na dele (encolhendo junto), então o que não se sobrepõe no editor não
//! se sobrepõe em nenhum celular.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const SETTINGS_VERSION: u32 = 1;
/// Layout de quem ainda não escolheu.
pub const DEFAULT_LAYOUT: &str = "advanced";
/// Proporção da tela de referência (largura ÷ altura), a área do controle abaixo da barra do topo.
pub const CANVAS: f32 = 2.17;
pub const MAX_MODES: usize = 6;

/// Peças e o tamanho base (largura, altura) em unidades da altura da tela de referência.
pub const PIECES: [(&str, f32, f32); 15] = [
    ("stickL", 0.5, 0.5),
    ("stickR", 0.36, 0.36),
    ("dpad", 0.36, 0.36),
    ("A", 0.2, 0.2),
    ("B", 0.2, 0.2),
    ("X", 0.2, 0.2),
    ("Y", 0.2, 0.2),
    ("LB", 0.36, 0.14),
    ("RB", 0.36, 0.14),
    ("LT", 0.36, 0.14),
    ("RT", 0.36, 0.14),
    ("BACK", 0.2, 0.11),
    ("START", 0.2, 0.11),
    ("LS", 0.2, 0.11),
    ("RS", 0.2, 0.11),
];

/// Uma peça: centro em (x, y) na tela de referência (0..1 nos dois eixos) e um multiplicador de tamanho.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Piece {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    #[serde(default = "one")]
    pub s: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mode {
    pub name: String,
    pub pieces: Vec<Piece>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
}

fn p(kind: &str, x_units: f32, y: f32, s: f32) -> Piece {
    Piece { kind: kind.into(), x: x_units / CANVAS, y, s }
}

/// O Avançado, peça por peça (posições em unidades da altura; x é convertido para 0..1 da largura).
fn advanced_pieces() -> Vec<Piece> {
    vec![
        p("LT", 0.24, 0.09, 1.0),
        p("LB", 0.64, 0.09, 1.0),
        p("RB", 1.53, 0.09, 1.0),
        p("RT", 1.93, 0.09, 1.0),
        p("stickL", 0.36, 0.44, 1.0),
        p("dpad", 0.8, 0.8, 1.0),
        p("BACK", 1.085, 0.3, 1.0),
        p("START", 1.085, 0.43, 1.0),
        p("LS", 1.085, 0.62, 1.0),
        p("RS", 1.085, 0.75, 1.0),
        p("Y", 1.62, 0.28, 1.0),
        p("X", 1.42, 0.48, 1.0),
        p("B", 1.82, 0.48, 1.0),
        p("A", 1.62, 0.68, 1.0),
        p("stickR", 1.91, 0.8, 1.0),
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
                p("Y", 1.7, 0.29, 1.25),
                p("X", 1.44, 0.55, 1.25),
                p("B", 1.96, 0.55, 1.25),
                p("A", 1.7, 0.81, 1.25),
            ]),
            controls: vec![],
            builtin: true,
        },
        Layout { id: "advanced".into(), name: "Avançado".into(), modes: one_mode(advanced_pieces()), controls: vec![], builtin: true },
    ]
}

/// Layout no formato antigo (lista de peças) → um modo com as peças nas posições do Avançado.
fn from_controls(controls: &[String]) -> Vec<Mode> {
    let pieces = advanced_pieces().into_iter().filter(|p| controls.contains(&p.kind)).collect();
    vec![Mode { name: "Padrão".into(), pieces }]
}

fn base_size(kind: &str) -> Option<(f32, f32)> {
    PIECES.iter().find(|(k, _, _)| *k == kind).map(|(_, w, h)| (*w, *h))
}

/// Retângulo da peça em unidades da altura (x0, y0, x1, y1).
pub fn rect(p: &Piece) -> Option<(f32, f32, f32, f32)> {
    let (w, h) = base_size(&p.kind)?;
    let (w, h) = (w * p.s, h * p.s);
    let cx = p.x * CANVAS;
    Some((cx - w / 2.0, p.y - h / 2.0, cx + w / 2.0, p.y + h / 2.0))
}

/// O modo pode ir para os celulares? Erro em português, dizendo o que ajustar.
pub fn check_mode(m: &Mode) -> Result<(), String> {
    let name = m.name.trim();
    if name.is_empty() || name.chars().count() > 20 {
        return Err("dê um nome de até 20 letras a cada modo (ex.: A pé, Veículo).".into());
    }
    let mut seen = Vec::new();
    for piece in &m.pieces {
        if seen.contains(&piece.kind) {
            return Err(format!("o modo {name} tem a peça {} duas vezes.", piece.kind));
        }
        seen.push(piece.kind.clone());
        if !(0.6..=1.6).contains(&piece.s) {
            return Err(format!("tamanho fora do limite em {} ({name}).", piece.kind));
        }
        let (x0, y0, x1, y1) = rect(piece).ok_or_else(|| format!("peça desconhecida: {}", piece.kind))?;
        if x0 < -0.001 || y0 < -0.001 || x1 > CANVAS + 0.001 || y1 > 1.001 {
            return Err(format!("{} passa da borda da tela no modo {name}.", piece.kind));
        }
    }
    let has_dir = seen.iter().any(|k| k == "stickL" || k == "stickR" || k == "dpad");
    let has_button = seen.iter().any(|k| ["A", "B", "X", "Y"].contains(&k.as_str()));
    if !has_dir || !has_button {
        return Err(format!("o modo {name} precisa de um analógico ou das setas e de ao menos um botão (A, B, X ou Y)."));
    }
    // peças sobrepostas: no celular, um toque cairia nas duas
    for (i, a) in m.pieces.iter().enumerate() {
        for b in &m.pieces[i + 1..] {
            let (a0, a1, a2, a3) = rect(a).unwrap();
            let (b0, b1, b2, b3) = rect(b).unwrap();
            if a0 < b2 - 0.005 && b0 < a2 - 0.005 && a1 < b3 - 0.005 && b1 < a3 - 0.005 {
                return Err(format!("{} e {} estão uma em cima da outra no modo {name}.", a.kind, b.kind));
            }
        }
    }
    Ok(())
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
}

impl Default for Settings {
    fn default() -> Self {
        Self { version: SETTINGS_VERSION, join_on_button: false, preferred_ip: None, layouts: vec![], phone_layout: HashMap::new() }
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

    /// Cria ou atualiza um layout da pessoa (todos os modos conferidos).
    pub fn upsert_layout(&mut self, id: Option<&str>, name: &str, modes: Vec<Mode>) -> Result<Layout, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 24 {
            return Err("dê um nome de até 24 letras ao layout.".into());
        }
        if modes.is_empty() || modes.len() > MAX_MODES {
            return Err(format!("um layout tem de 1 a {MAX_MODES} modos."));
        }
        for m in &modes {
            check_mode(m)?;
        }
        let modes: Vec<Mode> = modes.into_iter().map(|m| Mode { name: m.name.trim().into(), pieces: m.pieces }).collect();
        match id.and_then(|id| self.layouts.iter_mut().find(|l| l.id == id)) {
            Some(l) => {
                l.name = name.into();
                l.modes = modes;
                Ok(l.clone())
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
                let l = Layout { id, name: name.into(), modes, controls: vec![], builtin: false };
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
        assert!(s.upsert_layout(None, "X", vec![]).is_err(), "precisa de modo");
        let no_button = mode("A pé", vec![p("stickL", 0.4, 0.5, 1.0)]);
        assert!(s.upsert_layout(None, "X", vec![no_button]).unwrap_err().contains("botão"));
        let out = mode("A pé", vec![p("stickL", 0.1, 0.5, 1.0), p("A", 1.5, 0.5, 1.0)]);
        assert!(s.upsert_layout(None, "X", vec![out]).unwrap_err().contains("borda"));
        let overlap = mode("A pé", vec![p("stickL", 0.4, 0.5, 1.0), p("A", 0.45, 0.5, 1.0)]);
        assert!(s.upsert_layout(None, "X", vec![overlap]).unwrap_err().contains("uma em cima da outra"));
        let walk = mode("A pé", vec![p("stickL", 0.4, 0.5, 1.0), p("A", 1.6, 0.6, 1.0), p("B", 1.85, 0.4, 1.0)]);
        let drive = mode("Veículo", vec![p("stickL", 0.4, 0.5, 1.0), p("RT", 1.8, 0.5, 1.0), p("A", 1.6, 0.8, 1.0)]);
        let l = s.upsert_layout(None, "GTA", vec![walk, drive]).unwrap();
        assert_eq!((l.id.as_str(), l.modes.len()), ("custom-gta", 2));
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
        let l = s.upsert_layout(None, "Luta", vec![m]).unwrap();
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
        s.upsert_layout(None, "Mira", vec![mode("Padrão", vec![p("stickL", 0.4, 0.5, 1.0), p("A", 1.6, 0.6, 1.0)])]).unwrap();
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
    }
}
