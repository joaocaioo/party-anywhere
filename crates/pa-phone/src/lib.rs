//! Celular como controle (protocolo v1: `protocol.rs`).
//!
//! - [`protocol`]: codificação das mensagens, pura e testada.
//! - [`net`]: escolha do endereço da rede local que vai no QR.
//! - [`server`]: servidor HTTP + WebSocket que entrega eventos a quem o usa (o Hub, ou o spike).

pub mod net;
pub mod protocol;
pub mod server;

pub use server::{PhoneEvent, PhoneServer, ServerConfig};

/// Página do controle, embutida no binário (o celular não precisa de internet).
pub const CONTROLLER_PAGE: &str = include_str!("../web/controller.html");
