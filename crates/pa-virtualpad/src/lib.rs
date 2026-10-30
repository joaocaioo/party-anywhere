//! Controles virtuais (docs/ARCHITECTURE.md, D8 e "Modo Universal").
//!
//! Todo acesso ao driver acontece numa thread própria, dona do cliente e dos controles: quem usa
//! só manda comandos por um [`VirtualPads`] (barato de clonar, pode ir para qualquer thread).
//! A conversão do [`Pad`] para o formato XInput é pura e testada ([`to_xinput`]).

use std::fmt;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use pa_core::{buttons, Pad, MAX_SEATS};

/// Estado no formato do XInput (`XINPUT_GAMEPAD`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct XInputState {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
}

/// Bits do `XINPUT_GAMEPAD.wButtons`.
pub mod xbuttons {
    pub const DPAD_UP: u16 = 0x0001;
    pub const DPAD_DOWN: u16 = 0x0002;
    pub const DPAD_LEFT: u16 = 0x0004;
    pub const DPAD_RIGHT: u16 = 0x0008;
    pub const START: u16 = 0x0010;
    pub const BACK: u16 = 0x0020;
    pub const LEFT_THUMB: u16 = 0x0040;
    pub const RIGHT_THUMB: u16 = 0x0080;
    pub const LEFT_SHOULDER: u16 = 0x0100;
    pub const RIGHT_SHOULDER: u16 = 0x0200;
    pub const GUIDE: u16 = 0x0400;
    pub const A: u16 = 0x1000;
    pub const B: u16 = 0x2000;
    pub const X: u16 = 0x4000;
    pub const Y: u16 = 0x8000;
}

const BUTTON_MAP: [(u32, u16); 15] = [
    (buttons::A, xbuttons::A),
    (buttons::B, xbuttons::B),
    (buttons::X, xbuttons::X),
    (buttons::Y, xbuttons::Y),
    (buttons::LB, xbuttons::LEFT_SHOULDER),
    (buttons::RB, xbuttons::RIGHT_SHOULDER),
    (buttons::BACK, xbuttons::BACK),
    (buttons::START, xbuttons::START),
    (buttons::LS, xbuttons::LEFT_THUMB),
    (buttons::RS, xbuttons::RIGHT_THUMB),
    (buttons::UP, xbuttons::DPAD_UP),
    (buttons::DOWN, xbuttons::DPAD_DOWN),
    (buttons::LEFT, xbuttons::DPAD_LEFT),
    (buttons::RIGHT, xbuttons::DPAD_RIGHT),
    (buttons::GUIDE, xbuttons::GUIDE),
];

/// [`Pad`] → XInput. No XInput o eixo Y também é positivo para cima.
pub fn to_xinput(pad: &Pad) -> XInputState {
    let axis = |v: f32| (v.clamp(-1.0, 1.0) * 32767.0).round() as i16;
    let trigger = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let buttons = BUTTON_MAP.iter().filter(|(ours, _)| pad.buttons & ours != 0).fold(0u16, |acc, (_, x)| acc | x);
    XInputState {
        buttons,
        left_trigger: trigger(pad.lt),
        right_trigger: trigger(pad.rt),
        thumb_lx: axis(pad.lx),
        thumb_ly: axis(pad.ly),
        thumb_rx: axis(pad.rx),
        thumb_ry: axis(pad.ry),
    }
}

#[derive(Debug)]
pub enum Error {
    /// O driver ViGEmBus não está instalado (ou não respondeu).
    DriverMissing(String),
    /// Outro erro do driver ao criar ou atualizar um controle.
    Driver(String),
    /// A plataforma não tem driver de controle virtual.
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::DriverMissing(e) => write!(f, "driver ViGEmBus não encontrado ({e})"),
            Error::Driver(e) => write!(f, "erro do driver de controle virtual: {e}"),
            Error::Unsupported => write!(f, "controle virtual não suportado nesta plataforma"),
        }
    }
}

impl std::error::Error for Error {}

enum Command {
    Plug(u8),
    Update(u8, Pad, Instant),
    Unplug(u8),
}

/// Resultado de cada atualização, para medir o custo do driver.
#[derive(Clone, Copy, Debug)]
pub struct UpdateReport {
    pub seat: u8,
    /// Do pedido (`update`) até o driver aceitar o estado.
    pub latency: Duration,
    pub ok: bool,
}

/// Alça para os controles virtuais, um por vaga. Clonável.
#[derive(Clone)]
pub struct VirtualPads {
    tx: mpsc::Sender<Command>,
}

impl VirtualPads {
    /// Conecta ao driver e sobe a thread dos controles. `reports` recebe uma linha por atualização.
    pub fn start(reports: Option<mpsc::Sender<UpdateReport>>) -> Result<VirtualPads, Error> {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        thread::Builder::new()
            .name("pa-virtualpad".into())
            .spawn(move || backend::run(rx, ready_tx, reports))
            .map_err(|e| Error::Driver(e.to_string()))?;
        ready_rx.recv().map_err(|_| Error::Driver("a thread do driver terminou".into()))??;
        Ok(VirtualPads { tx })
    }

    /// Cria o controle de Xbox 360 da vaga (o Windows passa a ver um controle novo).
    pub fn plug(&self, seat: u8) {
        let _ = self.tx.send(Command::Plug(seat));
    }

    pub fn update(&self, seat: u8, pad: Pad) {
        let _ = self.tx.send(Command::Update(seat, pad, Instant::now()));
    }

    pub fn unplug(&self, seat: u8) {
        let _ = self.tx.send(Command::Unplug(seat));
    }
}

#[cfg(windows)]
mod backend {
    use super::*;
    use std::sync::Arc;
    use vigem_client::{Client, TargetId, XButtons, XGamepad, Xbox360Wired};

    pub fn run(
        rx: mpsc::Receiver<Command>,
        ready: mpsc::Sender<Result<(), Error>>,
        reports: Option<mpsc::Sender<UpdateReport>>,
    ) {
        let client = match Client::connect() {
            Ok(c) => Arc::new(c),
            Err(e) => {
                let _ = ready.send(Err(Error::DriverMissing(format!("{e:?}"))));
                return;
            }
        };
        let _ = ready.send(Ok(()));

        let mut targets: Vec<Option<Xbox360Wired<Arc<Client>>>> = (0..MAX_SEATS).map(|_| None).collect();
        while let Ok(cmd) = rx.recv() {
            match cmd {
                Command::Plug(seat) => {
                    let Some(slot) = targets.get_mut(seat as usize) else { continue };
                    if slot.is_some() {
                        continue;
                    }
                    let mut t = Xbox360Wired::new(client.clone(), TargetId::XBOX360_WIRED);
                    match t.plugin().and_then(|_| t.wait_ready()) {
                        Ok(()) => *slot = Some(t),
                        Err(e) => eprintln!("pa-virtualpad: não deu para criar o controle da vaga {seat}: {e:?}"),
                    }
                }
                Command::Update(seat, pad, asked) => {
                    let Some(Some(t)) = targets.get_mut(seat as usize) else { continue };
                    let x = to_xinput(&pad);
                    let g = XGamepad {
                        buttons: XButtons { raw: x.buttons },
                        left_trigger: x.left_trigger,
                        right_trigger: x.right_trigger,
                        thumb_lx: x.thumb_lx,
                        thumb_ly: x.thumb_ly,
                        thumb_rx: x.thumb_rx,
                        thumb_ry: x.thumb_ry,
                    };
                    let ok = t.update(&g).is_ok();
                    if let Some(r) = &reports {
                        let _ = r.send(UpdateReport { seat, latency: asked.elapsed(), ok });
                    }
                }
                Command::Unplug(seat) => {
                    if let Some(slot) = targets.get_mut(seat as usize) {
                        if let Some(mut t) = slot.take() {
                            let _ = t.unplug();
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod backend {
    use super::*;

    pub fn run(_: mpsc::Receiver<Command>, ready: mpsc::Sender<Result<(), Error>>, _: Option<mpsc::Sender<UpdateReport>>) {
        let _ = ready.send(Err(Error::Unsupported));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_is_zero() {
        assert_eq!(to_xinput(&Pad::NEUTRAL), XInputState::default());
    }

    #[test]
    fn maps_buttons_and_axes() {
        let pad = Pad { buttons: buttons::A | buttons::LB | buttons::UP | buttons::START, lx: 1.0, ly: -1.0, rx: 0.5, ry: 0.0, lt: 1.0, rt: 0.5 };
        let x = to_xinput(&pad);
        assert_eq!(x.buttons, xbuttons::A | xbuttons::LEFT_SHOULDER | xbuttons::DPAD_UP | xbuttons::START);
        assert_eq!((x.thumb_lx, x.thumb_ly, x.thumb_rx), (32767, -32767, 16384));
        assert_eq!((x.left_trigger, x.right_trigger), (255, 128));
    }

    #[test]
    fn every_button_has_a_distinct_xinput_bit() {
        let all = to_xinput(&Pad { buttons: buttons::ALL, ..Pad::NEUTRAL }).buttons;
        assert_eq!(all.count_ones(), 15);
    }
}
