//! O servidor de verdade, em 127.0.0.1, com celulares falsos por WebSocket: vagas vindas do lobby,
//! volta para a mesma vaga pelo token, remoção e volta apertando um botão.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use pa_core::{buttons, Lobby, Pad, SeatState};
use pa_phone::{protocol, PhoneEvent, PhoneServer, ServerConfig};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn start() -> (PhoneServer, UnboundedReceiver<PhoneEvent>, Arc<Mutex<Lobby>>) {
    let lobby = Arc::new(Mutex::new(Lobby::default()));
    let config = ServerConfig { port: 0, loopback_only: true, page: "<html></html>", lobby: lobby.clone() };
    let (server, events) = PhoneServer::start(config, Instant::now()).await.unwrap();
    (server, events, lobby)
}

async fn phone(server: &PhoneServer, token: Option<&str>) -> (Ws, Value) {
    let url = format!("ws://{}/ws", server.local_addr());
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    let hello = serde_json::json!({ "t": "hello", "v": 1, "device": "iPhone", "token": token });
    ws.send(Message::Text(hello.to_string().into())).await.unwrap();
    let welcome = next_text(&mut ws).await;
    (ws, welcome)
}

async fn next_text(ws: &mut Ws) -> Value {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(3), ws.next()).await.expect("sem resposta").unwrap().unwrap();
        if let Message::Text(t) = msg {
            return serde_json::from_str(t.as_str()).unwrap();
        }
    }
}

async fn next_event(rx: &mut UnboundedReceiver<PhoneEvent>) -> PhoneEvent {
    loop {
        let ev = tokio::time::timeout(Duration::from_secs(3), rx.recv()).await.expect("sem evento").unwrap();
        if !matches!(ev, PhoneEvent::ClockSample { .. }) {
            return ev;
        }
    }
}

#[tokio::test]
async fn phone_gets_a_token_and_comes_back_to_its_seat() {
    let (server, mut events, lobby) = start().await;

    let (ws, welcome) = phone(&server, None).await;
    assert_eq!((welcome["t"].as_str(), welcome["seat"].as_u64()), (Some("welcome"), Some(1)));
    let token = welcome["token"].as_str().unwrap().to_string();
    assert_eq!(token.len(), 32);
    assert!(matches!(next_event(&mut events).await, PhoneEvent::Connected { seat: 0, rejoined: false, .. }));

    drop(ws);
    assert!(matches!(next_event(&mut events).await, PhoneEvent::Disconnected { seat: 0 }));
    assert!(matches!(lobby.lock().unwrap().seat(0).unwrap().state, SeatState::Reconnecting { .. }));

    // outro celular não pega a vaga guardada
    let (_other, w2) = phone(&server, None).await;
    assert_eq!(w2["seat"], 2);
    next_event(&mut events).await;

    // reabriu a página: mesmo token, mesma vaga
    let (_ws, again) = phone(&server, Some(&token)).await;
    assert_eq!((again["seat"].as_u64(), again["token"].as_str()), (Some(1), Some(token.as_str())));
    assert!(matches!(next_event(&mut events).await, PhoneEvent::Connected { seat: 0, rejoined: true, .. }));
    assert_eq!(lobby.lock().unwrap().seat(0).unwrap().state, SeatState::Active);
}

#[tokio::test]
async fn removed_phone_returns_by_pressing_a_button() {
    let (server, mut events, lobby) = start().await;
    let (mut ws, _) = phone(&server, None).await;
    next_event(&mut events).await;

    lobby.lock().unwrap().remove(0);
    server.release_seat(0);
    let toast = next_text(&mut ws).await;
    assert_eq!(toast["t"], "toast");

    // sem vaga, estado neutro é ignorado; um botão faz voltar
    ws.send(Message::Binary(protocol::encode_state(1, &Pad::NEUTRAL, None).into())).await.unwrap();
    let press = Pad { buttons: buttons::A, ..Pad::NEUTRAL };
    ws.send(Message::Binary(protocol::encode_state(2, &press, None).into())).await.unwrap();
    let welcome = next_text(&mut ws).await;
    assert_eq!((welcome["t"].as_str(), welcome["seat"].as_u64()), (Some("welcome"), Some(1)));
    assert!(matches!(next_event(&mut events).await, PhoneEvent::Connected { seat: 0, .. }));

    // o toque que trouxe de volta não vira entrada; o próximo vira
    ws.send(Message::Binary(protocol::encode_state(3, &press, None).into())).await.unwrap();
    assert!(matches!(next_event(&mut events).await, PhoneEvent::State { seat: 0, seq: 3, .. }));
}

#[tokio::test]
async fn eleventh_phone_is_told_the_lobby_is_full() {
    let (server, _events, lobby) = start().await;
    {
        let mut l = lobby.lock().unwrap();
        for i in 0..10 {
            l.connected(pa_core::Device { key: pa_core::DeviceKey(format!("pad{i}")), kind: pa_core::DeviceKind::Gamepad, name: None });
        }
    }
    let (_ws, reply) = phone(&server, None).await;
    assert_eq!((reply["t"].as_str(), reply["reason"].as_str()), (Some("reject"), Some("full")));
    assert!(lobby.lock().unwrap().waiting().is_empty(), "quem foi recusado não fica na fila");
}
