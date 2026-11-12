//! Controles físicos (USB e Bluetooth já pareados) por gilrs, numa thread própria. Cada controle vira um
//! aparelho do lobby; apertar um botão faz entrar quem está esperando.
//!
//! Cuidado com o eco: os controles virtuais do Modo Universal (ViGEm) também aparecem como controles de
//! Xbox 360. Quem aparece logo depois de o Hub ligar um virtual é marcado como virtual e ignorado.
//!
//! E com a Steam: com um jogo dela aberto, o Steam Input cria um "Steam Virtual Gamepad" (Valve, 28DE:11FF)
//! para cada controle que ele assume, inclusive os nossos virtuais. Para o Hub ele seria um jogador a mais (e o
//! mesmo controle entraria duas vezes): é ignorado, aqui e nas vagas do XInput.

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
/// O controle virtual do Steam Input.
const STEAM_VIRTUAL_VID_PID: (u16, u16) = (0x28de, 0x11ff);

/// Controle que a Steam cria por cima de outro (nunca uma pessoa a mais).
pub fn is_steam_virtual(vid_pid: (Option<u16>, Option<u16>), name: &str) -> bool {
    vid_pid == (Some(STEAM_VIRTUAL_VID_PID.0), Some(STEAM_VIRTUAL_VID_PID.1)) || name.to_ascii_lowercase().contains("steam virtual")
}

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
    // Controles Xbox: lidos direto pelo XInput (id do gilrs -> vaga do XInput, último pacote, estado). O
    // Windows.Gaming.Input do gilrs pode não entregar os botões a um app Win32 (só "conectou"); o XInput
    // entrega sempre. Os outros controles seguem pelos eventos do gilrs.
    let mut xin: HashMap<GamepadId, (u32, u32, Pad)> = HashMap::new();
    let mut remap = true;

    // controles já ligados quando o Hub abriu
    let initial: Vec<GamepadId> = gilrs.gamepads().map(|(id, _)| id).collect();
    for id in initial {
        let gp = gilrs.gamepad(id);
        if is_steam_virtual((gp.vendor_id(), gp.product_id()), gp.name()) {
            ignored.insert(id);
            continue;
        }
        connect(hub, gilrs, id, &mut keys);
    }

    loop {
        loop {
            match rx.try_recv() {
                Ok(Command::ExpectVirtual) => echo.expect(Instant::now()),
                Ok(Command::Rumble(key)) => {
                    if let Some((&id, _)) = keys.iter().find(|(_, k)| **k == key) {
                        // controles Xbox: XInput, que vibra sempre (a vibração do Windows.Gaming.Input pode
                        // ser descartada sem erro); os outros: pelo gilrs, quando o Windows oferece
                        let xinput_slot = xbox_rank(gilrs, &ignored, id).and_then(|rank| xinput::physical_slots().get(rank).copied());
                        match xinput_slot {
                            Some(slot) => xinput::rumble(slot, 400),
                            None => {
                                if let Ok(effect) = rumble(gilrs, id) {
                                    effects.push((Instant::now(), effect));
                                }
                            }
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        effects.retain(|(t, _)| t.elapsed() < Duration::from_secs(2));

        if remap {
            remap = false;
            let slots = xinput::physical_slots();
            let ids: Vec<GamepadId> = keys.keys().copied().collect();
            let old = std::mem::take(&mut xin);
            for id in ids {
                if let Some(slot) = xbox_rank(gilrs, &ignored, id).and_then(|r| slots.get(r).copied()) {
                    let kept = old.get(&id).filter(|(s, _, _)| *s == slot).map(|&(_, p, pad)| (p, pad)).unwrap_or((u32::MAX, Pad::NEUTRAL));
                    xin.insert(id, (slot, kept.0, kept.1));
                }
            }
            if crate::nav::debug() {
                eprintln!("[input] XInput: {:?}", xin.iter().map(|(id, (slot, _, _))| (*id, *slot)).collect::<Vec<_>>());
            }
        }
        for (id, (slot, packet, prev)) in xin.iter_mut() {
            let Some((p, pad)) = xinput::state(*slot) else { continue };
            if p == *packet {
                continue;
            }
            *packet = p;
            let newly = pad.buttons & !prev.buttons;
            *prev = pad;
            if crate::nav::debug() && newly != 0 {
                eprintln!("[input] XInput vaga {slot}: botões {:#06x}", pad.buttons);
            }
            if let Some(key) = keys.get(id) {
                apply(hub, key, pad, newly != 0);
            }
        }

        let wait = if xin.is_empty() { 50 } else { 4 };
        let Some(ev) = gilrs.next_event_blocking(Some(Duration::from_millis(wait))) else { continue };
        if crate::nav::debug() && !matches!(ev.event, EventType::AxisChanged(..)) {
            eprintln!("[input] controle {:?}: {:?} (ignorado: {}, vaga: {:?})", ev.id, ev.event, ignored.contains(&ev.id), keys.get(&ev.id).and_then(|k| hub.lobby.lock().unwrap().seat_of_device(k)));
        }
        if ignored.contains(&ev.id) {
            continue;
        }
        match ev.event {
            EventType::Connected => {
                let gp = gilrs.gamepad(ev.id);
                let vid_pid = (gp.vendor_id(), gp.product_id());
                if is_steam_virtual(vid_pid, gp.name()) || echo.is_echo(vid_pid, Instant::now()) {
                    ignored.insert(ev.id);
                    continue;
                }
                connect(hub, gilrs, ev.id, &mut keys);
                remap = true;
            }
            EventType::Disconnected => {
                remap = true;
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
                if xin.contains_key(&ev.id) {
                    continue; // lido pelo XInput
                }
                let Some(key) = keys.get(&ev.id) else { continue };
                apply(hub, key, read_pad(gilrs, ev.id), matches!(ev.event, EventType::ButtonPressed(..)));
            }
            _ => {}
        }
    }
}

/// Um controle mudou: vai para a vaga dele; sem vaga, apertar um botão faz entrar.
fn apply(hub: &Hub, key: &DeviceKey, pad: Pad, newly_pressed: bool) {
    if newly_pressed || crate::hub::pressed(&pad) {
        hub.activity(key);
    }
    let seat = hub.lobby.lock().unwrap().seat_of_device(key);
    match seat {
        Some(s) => hub.set_pad(s, pad),
        None if newly_pressed => {
            let joined = hub.lobby.lock().unwrap().pressed(key);
            if matches!(joined, Some(JoinOutcome::Joined(_) | JoinOutcome::Rejoined(_))) {
                hub.emit_lobby();
            }
        }
        None => {}
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

/// Posição do controle entre os controles Xbox de verdade (sem os virtuais), na ordem em que o gilrs os vê.
/// Casa com a ordem das vagas do XInput que não são virtuais.
fn xbox_rank(gilrs: &Gilrs, ignored: &HashSet<GamepadId>, id: GamepadId) -> Option<usize> {
    let mut xbox: Vec<GamepadId> = gilrs
        .gamepads()
        .filter(|(gid, gp)| !ignored.contains(gid) && (gp.vendor_id() == Some(0x045e) || brand_of(gp.name()) == "xbox"))
        .map(|(gid, _)| gid)
        .collect();
    xbox.sort_by_key(|g| usize::from(*g));
    xbox.iter().position(|g| *g == id)
}

/// Pode vibrar pelo Hub? Controles Xbox (XInput) e os que o Windows.Gaming.Input aceita. PlayStation e
/// outros sem driver de vibração no Windows, não: a carta pisca em vez disso.
pub fn can_rumble(brand: &str) -> bool {
    brand == "xbox"
}

/// XInput carregado na hora (xinput1_4.dll), com a função 108 (XInputGetCapabilitiesEx, a mesma que o SDL
/// usa) para saber o fabricante e o modelo de cada vaga e não confundir com os nossos controles virtuais.
#[cfg(windows)]
mod xinput {
    use std::sync::OnceLock;

    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

    #[repr(C)]
    #[derive(Default)]
    struct Gamepad {
        buttons: u16,
        lt: u8,
        rt: u8,
        lx: i16,
        ly: i16,
        rx: i16,
        ry: i16,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Caps {
        kind: u8,
        sub: u8,
        flags: u16,
        gamepad: Gamepad,
        vib_l: u16,
        vib_r: u16,
    }
    #[repr(C)]
    #[derive(Default)]
    struct CapsEx {
        caps: Caps,
        vid: u16,
        pid: u16,
        ver: u16,
        unk1: u16,
        unk2: u32,
    }
    #[repr(C)]
    struct Vibration {
        left: u16,
        right: u16,
    }

    #[repr(C)]
    #[derive(Default)]
    struct State {
        packet: u32,
        gamepad: Gamepad,
    }

    type FarProc = unsafe extern "system" fn() -> isize;
    type CapsExFn = unsafe extern "system" fn(u32, u32, u32, *mut CapsEx) -> u32;
    type SetStateFn = unsafe extern "system" fn(u32, *mut Vibration) -> u32;
    type GetStateFn = unsafe extern "system" fn(u32, *mut State) -> u32;

    fn api() -> Option<&'static (CapsExFn, SetStateFn, GetStateFn)> {
        static API: OnceLock<Option<(CapsExFn, SetStateFn, GetStateFn)>> = OnceLock::new();
        API.get_or_init(|| unsafe {
            let lib = LoadLibraryA(c"xinput1_4.dll".as_ptr().cast());
            if lib.is_null() {
                return None;
            }
            let caps = GetProcAddress(lib, 108usize as *const u8)?;
            let set = GetProcAddress(lib, c"XInputSetState".as_ptr().cast())?;
            let get = GetProcAddress(lib, c"XInputGetState".as_ptr().cast())?;
            Some((
                std::mem::transmute::<FarProc, CapsExFn>(caps),
                std::mem::transmute::<FarProc, SetStateFn>(set),
                std::mem::transmute::<FarProc, GetStateFn>(get),
            ))
        })
        .as_ref()
    }

    /// Estado da vaga do XInput: o número do pacote (muda a cada mudança) e o controle. Sem alocar.
    pub fn state(slot: u32) -> Option<(u32, super::Pad)> {
        use super::{buttons as b, deadzone};
        let &(_, _, get) = api()?;
        let mut s = State::default();
        if unsafe { get(slot, &mut s) } != 0 {
            return None;
        }
        let g = &s.gamepad;
        let mut bits = 0u32;
        for (x, bit) in [
            (0x0001, b::UP),
            (0x0002, b::DOWN),
            (0x0004, b::LEFT),
            (0x0008, b::RIGHT),
            (0x0010, b::START),
            (0x0020, b::BACK),
            (0x0040, b::LS),
            (0x0080, b::RS),
            (0x0100, b::LB),
            (0x0200, b::RB),
            (0x1000, b::A),
            (0x2000, b::B),
            (0x4000, b::X),
            (0x8000, b::Y),
        ] {
            if g.buttons & x != 0 {
                bits |= bit;
            }
        }
        let axis = |v: i16| (v as f32 / 32767.0).clamp(-1.0, 1.0);
        let (lx, ly) = deadzone(axis(g.lx), axis(g.ly));
        let (rx, ry) = deadzone(axis(g.rx), axis(g.ry));
        let trig = |v: u8| if v < 30 { 0.0 } else { v as f32 / 255.0 };
        Some((s.packet, super::Pad { buttons: bits, lx, ly, rx, ry, lt: trig(g.lt), rt: trig(g.rt) }))
    }

    /// Vagas do XInput com controle de verdade (os Xbox 360 virtuais do ViGEm têm o modelo 028E; os do
    /// Steam Input, 28DE:11FF).
    pub fn physical_slots() -> Vec<u32> {
        let Some((caps_ex, _, _)) = api() else { return vec![] };
        (0..4u32)
            .filter(|&slot| {
                let mut c = CapsEx::default();
                unsafe { caps_ex(1, slot, 0, &mut c) == 0 && !(c.vid == 0x045e && c.pid == 0x028e) && !(c.vid == 0x28de && c.pid == 0x11ff) }
            })
            .collect()
    }

    /// Vibra os dois motores por `ms` e para (numa thread curta: não segura a dos controles).
    pub fn rumble(slot: u32, ms: u64) {
        let Some(&(_, set, _)) = api() else { return };
        std::thread::spawn(move || unsafe {
            set(slot, &mut Vibration { left: 45_000, right: 30_000 });
            std::thread::sleep(std::time::Duration::from_millis(ms));
            set(slot, &mut Vibration { left: 0, right: 0 });
        });
    }
}

#[cfg(not(windows))]
mod xinput {
    pub fn physical_slots() -> Vec<u32> {
        vec![]
    }
    pub fn rumble(_: u32, _: u64) {}
    pub fn state(_: u32) -> Option<(u32, super::Pad)> {
        None
    }
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
    fn steam_input_virtual_pads_are_not_players() {
        assert!(is_steam_virtual((Some(0x28de), Some(0x11ff)), "Xbox 360 Controller"));
        assert!(is_steam_virtual((None, None), "Steam Virtual Gamepad"));
        assert!(!is_steam_virtual((Some(0x045e), Some(0x02ea)), "Xbox One Controller"));
        // o Steam Controller e o Steam Deck de verdade (outros modelos da Valve) continuam sendo jogadores
        assert!(!is_steam_virtual((Some(0x28de), Some(0x1102)), "Steam Controller"));
    }

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
