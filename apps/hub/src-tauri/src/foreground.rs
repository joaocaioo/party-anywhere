//! O jogo vem para a frente quando abre.
//!
//! O Windows não deixa uma janela nova "roubar" a frente de quem está nela (aqui, o próprio Hub, onde a pessoa
//! acabou de apertar Jogar): o jogo abria atrás do Hub, e a pessoa ficava olhando a tela "O jogo abriu". Ao abrir,
//! o Hub libera a frente para a próxima janela (`AllowSetForegroundWindow`) e, por um minuto, procura a janela do
//! jogo e a traz para a frente, uma vez. Depois disso não insiste: se a pessoa voltar ao Hub, é escolha dela.
//!
//! Qual é a janela do jogo: a do executável que o Hub abriu (mods e jogos por `.exe`); aberto pela Steam ou pela
//! Epic (um link), a primeira janela grande nova que não é da loja nem do Windows.

use std::collections::HashSet;
use std::time::{Duration, Instant};

const LOOK_FOR: Duration = Duration::from_secs(60);
const EVERY: Duration = Duration::from_millis(250);
/// Janela de jogo, não um aviso ou um ícone: pelo menos isto de área (640x360).
const MIN_AREA: i64 = 640 * 360;
/// Processos que abrem janelas no caminho, mas não são o jogo.
const NOT_THE_GAME: [&str; 9] = [
    "steam.exe",
    "steamwebhelper.exe",
    "epicgameslauncher.exe",
    "epicwebhelper.exe",
    "explorer.exe",
    "unitycrashhandler64.exe",
    "unitycrashhandler32.exe",
    "msedgewebview2.exe",
    "applicationframehost.exe",
];

/// O que procurar.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// O nome do executável (`MicroWorks.exe`), sem diferença de maiúsculas.
    Exe(String),
    /// Aberto por um link (Steam, Epic): a primeira janela grande nova de outro programa.
    AnyNew,
}

/// Uma janela de primeiro nível, visível.
#[derive(Clone, Debug)]
pub struct Win {
    pub hwnd: isize,
    pub pid: u32,
    /// Nome do executável, minúsculo.
    pub exe: String,
    pub area: i64,
}

/// A janela do jogo entre as que existem agora (lógica pura, testada).
pub fn pick(wins: &[Win], target: &Target, before: &HashSet<isize>, own_pid: u32) -> Option<isize> {
    wins.iter()
        .filter(|w| w.pid != own_pid && w.area >= MIN_AREA)
        .find(|w| match target {
            Target::Exe(name) => w.exe.eq_ignore_ascii_case(name),
            Target::AnyNew => !before.contains(&w.hwnd) && !NOT_THE_GAME.contains(&w.exe.as_str()),
        })
        .map(|w| w.hwnd)
}

/// Chamado logo antes de abrir o jogo: libera a frente e procura a janela dele numa thread.
pub fn launching(target: Target) {
    let before: HashSet<isize> = sys::windows().iter().map(|w| w.hwnd).collect();
    sys::allow_any();
    std::thread::Builder::new()
        .name("pa-foreground".into())
        .spawn(move || {
            let start = Instant::now();
            let own = std::process::id();
            while start.elapsed() < LOOK_FOR {
                std::thread::sleep(EVERY);
                let Some(hwnd) = pick(&sys::windows(), &target, &before, own) else { continue };
                // achou: para a frente (algumas tentativas, o jogo pode estar terminando de criar a janela)
                for _ in 0..8 {
                    if sys::bring(hwnd) {
                        return;
                    }
                    std::thread::sleep(EVERY);
                }
                return;
            }
        })
        .ok();
}

#[cfg(windows)]
mod sys {
    use super::Win;
    use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, RECT};
    use windows_sys::Win32::System::Threading::{
        AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowRect,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow, ASFW_ANY, GWL_EXSTYLE, GW_OWNER,
        SW_RESTORE, WS_EX_TOOLWINDOW,
    };

    pub fn allow_any() {
        // SAFETY: só muda a permissão de quem pode ir para a frente
        unsafe { AllowSetForegroundWindow(ASFW_ANY) };
    }

    /// As janelas de primeiro nível visíveis (sem as de ferramenta e as que têm dono).
    pub fn windows() -> Vec<Win> {
        let mut out: Vec<Win> = Vec::new();
        // SAFETY: o callback recebe o ponteiro do Vec, vivo durante a chamada síncrona do EnumWindows
        unsafe { EnumWindows(Some(collect), &mut out as *mut Vec<Win> as LPARAM) };
        out
    }

    unsafe extern "system" fn collect(hwnd: HWND, data: LPARAM) -> BOOL {
        let out = &mut *(data as *mut Vec<Win>);
        if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() || (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW) != 0 {
            return 1;
        }
        let mut r: RECT = std::mem::zeroed();
        GetWindowRect(hwnd, &mut r);
        let area = (r.right - r.left) as i64 * (r.bottom - r.top) as i64;
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        out.push(Win { hwnd: hwnd as isize, pid, exe: exe_of(pid), area });
        1
    }

    fn exe_of(pid: u32) -> String {
        // SAFETY: o handle é fechado logo depois; o texto é lido até o tamanho devolvido
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return String::new();
            }
            let mut buf = [0u16; 512];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0;
            CloseHandle(h);
            if !ok {
                return String::new();
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            path.rsplit('\\').next().unwrap_or_default().to_lowercase()
        }
    }

    /// Traz a janela para a frente. true se ela ficou na frente.
    pub fn bring(hwnd: isize) -> bool {
        let hwnd = hwnd as HWND;
        // SAFETY: chamadas de janela com um HWND que pode ter sumido (o Windows só devolve erro)
        unsafe {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            // quem está na frente (o Hub) empresta a vez: as duas threads de entrada juntas por um instante
            let fg = GetForegroundWindow();
            let fg_thread = GetWindowThreadProcessId(fg, std::ptr::null_mut());
            let me = GetCurrentThreadId();
            let joined = fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, 1) != 0;
            SetForegroundWindow(hwnd);
            BringWindowToTop(hwnd);
            if joined {
                AttachThreadInput(me, fg_thread, 0);
            }
            GetForegroundWindow() == hwnd
        }
    }
}

#[cfg(not(windows))]
mod sys {
    use super::Win;
    pub fn allow_any() {}
    pub fn windows() -> Vec<Win> {
        Vec::new()
    }
    pub fn bring(_: isize) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(hwnd: isize, pid: u32, exe: &str, area: i64) -> Win {
        Win { hwnd, pid, exe: exe.into(), area }
    }

    #[test]
    fn finds_the_game_window_and_skips_the_rest() {
        let own = 1;
        let wins = vec![
            w(10, own, "party-anywhere.exe", 2_000_000), // o próprio Hub
            w(11, 2, "steam.exe", 2_000_000),            // a loja
            w(12, 3, "microworks.exe", 100 * 100),       // aviso pequeno do jogo
            w(13, 3, "microworks.exe", 1920 * 1080),     // a janela do jogo
            w(14, 4, "chrome.exe", 1920 * 1080),         // já estava aberta
        ];
        let before: HashSet<isize> = [10, 11, 14].into();
        assert_eq!(pick(&wins, &Target::Exe("MicroWorks.exe".into()), &before, own), Some(13));
        assert_eq!(pick(&wins, &Target::AnyNew, &before, own), Some(13));
        // a loja abriu uma janela nova grande: não é o jogo
        let only_store = vec![w(20, 2, "steamwebhelper.exe", 2_000_000)];
        assert_eq!(pick(&only_store, &Target::AnyNew, &HashSet::new(), own), None);
        assert_eq!(pick(&wins, &Target::Exe("outro.exe".into()), &before, own), None);
    }
}
