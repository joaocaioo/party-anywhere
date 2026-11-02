//! Servidor do celular: entrega a página do controle em `/` e fala o protocolo v1 em `/ws`.
//!
//! Quem usa o servidor recebe [`PhoneEvent`]s por um canal e responde pelos métodos de
//! [`PhoneServer`]. O servidor não sabe nada de controle virtual nem de jogo. As vagas vêm do
//! [`Lobby`] compartilhado com os outros aparelhos (controles, teclados): o celular é só mais um.
//!
//! Identidade: na primeira conexão o celular recebe um token no `welcome` e o guarda; nas próximas
//! ele manda o token no `hello` e volta para a mesma vaga, mesmo reabrindo a página.

use std::collections::VecDeque;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Query, State};
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::serve::ListenerExt;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use pa_core::{Device, DeviceKey, DeviceKind, JoinOutcome, Lobby, Pad, MAX_SEATS, SEAT_COLORS};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::protocol::{self, ClientMsg, ClientText, DecodeError, SampleKind, ServerText};

/// Sem nenhuma mensagem por este tempo, a conexão cai (a vaga fica para quem usa o servidor decidir).
const IDLE_TIMEOUT: Duration = Duration::from_secs(5);
/// O celular tem este tempo para mandar o `hello`.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// Acima disto por segundo, a conexão cai.
const MAX_MESSAGES_PER_SECOND: u32 = 1000;
/// Portas tentadas a partir da pedida, se ela estiver ocupada.
const PORT_ATTEMPTS: u16 = 20;

pub struct ServerConfig {
    pub port: u16,
    /// Escuta só em 127.0.0.1 (testes, sem alerta do firewall).
    pub loopback_only: bool,
    /// HTML da página do controle.
    pub page: &'static str,
    /// Lobby de onde saem as vagas, compartilhado com quem lê controles e teclados.
    pub lobby: Arc<Mutex<Lobby>>,
}

#[derive(Debug, Clone)]
pub enum PhoneEvent {
    /// `rejoined`: o mesmo celular voltou para a vaga que já era dele.
    Connected { seat: u8, device: String, peer: SocketAddr, rejoined: bool },
    Disconnected { seat: u8 },
    /// Estado novo do controle. `at_ms` é o instante da chegada no relógio do servidor
    /// ([`PhoneServer::now_ms`]), tomado antes de qualquer outro processamento.
    State { seat: u8, seq: u16, pad: Pad, at_ms: f64, timing: Option<(SampleKind, f64)> },
    /// Uma troca SYNC/SYNC_DONE completa: ida e volta e a diferença estimada entre os relógios
    /// (relógio do servidor ≈ relógio do celular + offset).
    ClockSample { seat: u8, rtt_ms: f64, offset_ms: f64 },
    /// O celular escolheu um layout no menu.
    LayoutChosen { seat: u8, id: String },
    /// O jogador trocou o nome pelo celular.
    Renamed { seat: u8, name: String },
    /// Ligou ou desligou a tela do jogo no celular.
    Screen { seat: u8, on: bool, aspect: f32 },
    /// O celular mostrou o quadro `id` da tela do jogo (`at_ms` no relógio do servidor).
    ViewAck { seat: u8, id: u16, at_ms: f64 },
    /// Câmera por deslocamento, em graus.
    Look { seat: u8, dx: f32, dy: f32 },
    /// "Sair da partida" no menu do celular.
    Leave { seat: u8 },
}

type Outbox = mpsc::UnboundedSender<Message>;

struct Shared {
    clock: Instant,
    /// Conexão dona de cada vaga: (id da conexão, caixa de saída).
    seats: Mutex<[Option<(u64, Outbox)>; MAX_SEATS]>,
    lobby: Arc<Mutex<Lobby>>,
    next_conn: AtomicU64,
    events: mpsc::UnboundedSender<PhoneEvent>,
    page: &'static str,
}

impl Shared {
    fn now_ms(&self) -> f64 {
        self.clock.elapsed().as_secs_f64() * 1000.0
    }
}

#[derive(Clone)]
pub struct PhoneServer {
    shared: Arc<Shared>,
    local_addr: SocketAddr,
}

impl PhoneServer {
    /// Sobe o servidor. `clock` é a origem de [`PhoneServer::now_ms`] e dos `at_ms` dos eventos.
    pub async fn start(
        config: ServerConfig,
        clock: Instant,
    ) -> io::Result<(PhoneServer, mpsc::UnboundedReceiver<PhoneEvent>)> {
        let (events, rx) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            clock,
            seats: Mutex::new(Default::default()),
            lobby: config.lobby,
            next_conn: AtomicU64::new(1),
            events,
            page: config.page,
        });

        let host = if config.loopback_only { Ipv4Addr::LOCALHOST } else { Ipv4Addr::UNSPECIFIED };
        let listener = bind_with_fallback(host, config.port).await?;
        let local_addr = listener.local_addr()?;
        // Nagle atrasaria PONG e SYNC_REPLY (e a medição de ida e volta junto)
        let listener = listener.tap_io(|tcp| {
            let _ = tcp.set_nodelay(true);
        });

        let app = Router::new()
            .route("/", get(page))
            .route("/manifest.webmanifest", get(web_manifest))
            .route("/icon-192.png", get(|| async { png(ICON_192) }))
            .route("/icon-512.png", get(|| async { png(ICON_512) }))
            .route("/ws", get(ws_upgrade))
            .with_state(shared.clone());
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await {
                eprintln!("pa-phone: servidor parou: {e}");
            }
        });

        Ok((PhoneServer { shared, local_addr }, rx))
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn now_ms(&self) -> f64 {
        self.shared.now_ms()
    }

    pub fn send_text(&self, seat: u8, msg: &ServerText) {
        if let Ok(json) = serde_json::to_string(msg) {
            self.send(seat, Message::Text(json.into()));
        }
    }

    /// "Identificar": a tela do celular pisca na cor do jogador e vibra (no Android; o iPhone não deixa uma
    /// página vibrar sem um toque na tela).
    pub fn identify(&self, seat: u8) {
        self.send_text(seat, &ServerText::Identify);
        self.rumble(seat, 300, 200);
    }

    /// Um quadro da tela do jogo (já codificado com [`protocol::encode_view`]).
    pub fn send_view(&self, seat: u8, frame: Vec<u8>) {
        self.send(seat, Message::Binary(frame.into()));
    }

    pub fn rumble(&self, seat: u8, ms: u16, strength: u8) {
        self.send(seat, Message::Binary(protocol::encode_rumble(ms, strength).into()));
    }

    /// O jogador saiu do lobby (removido no Hub). O celular continua conectado e volta ao apertar um botão.
    pub fn release_seat(&self, seat: u8) {
        let taken = self.shared.seats.lock().unwrap().get_mut(seat as usize).and_then(Option::take);
        if let Some((_, out)) = taken {
            let toast = ServerText::Toast { text: "Você saiu do lobby. Aperte qualquer botão para voltar." };
            let _ = out.send(Message::Text(serde_json::to_string(&toast).unwrap().into()));
        }
    }

    fn send(&self, seat: u8, msg: Message) {
        let seats = self.shared.seats.lock().unwrap();
        if let Some(Some((_, out))) = seats.get(seat as usize) {
            let _ = out.send(msg);
        }
    }
}

async fn bind_with_fallback(host: Ipv4Addr, port: u16) -> io::Result<TcpListener> {
    let mut last = None;
    for p in port..port.saturating_add(PORT_ATTEMPTS) {
        match TcpListener::bind((host, p)).await {
            Ok(l) => return Ok(l),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("nenhuma porta livre")))
}

async fn page(State(shared): State<Arc<Shared>>) -> impl IntoResponse {
    // sem cache: depois de uma atualização do Hub, o celular tem de pegar a página nova
    ([(header::CACHE_CONTROL, "no-store")], Html(shared.page))
}

// "Adicionar à Tela de Início": aberto pelo ícone, o controle roda sem a barra do navegador, deitado e com fundo
// preto (no iPhone é o único jeito de ter tela cheia; no Android também funciona).
const ICON_192: &[u8] = include_bytes!("../web/icon-192.png");
const ICON_512: &[u8] = include_bytes!("../web/icon-512.png");
// O ícone abre o controle com o token do celular (`/?t=...`): a mesma vaga, mesmo onde o app da Tela de Início
// não enxerga o que o navegador guardou (iPhone).
const WEB_MANIFEST: &str = r##"{
  "name": "Party Anywhere",
  "short_name": "Party Anywhere",
  "start_url": "/START",
  "id": "/",
  "display": "fullscreen",
  "orientation": "landscape",
  "background_color": "#000000",
  "theme_color": "#000000",
  "icons": [
    { "src": "/icon-192.png", "sizes": "192x192", "type": "image/png" },
    { "src": "/icon-512.png", "sizes": "512x512", "type": "image/png" }
  ]
}"##;

async fn web_manifest(Query(q): Query<ManifestQuery>) -> impl IntoResponse {
    let start = match q.t {
        Some(t) if valid_token(&t) => format!("/?t={t}"),
        _ => "/".to_owned(),
    };
    (
        [(header::CONTENT_TYPE, "application/manifest+json"), (header::CACHE_CONTROL, "no-store")],
        WEB_MANIFEST.replacen("/START", &start, 1),
    )
}

#[derive(serde::Deserialize)]
struct ManifestQuery {
    t: Option<String>,
}

fn valid_token(t: &str) -> bool {
    t.len() == 32 && t.bytes().all(|b| b.is_ascii_hexdigit())
}

fn png(bytes: &'static [u8]) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "max-age=86400")], bytes)
}

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(shared): State<Arc<Shared>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    ws.max_message_size(protocol::MAX_TEXT).on_upgrade(move |socket| connection(socket, shared, peer))
}

async fn connection(socket: WebSocket, shared: Arc<Shared>, peer: SocketAddr) {
    let (mut sink, mut stream) = socket.split();

    let (device, token) = match timeout(HELLO_TIMEOUT, stream.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => match serde_json::from_str::<ClientText>(t.as_str()) {
            Ok(ClientText::Hello { v, device, token }) if v == protocol::PROTOCOL_VERSION => (device, token),
            Ok(ClientText::Hello { .. }) => {
                let _ = sink.send(reject("version")).await;
                return;
            }
            // a primeira mensagem tem de ser o hello
            Ok(_) | Err(_) => return,
        },
        _ => return,
    };
    // token do celular, ou um novo de 128 bits; formato inválido é tratado como celular novo
    let token = token.filter(|t| valid_token(t)).unwrap_or_else(new_token);
    let key = DeviceKey(format!("phone:{token}"));
    let conn = shared.next_conn.fetch_add(1, Ordering::Relaxed);

    let outcome = shared.lobby.lock().unwrap().connected(Device { key: key.clone(), kind: DeviceKind::Phone, name: None });
    let (mut seat, rejoined) = match outcome {
        JoinOutcome::Joined(i) => (i as u8, false),
        JoinOutcome::Rejoined(i) => (i as u8, true),
        JoinOutcome::Full | JoinOutcome::Waiting => {
            shared.lobby.lock().unwrap().disconnected(&key, shared.now_ms() as u64);
            let _ = sink.send(reject("full")).await;
            return;
        }
    };

    let (out, mut outbox) = mpsc::unbounded_channel::<Message>();
    let writer = tokio::spawn(async move {
        while let Some(m) = outbox.recv().await {
            if sink.send(m).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    claim_seat(&shared, seat, conn, &out, &token);
    let _ = shared.events.send(PhoneEvent::Connected { seat, device: device.clone(), peer, rejoined });
    // sem vaga = removido do lobby; aperta um botão para voltar
    let mut has_seat = true;

    // SYNC aguardando o SYNC_DONE: (id, t0 no celular, chegada no servidor)
    let mut pending_sync: VecDeque<(u16, f64, f64)> = VecDeque::with_capacity(8);
    let mut window_start = Instant::now();
    let mut window_count = 0u32;

    loop {
        let msg = match timeout(IDLE_TIMEOUT, stream.next()).await {
            Ok(Some(Ok(m))) => m,
            _ => break,
        };
        let at_ms = shared.now_ms();

        if window_start.elapsed() >= Duration::from_secs(1) {
            window_start = Instant::now();
            window_count = 0;
        }
        window_count += 1;
        if window_count > MAX_MESSAGES_PER_SECOND {
            break;
        }

        match msg {
            Message::Binary(bytes) => match protocol::decode_client(&bytes) {
                Ok(ClientMsg::State { seq, pad }) => {
                    if owns(&shared, &mut has_seat, &mut seat, conn, &key, &out, &token, &device, peer, pad) {
                        let _ = shared.events.send(PhoneEvent::State { seat, seq, pad, at_ms, timing: None });
                    }
                }
                Ok(ClientMsg::StateTimed { seq, pad, kind, phone_ms }) => {
                    if owns(&shared, &mut has_seat, &mut seat, conn, &key, &out, &token, &device, peer, pad) {
                        let timing = Some((kind, phone_ms));
                        let _ = shared.events.send(PhoneEvent::State { seat, seq, pad, at_ms, timing });
                    }
                }
                Ok(ClientMsg::Ping { t }) => {
                    let _ = out.send(Message::Binary(protocol::encode_pong(t).into()));
                }
                Ok(ClientMsg::Sync { id, t0 }) => {
                    let _ = out.send(Message::Binary(protocol::encode_sync_reply(id).into()));
                    if pending_sync.len() == pending_sync.capacity() {
                        pending_sync.pop_front();
                    }
                    pending_sync.push_back((id, t0, at_ms));
                }
                Ok(ClientMsg::SyncDone { id, t1 }) => {
                    if let Some(i) = pending_sync.iter().position(|p| p.0 == id) {
                        let (_, t0, server_ms) = pending_sync.remove(i).unwrap();
                        let rtt_ms = t1 - t0;
                        if (0.0..5000.0).contains(&rtt_ms) {
                            let offset_ms = server_ms - (t0 + t1) / 2.0;
                            let _ = shared.events.send(PhoneEvent::ClockSample { seat, rtt_ms, offset_ms });
                        }
                    }
                }
                Ok(ClientMsg::ViewAck { id }) => {
                    if has_seat {
                        let _ = shared.events.send(PhoneEvent::ViewAck { seat, id, at_ms });
                    }
                }
                Ok(ClientMsg::Look { dx, dy }) => {
                    if has_seat {
                        let _ = shared.events.send(PhoneEvent::Look { seat, dx, dy });
                    }
                }
                // tipo novo de uma página mais nova: ignorar é compatível
                Err(DecodeError::UnknownType(_)) => {}
                Err(_) => break,
            },
            Message::Text(t) if has_seat => match serde_json::from_str::<ClientText>(t.as_str()) {
                Ok(ClientText::Layout { id }) if id.len() <= 64 => {
                    let _ = shared.events.send(PhoneEvent::LayoutChosen { seat, id });
                }
                Ok(ClientText::Rename { name }) => {
                    let _ = shared.events.send(PhoneEvent::Renamed { seat, name: name.chars().take(40).collect() });
                }
                Ok(ClientText::Screen { on, aspect }) => {
                    let _ = shared.events.send(PhoneEvent::Screen { seat, on, aspect });
                }
                Ok(ClientText::Leave) => {
                    let _ = shared.events.send(PhoneEvent::Leave { seat });
                }
                _ => {}
            },
            Message::Text(_) => {}
            Message::Close(_) => break,
            _ => {}
        }
    }

    // a vaga só cai se ainda é desta conexão (se o mesmo celular já reconectou, a vaga é da conexão nova)
    let still_mine = {
        let mut seats = shared.seats.lock().unwrap();
        let mine = seats[seat as usize].as_ref().is_some_and(|(id, _)| *id == conn);
        if mine {
            seats[seat as usize] = None;
        }
        mine
    };
    {
        let mut lobby = shared.lobby.lock().unwrap();
        if still_mine || lobby.seat_of_device(&key).is_none() {
            lobby.disconnected(&key, shared.now_ms() as u64);
        }
    }
    drop(out);
    if still_mine {
        let _ = shared.events.send(PhoneEvent::Disconnected { seat });
    }
    // o escritor termina sozinho quando a última cópia da caixa de saída some
    let _ = timeout(Duration::from_secs(1), writer).await;
}

/// Esta conexão passa a ser a dona da vaga (a antiga, se houver, perde a caixa de saída e fecha).
fn claim_seat(shared: &Shared, seat: u8, conn: u64, out: &Outbox, token: &str) {
    shared.seats.lock().unwrap()[seat as usize] = Some((conn, out.clone()));
    let name = shared
        .lobby
        .lock()
        .unwrap()
        .seat(seat as usize)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("Jogador {}", seat + 1));
    let welcome = ServerText::Welcome { seat: seat + 1, color: SEAT_COLORS[seat as usize], name: &name, token };
    let _ = out.send(Message::Text(serde_json::to_string(&welcome).unwrap().into()));
}

/// A conexão ainda tem vaga? Se foi removida do lobby, um botão apertado a traz de volta.
#[allow(clippy::too_many_arguments)]
fn owns(
    shared: &Shared,
    has_seat: &mut bool,
    seat: &mut u8,
    conn: u64,
    key: &DeviceKey,
    out: &Outbox,
    token: &str,
    device: &str,
    peer: SocketAddr,
    pad: Pad,
) -> bool {
    if *has_seat {
        let mine = shared.seats.lock().unwrap()[*seat as usize].as_ref().is_some_and(|(id, _)| *id == conn);
        if mine {
            return true;
        }
        *has_seat = false;
    }
    if pad.buttons == 0 {
        return false;
    }
    match shared.lobby.lock().unwrap().pressed(key) {
        Some(JoinOutcome::Joined(i) | JoinOutcome::Rejoined(i)) => *seat = i as u8,
        _ => return false,
    }
    claim_seat(shared, *seat, conn, out, token);
    *has_seat = true;
    let _ = shared.events.send(PhoneEvent::Connected { seat: *seat, device: device.to_string(), peer, rejoined: false });
    false // o toque que fez entrar não vira entrada no jogo
}

fn new_token() -> String {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).expect("o sistema sempre tem fonte de aleatoriedade");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn reject(reason: &str) -> Message {
    Message::Text(serde_json::to_string(&ServerText::Reject { reason }).unwrap().into())
}
