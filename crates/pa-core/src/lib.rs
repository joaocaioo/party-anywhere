//! Modelo comum do Party Anywhere (ver docs/ARCHITECTURE.md, "Modelo de dados").
//! Nada aqui faz I/O: tudo é testável sem hardware.

pub mod pad;

pub use pad::{buttons, Pad};

/// Quantidade máxima de vagas no lobby (P1..P10).
pub const MAX_SEATS: usize = 10;

/// Cor fixa de cada vaga, na ordem P1..P10 (docs/UX.md).
pub const SEAT_COLORS: [&str; MAX_SEATS] = [
    "#3B82F6", "#EF4444", "#22C55E", "#EAB308", "#A855F7", "#F97316", "#06B6D4", "#EC4899", "#84CC16", "#F5F5F4",
];
