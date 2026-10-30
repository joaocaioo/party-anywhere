//! O núcleo do Hub: um lobby compartilhado por todos os aparelhos (celulares, controles, teclados), o
//! estado de cada vaga e os controles virtuais do Modo Universal. A UI recebe uma "foto" do lobby
//! (evento `lobby`) sempre que algo muda.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use pa_core::{DeviceKey, DeviceKind, JoinPolicy, Lobby, Pad, Seat, SeatState, MAX_SEATS, SEAT_COLORS};
use pa_phone::{net, PhoneEvent, PhoneServer, ServerConfig, CONTROLLER_PAGE};
use pa_virtualpad::VirtualPads;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const PHONE_PORT: u16 = 27027;

/// O que o Hub sabe de cada aparelho além da vaga.
#[derive(Clone, Debug, Default)]
pub struct DeviceMeta {
    pub brand: String,
    pub device_name: String,
    pub via: String,
    pub battery: Option<u8>,
    pub ping_ms: Option<u32>,
}

pub struct Hub {
    pub app: AppHandle,
    pub clock: Instant,
    pub lobby: Arc<Mutex<Lobby>>,
    pub meta: Mutex<HashMap<DeviceKey, DeviceMeta>>,
    /// Estado atual de cada vaga (o que o SDK e os controles virtuais leem).
    pub pads: Mutex<[Pad; MAX_SEATS]>,
    pub phone: Mutex<Option<PhoneServer>>,
    pub join_url: Mutex<String>,
    pub virtual_pads: Option<VirtualPads>,
    pub virtual_error: Option<String>,
    /// Vagas com controle virtual ligado (celulares e teclados).
    plugged: Mutex<[bool; MAX_SEATS]>,
    /// Vagas removidas recentemente, para o "Desfazer".
    removed: Mutex<Vec<Seat>>,
    /// Avisa a thread dos controles (vibrar, ligar o eco dos virtuais).
    pub gamepad_tx: Mutex<Option<std::sync::mpsc::Sender<crate::gamepads::Command>>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeatView {
    pub index: usize,
    pub name: String,
    pub kind: &'static str,
    pub brand: String,
    pub device_name: String,
    pub device_key: String,
    pub via: String,
    pub state: &'static str,
    pub color: &'static str,
    pub battery: Option<u8>,
    pub ping: Option<u32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyView {
    pub seats: Vec<SeatView>,
    pub join_url: String,
    pub waiting: usize,
    pub policy: &'static str,
    pub virtual_pads: bool,
    pub virtual_error: Option<String>,
}

impl Hub {
    pub fn new(app: AppHandle) -> Arc<Self> {
        let (virtual_pads, virtual_error) = match VirtualPads::start(None) {
            Ok(v) => (Some(v), None),
            Err(e) => (None, Some(e.to_string())),
        };
        Arc::new(Self {
            app,
            clock: Instant::now(),
            lobby: Arc::new(Mutex::new(Lobby::default())),
            meta: Mutex::new(HashMap::new()),
            pads: Mutex::new([Pad::NEUTRAL; MAX_SEATS]),
            phone: Mutex::new(None),
            join_url: Mutex::new(String::new()),
            virtual_pads,
            virtual_error,
            plugged: Mutex::new([false; MAX_SEATS]),
            removed: Mutex::new(Vec::new()),
            gamepad_tx: Mutex::new(None),
        })
    }

    pub fn now_ms(&self) -> u64 {
        self.clock.elapsed().as_millis() as u64
    }

    /// Sobe o servidor do celular e fica ouvindo os eventos dele.
    pub fn start_phones(self: &Arc<Self>) -> Result<(), String> {
        let config = ServerConfig { port: PHONE_PORT, loopback_only: false, page: CONTROLLER_PAGE, lobby: self.lobby.clone() };
        let (server, mut events) =
            tauri::async_runtime::block_on(PhoneServer::start(config, self.clock)).map_err(|e| e.to_string())?;
        let port = server.local_addr().port();
        let ip = net::lan_candidates().first().map(|c| c.ip.to_string()).unwrap_or_else(|| "127.0.0.1".into());
        *self.join_url.lock().unwrap() = format!("http://{ip}:{port}/");
        *self.phone.lock().unwrap() = Some(server);
        let hub = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut last_ping_emit = Instant::now();
            while let Some(ev) = events.recv().await {
                match ev {
                    PhoneEvent::Connected { seat, device, .. } => {
                        if let Some(key) = hub.device_at(seat as usize) {
                            hub.meta.lock().unwrap().insert(
                                key,
                                DeviceMeta { brand: "phone".into(), device_name: device, via: "Wi-Fi".into(), ..Default::default() },
                            );
                        }
                        hub.plug(seat as usize);
                        hub.emit_lobby();
                    }
                    PhoneEvent::Disconnected { seat } => {
                        hub.set_pad(seat as usize, Pad::NEUTRAL);
                        hub.unplug(seat as usize);
                        hub.emit_lobby();
                    }
                    PhoneEvent::State { seat, pad, .. } => hub.set_pad(seat as usize, pad),
                    PhoneEvent::ClockSample { seat, rtt_ms, .. } => {
                        if let Some(key) = hub.device_at(seat as usize) {
                            if let Some(m) = hub.meta.lock().unwrap().get_mut(&key) {
                                m.ping_ms = Some(rtt_ms.round() as u32);
                            }
                        }
                        if last_ping_emit.elapsed().as_secs() >= 2 {
                            last_ping_emit = Instant::now();
                            hub.emit_lobby();
                        }
                    }
                }
            }
        });
        Ok(())
    }

    pub fn device_at(&self, seat: usize) -> Option<DeviceKey> {
        self.lobby.lock().unwrap().seat(seat).map(|s| s.device.clone())
    }

    /// Caminho da entrada: só copia o estado e repassa ao controle virtual (sem alocar).
    pub fn set_pad(&self, seat: usize, pad: Pad) {
        if seat >= MAX_SEATS {
            return;
        }
        self.pads.lock().unwrap()[seat] = pad;
        if self.plugged.lock().unwrap()[seat] {
            if let Some(v) = &self.virtual_pads {
                v.update(seat as u8, pad);
            }
        }
    }

    /// Celulares e teclados viram controles de Xbox virtuais (os jogos já veem os controles físicos).
    pub fn plug(&self, seat: usize) {
        let (Some(v), true) = (&self.virtual_pads, seat < MAX_SEATS) else { return };
        let mut plugged = self.plugged.lock().unwrap();
        if !plugged[seat] {
            plugged[seat] = true;
            if let Some(tx) = self.gamepad_tx.lock().unwrap().as_ref() {
                let _ = tx.send(crate::gamepads::Command::ExpectVirtual);
            }
            v.plug(seat as u8);
        }
    }

    pub fn unplug(&self, seat: usize) {
        let (Some(v), true) = (&self.virtual_pads, seat < MAX_SEATS) else { return };
        let mut plugged = self.plugged.lock().unwrap();
        if plugged[seat] {
            plugged[seat] = false;
            v.update(seat as u8, Pad::NEUTRAL);
            v.unplug(seat as u8);
        }
    }

    pub fn snapshot(&self) -> LobbyView {
        let lobby = self.lobby.lock().unwrap();
        let meta = self.meta.lock().unwrap();
        let seats = lobby
            .seats()
            .map(|s| {
                let m = meta.get(&s.device).cloned().unwrap_or_default();
                SeatView {
                    index: s.index,
                    name: s.name.clone(),
                    kind: match s.kind {
                        DeviceKind::Phone => "phone",
                        DeviceKind::Gamepad => "gamepad",
                        DeviceKind::Keyboard => "keyboard",
                    },
                    brand: m.brand,
                    device_name: m.device_name,
                    device_key: s.device.0.clone(),
                    via: m.via,
                    state: match s.state {
                        SeatState::Active => "active",
                        SeatState::Reconnecting { .. } => "reconnecting",
                    },
                    color: SEAT_COLORS[s.index],
                    battery: m.battery,
                    ping: m.ping_ms.filter(|_| s.kind == DeviceKind::Phone),
                }
            })
            .collect();
        LobbyView {
            seats,
            join_url: self.join_url.lock().unwrap().clone(),
            waiting: lobby.waiting().len(),
            policy: match lobby.policy {
                JoinPolicy::Automatic => "automatic",
                JoinPolicy::OnButtonPress => "button",
            },
            virtual_pads: self.virtual_pads.is_some(),
            virtual_error: self.virtual_error.clone(),
        }
    }

    pub fn emit_lobby(&self) {
        let _ = self.app.emit("lobby", self.snapshot());
    }

    pub fn remove_seat(&self, index: usize) -> bool {
        let Some(seat) = self.lobby.lock().unwrap().remove(index) else { return false };
        self.set_pad(index, Pad::NEUTRAL);
        self.unplug(index);
        if seat.kind == DeviceKind::Phone {
            if let Some(p) = self.phone.lock().unwrap().as_ref() {
                p.release_seat(index as u8);
            }
        }
        let mut removed = self.removed.lock().unwrap();
        removed.retain(|s| s.index != index);
        removed.push(seat);
        drop(removed);
        self.emit_lobby();
        true
    }

    pub fn undo_remove(&self, index: usize) -> bool {
        let seat = {
            let mut removed = self.removed.lock().unwrap();
            let Some(pos) = removed.iter().position(|s| s.index == index) else { return false };
            removed.remove(pos)
        };
        let (kind, active) = (seat.kind, seat.state == SeatState::Active);
        let ok = self.lobby.lock().unwrap().undo_remove(seat);
        if ok && active && kind != DeviceKind::Gamepad {
            self.plug(index);
        }
        self.emit_lobby();
        ok
    }

    pub fn move_seat(&self, from: usize, to: usize) -> bool {
        let ok = self.lobby.lock().unwrap().move_seat(from, to);
        if ok {
            // os controles virtuais seguem o número da vaga
            for i in [from, to] {
                self.unplug(i);
                let needs = self
                    .lobby
                    .lock()
                    .unwrap()
                    .seat(i)
                    .is_some_and(|s| s.state == SeatState::Active && s.kind != DeviceKind::Gamepad);
                if needs {
                    self.plug(i);
                }
            }
            let mut pads = self.pads.lock().unwrap();
            pads.swap(from, to);
        }
        self.emit_lobby();
        ok
    }
}
