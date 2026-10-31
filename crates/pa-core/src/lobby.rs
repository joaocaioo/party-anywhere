//! Regras do lobby: quem ocupa cada vaga (P1..P10), quem espera para entrar e o que acontece quando um
//! aparelho cai, volta ou é removido.
//!
//! Sem I/O e sem relógio próprio: quem chama informa os eventos dos aparelhos e o instante em ms.

use crate::{MAX_SEATS, SEAT_COLORS};

/// Nome mais longo que a pessoa pode dar a um jogador.
pub const MAX_NAME_CHARS: usize = 20;

/// Identidade estável de um aparelho: caminho HID ou serial do controle, endereço Bluetooth, token do
/// celular, ContainerId do teclado. A mesma chave que volta é o mesmo aparelho.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeviceKey(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Gamepad,
    Phone,
    Keyboard,
}

/// Um aparelho que acabou de aparecer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub key: DeviceKey,
    pub kind: DeviceKind,
    /// Nome lembrado de outra sessão ("Júlio"); sem ele, a vaga vira "Jogador N".
    pub name: Option<String>,
}

/// Como controles conectados entram. Celular sempre entra (a pessoa escaneou o QR de propósito) e
/// teclado nunca entra sozinho (todo notebook tem um).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JoinPolicy {
    /// Ligou, entrou.
    #[default]
    Automatic,
    /// "Entrar só ao apertar um botão": útil com controles parados na sala.
    OnButtonPress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeatState {
    Active,
    /// Caiu; a vaga fica guardada até o aparelho voltar ou alguém remover.
    Reconnecting { since_ms: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seat {
    /// 0..9 → P1..P10.
    pub index: usize,
    pub device: DeviceKey,
    pub kind: DeviceKind,
    pub name: String,
    pub state: SeatState,
}

impl Seat {
    /// A cor é da vaga, não do aparelho: P1 é sempre azul.
    pub fn color(&self) -> &'static str {
        SEAT_COLORS[self.index]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinOutcome {
    /// Entrou numa vaga nova.
    Joined(usize),
    /// Voltou para a vaga que já era dele.
    Rejoined(usize),
    /// Conectado, mas esperando: apertar um botão (ou, no teclado, a ação explícita) faz entrar.
    Waiting,
    /// As 10 vagas estão ocupadas; o aparelho fica esperando uma vaga abrir.
    Full,
}

#[derive(Clone, Debug, Default)]
pub struct Lobby {
    seats: [Option<Seat>; MAX_SEATS],
    /// Conectados sem vaga: aguardando botão, teclados, removidos ainda ligados, ou sem lugar.
    waiting: Vec<Device>,
    pub policy: JoinPolicy,
}

impl Lobby {
    pub fn new(policy: JoinPolicy) -> Self {
        Self { policy, ..Self::default() }
    }

    /// Um aparelho apareceu (controle ligado, celular deu `hello`, teclado detectado).
    pub fn connected(&mut self, device: Device) -> JoinOutcome {
        // a vaga é dele mesmo se ainda constar como ativa: o celular reabriu a página ou o controle
        // reconectou antes de o Hub perceber a queda. O transporte fecha a conexão antiga.
        if let Some(i) = self.seat_of(&device.key) {
            self.seats[i].as_mut().expect("seat_of devolve vaga ocupada").state = SeatState::Active;
            return JoinOutcome::Rejoined(i);
        }
        let auto = match device.kind {
            DeviceKind::Phone => true,
            DeviceKind::Keyboard => false,
            DeviceKind::Gamepad => self.policy == JoinPolicy::Automatic,
        };
        if auto {
            self.join(device)
        } else {
            self.park(device);
            JoinOutcome::Waiting
        }
    }

    /// Um aparelho sem vaga apertou um botão. Teclado não entra assim: precisa de [`Lobby::add`].
    pub fn pressed(&mut self, key: &DeviceKey) -> Option<JoinOutcome> {
        let pos = self.waiting.iter().position(|d| &d.key == key)?;
        if self.waiting[pos].kind == DeviceKind::Keyboard {
            return None;
        }
        Some(self.join_waiting(pos))
    }

    /// Ação explícita na UI: "Conectar" ou Enter na tela Adicionar jogador → Teclado.
    pub fn add(&mut self, key: &DeviceKey) -> Option<JoinOutcome> {
        if let Some(i) = self.seat_of(key) {
            return Some(JoinOutcome::Rejoined(i));
        }
        let pos = self.waiting.iter().position(|d| &d.key == key)?;
        Some(self.join_waiting(pos))
    }

    /// O aparelho sumiu. Quem tinha vaga fica em `Reconnecting`; quem esperava sai da fila.
    pub fn disconnected(&mut self, key: &DeviceKey, now_ms: u64) {
        if let Some(i) = self.seat_of(key) {
            self.seats[i].as_mut().expect("seat_of devolve vaga ocupada").state =
                SeatState::Reconnecting { since_ms: now_ms };
        }
        self.waiting.retain(|d| &d.key != key);
    }

    /// Botão de remover na carta. Devolve a vaga para o "Desfazer". Se o aparelho ainda está ligado, ele
    /// vai para a espera: não volta sozinho, só apertando um botão (ou pela ação explícita, no teclado).
    pub fn remove(&mut self, index: usize) -> Option<Seat> {
        let seat = self.seats.get_mut(index)?.take()?;
        if seat.state == SeatState::Active {
            self.park(Device { key: seat.device.clone(), kind: seat.kind, name: Some(seat.name.clone()) });
        }
        Some(seat)
    }

    /// "Desfazer" a remoção: só se a vaga continua livre e o aparelho não entrou em outra.
    pub fn undo_remove(&mut self, seat: Seat) -> bool {
        if seat.index >= MAX_SEATS || self.seats[seat.index].is_some() || self.seat_of(&seat.device).is_some() {
            return false;
        }
        let was_waiting = self.waiting.len();
        self.waiting.retain(|d| d.key != seat.device);
        // se não estava na espera, o aparelho caiu enquanto isso: volta como "Reconnecting"
        let seat = match (was_waiting != self.waiting.len(), seat.state) {
            (false, SeatState::Active) => Seat { state: SeatState::Reconnecting { since_ms: 0 }, ..seat },
            _ => seat,
        };
        let index = seat.index;
        self.seats[index] = Some(seat);
        true
    }

    /// Reordenar: troca as duas vagas (uma pode estar vazia). Cada jogador leva o nome; a cor fica com o
    /// número da vaga.
    pub fn move_seat(&mut self, from: usize, to: usize) -> bool {
        if from >= MAX_SEATS || to >= MAX_SEATS || from == to || self.seats[from].is_none() {
            return false;
        }
        self.seats.swap(from, to);
        for i in [from, to] {
            if let Some(s) = self.seats[i].as_mut() {
                s.index = i;
            }
        }
        true
    }

    /// Renomear. Vazio volta para "Jogador N"; o excesso é cortado.
    pub fn rename(&mut self, index: usize, name: &str) -> bool {
        let Some(seat) = self.seats.get_mut(index).and_then(Option::as_mut) else { return false };
        seat.name = clean_name(name).unwrap_or_else(|| default_name(index));
        true
    }

    pub fn seat(&self, index: usize) -> Option<&Seat> {
        self.seats.get(index)?.as_ref()
    }

    pub fn seats(&self) -> impl Iterator<Item = &Seat> {
        self.seats.iter().flatten()
    }

    pub fn waiting(&self) -> &[Device] {
        &self.waiting
    }

    /// Vagas ocupadas, contando quem está reconectando (a vaga é dele).
    pub fn occupied(&self) -> usize {
        self.seats().count()
    }

    pub fn is_full(&self) -> bool {
        self.occupied() == MAX_SEATS
    }

    /// Vaga do aparelho, se ele tem uma (ativa ou reconectando).
    pub fn seat_of_device(&self, key: &DeviceKey) -> Option<usize> {
        self.seat_of(key)
    }

    fn seat_of(&self, key: &DeviceKey) -> Option<usize> {
        self.seats.iter().position(|s| s.as_ref().is_some_and(|s| &s.device == key))
    }

    fn join(&mut self, device: Device) -> JoinOutcome {
        // menor vaga vazia; vaga em "Reconnecting" não conta como vazia
        let Some(index) = self.seats.iter().position(Option::is_none) else {
            self.park(device);
            return JoinOutcome::Full;
        };
        let name = device.name.as_deref().and_then(clean_name).unwrap_or_else(|| default_name(index));
        self.seats[index] = Some(Seat { index, device: device.key, kind: device.kind, name, state: SeatState::Active });
        JoinOutcome::Joined(index)
    }

    fn join_waiting(&mut self, pos: usize) -> JoinOutcome {
        if self.is_full() {
            return JoinOutcome::Full;
        }
        let device = self.waiting.remove(pos);
        self.join(device)
    }

    fn park(&mut self, device: Device) {
        if !self.waiting.iter().any(|d| d.key == device.key) {
            self.waiting.push(device);
        }
    }
}

fn default_name(index: usize) -> String {
    format!("Jogador {}", index + 1)
}

fn clean_name(name: &str) -> Option<String> {
    let name: String = name.trim().chars().take(MAX_NAME_CHARS).collect();
    let name = name.trim_end().to_string();
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(key: &str, kind: DeviceKind) -> Device {
        Device { key: DeviceKey(key.into()), kind, name: None }
    }
    fn pad(key: &str) -> Device {
        dev(key, DeviceKind::Gamepad)
    }
    fn phone(key: &str) -> Device {
        dev(key, DeviceKind::Phone)
    }
    fn key(k: &str) -> DeviceKey {
        DeviceKey(k.into())
    }

    #[test]
    fn new_devices_take_the_lowest_free_seat() {
        let mut l = Lobby::default();
        assert_eq!(l.connected(pad("a")), JoinOutcome::Joined(0));
        assert_eq!(l.connected(phone("b")), JoinOutcome::Joined(1));
        assert_eq!(l.connected(pad("c")), JoinOutcome::Joined(2));
        l.remove(1);
        assert_eq!(l.connected(pad("d")), JoinOutcome::Joined(1));
        assert_eq!(l.seat(1).unwrap().name, "Jogador 2");
        assert_eq!(l.seat(0).unwrap().color(), SEAT_COLORS[0]);
    }

    #[test]
    fn eleventh_device_waits_for_a_free_seat() {
        let mut l = Lobby::default();
        for i in 0..MAX_SEATS {
            assert_eq!(l.connected(pad(&format!("p{i}"))), JoinOutcome::Joined(i));
        }
        assert!(l.is_full());
        assert_eq!(l.connected(phone("extra")), JoinOutcome::Full);
        assert_eq!(l.pressed(&key("extra")), Some(JoinOutcome::Full));
        l.remove(4);
        assert_eq!(l.pressed(&key("extra")), Some(JoinOutcome::Joined(4)));
    }

    #[test]
    fn dropped_device_keeps_its_seat_and_gets_it_back() {
        let mut l = Lobby::default();
        l.connected(pad("a"));
        l.connected(phone("b"));
        l.rename(1, "Ana");
        l.disconnected(&key("b"), 5_000);
        assert_eq!(l.seat(1).unwrap().state, SeatState::Reconnecting { since_ms: 5_000 });

        // a vaga guardada não é dada a ninguém
        assert_eq!(l.connected(pad("c")), JoinOutcome::Joined(2));

        assert_eq!(l.connected(phone("b")), JoinOutcome::Rejoined(1));
        let s = l.seat(1).unwrap();
        assert_eq!((s.state, s.name.as_str()), (SeatState::Active, "Ana"));
    }

    #[test]
    fn phone_reopening_the_page_keeps_its_seat() {
        // medido na fase 1: reabrir a página virava P2, P3... A mesma chave é o mesmo jogador.
        let mut l = Lobby::default();
        l.connected(phone("token-1"));
        assert_eq!(l.connected(phone("token-1")), JoinOutcome::Rejoined(0));
        assert_eq!(l.occupied(), 1);
    }

    #[test]
    fn full_lobby_counts_reconnecting_seats() {
        let mut l = Lobby::default();
        for i in 0..MAX_SEATS {
            l.connected(pad(&format!("p{i}")));
        }
        l.disconnected(&key("p3"), 1);
        assert_eq!(l.connected(pad("new")), JoinOutcome::Full);
        assert_eq!(l.connected(pad("p3")), JoinOutcome::Rejoined(3));
    }

    #[test]
    fn button_press_policy_parks_gamepads_but_not_phones() {
        let mut l = Lobby::new(JoinPolicy::OnButtonPress);
        assert_eq!(l.connected(pad("a")), JoinOutcome::Waiting);
        assert_eq!(l.connected(phone("b")), JoinOutcome::Joined(0));
        assert_eq!(l.pressed(&key("a")), Some(JoinOutcome::Joined(1)));
        assert!(l.waiting().is_empty());
        // quem não está esperando não muda nada ao apertar
        assert_eq!(l.pressed(&key("a")), None);
    }

    #[test]
    fn keyboard_only_joins_by_explicit_action() {
        let mut l = Lobby::default();
        let kb = dev("kb-notebook", DeviceKind::Keyboard);
        assert_eq!(l.connected(kb), JoinOutcome::Waiting);
        assert_eq!(l.pressed(&key("kb-notebook")), None, "tecla qualquer não faz o notebook virar P1");
        assert_eq!(l.add(&key("kb-notebook")), Some(JoinOutcome::Joined(0)));
        assert_eq!(l.seat(0).unwrap().kind, DeviceKind::Keyboard);
        assert_eq!(l.add(&key("kb-notebook")), Some(JoinOutcome::Rejoined(0)));
        assert_eq!(l.add(&key("unknown")), None);
    }

    #[test]
    fn removed_device_that_is_still_plugged_waits_for_a_button() {
        let mut l = Lobby::default();
        l.connected(pad("a"));
        let removed = l.remove(0).unwrap();
        assert_eq!(removed.device, key("a"));
        assert_eq!(l.occupied(), 0);
        assert_eq!(l.waiting().len(), 1, "não volta sozinho para o lobby");
        assert_eq!(l.pressed(&key("a")), Some(JoinOutcome::Joined(0)));
    }

    #[test]
    fn removing_a_dropped_seat_forgets_the_device() {
        let mut l = Lobby::default();
        l.connected(pad("a"));
        l.disconnected(&key("a"), 1);
        l.remove(0);
        assert!(l.waiting().is_empty());
        assert_eq!(l.connected(pad("a")), JoinOutcome::Joined(0));
    }

    #[test]
    fn undo_restores_the_seat_only_while_it_is_free() {
        let mut l = Lobby::default();
        l.connected(pad("a"));
        l.rename(0, "Duda");
        let seat = l.remove(0).unwrap();
        assert!(l.undo_remove(seat.clone()));
        let s = l.seat(0).unwrap();
        assert_eq!((s.name.as_str(), s.state), ("Duda", SeatState::Active));
        assert!(l.waiting().is_empty());

        let seat = l.remove(0).unwrap();
        l.connected(phone("b"));
        assert!(!l.undo_remove(seat), "P1 já é de outra pessoa");
    }

    #[test]
    fn undo_after_the_device_left_brings_it_back_as_reconnecting() {
        let mut l = Lobby::default();
        l.connected(pad("a"));
        let seat = l.remove(0).unwrap();
        l.disconnected(&key("a"), 9);
        assert!(l.undo_remove(seat));
        assert!(matches!(l.seat(0).unwrap().state, SeatState::Reconnecting { .. }));
    }

    #[test]
    fn moving_seats_swaps_players_and_keeps_colors_on_the_number() {
        let mut l = Lobby::default();
        l.connected(pad("a"));
        l.connected(phone("b"));
        l.rename(0, "Ana");
        assert!(l.move_seat(0, 1));
        assert_eq!(l.seat(0).unwrap().device, key("b"));
        assert_eq!((l.seat(1).unwrap().name.as_str(), l.seat(1).unwrap().index), ("Ana", 1));
        assert_eq!(l.seat(1).unwrap().color(), SEAT_COLORS[1]);

        assert!(l.move_seat(1, 7), "pode ir para uma vaga vazia");
        assert!(l.seat(1).is_none());
        assert_eq!(l.seat(7).unwrap().index, 7);
        assert!(!l.move_seat(1, 2), "vaga vazia não se move");
        assert!(!l.move_seat(0, MAX_SEATS));
    }

    #[test]
    fn names_are_trimmed_capped_and_never_empty() {
        let mut l = Lobby::default();
        l.connected(Device { name: Some("  Júlio ".into()), ..pad("a") });
        assert_eq!(l.seat(0).unwrap().name, "Júlio");
        l.rename(0, "   ");
        assert_eq!(l.seat(0).unwrap().name, "Jogador 1");
        l.rename(0, "Um nome bem comprido demais para caber");
        assert_eq!(l.seat(0).unwrap().name.chars().count(), MAX_NAME_CHARS);
        assert!(!l.rename(5, "Ninguém"));
    }

    #[test]
    fn waiting_device_that_unplugs_leaves_the_queue() {
        let mut l = Lobby::new(JoinPolicy::OnButtonPress);
        l.connected(pad("a"));
        l.disconnected(&key("a"), 1);
        assert!(l.waiting().is_empty());
        assert_eq!(l.pressed(&key("a")), None);
    }
}
