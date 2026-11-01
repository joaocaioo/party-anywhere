//! O que o jogo do SDK pede para o controle do celular de cada vaga: layout (`pa_set_layout`), modo
//! (`pa_set_mode`: "A pé" → "Veículo") e nomes nos botões (`pa_set_label`). Vale enquanto o jogo está
//! conectado; o Hub aplica ao mandar os layouts para o celular (`Hub::send_layouts`).

use pa_core::MAX_SEATS;
use pa_manifest::controls::MAX_LABEL_CHARS;

#[derive(Default)]
pub struct SdkPhone {
    layout: [Option<&'static str>; MAX_SEATS],
    mode: [Option<u8>; MAX_SEATS],
    labels: [Vec<(&'static str, String)>; MAX_SEATS],
}

/// Bit do `pa_set_label` → a peça do controle do celular. Os direcionais dividem uma peça só.
const LABEL_BITS: [(u32, &str); 18] = [
    (1 << 0, "A"),
    (1 << 1, "B"),
    (1 << 2, "X"),
    (1 << 3, "Y"),
    (1 << 4, "LB"),
    (1 << 5, "RB"),
    (1 << 6, "BACK"),
    (1 << 7, "START"),
    (1 << 8, "LS"),
    (1 << 9, "RS"),
    (1 << 10, "dpad"),
    (1 << 11, "dpad"),
    (1 << 12, "dpad"),
    (1 << 13, "dpad"),
    (1 << 16, "LT"),
    (1 << 17, "RT"),
    (1 << 18, "stickL"),
    (1 << 19, "stickR"),
];

impl SdkPhone {
    /// `PA_LAYOUT_GAMEPAD` e `PA_LAYOUT_SIMPLE` viram os layouts prontos; `DRIVING` e `MENU` ficam
    /// reservados (o modo do controle do mod, com `pa_set_mode`, cobre "dirigindo").
    pub fn set_layout(&mut self, seat: usize, layout: u32) -> bool {
        let id = match layout {
            0 => "advanced",
            1 => "simple",
            _ => return false,
        };
        self.layout[seat] = Some(id);
        true
    }

    pub fn set_mode(&mut self, seat: usize, mode: u32) {
        self.mode[seat] = u8::try_from(mode).ok();
    }

    pub fn set_label(&mut self, seat: usize, button: u32, text: &str) {
        let text: String = text.trim().chars().take(MAX_LABEL_CHARS).collect();
        let labels = &mut self.labels[seat];
        for kind in LABEL_BITS.iter().filter(|(bit, _)| button & bit != 0).map(|(_, k)| *k) {
            labels.retain(|(k, _)| *k != kind);
            if !text.is_empty() {
                labels.push((kind, text.clone()));
            }
        }
    }

    /// O jogador trocou de layout no menu do celular: a escolha dele vale mais que a do jogo.
    pub fn player_chose(&mut self, seat: usize) {
        self.layout[seat] = None;
    }

    pub fn layout(&self, seat: usize) -> Option<&'static str> {
        self.layout[seat]
    }

    pub fn mode(&self, seat: usize) -> Option<u8> {
        self.mode[seat]
    }

    /// Põe os nomes pedidos pelo jogo nas peças do layout `current` (em todos os modos dele).
    pub fn apply_labels(&self, seat: usize, items: &mut serde_json::Value, current: &str) {
        let labels = &self.labels[seat];
        if labels.is_empty() {
            return;
        }
        let Some(layout) = items.as_array_mut().and_then(|a| a.iter_mut().find(|l| l["id"] == current)) else { return };
        // os modos e a versão para a tela do jogo no celular (`screen`, nos prontos)
        for list in ["modes", "screen"] {
            let Some(modes) = layout[list].as_array_mut() else { continue };
            for piece in modes.iter_mut().filter_map(|m| m["pieces"].as_array_mut()).flatten() {
                if let Some((_, text)) = labels.iter().find(|(k, _)| piece["kind"] == *k) {
                    piece["label"] = serde_json::Value::String(text.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn labels_land_on_the_pieces_of_the_current_layout_only() {
        let mut s = SdkPhone::default();
        s.set_label(0, 1 << 17, "Acelerar");
        s.set_label(0, 1 << 0 | 1 << 1, "Pular");
        s.set_label(0, 1 << 1, ""); // tira o do B
        s.set_label(0, 1 << 2, "Um nome comprido demais");
        let mut items = json!([
            { "id": "game-sa", "modes": [
                { "name": "A pé", "pieces": [{ "kind": "A", "label": "" }, { "kind": "B", "label": "Correr" }, { "kind": "X" }] },
                { "name": "Veículo", "pieces": [{ "kind": "RT", "label": "" }] }
            ] },
            { "id": "simple", "modes": [{ "name": "", "pieces": [{ "kind": "A", "label": "" }] }],
              "screen": [{ "name": "", "pieces": [{ "kind": "A" }] }] }
        ]);
        s.apply_labels(0, &mut items, "game-sa");
        assert_eq!(items[0]["modes"][0]["pieces"][0]["label"], "Pular");
        assert_eq!(items[0]["modes"][0]["pieces"][1]["label"], "Correr", "sem nome do jogo, fica o do layout");
        assert_eq!(items[0]["modes"][0]["pieces"][2]["label"], "Um nome comp");
        assert_eq!(items[0]["modes"][1]["pieces"][0]["label"], "Acelerar");
        assert_eq!(items[1]["modes"][0]["pieces"][0]["label"], "", "outro layout não muda");
        let mut simple = items.clone();
        s.apply_labels(0, &mut simple, "simple");
        assert_eq!(simple[1]["screen"][0]["pieces"][0]["label"], "Pular", "a versão da tela do jogo também");
        let mut other = items.clone();
        s.apply_labels(1, &mut other, "game-sa");
        assert_eq!(other, items, "cada vaga tem os seus nomes");
    }

    #[test]
    fn layout_and_mode_requests() {
        let mut s = SdkPhone::default();
        assert!(s.set_layout(3, 1));
        assert_eq!(s.layout(3), Some("simple"));
        assert!(!s.set_layout(3, 2), "DRIVING é reservado");
        s.player_chose(3);
        assert_eq!(s.layout(3), None);
        s.set_mode(3, 1);
        assert_eq!(s.mode(3), Some(1));
        s.set_mode(3, 999);
        assert_eq!(s.mode(3), None);
    }
}
