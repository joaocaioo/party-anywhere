//! A tela do jogo no celular, lado do Hub: uma memória por vaga ("Local\PartyAnywhere.v1.view.<vaga>"),
//! criada quando o celular liga a tela. O Hub marca `wanted`; o jogo (`pa_submit_view`) escreve a imagem
//! daquele jogador, BGRA, com seqlock; o Hub lê o quadro mais novo quando o celular está pronto para outro.
//! O mesmo layout do `pa_view_header` em sdk/c/party_anywhere.h.

use std::sync::atomic::{AtomicU32, Ordering};

pub const VIEW_PREFIX: &str = r"Local\PartyAnywhere.v1.view.";
pub const VIEW_MAGIC: u32 = 0x5745_4956; // "VIEW"
pub const VIEW_MAX_W: u32 = 1280;
pub const VIEW_MAX_H: u32 = 720;
pub const VIEW_HEADER: usize = 64;
pub const VIEW_SIZE: usize = VIEW_HEADER + (VIEW_MAX_W * VIEW_MAX_H * 4) as usize;

#[repr(C)]
struct Header {
    magic: u32,
    version: u32,
    wanted: u32,
    width: u32,
    height: u32,
    stride: u32,
    seq: u32,
    reserved: u32,
    timestamp_us: u64,
    pad: [u8; 24],
}
const _: () = assert!(std::mem::size_of::<Header>() == VIEW_HEADER);

/// Um quadro lido: tamanho e o instante (QPC, microssegundos) em que o jogo terminou de escrever.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub seq: u32,
    pub timestamp_us: u64,
}

pub struct ViewShm {
    ptr: *mut u8,
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

// compartilhada por desenho; o acesso é coordenado pelo seqlock
unsafe impl Send for ViewShm {}

impl ViewShm {
    /// Cria (ou reabre, se o jogo ainda segura a anterior) a memória da tela da vaga.
    #[cfg(windows)]
    pub fn create(seat: usize) -> Result<Self, String> {
        Self::create_named(&format!("{VIEW_PREFIX}{seat}"))
    }

    #[cfg(windows)]
    pub fn create_named(name: &str) -> Result<Self, String> {
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows_sys::Win32::System::Memory::{CreateFileMappingW, MapViewOfFile, FILE_MAP_ALL_ACCESS, PAGE_READWRITE};
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        unsafe {
            let h = CreateFileMappingW(INVALID_HANDLE_VALUE, std::ptr::null(), PAGE_READWRITE, 0, VIEW_SIZE as u32, wide.as_ptr());
            if h.is_null() {
                return Err("não consegui criar a memória da tela".into());
            }
            let v = MapViewOfFile(h, FILE_MAP_ALL_ACCESS, 0, 0, VIEW_SIZE);
            if v.Value.is_null() {
                windows_sys::Win32::Foundation::CloseHandle(h);
                return Err("não consegui mapear a memória da tela".into());
            }
            let view = Self { ptr: v.Value as *mut u8, handle: h };
            let hd = view.header();
            std::ptr::addr_of_mut!((*hd).magic).write_volatile(VIEW_MAGIC);
            std::ptr::addr_of_mut!((*hd).version).write_volatile(1);
            Ok(view)
        }
    }

    fn header(&self) -> *mut Header {
        self.ptr as *mut Header
    }

    fn atomic(&self, f: impl FnOnce(*mut Header) -> *mut u32) -> &AtomicU32 {
        unsafe { AtomicU32::from_ptr(f(self.header())) }
    }

    /// O celular quer (ou não) a tela deste jogador: o jogo lê com `pa_view_wanted`.
    pub fn set_wanted(&self, on: bool) {
        self.atomic(|h| unsafe { std::ptr::addr_of_mut!((*h).wanted) }).store(on as u32, Ordering::Release);
    }

    /// O quadro mais novo, se mudou desde `last_seq`: os pixels BGRA (`width * height * 4`) vão para `out`.
    pub fn read(&self, last_seq: u32, out: &mut Vec<u8>) -> Option<Frame> {
        let seq = self.atomic(|h| unsafe { std::ptr::addr_of_mut!((*h).seq) });
        for _ in 0..3 {
            let s0 = seq.load(Ordering::Acquire);
            if s0 % 2 == 1 || s0 == last_seq || s0 == 0 {
                if s0 % 2 == 1 {
                    std::hint::spin_loop();
                    continue;
                }
                return None;
            }
            let (w, h, stride, ts) = unsafe {
                let hd = self.header();
                (
                    std::ptr::addr_of!((*hd).width).read_volatile(),
                    std::ptr::addr_of!((*hd).height).read_volatile(),
                    std::ptr::addr_of!((*hd).stride).read_volatile(),
                    std::ptr::addr_of!((*hd).timestamp_us).read_volatile(),
                )
            };
            if w == 0 || h == 0 || w > VIEW_MAX_W || h > VIEW_MAX_H || stride != w * 4 {
                return None;
            }
            let len = (stride * h) as usize;
            out.resize(len, 0);
            unsafe { std::ptr::copy_nonoverlapping(self.ptr.add(VIEW_HEADER), out.as_mut_ptr(), len) };
            if seq.load(Ordering::Acquire) == s0 {
                return Some(Frame { width: w, height: h, seq: s0, timestamp_us: ts });
            }
        }
        None
    }
}

impl Drop for ViewShm {
    fn drop(&mut self) {
        self.set_wanted(false);
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::System::Memory::{UnmapViewOfFile, MEMORY_MAPPED_VIEW_ADDRESS};
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: self.ptr as *mut _ });
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

/// O relógio do `timestamp_us` (QueryPerformanceCounter em microssegundos): a idade de um quadro.
#[cfg(windows)]
pub fn qpc_us() -> u64 {
    use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    let (mut t, mut f) = (0i64, 0i64);
    unsafe {
        QueryPerformanceCounter(&mut t);
        QueryPerformanceFrequency(&mut f);
    }
    let (t, f) = (t as u64, f.max(1) as u64);
    t / f * 1_000_000 + t % f * 1_000_000 / f
}

#[cfg(all(test, windows))]
mod tests {
    use std::ffi::{c_int, c_void};

    use super::*;
    use crate::{Device, HubShm, SeatState};

    extern "C" {
        fn pa__attach(shm: *mut c_void, pid: i32, out: *mut *mut c_void) -> c_int;
        fn pa_disconnect(pa: *mut c_void);
        fn pa_poll(pa: *mut c_void);
        fn pa_view_wanted(pa: *mut c_void, seat: c_int) -> bool;
        fn pa_submit_view(pa: *mut c_void, seat: c_int, bgra: *const c_void, w: c_int, h: c_int, stride: c_int) -> bool;
    }

    #[test]
    fn the_c_game_hands_the_players_view_to_the_hub() {
        // shim.c usa o prefixo de teste: não esbarra no Hub de verdade aberto no PC
        let hub = HubShm::heap();
        hub.heartbeat();
        let mut game = std::ptr::null_mut();
        assert_eq!(unsafe { pa__attach(hub.as_ptr() as *mut c_void, 77, &mut game) }, 0);
        hub.write_seat(3, SeatState::Active, Device::Phone, [1, 2, 3], "Ana");
        unsafe { pa_poll(game) };

        // 4 px por linha, mas a linha da origem tem 6 px (stride maior que a largura, como num alvo de render)
        let pixels: Vec<u8> = (0..(6 * 4 * 2)).map(|i| i as u8).collect();
        assert!(!unsafe { pa_view_wanted(game, 3) }, "sem a memória da tela: a TV continua como sempre");

        let view = ViewShm::create_named(r"Local\PartyAnywhereTest.view.3").unwrap();
        view.set_wanted(true);
        // o SDK procura a memória de novo a cada ~1 s
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert!(unsafe { pa_view_wanted(game, 3) });
        assert!(unsafe { pa_submit_view(game, 3, pixels.as_ptr() as *const c_void, 4, 2, 6 * 4) });

        let mut out = Vec::new();
        let f = view.read(0, &mut out).expect("um quadro");
        assert_eq!((f.width, f.height), (4, 2));
        assert_eq!(&out[..16], &pixels[..16], "primeira linha");
        assert_eq!(&out[16..32], &pixels[24..40], "segunda linha, respeitando o stride");
        assert!(qpc_us() >= f.timestamp_us && qpc_us() - f.timestamp_us < 5_000_000);
        assert!(view.read(f.seq, &mut out).is_none(), "nada novo");

        view.set_wanted(false);
        assert!(!unsafe { pa_view_wanted(game, 3) });
        unsafe { pa_disconnect(game) };
    }
}
