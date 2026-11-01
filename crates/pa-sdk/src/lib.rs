//! Lado do Hub do SDK (docs/SDK.md): a memória compartilhada `Local\PartyAnywhere.v1` que o
//! `party_anywhere.h` lê. O Hub escreve as vagas (com seqlock: o jogo nunca lê um estado pela metade) e lê
//! os comandos do jogo (vibrar, aviso no celular, lobby aberto).
//!
//! O layout espelha `pa_shm` do cabeçalho. Os testes compilam o cabeçalho de verdade (build.rs) e conversam
//! com ele pela mesma memória: mudar um lado sem o outro quebra o teste.

pub mod view;

use std::ptr::{addr_of, addr_of_mut, NonNull};
use std::sync::atomic::{fence, AtomicI32, AtomicU32, Ordering};

use pa_core::{Pad, MAX_SEATS};

pub const ABI: u32 = 1;
pub const MAGIC: u32 = 0x3148_4150; // "PAH1"
pub const NAME: &str = r"Local\PartyAnywhere.v1";
pub const CMDS: usize = 64;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ShmSeat {
    pub seq: u32,
    pub state: u8,
    pub device: u8,
    pub rgb: [u8; 3],
    pub pad_: [u8; 3],
    pub name: [u8; 32],
    pub buttons: u32,
    pub axes: [f32; 6], // lx ly rx ry lt rt
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ShmCmd {
    pub kind: u8,
    pub seat: i8,
    pub duration_ms: u16,
    pub low: f32,
    pub high: f32,
    pub value: u32,
    pub text: [u8; 48],
}

#[repr(C)]
pub struct Shm {
    pub magic: u32,
    pub abi: u32,
    pub size: u32,
    pub hub_heartbeat: u32,
    pub game_pid: i32,
    /// O jogo escreve o que ele usa (`GAME_CAP_*`), para o Hub ajustar o celular. 0 = nada além do básico.
    pub game_caps: u32,
    pub join_url: [u8; 128],
    pub seats: [ShmSeat; MAX_SEATS],
    pub cmd_write: u32,
    pub cmd_read: u32,
    pub cmds: [ShmCmd; CMDS],
    /// Câmera por deslocamento (arrastar o dedo), por vaga: milésimos de grau acumulados. O Hub soma, o jogo
    /// pega e zera (`pa_take_look`). Entrou depois na ABI 1: um jogo mais velho não lê, um Hub mais velho não tem.
    pub look: [[i32; 2]; MAX_SEATS],
}

pub const SHM_SIZE: usize = 5056;
/// O tamanho da primeira versão da ABI 1 (sem `look`): jogos e Hubs desse tamanho continuam compatíveis.
pub const SHM_SIZE_V1: usize = 4976;
/// `game_caps`: o jogo lê a câmera por deslocamento (`pa_take_look`).
pub const GAME_CAP_LOOK: u32 = 1;
const _: () = assert!(std::mem::size_of::<Shm>() == SHM_SIZE);
const _: () = assert!(std::mem::size_of::<ShmSeat>() == 72);
const _: () = assert!(std::mem::size_of::<ShmCmd>() == 64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SeatState {
    Empty = 0,
    Active = 1,
    Reconnecting = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Device {
    Gamepad = 0,
    Phone = 1,
    Keyboard = 2,
}

/// Pedido do jogo para o Hub.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Rumble { seat: Option<usize>, low: f32, high: f32, ms: u16 },
    Toast { seat: Option<usize>, text: String },
    Accepting(bool),
    Label { seat: Option<usize>, button: u32, text: String },
    Layout { seat: Option<usize>, layout: u32 },
    Light { seat: Option<usize>, rgb: u32 },
    /// Modo do controle do celular (índice nos modos do layout: "A pé", "Veículo").
    Mode { seat: Option<usize>, mode: u32 },
}

enum Owner {
    #[cfg(test)]
    Heap(#[allow(dead_code)] Box<std::mem::MaybeUninit<Shm>>),
    #[cfg(windows)]
    Mapping(windows_sys::Win32::Foundation::HANDLE),
}

/// A memória do SDK do lado do Hub. Um só escritor por vez: o Hub guarda isto atrás de um Mutex.
pub struct HubShm {
    ptr: NonNull<Shm>,
    owner: Owner,
}

// a memória é compartilhada por desenho; o acesso é coordenado por seqlock e contadores atômicos
unsafe impl Send for HubShm {}

impl HubShm {
    /// Cria `Local\PartyAnywhere.v1`. Se a memória já existe e o Hub dela está vivo, falha (só um Hub por
    /// vez). Se ela sobrou de um Hub que fechou com um jogo ainda aberto (o jogo segura a memória), o Hub novo
    /// assume: o jogo volta a ver os jogadores sem precisar reiniciar.
    #[cfg(windows)]
    pub fn create() -> Result<Self, String> {
        Self::create_named(NAME)
    }

    #[cfg(windows)]
    fn create_named(name: &str) -> Result<Self, String> {
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Memory::{CreateFileMappingW, MapViewOfFile, FILE_MAP_ALL_ACCESS, PAGE_READWRITE};
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        unsafe {
            let h = CreateFileMappingW(INVALID_HANDLE_VALUE, std::ptr::null(), PAGE_READWRITE, 0, SHM_SIZE as u32, name.as_ptr());
            if h.is_null() {
                return Err("não consegui criar a memória do SDK".into());
            }
            let existed = GetLastError() == ERROR_ALREADY_EXISTS;
            let view = MapViewOfFile(h, FILE_MAP_ALL_ACCESS, 0, 0, SHM_SIZE);
            let Some(ptr) = NonNull::new(view.Value as *mut Shm) else {
                CloseHandle(h);
                return Err("não consegui mapear a memória do SDK".into());
            };
            let shm = Self { ptr, owner: Owner::Mapping(h) };
            if existed {
                if shm.hub_alive() {
                    return Err("outro Hub já está aberto".into()); // o Drop solta a memória
                }
                shm.repair();
            }
            shm.init();
            Ok(shm)
        }
    }

    /// O heartbeat de outro Hub ainda anda? (o Hub bate a cada 100 ms)
    #[cfg(windows)]
    fn hub_alive(&self) -> bool {
        let beat = self.atomic_u32(|p| unsafe { addr_of_mut!((*p).hub_heartbeat) });
        let before = beat.load(Ordering::Acquire);
        std::thread::sleep(std::time::Duration::from_millis(350));
        beat.load(Ordering::Acquire) != before
    }

    /// Memória de um Hub que morreu: uma vaga pode ter ficado "escrevendo" (seq ímpar). Fecha a escrita para o
    /// jogo voltar a ler; o Hub novo regrava as vagas logo em seguida.
    #[cfg(windows)]
    fn repair(&self) {
        for i in 0..MAX_SEATS {
            let seq = unsafe { AtomicU32::from_ptr(addr_of_mut!((*self.as_ptr()).seats[i].seq)) };
            let s = seq.load(Ordering::Acquire);
            if s % 2 == 1 {
                seq.store(s.wrapping_add(1), Ordering::Release);
            }
        }
    }

    #[cfg(test)]
    fn heap() -> Self {
        let mut b = Box::new(std::mem::MaybeUninit::<Shm>::zeroed());
        let ptr = NonNull::new(b.as_mut_ptr()).unwrap();
        let shm = Self { ptr, owner: Owner::Heap(b) };
        shm.init();
        shm
    }

    pub fn as_ptr(&self) -> *mut Shm {
        self.ptr.as_ptr()
    }

    fn init(&self) {
        let p = self.as_ptr();
        unsafe {
            addr_of_mut!((*p).magic).write_volatile(MAGIC);
            addr_of_mut!((*p).abi).write_volatile(ABI);
            addr_of_mut!((*p).size).write_volatile(SHM_SIZE as u32);
        }
    }

    fn atomic_u32(&self, f: impl FnOnce(*mut Shm) -> *mut u32) -> &AtomicU32 {
        unsafe { AtomicU32::from_ptr(f(self.as_ptr())) }
    }

    /// O Hub está vivo (o jogo detecta o Hub fechado quando isto para).
    pub fn heartbeat(&self) {
        self.atomic_u32(|p| unsafe { addr_of_mut!((*p).hub_heartbeat) }).fetch_add(1, Ordering::Release);
    }

    pub fn set_join_url(&self, url: &str) {
        let p = self.as_ptr();
        let mut buf = [0u8; 128];
        copy_text(&mut buf, url);
        unsafe { addr_of_mut!((*p).join_url).write_volatile(buf) };
    }

    /// Vaga inteira (estado, aparelho, cor, nome), mantendo o controle atual.
    pub fn write_seat(&self, i: usize, state: SeatState, device: Device, rgb: [u8; 3], name: &str) {
        self.with_seat(i, |s| {
            s.state = state as u8;
            s.device = device as u8;
            s.rgb = rgb;
            s.name = [0; 32];
            copy_text(&mut s.name, name);
            if state != SeatState::Active {
                s.buttons = 0;
                s.axes = [0.0; 6];
            }
        });
    }

    /// Caminho da entrada: só memória, sem alocar.
    pub fn write_pad(&self, i: usize, pad: Pad) {
        self.with_seat(i, |s| {
            s.buttons = pad.buttons;
            s.axes = [pad.lx, pad.ly, pad.rx, pad.ry, pad.lt, pad.rt];
        });
    }

    fn with_seat(&self, i: usize, f: impl FnOnce(&mut ShmSeat)) {
        if i >= MAX_SEATS {
            return;
        }
        let seat = unsafe { addr_of_mut!((*self.as_ptr()).seats[i]) };
        let seq = unsafe { AtomicU32::from_ptr(addr_of_mut!((*seat).seq)) };
        let s = seq.load(Ordering::Relaxed);
        seq.store(s.wrapping_add(1), Ordering::Relaxed); // ímpar: escrevendo
        fence(Ordering::SeqCst);
        unsafe {
            let mut copy = seat.read_volatile();
            f(&mut copy);
            copy.seq = s.wrapping_add(1);
            seat.write_volatile(copy);
        }
        fence(Ordering::SeqCst);
        seq.store(s.wrapping_add(2), Ordering::Release);
    }

    /// Pid do jogo conectado (0 = nenhum).
    pub fn game_pid(&self) -> i32 {
        unsafe { AtomicI32::from_ptr(addr_of_mut!((*self.as_ptr()).game_pid)) }.load(Ordering::Acquire)
    }

    /// O que o jogo conectado usa (`GAME_CAP_*`).
    pub fn game_caps(&self) -> u32 {
        self.atomic_u32(|p| unsafe { addr_of_mut!((*p).game_caps) }).load(Ordering::Acquire)
    }

    /// Sem jogo: nada do que o último usava vale mais.
    pub fn clear_game_caps(&self) {
        self.atomic_u32(|p| unsafe { addr_of_mut!((*p).game_caps) }).store(0, Ordering::Release);
    }

    /// Soma um deslocamento da câmera (graus) ao da vaga. Caminho da entrada: só atômicos.
    pub fn add_look(&self, i: usize, dx_deg: f32, dy_deg: f32) {
        if i >= MAX_SEATS || !dx_deg.is_finite() || !dy_deg.is_finite() {
            return;
        }
        let p = self.as_ptr();
        for (axis, v) in [(0, dx_deg), (1, dy_deg)] {
            let milli = (v * 1000.0).clamp(-1.0e6, 1.0e6) as i32;
            unsafe { AtomicI32::from_ptr(addr_of_mut!((*p).look[i][axis])) }.fetch_add(milli, Ordering::AcqRel);
        }
    }

    /// O jogo morreu sem desconectar: libera para o próximo.
    pub fn release_game(&self, pid: i32) {
        let _ = unsafe { AtomicI32::from_ptr(addr_of_mut!((*self.as_ptr()).game_pid)) }.compare_exchange(
            pid,
            0,
            Ordering::AcqRel,
            Ordering::Relaxed,
        );
    }

    /// Lê os comandos novos do jogo (fora do caminho da entrada; aqui pode alocar).
    pub fn drain_commands(&self) -> Vec<Command> {
        let write = self.atomic_u32(|p| unsafe { addr_of_mut!((*p).cmd_write) }).load(Ordering::Acquire);
        let read_ctr = self.atomic_u32(|p| unsafe { addr_of_mut!((*p).cmd_read) });
        let mut read = read_ctr.load(Ordering::Relaxed);
        let mut out = Vec::new();
        // jogo que escreveu além do anel (bug ou corrupção): pula para o que ainda está inteiro
        if write.wrapping_sub(read) as usize > CMDS {
            read = write.wrapping_sub(CMDS as u32);
        }
        while read != write {
            let c = unsafe { addr_of!((*self.as_ptr()).cmds[read as usize % CMDS]).read_volatile() };
            let seat = (c.seat >= 0 && (c.seat as usize) < MAX_SEATS).then_some(c.seat as usize);
            let text = text_of(&c.text);
            match c.kind {
                1 => out.push(Command::Rumble { seat, low: c.low.clamp(0.0, 1.0), high: c.high.clamp(0.0, 1.0), ms: c.duration_ms.min(5000) }),
                2 => out.push(Command::Toast { seat, text }),
                3 => out.push(Command::Accepting(c.value != 0)),
                4 => out.push(Command::Label { seat, button: c.value, text }),
                5 => out.push(Command::Layout { seat, layout: c.value }),
                6 => out.push(Command::Light { seat, rgb: c.value }),
                7 => out.push(Command::Mode { seat, mode: c.value }),
                _ => {}
            }
            read = read.wrapping_add(1);
        }
        read_ctr.store(read, Ordering::Release);
        out
    }
}

impl Drop for HubShm {
    fn drop(&mut self) {
        match self.owner {
            #[cfg(test)]
            Owner::Heap(_) => {}
            #[cfg(windows)]
            Owner::Mapping(h) => unsafe {
                use windows_sys::Win32::System::Memory::{UnmapViewOfFile, MEMORY_MAPPED_VIEW_ADDRESS};
                // o jogo vê o Hub sumir pelo heartbeat parado e pela memória sem dono
                UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: self.as_ptr() as *mut _ });
                windows_sys::Win32::Foundation::CloseHandle(h);
            },
        }
    }
}

/// O endereço do controle do celular do Hub aberto (o mesmo do QR), lido da memória do SDK. `None` sem Hub.
/// Para ferramentas (o `pa phone-sim`), não para jogos.
#[cfg(windows)]
pub fn hub_join_url() -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Memory::{OpenFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS};
    let name: Vec<u16> = NAME.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let h = OpenFileMappingW(FILE_MAP_READ, 0, name.as_ptr());
        if h.is_null() {
            return None;
        }
        let view = MapViewOfFile(h, FILE_MAP_READ, 0, 0, SHM_SIZE_V1);
        let p = view.Value as *const Shm;
        let url = if p.is_null() || addr_of!((*p).magic).read_volatile() != MAGIC {
            None
        } else {
            Some(text_of(&addr_of!((*p).join_url).read_volatile())).filter(|u| !u.is_empty())
        };
        if !p.is_null() {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: view.Value });
        }
        CloseHandle(h);
        url
    }
}

/// O processo do jogo ainda existe?
#[cfg(windows)]
pub fn process_alive(pid: i32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid as u32);
        if h.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code) != 0;
        CloseHandle(h);
        ok && code == STILL_ACTIVE as u32
    }
}

fn copy_text(dst: &mut [u8], src: &str) {
    // corta num limite de caractere UTF-8, deixando o zero final
    let mut n = src.len().min(dst.len() - 1);
    while !src.is_char_boundary(n) {
        n -= 1;
    }
    dst[..n].copy_from_slice(&src.as_bytes()[..n]);
    dst[n..].fill(0);
}

fn text_of(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

#[cfg(all(test, windows))]
mod tests {
    use std::ffi::{c_char, c_int, c_void, CStr};

    use pa_core::buttons;

    use super::*;

    // o SDK em C, compilado pelo build.rs a partir do party_anywhere.h
    #[repr(C)]
    #[derive(Default, Debug)]
    struct PaPad {
        buttons: u32,
        pressed: u32,
        released: u32,
        axes: [f32; 6],
    }
    #[repr(C)]
    #[derive(Default, Debug, Clone, Copy)]
    struct PaEvent {
        kind: c_int,
        seat: c_int,
    }
    #[repr(C)]
    struct PaSeatInfo {
        index: u8,
        state: c_int,
        device: c_int,
        rgb: [u8; 3],
        name: [c_char; 32],
    }
    extern "C" {
        fn pa__attach(shm: *mut c_void, pid: i32, out: *mut *mut c_void) -> c_int;
        fn pa_disconnect(pa: *mut c_void);
        fn pa_poll(pa: *mut c_void);
        fn pa_next_event(pa: *mut c_void, out: *mut PaEvent) -> bool;
        fn pa_get_pad(pa: *const c_void, seat: c_int, out: *mut PaPad) -> bool;
        fn pa_get_seat(pa: *const c_void, seat: c_int, out: *mut PaSeatInfo) -> bool;
        fn pa_active_seat_count(pa: *const c_void) -> c_int;
        fn pa_join_url(pa: *const c_void) -> *const c_char;
        fn pa_rumble(pa: *mut c_void, seat: c_int, low: f32, high: f32, ms: c_int);
        fn pa_toast(pa: *mut c_void, seat: c_int, text: *const c_char);
        fn pa_set_accepting_players(pa: *mut c_void, accepting: bool);
        fn pa_set_label(pa: *mut c_void, seat: c_int, button: u32, label: *const c_char);
        fn pa_set_layout(pa: *mut c_void, seat: c_int, layout: c_int);
        fn pa_set_mode(pa: *mut c_void, seat: c_int, mode: c_int);
        fn pa_take_look(pa: *mut c_void, seat: c_int, dx: *mut f32, dy: *mut f32) -> bool;
    }

    struct Game(*mut c_void);
    impl Game {
        fn attach(hub: &HubShm, pid: i32) -> Result<Self, c_int> {
            let mut out = std::ptr::null_mut();
            match unsafe { pa__attach(hub.as_ptr() as *mut c_void, pid, &mut out) } {
                0 => Ok(Game(out)),
                e => Err(e),
            }
        }
        fn events(&self) -> Vec<(c_int, c_int)> {
            unsafe { pa_poll(self.0) };
            let mut v = vec![];
            let mut e = PaEvent::default();
            while unsafe { pa_next_event(self.0, &mut e) } {
                v.push((e.kind, e.seat));
            }
            v
        }
        fn pad(&self, seat: c_int) -> Option<PaPad> {
            let mut p = PaPad::default();
            unsafe { pa_get_pad(self.0, seat, &mut p) }.then_some(p)
        }
    }
    impl Drop for Game {
        fn drop(&mut self) {
            unsafe { pa_disconnect(self.0) }
        }
    }

    const JOINED: c_int = 1;
    const RECONNECTING: c_int = 2;
    const RESUMED: c_int = 3;
    const LEFT: c_int = 4;
    const RENAMED: c_int = 5;

    #[test]
    fn c_game_sees_seats_pads_and_events_written_by_the_hub() {
        let hub = HubShm::heap();
        hub.set_join_url("http://192.168.0.12:27027/");
        hub.heartbeat();
        let game = Game::attach(&hub, 4242).unwrap();
        assert_eq!(hub.game_pid(), 4242);
        assert_eq!(unsafe { CStr::from_ptr(pa_join_url(game.0)) }.to_str().unwrap(), "http://192.168.0.12:27027/");

        hub.write_seat(2, SeatState::Active, Device::Phone, [0x22, 0xc5, 0x5e], "João");
        hub.write_pad(2, Pad { buttons: buttons::A, lx: 0.5, ..Pad::NEUTRAL });
        hub.heartbeat();
        assert_eq!(game.events(), vec![(JOINED, 2)]);
        let p = game.pad(2).unwrap();
        assert_eq!((p.buttons, p.pressed, p.axes[0]), (buttons::A, buttons::A, 0.5));
        let mut info: PaSeatInfo = unsafe { std::mem::zeroed() };
        assert!(unsafe { pa_get_seat(game.0, 2, &mut info) });
        assert_eq!(unsafe { CStr::from_ptr(info.name.as_ptr()) }.to_str().unwrap(), "João");
        assert_eq!((info.device, info.rgb), (1, [0x22, 0xc5, 0x5e]));
        assert_eq!(unsafe { pa_active_seat_count(game.0) }, 1);

        // segurou o A: não é "apertou" de novo; soltou: "released"
        hub.heartbeat();
        game.events();
        assert_eq!(game.pad(2).unwrap().pressed, 0);
        hub.write_pad(2, Pad::NEUTRAL);
        game.events();
        assert_eq!(game.pad(2).unwrap().released, buttons::A);

        // caiu: evento, e o jogo recebe controle neutro (nunca o último estado preso)
        hub.write_pad(2, Pad { buttons: buttons::B, ..Pad::NEUTRAL });
        hub.write_seat(2, SeatState::Reconnecting, Device::Phone, [0x22, 0xc5, 0x5e], "João");
        hub.heartbeat();
        assert_eq!(game.events(), vec![(RECONNECTING, 2)]);
        assert!(game.pad(2).is_none());
        hub.write_seat(2, SeatState::Active, Device::Phone, [0x22, 0xc5, 0x5e], "João");
        hub.heartbeat();
        assert_eq!(game.events(), vec![(RESUMED, 2)]);
        hub.write_seat(2, SeatState::Active, Device::Phone, [0x22, 0xc5, 0x5e], "Jão");
        hub.heartbeat();
        assert_eq!(game.events(), vec![(RENAMED, 2)]);
        hub.write_seat(2, SeatState::Empty, Device::Phone, [0, 0, 0], "");
        hub.heartbeat();
        assert_eq!(game.events(), vec![(LEFT, 2)]);
    }

    #[test]
    fn only_one_game_at_a_time_and_dead_games_are_released() {
        let hub = HubShm::heap();
        let first = Game::attach(&hub, 100).unwrap();
        assert_eq!(Game::attach(&hub, 200).err(), Some(3), "PA_ERR_BUSY");
        drop(first);
        assert_eq!(hub.game_pid(), 0, "pa_disconnect libera");
        let _crashed = std::mem::ManuallyDrop::new(Game::attach(&hub, 300).unwrap());
        hub.release_game(300);
        assert!(Game::attach(&hub, 400).is_ok());
    }

    #[test]
    fn game_commands_reach_the_hub() {
        let hub = HubShm::heap();
        let game = Game::attach(&hub, 1).unwrap();
        let text = std::ffi::CString::new("Sua vez!").unwrap();
        unsafe {
            pa_rumble(game.0, 3, 0.4, 2.0, 250);
            pa_toast(game.0, -1, text.as_ptr());
            pa_set_accepting_players(game.0, true);
            pa_set_accepting_players(game.0, true); // repetido: sem tráfego
            let drift = std::ffi::CString::new("Drift").unwrap();
            pa_set_label(game.0, 0, 1 << 17, drift.as_ptr()); // PA_LABEL_RT
            pa_set_layout(game.0, -1, 1);
            pa_set_mode(game.0, 2, 1);
            pa_set_mode(game.0, 2, -1); // inválido: nada
        }
        assert_eq!(
            hub.drain_commands(),
            vec![
                Command::Rumble { seat: Some(3), low: 0.4, high: 1.0, ms: 250 },
                Command::Toast { seat: None, text: "Sua vez!".into() },
                Command::Accepting(true),
                Command::Label { seat: Some(0), button: 1 << 17, text: "Drift".into() },
                Command::Layout { seat: None, layout: 1 },
                Command::Mode { seat: Some(2), mode: 1 },
            ]
        );
        assert!(hub.drain_commands().is_empty());
    }

    #[test]
    fn look_is_summed_by_the_hub_and_taken_by_the_game() {
        let hub = HubShm::heap();
        let game = Game::attach(&hub, 1).unwrap();
        let take = |seat| {
            let (mut x, mut y) = (0.0f32, 0.0f32);
            let moved = unsafe { pa_take_look(game.0, seat, &mut x, &mut y) };
            (moved, x, y)
        };
        assert_eq!(hub.game_caps(), 0);
        assert_eq!(take(1), (false, 0.0, 0.0));
        assert_eq!(hub.game_caps(), GAME_CAP_LOOK, "ler a câmera avisa o Hub");
        hub.add_look(1, 1.5, -0.25);
        hub.add_look(1, 0.5, 0.0);
        hub.add_look(2, f32::NAN, 1.0); // lixo não entra
        assert_eq!(take(1), (true, 2.0, -0.25));
        assert_eq!(take(1), (false, 0.0, 0.0), "pegou, zerou");
        assert_eq!(take(2), (false, 0.0, 0.0));
        hub.clear_game_caps();
        assert_eq!(hub.game_caps(), 0);
    }

    #[test]
    fn a_hub_without_look_still_serves_new_games() {
        let hub = HubShm::heap();
        unsafe { (*hub.as_ptr()).size = SHM_SIZE_V1 as u32 };
        let game = Game::attach(&hub, 1).expect("Hub da primeira ABI 1");
        hub.add_look(0, 3.0, 3.0);
        let (mut x, mut y) = (9.0f32, 9.0f32);
        assert!(!unsafe { pa_take_look(game.0, 0, &mut x, &mut y) }, "sem a câmera nova: nada");
        assert_eq!((x, y), (0.0, 0.0));
        assert_eq!(hub.game_caps(), 0);
    }

    #[test]
    fn version_and_magic_are_checked() {
        let hub = HubShm::heap();
        unsafe { (*hub.as_ptr()).abi = 2 };
        assert_eq!(Game::attach(&hub, 1).err(), Some(2), "PA_ERR_VERSION");
        unsafe { (*hub.as_ptr()).magic = 0 };
        assert_eq!(Game::attach(&hub, 1).err(), Some(1), "sem Hub");
    }

    #[test]
    fn hub_closing_is_noticed_and_pads_go_neutral() {
        let hub = HubShm::heap();
        let game = Game::attach(&hub, 1).unwrap();
        hub.write_seat(0, SeatState::Active, Device::Gamepad, [0x3b, 0x82, 0xf6], "Ana");
        hub.write_pad(0, Pad { buttons: buttons::A, ..Pad::NEUTRAL });
        hub.heartbeat();
        game.events();
        assert!(game.pad(0).is_some());
        std::thread::sleep(std::time::Duration::from_millis(1700)); // heartbeat parado
        assert_eq!(game.events(), vec![(6, -1)]);
        assert!(game.pad(0).is_none());
    }

    #[test]
    fn a_hub_killed_mid_write_never_freezes_the_game() {
        let hub = HubShm::heap();
        let game = Game::attach(&hub, 1).unwrap();
        hub.write_seat(0, SeatState::Active, Device::Gamepad, [0x3b, 0x82, 0xf6], "Ana");
        hub.write_pad(0, Pad { buttons: buttons::A, ..Pad::NEUTRAL });
        hub.heartbeat();
        game.events();
        // o Hub "morre" no meio de uma escrita: a vaga fica com seq ímpar para sempre
        let seq = unsafe { AtomicU32::from_ptr(addr_of_mut!((*hub.as_ptr()).seats[0].seq)) };
        seq.fetch_add(1, Ordering::Release);
        let t = std::time::Instant::now();
        assert!(game.pad(0).is_some(), "fica a leitura anterior");
        assert!(t.elapsed() < std::time::Duration::from_millis(500), "pa_poll não pode travar");
    }

    #[cfg(windows)]
    #[test]
    fn a_new_hub_takes_over_memory_left_by_a_closed_hub_but_not_a_live_one() {
        let name = r"Local\PartyAnywhereTest.takeover";
        let old = HubShm::create_named(name).unwrap(); // faz o papel do jogo segurando a memória
        old.write_seat(0, SeatState::Active, Device::Gamepad, [1, 2, 3], "Ana");
        let seq = unsafe { AtomicU32::from_ptr(addr_of_mut!((*old.as_ptr()).seats[0].seq)) };
        seq.fetch_add(1, Ordering::Release); // morreu escrevendo
        let new = HubShm::create_named(name).expect("memória sem Hub vivo é assumida");
        assert_eq!(seq.load(Ordering::Acquire) % 2, 0, "a vaga volta a ser legível");

        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let beat = {
            let stop = stop.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    new.heartbeat();
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                drop(new);
            })
        };
        let beating = HubShm::create_named(name).is_err();
        stop.store(true, Ordering::Relaxed);
        beat.join().unwrap();
        assert!(beating, "com o Hub vivo, o segundo não abre");
    }

    #[test]
    fn long_names_are_cut_on_a_char_boundary() {
        let mut buf = [0u8; 8];
        copy_text(&mut buf, "ãããã");
        assert_eq!(text_of(&buf), "ããã");
    }
}
