//! O controle do celular que um jogo pode trazer (`controls` no `party-anywhere.json`) e os layouts que a pessoa
//! monta no Hub: as mesmas peças e as mesmas regras (docs/GAME-MANIFEST.md, "Controle no celular").
//!
//! Um layout tem de 1 a 6 **modos** (ex.: "A pé" e "Veículo"); cada modo, peças posicionadas numa tela de
//! referência deitada de proporção `CANVAS`. O celular encaixa essa tela inteira na dele, então o que passa
//! aqui não se sobrepõe em nenhum celular. No meio do topo fica a pílula do jogador e do Menu (`TOP_PILL`).

use serde::{Deserialize, Serialize};

/// Proporção da tela de referência (largura ÷ altura): a tela inteira de um celular atual, deitado.
pub const CANVAS: f32 = 2.17;
pub const MAX_MODES: usize = 6;
/// Área reservada à pílula do topo (jogador, conexão, Menu): meia largura e altura, em unidades da altura.
pub const TOP_PILL: (f32, f32) = (0.22, 0.14);

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

/// Nome mais longo num botão ("Impulso", "Pular").
pub const MAX_LABEL_CHARS: usize = 12;

/// Uma peça: centro em (x, y) na tela de referência (0..1 nos dois eixos), um multiplicador de tamanho e,
/// opcional, o nome da ação no jogo, que aparece no botão.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Piece {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    #[serde(default = "one")]
    pub s: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mode {
    pub name: String,
    pub pieces: Vec<Piece>,
}

/// O controle que um jogo traz: os modos, na ordem em que aparecem no celular.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Controls {
    pub modes: Vec<Mode>,
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
        if let Some(label) = &piece.label {
            let n = label.trim().chars().count();
            if n == 0 || n > MAX_LABEL_CHARS {
                return Err(format!("o nome no botão {} ({name}) precisa ter de 1 a {MAX_LABEL_CHARS} letras.", piece.kind));
            }
        }
        let (x0, y0, x1, y1) = rect(piece).ok_or_else(|| format!("peça desconhecida: {}", piece.kind))?;
        if x0 < -0.001 || y0 < -0.001 || x1 > CANVAS + 0.001 || y1 > 1.001 {
            return Err(format!("{} passa da borda da tela no modo {name}.", piece.kind));
        }
        let mid = CANVAS / 2.0;
        if x0 < mid + TOP_PILL.0 - 0.005 && x1 > mid - TOP_PILL.0 + 0.005 && y0 < TOP_PILL.1 - 0.005 {
            return Err(format!("{} está embaixo do Menu (no meio do topo) no modo {name}.", piece.kind));
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

/// Todos os modos de um controle (1 a `MAX_MODES`, cada um válido).
pub fn check_modes(modes: &[Mode]) -> Result<(), String> {
    if modes.is_empty() || modes.len() > MAX_MODES {
        return Err(format!("um controle tem de 1 a {MAX_MODES} modos."));
    }
    modes.iter().try_for_each(check_mode)
}
