//! Spike de latência (docs/ROADMAP.md, fase 1).
//!
//! Sobe o servidor do celular, mostra o QR no terminal, cria um controle virtual de Xbox por celular
//! (se o ViGEmBus estiver instalado) e mede:
//!
//! - **toque → PC**: do evento de toque no navegador até a mensagem chegar aqui;
//! - **rede (rajada)**: do envio até a chegada, com o celular parado mandando 60 estados por segundo;
//! - **ida e volta**: das trocas de sincronização de relógio;
//! - **ViGEm**: do pedido até o driver aceitar o estado.
//!
//! Critério da fase: toque → PC com p50 < 20 ms e p99 < 50 ms.

mod stats;

use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pa_core::{Pad, MAX_SEATS};
use pa_phone::protocol::{SampleKind, ServerText};
use pa_phone::{net, PhoneEvent, PhoneServer, ServerConfig, CONTROLLER_PAGE};
use pa_virtualpad::{UpdateReport, VirtualPads};
use serde::Serialize;
use stats::{ClockSync, Samples, Summary};

const DEFAULT_PORT: u16 = 27027;
/// Sem estado novo por este tempo, o controle virtual volta ao neutro (docs/ARCHITECTURE.md).
const NEUTRAL_AFTER_MS: f64 = 500.0;
// meta revista depois das medições no iPhone (ver README): o resto do atraso é do navegador do celular
const TARGET_P50_MS: f64 = 25.0;
const TARGET_P99_MS: f64 = 100.0;

struct Args {
    port: u16,
    duration: Option<Duration>,
    vigem: bool,
    loopback: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args { port: DEFAULT_PORT, duration: None, vigem: true, loopback: false };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--port" => a.port = it.next().and_then(|v| v.parse().ok()).ok_or("--port precisa de um número")?,
            "--duration" => {
                let s: u64 = it.next().and_then(|v| v.parse().ok()).ok_or("--duration precisa de segundos")?;
                a.duration = Some(Duration::from_secs(s));
            }
            "--no-vigem" => a.vigem = false,
            "--loopback" => a.loopback = true,
            "-h" | "--help" => {
                println!(
                    "pa-latency-spike [--port N] [--duration SEGUNDOS] [--no-vigem] [--loopback]\n\n\
                     Abra o endereço mostrado no celular (mesmo Wi-Fi). Ctrl+C encerra e grava o relatório."
                );
                std::process::exit(0);
            }
            other => return Err(format!("argumento desconhecido: {other} (use --help)")),
        }
    }
    Ok(a)
}

#[derive(Default)]
struct SeatStats {
    device: String,
    connected: bool,
    clock: ClockSync,
    touch: Samples,
    stick: Samples,
    burst: Samples,
    heartbeat: Samples,
    rtt: Samples,
    vigem: Samples,
    states: u64,
    lost: u64,
    stale: u64,
    last_seq: Option<u16>,
    last_pad: Pad,
    last_at_ms: f64,
    neutral_sent: bool,
}

impl SeatStats {
    fn on_state(&mut self, seq: u16, pad: Pad, at_ms: f64, timing: Option<(SampleKind, f64)>) -> bool {
        self.states += 1;
        if let Some(last) = self.last_seq {
            let diff = seq.wrapping_sub(last) as i16;
            if diff <= 0 {
                // mais velha que a última: não pode sobrescrever o estado atual
                self.stale += 1;
                return false;
            }
            self.lost += (diff - 1) as u64;
        }
        self.last_seq = Some(seq);

        if let (Some((kind, phone_ms)), Some(offset)) = (timing, self.clock.offset()) {
            let latency = at_ms - (phone_ms + offset);
            // fora disto é relógio ainda não sincronizado ou celular dormindo, não latência
            if (-50.0..10_000.0).contains(&latency) {
                match kind {
                    SampleKind::Touch => self.touch.push(latency),
                    SampleKind::Stick => self.stick.push(latency),
                    SampleKind::Burst => self.burst.push(latency),
                    SampleKind::Heartbeat => self.heartbeat.push(latency),
                }
            }
        }

        let changed = pad != self.last_pad || self.neutral_sent;
        self.last_pad = pad;
        self.last_at_ms = at_ms;
        self.neutral_sent = false;
        changed
    }
}

#[derive(Serialize)]
struct SeatReport {
    seat: u8,
    device: String,
    /// botões (e soltar o analógico): é o número da meta
    touch_to_pc_ms: Option<Summary>,
    /// movimento contínuo do analógico
    stick_to_pc_ms: Option<Summary>,
    burst_network_ms: Option<Summary>,
    heartbeat_ms: Option<Summary>,
    round_trip_ms: Option<Summary>,
    vigem_update_ms: Option<Summary>,
    clock_best_rtt_ms: Option<f64>,
    states: u64,
    lost: u64,
    stale: u64,
    meets_target: Option<bool>,
}

#[derive(Serialize)]
struct Report {
    generated_utc: String,
    duration_s: f64,
    virtual_pad: bool,
    target: Target,
    seats: Vec<SeatReport>,
}

#[derive(Serialize)]
struct Target {
    touch_p50_ms: f64,
    touch_p99_ms: f64,
}

#[tokio::main]
async fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let clock = Instant::now();
    let lobby = std::sync::Arc::new(std::sync::Mutex::new(pa_core::Lobby::default()));
    let config = ServerConfig { port: args.port, loopback_only: args.loopback, page: CONTROLLER_PAGE, lobby };
    let (server, mut events) = match PhoneServer::start(config, clock).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Não deu para abrir o servidor: {e}");
            std::process::exit(1);
        }
    };

    let (report_tx, report_rx) = std_mpsc::channel::<UpdateReport>();
    let pads = if args.vigem {
        match VirtualPads::start(Some(report_tx)) {
            Ok(p) => Some(p),
            Err(e) => {
                println!("⚠  Controle virtual desligado: {e}.");
                println!("   A medição celular → PC funciona igual. Para o jogo ver o celular como controle,");
                println!("   instale o ViGEmBus (.downloads\\vigembus\\) e rode de novo.\n");
                None
            }
        }
    } else {
        None
    };

    print_banner(&server, args.loopback, pads.is_some());
    if let Some(d) = args.duration {
        println!("  A medição termina sozinha em {} s e mostra o resultado aqui.
", d.as_secs());
    }

    let mut seats: Vec<SeatStats> = (0..MAX_SEATS).map(|_| SeatStats::default()).collect();
    let mut stats_tick = tokio::time::interval(Duration::from_secs(1));
    let mut watchdog = tokio::time::interval(Duration::from_millis(50));
    let deadline = async {
        match args.duration {
            Some(d) => tokio::time::sleep(d).await,
            None => std::future::pending().await,
        }
    };
    tokio::pin!(deadline);
    let mut ticks = 0u64;

    loop {
        tokio::select! {
            ev = events.recv() => {
                let Some(ev) = ev else { break };
                handle_event(ev, &mut seats, pads.as_ref());
            }
            _ = stats_tick.tick() => {
                drain_vigem(&report_rx, &mut seats);
                ticks += 1;
                for (i, s) in seats.iter().enumerate().filter(|(_, s)| s.connected) {
                    let data = serde_json::json!({
                        "touch": s.touch.summary(),
                        "burst": s.burst.summary(),
                        "rtt": s.rtt.summary(),
                        "lost": s.lost,
                    });
                    server.send_text(i as u8, &ServerText::Stats { data });
                }
                if ticks.is_multiple_of(5) {
                    print_live(&seats);
                }
            }
            _ = watchdog.tick() => {
                let now = server.now_ms();
                for (i, s) in seats.iter_mut().enumerate() {
                    if s.connected && !s.neutral_sent && !s.last_pad.is_neutral() && now - s.last_at_ms > NEUTRAL_AFTER_MS {
                        s.neutral_sent = true;
                        if let Some(p) = &pads { p.update(i as u8, Pad::NEUTRAL); }
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => break,
            _ = &mut deadline => break,
        }
    }

    drain_vigem(&report_rx, &mut seats);
    finish(&seats, clock.elapsed(), pads.is_some());
}

fn handle_event(ev: PhoneEvent, seats: &mut [SeatStats], pads: Option<&VirtualPads>) {
    match ev {
        PhoneEvent::Connected { seat, device, peer, rejoined } => {
            let s = &mut seats[seat as usize];
            let returning = rejoined && s.device == device;
            if !returning {
                *s = SeatStats { device: device.clone(), ..Default::default() };
            }
            s.connected = true;
            s.last_seq = None;
            if let Some(p) = pads {
                p.plug(seat);
            }
            println!("➕ P{} entrou: {} ({})", seat + 1, device, peer.ip());
        }
        PhoneEvent::Disconnected { seat } => {
            let s = &mut seats[seat as usize];
            s.connected = false;
            if let Some(p) = pads {
                p.update(seat, Pad::NEUTRAL);
                p.unplug(seat);
            }
            println!("➖ P{} saiu ({})", seat + 1, s.device);
        }
        PhoneEvent::State { seat, seq, pad, at_ms, timing } => {
            let changed = seats[seat as usize].on_state(seq, pad, at_ms, timing);
            if changed {
                if let Some(p) = pads {
                    p.update(seat, pad);
                }
            }
        }
        PhoneEvent::LayoutChosen { .. } | PhoneEvent::Renamed { .. } => {}
        PhoneEvent::ClockSample { seat, rtt_ms, offset_ms } => {
            let s = &mut seats[seat as usize];
            s.clock.add(rtt_ms, offset_ms);
            s.rtt.push(rtt_ms);
        }
    }
}

fn drain_vigem(rx: &std_mpsc::Receiver<UpdateReport>, seats: &mut [SeatStats]) {
    while let Ok(r) = rx.try_recv() {
        if r.ok {
            seats[r.seat as usize].vigem.push(r.latency.as_secs_f64() * 1000.0);
        }
    }
}

fn print_banner(server: &PhoneServer, loopback: bool, vigem: bool) {
    let port = server.local_addr().port();
    let candidates = if loopback {
        vec![net::Candidate { ip: std::net::Ipv4Addr::LOCALHOST, interface: "loopback".into(), score: 0 }]
    } else {
        net::lan_candidates()
    };
    println!("\n  PARTY ANYWHERE · spike de latência");
    println!("  Controle virtual: {}\n", if vigem { "ligado (um Xbox 360 por celular)" } else { "desligado" });
    let Some(best) = candidates.first() else {
        println!("  Nenhuma rede local encontrada. O PC está conectado ao Wi-Fi ou a um cabo?");
        return;
    };
    let url = format!("http://{}:{}/", best.ip, port);
    if let Ok(code) = qrcode::QrCode::new(url.as_bytes()) {
        use qrcode::render::unicode::Dense1x2;
        let qr = code.render::<Dense1x2>().dark_color(Dense1x2::Light).light_color(Dense1x2::Dark).quiet_zone(true).build();
        for line in qr.lines() {
            println!("  {line}");
        }
    }
    println!("\n  Escaneie com o celular (mesmo Wi-Fi) ou abra: {url}   [{}]", best.interface);
    for c in candidates.iter().skip(1) {
        println!("  Não conectou? Tente: http://{}:{}/   [{}]", c.ip, port, c.interface);
    }
    println!("\n  No celular: toque em TESTE para ver os números. Ctrl+C aqui encerra e grava o relatório.\n");
}

fn ms(s: Option<Summary>, f: impl Fn(&Summary) -> f64) -> String {
    s.map(|s| format!("{:.1}", f(&s))).unwrap_or_else(|| "--".into())
}

fn print_live(seats: &[SeatStats]) {
    for (i, s) in seats.iter().enumerate().filter(|(_, s)| s.connected) {
        let t = s.touch.summary();
        let st = s.stick.summary();
        let b = s.burst.summary();
        let r = s.rtt.summary();
        let v = s.vigem.summary();
        println!(
            "P{:<2} {:<14} botões→PC p50 {:>5} p95 {:>5} p99 {:>5} (n={:<5}) | analógico p50 {:>5} p99 {:>5} | rajada p50 {:>5} p99 {:>5} | ida+volta p50 {:>5} | perdidas {} | ViGEm p99 {}",
            i + 1,
            truncate(&s.device, 14),
            ms(t, |x| x.p50),
            ms(t, |x| x.p95),
            ms(t, |x| x.p99),
            s.touch.len(),
            ms(st, |x| x.p50),
            ms(st, |x| x.p99),
            ms(b, |x| x.p50),
            ms(b, |x| x.p99),
            ms(r, |x| x.p50),
            s.lost,
            ms(v, |x| x.p99),
        );
    }
}

fn finish(seats: &[SeatStats], elapsed: Duration, vigem: bool) {
    let reports: Vec<SeatReport> = seats
        .iter()
        .enumerate()
        .filter(|(_, s)| s.states > 0)
        .map(|(i, s)| {
            let touch = s.touch.summary();
            SeatReport {
                seat: i as u8 + 1,
                device: s.device.clone(),
                meets_target: touch.map(|t| t.p50 < TARGET_P50_MS && t.p99 < TARGET_P99_MS),
                touch_to_pc_ms: touch,
                stick_to_pc_ms: s.stick.summary(),
                burst_network_ms: s.burst.summary(),
                heartbeat_ms: s.heartbeat.summary(),
                round_trip_ms: s.rtt.summary(),
                vigem_update_ms: s.vigem.summary(),
                clock_best_rtt_ms: s.clock.best_rtt(),
                states: s.states,
                lost: s.lost,
                stale: s.stale,
            }
        })
        .collect();

    println!("\n  RESULTADO ({:.0} s)", elapsed.as_secs_f64());
    if reports.is_empty() {
        println!("  Nenhum celular mandou dados. Nada para gravar.");
        return;
    }
    for r in &reports {
        let verdict = match r.meets_target {
            Some(true) => "✅ dentro da meta",
            Some(false) => "❌ fora da meta",
            None => "sem toques medidos",
        };
        println!(
            "  P{} {}: botões→PC p50 {} ms, p99 {} ms ({} toques) · analógico p50 {} ms, p99 {} ms · rajada p50 {} ms, p99 {} ms · erro da medição ≤ {} ms · {}",
            r.seat,
            r.device,
            ms(r.touch_to_pc_ms, |x| x.p50),
            ms(r.touch_to_pc_ms, |x| x.p99),
            r.touch_to_pc_ms.map_or(0, |x| x.n),
            ms(r.stick_to_pc_ms, |x| x.p50),
            ms(r.stick_to_pc_ms, |x| x.p99),
            ms(r.burst_network_ms, |x| x.p50),
            ms(r.burst_network_ms, |x| x.p99),
            r.clock_best_rtt_ms.map(|v| format!("{:.1}", v / 2.0)).unwrap_or_else(|| "--".into()),
            verdict
        );
    }
    println!("  Meta da fase 1: p50 < {TARGET_P50_MS} ms e p99 < {TARGET_P99_MS} ms.");

    let report = Report {
        generated_utc: utc_stamp(),
        duration_s: elapsed.as_secs_f64(),
        virtual_pad: vigem,
        target: Target { touch_p50_ms: TARGET_P50_MS, touch_p99_ms: TARGET_P99_MS },
        seats: reports,
    };
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("reports");
    let path = dir.join(format!("{}.json", report.generated_utc.replace(':', "-")));
    let written = std::fs::create_dir_all(&dir)
        .and_then(|_| std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap_or_default()));
    match written {
        Ok(()) => println!("  Relatório: {}\n", path.display()),
        Err(e) => println!("  Não deu para gravar o relatório: {e}\n"),
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n - 1).chain(['…']).collect() }
}

/// Data e hora UTC sem dependência de calendário (algoritmo civil de Howard Hinnant).
fn utc_stamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seat_measures_with_clock_offset() {
        let mut s = SeatStats::default();
        s.clock.add(4.0, 1000.0);
        let pressed = Pad { buttons: pa_core::buttons::A, ..Pad::NEUTRAL };
        assert!(s.on_state(1, pressed, 1012.0, Some((SampleKind::Touch, 5.0))));
        assert_eq!(s.touch.summary().unwrap().p50, 7.0);
        // mesmo estado de novo: nada para mandar ao controle virtual
        assert!(!s.on_state(2, pressed, 1020.0, None));
    }

    #[test]
    fn seat_counts_lost_and_stale() {
        let mut s = SeatStats::default();
        s.on_state(10, Pad::NEUTRAL, 0.0, None);
        s.on_state(13, Pad::NEUTRAL, 1.0, None);
        assert!(!s.on_state(12, Pad::NEUTRAL, 2.0, None));
        assert_eq!((s.lost, s.stale), (2, 1));
    }

    #[test]
    fn sequence_wraps_without_loss() {
        let mut s = SeatStats::default();
        s.on_state(u16::MAX - 1, Pad::NEUTRAL, 0.0, None);
        s.on_state(u16::MAX, Pad::NEUTRAL, 1.0, None);
        s.on_state(0, Pad::NEUTRAL, 2.0, None);
        s.on_state(1, Pad::NEUTRAL, 3.0, None);
        assert_eq!((s.lost, s.stale), (0, 0));
    }

    #[test]
    fn no_sample_before_clock_sync() {
        let mut s = SeatStats::default();
        s.on_state(1, Pad::NEUTRAL, 10.0, Some((SampleKind::Touch, 5.0)));
        assert_eq!(s.touch.len(), 0);
    }

    #[test]
    fn utc_stamp_shape() {
        let s = utc_stamp();
        assert_eq!(s.len(), 20);
        assert!(s.ends_with('Z'));
    }
}
