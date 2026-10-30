//! Servidor do celular: entrega a página do controle em `/` e fala o protocolo v1 em `/ws`.
//!
//! Quem usa o servidor recebe [`PhoneEvent`]s por um canal e responde pelos métodos de
//! [`PhoneServer`]. O servidor não sabe nada de controle virtual nem de jogo.

use std::collections::VecDeque;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::serve::ListenerExt;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use pa_core::{Pad, MAX_SEATS, SEAT_COLORS};
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
}

#[derive(Debug, Clone)]
pub enum PhoneEvent {
    Connected { seat: u8, device: String, peer: SocketAddr },
    Disconnected { seat: u8 },
    /// Estado novo do controle. `at_ms` é o instante da chegada no relógio do servidor
    /// ([`PhoneServer::now_ms`]), tomado antes de qualquer outro processamento.
    State { seat: u8, seq: u16, pad: Pad, at_ms: f64, timing: Option<(SampleKind, f64)> },
    /// Uma troca SYNC/SYNC_DONE completa: ida e volta e a diferença estimada entre os relógios
    /// (relógio do servidor ≈ relógio do celular + offset).
    ClockSample { seat: u8, rtt_ms: f64, offset_ms: f64 },
}

type Outbox = mpsc::UnboundedSender<Message>;

struct Shared {
    clock: Instant,
    seats: Mutex<[Option<Outbox>; MAX_SEATS]>,
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
        let shared = Arc::new(Shared { clock, seats: Mutex::new(Default::default()), events, page: config.page });

        let host = if config.loopback_only { Ipv4Addr::LOCALHOST } else { Ipv4Addr::UNSPECIFIED };
        let listener = bind_with_fallback(host, config.port).await?;
        let local_addr = listener.local_addr()?;
        // Nagle atrasaria PONG e SYNC_REPLY (e a medição de ida e volta junto)
        let listener = listener.tap_io(|tcp| {
            let _ = tcp.set_nodelay(true);
        });

        let app = Router::new()
            .route("/", get(page))
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

    pub fn rumble(&self, seat: u8, ms: u16, strength: u8) {
        self.send(seat, Message::Binary(protocol::encode_rumble(ms, strength).into()));
    }

    fn send(&self, seat: u8, msg: Message) {
        let seats = self.shared.seats.lock().unwrap();
        if let Some(Some(out)) = seats.get(seat as usize) {
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

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(shared): State<Arc<Shared>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    ws.max_message_size(protocol::MAX_TEXT).on_upgrade(move |socket| connection(socket, shared, peer))
}

async fn connection(socket: WebSocket, shared: Arc<Shared>, peer: SocketAddr) {
    let (mut sink, mut stream) = socket.split();

    let device = match timeout(HELLO_TIMEOUT, stream.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => match serde_json::from_str::<ClientText>(t.as_str()) {
            Ok(ClientText::Hello { v, device, .. }) if v == protocol::PROTOCOL_VERSION => device,
            Ok(ClientText::Hello { .. }) => {
                let _ = sink.send(reject("version")).await;
                return;
            }
            Err(_) => return,
        },
        _ => return,
    };

    let (out, mut outbox) = mpsc::unbounded_channel::<Message>();
    let seat = {
        let mut seats = shared.seats.lock().unwrap();
        let free = seats.iter().position(Option::is_none);
        if let Some(i) = free {
            seats[i] = Some(out.clone());
        }
        free
    };
    let Some(seat) = seat else {
        let _ = sink.send(reject("full")).await;
        return;
    };
    let seat = seat as u8;
    let name = format!("Jogador {}", seat + 1);
    let welcome = ServerText::Welcome { seat: seat + 1, color: SEAT_COLORS[seat as usize], name: &name };
    let _ = out.send(Message::Text(serde_json::to_string(&welcome).unwrap().into()));
    let _ = shared.events.send(PhoneEvent::Connected { seat, device, peer });

    let writer = tokio::spawn(async move {
        while let Some(m) = outbox.recv().await {
            if sink.send(m).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

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
                    let _ = shared.events.send(PhoneEvent::State { seat, seq, pad, at_ms, timing: None });
                }
                Ok(ClientMsg::StateTimed { seq, pad, kind, phone_ms }) => {
                    let timing = Some((kind, phone_ms));
                    let _ = shared.events.send(PhoneEvent::State { seat, seq, pad, at_ms, timing });
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
                // tipo novo de uma página mais nova: ignorar é compatível
                Err(DecodeError::UnknownType(_)) => {}
                Err(_) => break,
            },
            Message::Text(_) => {}
            Message::Close(_) => break,
            _ => {}
        }
    }

    shared.seats.lock().unwrap()[seat as usize] = None;
    drop(out);
    let _ = shared.events.send(PhoneEvent::Disconnected { seat });
    // o escritor termina sozinho quando a última cópia da caixa de saída some
    let _ = timeout(Duration::from_secs(1), writer).await;
}

fn reject(reason: &str) -> Message {
    Message::Text(serde_json::to_string(&ServerText::Reject { reason }).unwrap().into())
}
