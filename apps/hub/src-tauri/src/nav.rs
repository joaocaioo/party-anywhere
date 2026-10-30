//! Navegar o Hub com o controle ou o celular: qualquer jogador ativo (menos teclado: o teclado do PC é para
//! escrever). Lê o estado das vagas a 60 Hz, acha as bordas (apertou agora) e a direção do analógico/direcional
//! com repetição, e manda ações para a UI (evento `nav`). Só com a janela do Hub em foco e sem jogo do SDK
//! conectado: enquanto alguém joga, os botões são do jogo.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pa_core::{buttons, DeviceKind, Pad, SeatState, MAX_SEATS, SEAT_COLORS};
use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::hub::Hub;

/// Primeira repetição da direção segurada, e as seguintes.
const FIRST_REPEAT: Duration = Duration::from_millis(380);
const NEXT_REPEAT: Duration = Duration::from_millis(170);
const STICK: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Y,
    Lb,
    Rb,
    Start,
}

#[derive(Clone, Serialize)]
struct NavEvent {
    action: Action,
    seat: usize,
    color: &'static str,
}

/// Bordas e repetição de uma vaga. Puro, testado.
#[derive(Default, Clone, Copy)]
pub struct SeatNav {
    buttons: u32,
    dir: Option<Action>,
    next: Option<Instant>,
}

impl SeatNav {
    pub fn step(&mut self, pad: Pad, now: Instant, out: &mut Vec<Action>) {
        let pressed = pad.buttons & !self.buttons;
        self.buttons = pad.buttons;
        for (bit, action) in [
            (buttons::A, Action::A),
            (buttons::B, Action::B),
            (buttons::Y, Action::Y),
            (buttons::LB, Action::Lb),
            (buttons::RB, Action::Rb),
            (buttons::START, Action::Start),
        ] {
            if pressed & bit != 0 {
                out.push(action);
            }
        }
        let b = pad.buttons;
        let dir = if b & buttons::LEFT != 0 || pad.lx < -STICK {
            Some(Action::Left)
        } else if b & buttons::RIGHT != 0 || pad.lx > STICK {
            Some(Action::Right)
        } else if b & buttons::UP != 0 || pad.ly > STICK {
            Some(Action::Up)
        } else if b & buttons::DOWN != 0 || pad.ly < -STICK {
            Some(Action::Down)
        } else {
            None
        };
        match dir {
            Some(d) if Some(d) != self.dir => {
                out.push(d);
                self.next = Some(now + FIRST_REPEAT);
            }
            Some(d) if self.next.is_some_and(|t| now >= t) => {
                out.push(d);
                self.next = Some(now + NEXT_REPEAT);
            }
            None => self.next = None,
            _ => {}
        }
        self.dir = dir;
    }
}

pub fn start(hub: Arc<Hub>) {
    std::thread::Builder::new()
        .name("pa-nav".into())
        .spawn(move || {
            let mut state = [SeatNav::default(); MAX_SEATS];
            let mut actions = Vec::with_capacity(8);
            loop {
                std::thread::sleep(Duration::from_millis(16));
                let focused = hub.app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
                let pads = *hub.pads.lock().unwrap();
                let allowed: Vec<bool> = {
                    let lobby = hub.lobby.lock().unwrap();
                    (0..MAX_SEATS)
                        .map(|i| lobby.seat(i).is_some_and(|s| s.state == SeatState::Active && s.kind != DeviceKind::Keyboard))
                        .collect()
                };
                let now = Instant::now();
                for i in 0..MAX_SEATS {
                    actions.clear();
                    // fora de foco ou com jogo do SDK: só acompanha o estado, sem mandar nada
                    state[i].step(if allowed[i] { pads[i] } else { Pad::NEUTRAL }, now, &mut actions);
                    if !focused || hub.game_connected.load(Ordering::Acquire) {
                        continue;
                    }
                    for &action in &actions {
                        let _ = hub.app.emit("nav", NavEvent { action, seat: i, color: SEAT_COLORS[i] });
                    }
                }
            }
        })
        .expect("thread de navegação");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(b: u32, lx: f32, ly: f32) -> Pad {
        Pad { buttons: b, lx, ly, ..Pad::NEUTRAL }
    }

    #[test]
    fn buttons_fire_once_per_press() {
        let mut n = SeatNav::default();
        let t = Instant::now();
        let mut out = vec![];
        n.step(pad(buttons::A | buttons::RB, 0.0, 0.0), t, &mut out);
        assert_eq!(out, [Action::A, Action::Rb]);
        out.clear();
        n.step(pad(buttons::A | buttons::RB, 0.0, 0.0), t, &mut out);
        assert!(out.is_empty(), "segurar não repete botão");
    }

    #[test]
    fn held_direction_repeats_after_a_pause() {
        let mut n = SeatNav::default();
        let t = Instant::now();
        let mut out = vec![];
        n.step(pad(0, 0.9, 0.0), t, &mut out);
        assert_eq!(out, [Action::Right]);
        out.clear();
        n.step(pad(0, 0.9, 0.0), t + Duration::from_millis(200), &mut out);
        assert!(out.is_empty());
        n.step(pad(0, 0.9, 0.0), t + Duration::from_millis(400), &mut out);
        assert_eq!(out, [Action::Right]);
        out.clear();
        n.step(pad(buttons::UP, 0.0, 0.0), t + Duration::from_millis(410), &mut out);
        assert_eq!(out, [Action::Up], "trocar de direção é imediato");
        out.clear();
        n.step(pad(0, 0.0, -0.9), t + Duration::from_millis(420), &mut out);
        assert_eq!(out, [Action::Down], "analógico para baixo (eixo y positivo é cima)");
    }
}
