/// Estado de um controle no formato comum (controle de Xbox), venha de onde vier:
/// controle físico, celular ou teclado. É sempre o estado completo, nunca um evento.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad {
    /// Bits de [`buttons`].
    pub buttons: u32,
    /// Analógicos em -1..1, direita e cima positivos, zona morta já aplicada.
    pub lx: f32,
    pub ly: f32,
    pub rx: f32,
    pub ry: f32,
    /// Gatilhos em 0..1.
    pub lt: f32,
    pub rt: f32,
}

impl Pad {
    pub const NEUTRAL: Pad = Pad { buttons: 0, lx: 0.0, ly: 0.0, rx: 0.0, ry: 0.0, lt: 0.0, rt: 0.0 };

    pub fn is_neutral(&self) -> bool {
        *self == Self::NEUTRAL
    }
}

/// Bits de botão. Mesma numeração no protocolo do celular e no SDK (docs/SDK.md).
pub mod buttons {
    pub const A: u32 = 1 << 0;
    pub const B: u32 = 1 << 1;
    pub const X: u32 = 1 << 2;
    pub const Y: u32 = 1 << 3;
    pub const LB: u32 = 1 << 4;
    pub const RB: u32 = 1 << 5;
    pub const BACK: u32 = 1 << 6;
    pub const START: u32 = 1 << 7;
    pub const LS: u32 = 1 << 8;
    pub const RS: u32 = 1 << 9;
    pub const UP: u32 = 1 << 10;
    pub const DOWN: u32 = 1 << 11;
    pub const LEFT: u32 = 1 << 12;
    pub const RIGHT: u32 = 1 << 13;
    pub const GUIDE: u32 = 1 << 14;

    pub const ALL: u32 = (1 << 15) - 1;
}
