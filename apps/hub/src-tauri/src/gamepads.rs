//! Controles físicos (USB e Bluetooth já pareados) por gilrs, numa thread própria. Cada controle vira um
//! aparelho do lobby; apertar um botão faz entrar quem está esperando.
//!
//! Cuidado com o eco: os controles virtuais do Modo Universal (ViGEm) também aparecem como controles de
//! Xbox 360. Quem aparece logo depois de o Hub ligar um virtual é marcado como virtual e ignorado.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gilrs::ff::{BaseEffect, BaseEffectType, EffectBuilder, Replay, Ticks};
use gilrs::{Axis, Button, EventType, GamepadId, Gilrs, PowerInfo};
use pa_core::{buttons, Device, DeviceKey, DeviceKind, JoinOutcome, Pad};

use crate::hub::{DeviceMeta, Hub};

/// Zona morta radial dos analógicos.
const DEADZONE: f32 = 0.15;
/// Um Xbox 360 que aparece até este tempo depois de ligar um virtual é o próprio virtual.
const ECHO_WINDOW: Duration = Duration::from_secs(3);
const XBOX360_VID_PID: (u16, u16) = (0x045e, 0x028e);

pub enum Command {
    /// O Hub vai ligar um controle virtual agora.
    ExpectVirtual,
    /// Vibrar o controle desta vaga (Identificar).
    Rumble(DeviceKey),
}

/// Decide se um controle recém-conectado é eco de um virtual nosso.
#[derive(Default)]
pub struct EchoFilter {
    expected: VecDeque<Instant>,
}

impl EchoFilter {
    pub fn expect(&mut self, now: Instant) {
        self.expected.push_back(now);
    }

    pub fn is_echo(&mut self, vid_pid: (Option<u16>, Option<u16>), now: Instant) -> bool {
        while self.expected.front().is_some_and(|t| now.duration_since(*t) > ECHO_WINDOW) {
            self.expected.pop_front();
        }
        let x360 = vid_pid == (Some(XBOX360_VID_PID.0), Some(XBOX360_VID_PID.1));
        x360 && self.expected.pop_front().is_some()
    }
}

pub fn start(hub: Arc<Hub>) {
    let (tx, rx) = std::sync::mpsc::channel();
    *hub.gamepad_tx.lock().unwrap() = Some(tx);
    std::thread::Builder::new()
        .name("pa-gamepads".into())
        .spawn(move || {
            let Ok(mut gilrs) = Gilrs::new() else { return };
            run(&hub, &mut gilrs, &rx);
        })
        .expect("thread dos controles");
}

fn run(hub: &Hub, gilrs: &mut Gilrs, rx: &Receiver<Command>) {
    let mut echo = EchoFilter::default();
    let mut ignored: HashSet<GamepadId> = HashSet::new();
    let mut keys: HashMap<GamepadId, DeviceKey> = HashMap::new();
    let mut effects = Vec::new();

    // controles já ligados quando o Hub abriu
    let initial: Vec<GamepadId> = gilrs.gamepads().map(|(id, _)| id).collect();
    for id in initial {
        connect(hub, gilrs, id, &mut keys);
    }

    loop {
        loop {
            match rx.try_recv() {
                Ok(Command::ExpectVirtual) => echo.expect(Instant::now()),
                Ok(Command::Rumble(key)) => {
                    if let Some((&id, _)) = keys.iter().find(|(_, k)| **k == key) {
                        if let Ok(effect) = rumble(gilrs, id) {
                            effects.push((Instant::now(), effect));
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        effects.retain(|(t, _)| t.elapsed() < Duration::from_secs(2));

        let Some(ev) = gilrs.next_event_blocking(Some(Duration::from_millis(50))) else { continue };
        if ignored.contains(&ev.id) {
            continue;
        }
        match ev.event {
            EventType::Connected => {
                let gp = gilrs.gamepad(ev.id);
                if echo.is_echo((gp.vendor_id(), gp.product_id()), Instant::now()) {
                    ignored.insert(ev.id);
                    continue;
                }
                connect(hub, gilrs, ev.id, &mut keys);
            }
            EventType::Disconnected => {
                if let Some(key) = keys.remove(&ev.id) {
                    let seat = hub.lobby.lock().unwrap().seat_of_device(&key);
                    hub.lobby.lock().unwrap().disconnected(&key, hub.now_ms());
                    if let Some(s) = seat {
                        hub.set_pad(s, Pad::NEUTRAL);
                    }
                    hub.emit_lobby();
                }
            }
            EventType::ButtonPressed(..) | EventType::ButtonReleased(..) | EventType::ButtonChanged(..) | EventType::AxisChanged(..) => {
                let Some(key) = keys.get(&ev.id) else { continue };
                let pad = read_pad(gilrs, ev.id);
                let seat = hub.lobby.lock().unwrap().seat_of_device(key);
                match seat {
                    Some(s) => hub.set_pad(s, pad),
                    None if matches!(ev.event, EventType::ButtonPressed(..)) => {
                        let joined = hub.lobby.lock().unwrap().pressed(key);
                        if matches!(joined, Some(JoinOutcome::Joined(_) | JoinOutcome::Rejoined(_))) {
                            hub.emit_lobby();
                        }
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }
}

fn connect(hub: &Hub, gilrs: &Gilrs, id: GamepadId, keys: &mut HashMap<GamepadId, DeviceKey>) {
    let gp = gilrs.gamepad(id);
    let name = gp.name().to_string();
    // uuid = modelo; o id do gilrs separa dois controles iguais
    let uuid: String = gp.uuid().iter().map(|b| format!("{b:02x}")).collect();
    let key = DeviceKey(format!("pad:{uuid}:{}", usize::from(id)));
    let (via, battery) = match gp.power_info() {
        PowerInfo::Wired => ("Com fio".to_string(), None),
        PowerInfo::Discharging(p) | PowerInfo::Charging(p) => ("Sem fio".to_string(), Some(p)),
        PowerInfo::Charged => ("Sem fio".to_string(), Some(100)),
        _ => ("Conectado".to_string(), None),
    };
    hub.meta.lock().unwrap().insert(
        key.clone(),
        DeviceMeta { brand: brand_of(&name).into(), device_name: display_name(&name), via, battery, ping_ms: None },
    );
    hub.lobby.lock().unwrap().connected(Device { key: key.clone(), kind: DeviceKind::Gamepad, name: None });
    keys.insert(id, key);
    hub.emit_lobby();
}

fn read_pad(gilrs: &Gilrs, id: GamepadId) -> Pad {
    let gp = gilrs.gamepad(id);
    let mut b = 0u32;
    for (btn, bit) in [
        (Button::South, buttons::A),
        (Button::East, buttons::B),
        (Button::West, buttons::X),
        (Button::North, buttons::Y),
        (Button::LeftTrigger, buttons::LB),
        (Button::RightTrigger, buttons::RB),
        (Button::Select, buttons::BACK),
        (Button::Start, buttons::START),
        (Button::LeftThumb, buttons::LS),
        (Button::RightThumb, buttons::RS),
        (Button::DPadUp, buttons::UP),
        (Button::DPadDown, buttons::DOWN),
        (Button::DPadLeft, buttons::LEFT),
        (Button::DPadRight, buttons::RIGHT),
        (Button::Mode, buttons::GUIDE),
    ] {
        if gp.is_pressed(btn) {
            b |= bit;
        }
    }
    let (lx, ly) = deadzone(gp.value(Axis::LeftStickX), gp.value(Axis::LeftStickY));
    let (rx, ry) = deadzone(gp.value(Axis::RightStickX), gp.value(Axis::RightStickY));
    let trig = |btn| gp.button_data(btn).map(|d| d.value()).unwrap_or(0.0).clamp(0.0, 1.0);
    Pad { buttons: b, lx, ly, rx, ry, lt: trig(Button::LeftTrigger2), rt: trig(Button::RightTrigger2) }
}

/// Zona morta radial, reescalada para o eixo ainda chegar a 1.
pub fn deadzone(x: f32, y: f32) -> (f32, f32) {
    let mag = (x * x + y * y).sqrt().min(1.0);
    if mag < DEADZONE || mag == 0.0 {
        return (0.0, 0.0);
    }
    let k = (mag - DEADZONE) / (1.0 - DEADZONE) / mag;
    ((x * k).clamp(-1.0, 1.0), (y * k).clamp(-1.0, 1.0))
}

fn rumble(gilrs: &mut Gilrs, id: GamepadId) -> Result<gilrs::ff::Effect, gilrs::ff::Error> {
    let effect = EffectBuilder::new()
        .add_effect(BaseEffect {
            kind: BaseEffectType::Strong { magnitude: 45_000 },
            scheduling: Replay { play_for: Ticks::from_ms(400), ..Default::default() },
            envelope: Default::default(),
        })
        .gamepads(&[id])
        .finish(gilrs)?;
    effect.play()?;
    Ok(effect)
}

/// Mesma marca que a UI usa para o desenho e as cores.
pub fn brand_of(name: &str) -> &'static str {
    let n = name.to_ascii_lowercase();
    if n.contains("dualsense") || n.contains("dualshock") || n.contains("playstation") || n.contains("wireless controller") {
        "playstation"
    } else if n.contains("xbox") || n.contains("xinput") {
        "xbox"
    } else if n.contains("pro controller") || n.contains("joy-con") || n.contains("nintendo") {
        "nintendo"
    } else {
        "generic"
    }
}

fn display_name(raw: &str) -> String {
    match brand_of(raw) {
        "playstation" if raw.to_ascii_lowercase().contains("dualsense") => "DualSense".into(),
        "playstation" => "Controle PlayStation".into(),
        "xbox" => "Controle Xbox".into(),
        "nintendo" => "Switch Pro".into(),
        _ => raw.chars().take(24).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_filter_only_catches_x360_right_after_a_plug() {
        let mut f = EchoFilter::default();
        let t0 = Instant::now();
        let x360 = (Some(0x045e), Some(0x028e));
        assert!(!f.is_echo(x360, t0), "sem virtual ligado, é um controle de verdade");
        f.expect(t0);
        assert!(!f.is_echo((Some(0x054c), Some(0x0ce6)), t0), "DualSense nunca é eco");
        assert!(f.is_echo(x360, t0 + Duration::from_millis(400)));
        assert!(!f.is_echo(x360, t0 + Duration::from_millis(500)), "um eco por virtual");
        f.expect(t0);
        assert!(!f.is_echo(x360, t0 + Duration::from_secs(4)), "fora da janela");
    }

    #[test]
    fn deadzone_is_radial_and_reaches_one() {
        assert_eq!(deadzone(0.1, 0.05), (0.0, 0.0));
        let (x, y) = deadzone(1.0, 0.0);
        assert!((x - 1.0).abs() < 1e-6 && y == 0.0);
        let (x, _) = deadzone(0.5, 0.0);
        assert!(x > 0.0 && x < 0.5);
    }

    #[test]
    fn brands() {
        assert_eq!(brand_of("DualSense Wireless Controller"), "playstation");
        assert_eq!(brand_of("Xbox Series X Controller"), "xbox");
        assert_eq!(brand_of("Pro Controller"), "nintendo");
        assert_eq!(brand_of("8BitDo SN30"), "generic");
    }
}
