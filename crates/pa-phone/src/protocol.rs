//! Protocolo v1 entre a página do controle e o Hub.
//!
//! Binário (caminho quente), little endian, primeiro byte = tipo:
//!
//! | Dir | Tipo | Formato |
//! |---|---|---|
//! | C→S | `0x02 STATE`   | `[02][seq u16][botões u16][lx i8][ly i8][rx i8][ry i8][lt u8][rt u8]` (11 bytes) |
//! | C→S | `0x03 PING`    | `[03][t u16]` |
//! | C→S | `0x05 STATE_T` | STATE + `[tipo u8][t f64]` (20 bytes): estado com o instante do toque no relógio do celular |
//! | C→S | `0x06 SYNC`    | `[06][id u16][t0 f64]` |
//! | C→S | `0x07 SYNC_DONE` | `[07][id u16][t1 f64]` |
//! | S→C | `0x83 PONG`    | `[83][t u16]` |
//! | S→C | `0x84 RUMBLE`  | `[84][ms u16][força u8]` |
//! | S→C | `0x85 SYNC_REPLY` | `[85][id u16]` |
//!
//! STATE_T, SYNC e SYNC_DONE são as mensagens de medição: com elas o Hub estima a diferença entre
//! os relógios (como no NTP, usando a amostra de menor ida e volta) e mede quanto tempo um toque
//! leva para chegar ao PC. A página do Hub usa STATE_T sempre; uma página mínima pode usar só STATE.
//!
//! Texto (JSON, baixa frequência): `hello` / `welcome` / `stats` / `toast` (ver [`ClientText`], [`ServerText`]).

use pa_core::{buttons, Pad};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u8 = 1;
/// Mensagem binária maior que isto derruba a conexão.
pub const MAX_BINARY: usize = 64;
/// Mensagem de texto maior que isto derruba a conexão.
pub const MAX_TEXT: usize = 4096;

pub const T_STATE: u8 = 0x02;
pub const T_PING: u8 = 0x03;
pub const T_STATE_T: u8 = 0x05;
pub const T_SYNC: u8 = 0x06;
pub const T_SYNC_DONE: u8 = 0x07;
pub const T_PONG: u8 = 0x83;
pub const T_RUMBLE: u8 = 0x84;
pub const T_SYNC_REPLY: u8 = 0x85;

/// De onde veio uma amostra STATE_T. Cada tipo mede uma coisa diferente.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SampleKind {
    /// Reenvio sem mudança (heartbeat): instante = envio.
    Heartbeat,
    /// Mudança causada por um toque: instante = criação do evento de toque no navegador.
    Touch,
    /// Rajada automática do teste: instante = envio. Mede só rede + servidor.
    Burst,
    /// Movimento do analógico: instante = criação do `pointermove`. Separado do toque porque o navegador
    /// agrupa movimentos contínuos, e isso não é o atraso de apertar um botão.
    Stick,
}

impl SampleKind {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Heartbeat),
            1 => Some(Self::Touch),
            2 => Some(Self::Burst),
            3 => Some(Self::Stick),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::Heartbeat => 0,
            Self::Touch => 1,
            Self::Burst => 2,
            Self::Stick => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClientMsg {
    State { seq: u16, pad: Pad },
    StateTimed { seq: u16, pad: Pad, kind: SampleKind, phone_ms: f64 },
    Ping { t: u16 },
    Sync { id: u16, t0: f64 },
    SyncDone { id: u16, t1: f64 },
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    Empty,
    TooLong,
    UnknownType(u8),
    BadLength { kind: u8, len: usize },
    BadField,
}

/// Decodifica uma mensagem binária vinda do celular.
pub fn decode_client(data: &[u8]) -> Result<ClientMsg, DecodeError> {
    if data.len() > MAX_BINARY {
        return Err(DecodeError::TooLong);
    }
    let (&kind, rest) = data.split_first().ok_or(DecodeError::Empty)?;
    let need = |n: usize| if rest.len() == n { Ok(()) } else { Err(DecodeError::BadLength { kind, len: data.len() }) };
    match kind {
        T_STATE => {
            need(10)?;
            let (seq, pad) = decode_state_body(rest);
            Ok(ClientMsg::State { seq, pad })
        }
        T_STATE_T => {
            need(19)?;
            let (seq, pad) = decode_state_body(&rest[..10]);
            let kind = SampleKind::from_u8(rest[10]).ok_or(DecodeError::BadField)?;
            let phone_ms = f64_at(rest, 11);
            if !phone_ms.is_finite() {
                return Err(DecodeError::BadField);
            }
            Ok(ClientMsg::StateTimed { seq, pad, kind, phone_ms })
        }
        T_PING => {
            need(2)?;
            Ok(ClientMsg::Ping { t: u16_at(rest, 0) })
        }
        T_SYNC | T_SYNC_DONE => {
            need(10)?;
            let id = u16_at(rest, 0);
            let t = f64_at(rest, 2);
            if !t.is_finite() {
                return Err(DecodeError::BadField);
            }
            Ok(if kind == T_SYNC { ClientMsg::Sync { id, t0: t } } else { ClientMsg::SyncDone { id, t1: t } })
        }
        other => Err(DecodeError::UnknownType(other)),
    }
}

/// Codifica STATE / STATE_T como a página faz (usado nos testes e em ferramentas).
pub fn encode_state(seq: u16, pad: &Pad, timed: Option<(SampleKind, f64)>) -> Vec<u8> {
    let mut out = Vec::with_capacity(20);
    out.push(if timed.is_some() { T_STATE_T } else { T_STATE });
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&((pad.buttons & buttons::ALL) as u16).to_le_bytes());
    for a in [pad.lx, pad.ly, pad.rx, pad.ry] {
        out.push(axis_to_byte(a) as u8);
    }
    out.push(trigger_to_byte(pad.lt));
    out.push(trigger_to_byte(pad.rt));
    if let Some((kind, t)) = timed {
        out.push(kind.as_u8());
        out.extend_from_slice(&t.to_le_bytes());
    }
    out
}

pub fn encode_pong(t: u16) -> Vec<u8> {
    let mut v = vec![T_PONG];
    v.extend_from_slice(&t.to_le_bytes());
    v
}

pub fn encode_sync_reply(id: u16) -> Vec<u8> {
    let mut v = vec![T_SYNC_REPLY];
    v.extend_from_slice(&id.to_le_bytes());
    v
}

pub fn encode_rumble(ms: u16, strength: u8) -> Vec<u8> {
    let mut v = vec![T_RUMBLE];
    v.extend_from_slice(&ms.to_le_bytes());
    v.push(strength);
    v
}

/// Mensagens de texto do celular.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ClientText {
    Hello {
        v: u8,
        #[serde(default)]
        device: String,
        #[serde(default)]
        token: Option<String>,
    },
}

/// Mensagens de texto para o celular.
#[derive(Debug, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerText<'a> {
    Welcome { seat: u8, color: &'a str, name: &'a str },
    Reject { reason: &'a str },
    Toast { text: &'a str },
    /// Resultado da medição, para a página mostrar ao vivo. Formato livre (JSON do spike).
    Stats { data: serde_json::Value },
}

fn decode_state_body(b: &[u8]) -> (u16, Pad) {
    let seq = u16_at(b, 0);
    let pad = Pad {
        buttons: u32::from(u16_at(b, 2)) & buttons::ALL,
        lx: axis_from_byte(b[4] as i8),
        ly: axis_from_byte(b[5] as i8),
        rx: axis_from_byte(b[6] as i8),
        ry: axis_from_byte(b[7] as i8),
        lt: f32::from(b[8]) / 255.0,
        rt: f32::from(b[9]) / 255.0,
    };
    (seq, pad)
}

fn axis_from_byte(v: i8) -> f32 {
    // -128 também vale -1, para o eixo ser simétrico
    (f32::from(v) / 127.0).clamp(-1.0, 1.0)
}

fn axis_to_byte(v: f32) -> i8 {
    (v.clamp(-1.0, 1.0) * 127.0).round() as i8
}

fn trigger_to_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn f64_at(b: &[u8], i: usize) -> f64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[i..i + 8]);
    f64::from_le_bytes(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pad() -> Pad {
        Pad { buttons: buttons::A | buttons::RB | buttons::UP, lx: 1.0, ly: -0.5, rx: 0.0, ry: -1.0, lt: 0.0, rt: 1.0 }
    }

    #[test]
    fn state_round_trip() {
        let bytes = encode_state(513, &sample_pad(), None);
        assert_eq!(bytes.len(), 11);
        let ClientMsg::State { seq, pad } = decode_client(&bytes).unwrap() else { panic!() };
        assert_eq!(seq, 513);
        assert_eq!(pad.buttons, sample_pad().buttons);
        assert!((pad.ly - (-64.0 / 127.0)).abs() < 1e-6);
        assert_eq!((pad.lx, pad.ry, pad.rt), (1.0, -1.0, 1.0));
    }

    #[test]
    fn timed_state_round_trip() {
        let bytes = encode_state(7, &sample_pad(), Some((SampleKind::Touch, 12345.678)));
        assert_eq!(bytes.len(), 20);
        let ClientMsg::StateTimed { seq, kind, phone_ms, .. } = decode_client(&bytes).unwrap() else { panic!() };
        assert_eq!((seq, kind, phone_ms), (7, SampleKind::Touch, 12345.678));
    }

    #[test]
    fn minus_128_is_full_left() {
        let mut bytes = encode_state(0, &Pad::NEUTRAL, None);
        bytes[5] = 0x80;
        let ClientMsg::State { pad, .. } = decode_client(&bytes).unwrap() else { panic!() };
        assert_eq!(pad.lx, -1.0);
    }

    #[test]
    fn unknown_buttons_are_dropped() {
        let mut bytes = encode_state(0, &Pad::NEUTRAL, None);
        bytes[3] = 0xFF;
        bytes[4] = 0xFF;
        let ClientMsg::State { pad, .. } = decode_client(&bytes).unwrap() else { panic!() };
        assert_eq!(pad.buttons, buttons::ALL);
    }

    #[test]
    fn sync_messages() {
        let mut m = vec![T_SYNC, 3, 0];
        m.extend_from_slice(&1.5f64.to_le_bytes());
        assert_eq!(decode_client(&m), Ok(ClientMsg::Sync { id: 3, t0: 1.5 }));
        m[0] = T_SYNC_DONE;
        assert_eq!(decode_client(&m), Ok(ClientMsg::SyncDone { id: 3, t1: 1.5 }));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(decode_client(&[]), Err(DecodeError::Empty));
        assert_eq!(decode_client(&[0x42]), Err(DecodeError::UnknownType(0x42)));
        assert!(matches!(decode_client(&[T_STATE, 1, 2]), Err(DecodeError::BadLength { .. })));
        assert_eq!(decode_client(&[0u8; 65]), Err(DecodeError::TooLong));
        let mut nan = vec![T_SYNC, 0, 0];
        nan.extend_from_slice(&f64::NAN.to_le_bytes());
        assert_eq!(decode_client(&nan), Err(DecodeError::BadField));
        let stick = encode_state(1, &Pad::NEUTRAL, Some((SampleKind::Stick, 1.0)));
        assert!(matches!(decode_client(&stick), Ok(ClientMsg::StateTimed { kind: SampleKind::Stick, .. })));

        let mut bad_kind = encode_state(0, &Pad::NEUTRAL, Some((SampleKind::Burst, 0.0)));
        bad_kind[11] = 9;
        assert_eq!(decode_client(&bad_kind), Err(DecodeError::BadField));
    }

    #[test]
    fn hello_parses() {
        let h: ClientText = serde_json::from_str(r#"{"t":"hello","v":1,"device":"iPhone"}"#).unwrap();
        assert_eq!(h, ClientText::Hello { v: 1, device: "iPhone".into(), token: None });
    }
}
