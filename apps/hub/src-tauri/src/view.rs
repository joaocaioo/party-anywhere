//! A tela do jogo no celular (docs/SDK.md, "Tela do jogo no celular"): quem joga pelo celular vê a
//! própria parte do jogo nele. O jogo do SDK entrega a imagem daquele jogador (`pa_submit_view`) numa memória
//! por vaga; aqui o Hub lê o quadro mais novo, comprime em JPEG e manda pela conexão do controle.
//!
//! Controle de fluxo: no máximo 2 quadros a caminho por celular (o celular confirma cada um ao mostrar). Rede
//! lenta = menos quadros, nunca atraso acumulando. Cada confirmação também mede o atraso do jogo até a tela.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pa_phone::protocol::{encode_view, ServerText};
use pa_sdk::view::{qpc_us, ViewShm};

use crate::hub::Hub;

const MAX_IN_FLIGHT: usize = 1;
const MAX_FPS: f64 = 30.0;
const JPEG_QUALITY: u8 = 70;
/// Sem confirmação por este tempo: o celular perdeu quadros (tela apagada, rede): libera de novo.
const ACK_TIMEOUT: Duration = Duration::from_secs(1);

struct Stream {
    shm: ViewShm,
    last_seq: u32,
    next_id: u16,
    /// (id, quando foi mandado em ms do relógio dos celulares, idade do quadro ao mandar)
    in_flight: VecDeque<(u16, f64, f64)>,
    last_send: Instant,
    shown: u32,
    latency_sum: f64,
    since: Instant,
}

#[derive(Default)]
pub struct Views {
    streams: HashMap<u8, Stream>,
}

impl Views {
    /// O celular ligou (ou desligou) a tela do jogo, ou a tela dele mudou de formato (`aspect`).
    pub fn set(&mut self, seat: u8, on: bool, aspect: f32) {
        if !on {
            self.streams.remove(&seat);
            return;
        }
        if let Some(s) = self.streams.get(&seat) {
            s.shm.set_aspect(aspect);
            return;
        }
        match ViewShm::create(seat as usize) {
            Ok(shm) => {
                shm.set_aspect(aspect);
                shm.set_wanted(true);
                let now = Instant::now();
                self.streams.insert(
                    seat,
                    Stream { shm, last_seq: 0, next_id: 0, in_flight: VecDeque::new(), last_send: now, shown: 0, latency_sum: 0.0, since: now },
                );
            }
            Err(e) => eprintln!("party-anywhere: tela do celular {seat}: {e}"),
        }
    }

    /// O celular mostrou o quadro `id` (`at_ms`: relógio dos celulares, o de `PhoneServer::now_ms`).
    pub fn ack(&mut self, seat: u8, id: u16, at_ms: f64) {
        let Some(s) = self.streams.get_mut(&seat) else { return };
        if let Some(i) = s.in_flight.iter().position(|f| f.0 == id) {
            let (_, sent, age) = s.in_flight.remove(i).unwrap();
            s.shown += 1;
            s.latency_sum += age + (at_ms - sent);
        }
    }
}

pub fn start(hub: Arc<Hub>) {
    std::thread::Builder::new()
        .name("pa-view".into())
        .spawn(move || {
            let mut pixels = Vec::new();
            let mut jpeg = Vec::new();
            loop {
                std::thread::sleep(Duration::from_millis(4));
                let mut views = hub.views.lock().unwrap();
                if views.streams.is_empty() {
                    drop(views);
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                let now = Instant::now();
                let phone_now = hub.phone.lock().unwrap().as_ref().map(|p| p.now_ms()).unwrap_or(0.0);
                for (&seat, s) in views.streams.iter_mut() {
                    // a cada segundo: quantos quadros chegaram e com quanto atraso (vai para o Diagnóstico)
                    if now.duration_since(s.since) >= Duration::from_secs(1) {
                        if s.shown > 0 {
                            let ms = (s.latency_sum / s.shown as f64).round() as u32;
                            if let Some(p) = hub.phone.lock().unwrap().as_ref() {
                                p.send_text(seat, &ServerText::Screen { fps: s.shown, ms });
                            }
                        }
                        s.shown = 0;
                        s.latency_sum = 0.0;
                        s.since = now;
                    }
                    s.in_flight.retain(|f| phone_now - f.1 < ACK_TIMEOUT.as_secs_f64() * 1000.0);
                    if s.in_flight.len() >= MAX_IN_FLIGHT || now.duration_since(s.last_send).as_secs_f64() < 1.0 / MAX_FPS {
                        continue;
                    }
                    let Some(frame) = s.shm.read(s.last_seq, &mut pixels) else { continue };
                    s.last_seq = frame.seq;
                    jpeg.clear();
                    let enc = jpeg_encoder::Encoder::new(&mut jpeg, JPEG_QUALITY);
                    if enc.encode(&pixels, frame.width as u16, frame.height as u16, jpeg_encoder::ColorType::Bgra).is_err() {
                        continue;
                    }
                    let age_ms = (qpc_us().saturating_sub(frame.timestamp_us) as f64 / 1000.0).min(60_000.0);
                    let id = s.next_id;
                    s.next_id = s.next_id.wrapping_add(1);
                    let msg = encode_view(id, age_ms as u16, frame.width as u16, frame.height as u16, &jpeg);
                    if let Some(p) = hub.phone.lock().unwrap().as_ref() {
                        p.send_view(seat, msg);
                    }
                    s.in_flight.push_back((id, phone_now, age_ms));
                    s.last_send = now;
                }
            }
        })
        .expect("thread da tela do celular");
}
