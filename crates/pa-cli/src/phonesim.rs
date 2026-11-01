//! `pa phone-sim`: celulares de mentira, para testar um jogo ou mod com 1 a 10 jogadores sem chamar ninguém.
//! Cada um entra no Hub pelo mesmo caminho de um celular de verdade (WebSocket do controle, protocolo v1):
//! ganha uma vaga, manda o controle de um roteiro, pode ligar a tela do jogo (e confirma cada quadro, como a
//! página faz) e guarda o último quadro em disco. Ao terminar (tempo, Ctrl+C), cada um sai da partida: nenhuma vaga
//! fica presa em "Reconectando".

use std::io::ErrorKind;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

const SEND_EVERY: Duration = Duration::from_millis(50);
const SAVE_EVERY: Duration = Duration::from_secs(1);

/// Um passo do roteiro do controle (`--pad <arquivo.json>`: uma lista deles, repetida sem parar).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Step {
    /// Quanto tempo o passo dura.
    pub ms: u64,
    /// Botões apertados: A B X Y LB RB BACK START LS RS UP DOWN LEFT RIGHT.
    pub buttons: Vec<String>,
    pub lx: f32,
    pub ly: f32,
    pub rx: f32,
    pub ry: f32,
    pub lt: f32,
    pub rt: f32,
    /// Câmera por deslocamento, graus por segundo enquanto o passo dura: `[dx, dy]`.
    pub look: Option<[f32; 2]>,
}

const BUTTONS: [&str; 14] = ["A", "B", "X", "Y", "LB", "RB", "BACK", "START", "LS", "RS", "UP", "DOWN", "LEFT", "RIGHT"];

pub fn parse_script(json: &str) -> Result<Vec<Step>, String> {
    let steps: Vec<Step> = serde_json::from_str(json).map_err(|e| format!("roteiro do controle inválido: {e}"))?;
    if steps.is_empty() {
        return Err("o roteiro do controle está vazio".into());
    }
    for (i, s) in steps.iter().enumerate() {
        if s.ms == 0 {
            return Err(format!("passo {}: diga quanto tempo ele dura em \"ms\"", i + 1));
        }
        if let Some(b) = s.buttons.iter().find(|b| button_bit(b).is_none()) {
            return Err(format!("passo {}: botão desconhecido \"{b}\" (use {})", i + 1, BUTTONS.join(" ")));
        }
    }
    Ok(steps)
}

fn button_bit(name: &str) -> Option<u16> {
    BUTTONS.iter().position(|b| b.eq_ignore_ascii_case(name)).map(|i| 1 << i)
}

/// STATE: `[02][seq u16][botões u16][lx i8][ly i8][rx i8][ry i8][lt u8][rt u8]`.
pub fn encode_state(seq: u16, step: &Step) -> Vec<u8> {
    let buttons = step.buttons.iter().filter_map(|b| button_bit(b)).fold(0u16, |a, b| a | b);
    let axis = |v: f32| (v.clamp(-1.0, 1.0) * 127.0).round() as i8 as u8;
    let trig = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let mut out = vec![0x02];
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&buttons.to_le_bytes());
    out.extend_from_slice(&[axis(step.lx), axis(step.ly), axis(step.rx), axis(step.ry), trig(step.lt), trig(step.rt)]);
    out
}

/// LOOK: `[09][dx i16][dy i16]`, centésimos de grau.
pub fn encode_look(dx_deg: f32, dy_deg: f32) -> Vec<u8> {
    let c = |v: f32| ((v * 100.0).round().clamp(-32767.0, 32767.0) as i16).to_le_bytes();
    let mut out = vec![0x09];
    out.extend_from_slice(&c(dx_deg));
    out.extend_from_slice(&c(dy_deg));
    out
}

pub struct Options {
    pub players: usize,
    /// Endereço do controle (o do QR). Sem ele, o do Hub aberto neste PC.
    pub url: Option<String>,
    pub screen: bool,
    /// Formato da tela do celular de mentira (largura / altura). 0 = 19,5:9, um celular comum deitado.
    pub aspect: f32,
    pub script: Option<Vec<Step>>,
    pub save_view: Option<PathBuf>,
    pub seconds: Option<u64>,
}

#[derive(Default)]
struct Status {
    seat: Option<u8>,
    frames: u32,
    size: (u16, u16),
    note: String,
}

/// Roda até `seconds` (ou para sempre), mostrando a cada 2 s o que cada celular vê.
pub fn run(opts: Options) -> Result<(), String> {
    if !(1..=10).contains(&opts.players) {
        return Err("de 1 a 10 celulares: --players N".into());
    }
    let url = match &opts.url {
        Some(u) => u.clone(),
        None => hub_url().ok_or("o Party Anywhere não está aberto neste PC (ou passe --url com o endereço do QR)")?,
    };
    let ws_url = format!("ws://{}/ws", url.trim_start_matches("http://").trim_end_matches('/'));
    if let Some(dir) = &opts.save_view {
        std::fs::create_dir_all(dir).map_err(|e| format!("não consegui criar {}: {e}", dir.display()))?;
    }
    println!("{} celular(es) de mentira em {ws_url}. Ctrl+C para sair.", opts.players);

    let stop = Arc::new(AtomicBool::new(false));
    on_ctrl_c();
    let opts = Arc::new(opts);
    let statuses: Vec<Arc<Mutex<Status>>> = (0..opts.players).map(|_| Arc::default()).collect();
    let mut threads = Vec::new();
    for (k, status) in statuses.iter().enumerate() {
        let (opts, stop, status, ws_url) = (opts.clone(), stop.clone(), status.clone(), ws_url.clone());
        threads.push(std::thread::spawn(move || {
            if let Err(e) = phone(k, &ws_url, &opts, &stop, &status) {
                status.lock().unwrap().note = e;
            }
        }));
        std::thread::sleep(Duration::from_millis(150)); // entram em ordem: P1, P2...
    }

    let started = Instant::now();
    let mut last = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(250));
        let done = opts.seconds.is_some_and(|s| started.elapsed() >= Duration::from_secs(s));
        if last.elapsed() >= Duration::from_secs(2) || done {
            let secs = last.elapsed().as_secs_f32();
            last = Instant::now();
            let line: Vec<String> = statuses
                .iter()
                .enumerate()
                .map(|(k, s)| {
                    let mut s = s.lock().unwrap();
                    let who = s.seat.map(|n| format!("P{n}")).unwrap_or_else(|| format!("#{}", k + 1));
                    let what = if !s.note.is_empty() {
                        s.note.clone()
                    } else if opts.screen {
                        format!("tela {:.0} q/s {}x{}", s.frames as f32 / secs, s.size.0, s.size.1)
                    } else {
                        "no lobby".into()
                    };
                    s.frames = 0;
                    format!("{who}: {what}")
                })
                .collect();
            println!("{}", line.join("  ·  "));
        }
        if done || CTRL_C.load(Ordering::Relaxed) || threads.iter().all(|t| t.is_finished()) {
            break;
        }
    }
    stop.store(true, Ordering::Relaxed);
    for t in threads {
        let _ = t.join();
    }
    Ok(())
}

static CTRL_C: AtomicBool = AtomicBool::new(false);

/// Ctrl+C só pede para parar: os celulares saem da partida antes de o programa fechar.
#[cfg(windows)]
fn on_ctrl_c() {
    unsafe extern "system" fn handler(_: u32) -> windows_sys::Win32::Foundation::BOOL {
        CTRL_C.store(true, Ordering::Relaxed);
        1
    }
    unsafe { windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(handler), 1) };
}

#[cfg(not(windows))]
fn on_ctrl_c() {}

#[cfg(windows)]
fn hub_url() -> Option<String> {
    pa_sdk::hub_join_url()
}

#[cfg(not(windows))]
fn hub_url() -> Option<String> {
    None
}

fn phone(k: usize, ws_url: &str, opts: &Options, stop: &AtomicBool, status: &Mutex<Status>) -> Result<(), String> {
    let (mut ws, _) = tungstenite::connect(ws_url).map_err(|e| format!("não conectou: {e}"))?;
    if let MaybeTlsStream::Plain(s) = ws.get_ref() {
        let _ = s.set_nodelay(true);
        let _ = s.set_read_timeout(Some(Duration::from_millis(5)));
    }
    let hello = serde_json::json!({ "t": "hello", "v": 1, "device": format!("Simulado {}", k + 1), "token": null });
    send(&mut ws, Message::text(hello.to_string()))?;

    let idle = vec![Step { ms: 1000, ..Default::default() }];
    let script = opts.script.as_ref().unwrap_or(&idle);
    let aspect = if opts.aspect > 0.0 { opts.aspect } else { 19.5 / 9.0 };
    let (mut seq, mut step_i, mut step_at, mut last_send) = (0u16, 0usize, Instant::now(), Instant::now() - SEND_EVERY);
    let mut last_save = Instant::now() - SAVE_EVERY;
    let mut screen_asked = false;

    while !stop.load(Ordering::Relaxed) && !CTRL_C.load(Ordering::Relaxed) {
        // tudo o que chegou
        loop {
            match ws.read() {
                Ok(Message::Text(t)) => {
                    let v: serde_json::Value = serde_json::from_str(t.as_str()).unwrap_or_default();
                    match v["t"].as_str() {
                        Some("welcome") => status.lock().unwrap().seat = v["seat"].as_u64().map(|n| n as u8),
                        Some("reject") => return Err(format!("recusado: {}", v["reason"].as_str().unwrap_or("?"))),
                        _ => {}
                    }
                }
                Ok(Message::Binary(b)) if b.first() == Some(&0x86) && b.len() > 9 => {
                    // VIEW: confirma (sem isso o Hub manda 1 quadro por segundo) e guarda o mais novo
                    let id = [b[1], b[2]];
                    let size = (u16::from_le_bytes([b[5], b[6]]), u16::from_le_bytes([b[7], b[8]]));
                    send(&mut ws, Message::binary(vec![0x08, id[0], id[1]]))?;
                    let seat = {
                        let mut s = status.lock().unwrap();
                        s.frames += 1;
                        s.size = size;
                        s.seat
                    };
                    if let (Some(dir), Some(seat)) = (&opts.save_view, seat) {
                        if last_save.elapsed() >= SAVE_EVERY {
                            last_save = Instant::now();
                            save(dir, seat, &b[9..]);
                        }
                    }
                }
                Ok(Message::Close(_)) => return Err("o Hub fechou a conexão".into()),
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => break,
                Err(e) => return Err(format!("conexão caiu: {e}")),
            }
        }
        if opts.screen && !screen_asked && status.lock().unwrap().seat.is_some() {
            screen_asked = true;
            send(&mut ws, Message::text(serde_json::json!({ "t": "screen", "on": true, "aspect": aspect }).to_string()))?;
        }
        // o controle do roteiro, a cada 50 ms (também mantém a conexão viva)
        if last_send.elapsed() >= SEND_EVERY {
            let dt = last_send.elapsed().as_secs_f32().min(0.2);
            last_send = Instant::now();
            if step_at.elapsed() >= Duration::from_millis(script[step_i].ms) {
                step_i = (step_i + 1) % script.len();
                step_at = Instant::now();
            }
            let step = &script[step_i];
            send(&mut ws, Message::binary(encode_state(seq, step)))?;
            seq = seq.wrapping_add(1);
            if let Some([dx, dy]) = step.look {
                send(&mut ws, Message::binary(encode_look(dx * dt, dy * dt)))?;
            }
        }
    }
    // sai da partida (a vaga fica livre) em vez de deixar "Reconectando"
    let _ = ws.send(Message::text(r#"{"t":"leave"}"#));
    let _ = ws.flush();
    std::thread::sleep(Duration::from_millis(100));
    let _ = ws.close(None);
    Ok(())
}

fn send(ws: &mut WebSocket<MaybeTlsStream<TcpStream>>, m: Message) -> Result<(), String> {
    ws.send(m).map_err(|e| format!("conexão caiu: {e}"))
}

fn save(dir: &Path, seat: u8, jpeg: &[u8]) {
    let path = dir.join(format!("P{seat}.jpg"));
    let tmp = dir.join(format!("P{seat}.jpg.tmp"));
    if std::fs::write(&tmp, jpeg).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_steps_and_messages() {
        let s = parse_script(r#"[{ "ms": 500, "buttons": ["A", "b"], "lx": 1, "ly": -0.5, "rt": 1 }, { "ms": 200, "look": [90, 0] }]"#).unwrap();
        assert_eq!(s.len(), 2);
        let state = encode_state(7, &s[0]);
        assert_eq!(state[0], 0x02);
        assert_eq!(u16::from_le_bytes([state[1], state[2]]), 7);
        assert_eq!(u16::from_le_bytes([state[3], state[4]]), 0b11, "A e B (sem diferença de maiúscula)");
        assert_eq!((state[5] as i8, state[6] as i8), (127, -64));
        assert_eq!(state[10], 255, "gatilho direito");
        assert_eq!(state.len(), 11);
        assert_eq!(encode_look(1.5, -0.25), vec![0x09, 150, 0, 0xe7, 0xff]);

        assert!(parse_script("[]").unwrap_err().contains("vazio"));
        assert!(parse_script(r#"[{ "ms": 0 }]"#).unwrap_err().contains("ms"));
        assert!(parse_script(r#"[{ "ms": 10, "buttons": ["Z"] }]"#).unwrap_err().contains("Z"));
        assert!(parse_script(r#"[{ "ms": 10, "jump": true }]"#).is_err(), "campo errado não passa calado");
    }
}
