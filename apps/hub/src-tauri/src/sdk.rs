//! O Hub servindo o SDK: mantém a memória compartilhada viva (heartbeat), aplica os pedidos do jogo
//! (vibrar, aviso no celular) e, enquanto um jogo do SDK está conectado, desliga os controles virtuais do
//! Modo Universal para não haver entrada dupla (docs/SDK.md, "Garantias do Hub").

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use pa_core::{DeviceKind, SeatState, MAX_SEATS, SEAT_COLORS};
use pa_phone::protocol::ServerText;
use pa_sdk::{Command, Device, HubShm};
use tauri::Emitter;

use crate::hub::Hub;

pub fn start(hub: Arc<Hub>) {
    #[cfg(windows)]
    match HubShm::create() {
        Ok(shm) => {
            shm.set_join_url(&hub.join_url.lock().unwrap());
            *hub.sdk.lock().unwrap() = Some(shm);
        }
        Err(e) => {
            eprintln!("party-anywhere: SDK indisponível: {e}");
            return;
        }
    }
    write_seats(&hub);
    std::thread::Builder::new()
        .name("pa-sdk".into())
        .spawn(move || loop {
            tick(&hub);
            std::thread::sleep(Duration::from_millis(100));
        })
        .expect("thread do SDK");
}

fn tick(hub: &Hub) {
    let (pid, commands) = {
        let guard = hub.sdk.lock().unwrap();
        let Some(shm) = guard.as_ref() else { return };
        shm.heartbeat();
        let mut pid = shm.game_pid();
        #[cfg(windows)]
        if pid != 0 && !pa_sdk::process_alive(pid) {
            shm.release_game(pid);
            pid = 0;
        }
        (pid, shm.drain_commands())
    };

    let connected = pid != 0;
    if hub.game_connected.swap(connected, Ordering::AcqRel) != connected {
        if connected {
            for i in 0..MAX_SEATS {
                hub.unplug(i);
            }
        } else {
            hub.replug_all();
            // o que o jogo pediu para os celulares acaba com ele
            *hub.sdk_phone.lock().unwrap() = Default::default();
            hub.broadcast_layouts();
        }
        let _ = hub.app.emit("sdk-game", connected);
    }

    for c in commands {
        match c {
            Command::Rumble { seat, ms, .. } => for_seats(hub, seat, |i, kind| match kind {
                DeviceKind::Phone => {
                    if let Some(p) = hub.phone.lock().unwrap().as_ref() {
                        p.rumble(i as u8, ms, 200);
                    }
                }
                DeviceKind::Gamepad => {
                    let key = hub.device_at(i);
                    if let (Some(key), Some(tx)) = (key, hub.gamepad_tx.lock().unwrap().as_ref()) {
                        let _ = tx.send(crate::gamepads::Command::Rumble(key));
                    }
                }
                DeviceKind::Keyboard => {}
            }),
            Command::Toast { seat, text } => for_seats(hub, seat, |i, kind| {
                if kind == DeviceKind::Phone {
                    if let Some(p) = hub.phone.lock().unwrap().as_ref() {
                        p.send_text(i as u8, &ServerText::Toast { text: &text });
                    }
                }
            }),
            // layout, modo e nomes nos botões: guardados por vaga (valem também para quem reconectar depois)
            // e aplicados ao controle do celular
            Command::Label { seat, button, text } => phone_request(hub, seat, |s, i| s.set_label(i, button, &text)),
            Command::Layout { seat, layout } => phone_request(hub, seat, |s, i| {
                s.set_layout(i, layout);
            }),
            Command::Mode { seat, mode } => phone_request(hub, seat, |s, i| s.set_mode(i, mode)),
            // lobby fechado e luz do controle: ainda não aplicados (docs/SDK.md)
            Command::Accepting(_) | Command::Light { .. } => {}
        }
    }
}

fn phone_request(hub: &Hub, seat: Option<usize>, mut f: impl FnMut(&mut crate::sdk_phone::SdkPhone, usize)) {
    let seats: Vec<usize> = match seat {
        Some(i) if i < MAX_SEATS => vec![i],
        Some(_) => return,
        None => (0..MAX_SEATS).collect(),
    };
    {
        let mut s = hub.sdk_phone.lock().unwrap();
        for &i in &seats {
            f(&mut s, i);
        }
    }
    let phones: Vec<usize> = {
        let lobby = hub.lobby.lock().unwrap();
        lobby.seats().filter(|s| s.kind == DeviceKind::Phone && seats.contains(&s.index)).map(|s| s.index).collect()
    };
    for i in phones {
        hub.send_layouts(i as u8);
    }
}

fn for_seats(hub: &Hub, seat: Option<usize>, mut f: impl FnMut(usize, DeviceKind)) {
    let seats: Vec<(usize, DeviceKind)> = {
        let lobby = hub.lobby.lock().unwrap();
        lobby.seats().filter(|s| s.state == SeatState::Active && seat.is_none_or(|x| x == s.index)).map(|s| (s.index, s.kind)).collect()
    };
    for (i, k) in seats {
        f(i, k);
    }
}

/// Copia o lobby para a memória do SDK (fora do caminho da entrada: só quando o lobby muda).
pub fn write_seats(hub: &Hub) {
    let guard = hub.sdk.lock().unwrap();
    let Some(shm) = guard.as_ref() else { return };
    let lobby = hub.lobby.lock().unwrap();
    for (i, color) in SEAT_COLORS.iter().enumerate() {
        let rgb = hex_rgb(color);
        match lobby.seat(i).filter(|_| !hub.is_benched(i)) {
            Some(s) => shm.write_seat(
                i,
                match s.state {
                    SeatState::Active => pa_sdk::SeatState::Active,
                    SeatState::Reconnecting { .. } => pa_sdk::SeatState::Reconnecting,
                },
                match s.kind {
                    DeviceKind::Gamepad => Device::Gamepad,
                    DeviceKind::Phone => Device::Phone,
                    DeviceKind::Keyboard => Device::Keyboard,
                },
                rgb,
                &s.name,
            ),
            None => shm.write_seat(i, pa_sdk::SeatState::Empty, Device::Gamepad, rgb, ""),
        }
    }
}

fn hex_rgb(hex: &str) -> [u8; 3] {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

#[cfg(test)]
mod tests {
    #[test]
    fn colors() {
        assert_eq!(super::hex_rgb("#3B82F6"), [0x3b, 0x82, 0xf6]);
    }
}
