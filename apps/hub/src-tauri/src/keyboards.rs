//! Teclados como jogadores (docs/ARCHITECTURE.md, "Teclados como jogadores"). Raw Input separa cada
//! teclado; o caminho do aparelho diz se é do notebook, USB ou Bluetooth. Teclado nunca entra sozinho:
//! só com Enter nele enquanto a tela "Adicionar jogador → Teclado" está aberta (ou "Conectar").

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use pa_core::{buttons, Device, DeviceKey, DeviceKind, JoinOutcome, Pad};
use serde::Serialize;
use tauri::Emitter;

use crate::hub::{DeviceMeta, Hub};

/// A tela de teclado está aberta: Enter num teclado faz ele entrar.
pub static LISTENING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Bus {
    Notebook,
    Usb,
    Bluetooth,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardView {
    pub key: String,
    pub device_name: String,
    pub via: &'static str,
    pub seat: Option<usize>,
}

/// Teclados conhecidos (chave do aparelho físico → nome e barramento).
static KNOWN: OnceLock<Mutex<HashMap<String, (String, Bus)>>> = OnceLock::new();

fn known() -> &'static Mutex<HashMap<String, (String, Bus)>> {
    KNOWN.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Um teclado físico aparece como várias coleções HID: `HID#VID_..&MI_00&Col01#8&2a1&0&0000#{guid}`,
/// `..&Col02#8&2a1&0&0001#{guid}`. Sem o `&ColNN`, sem o último pedaço da instância e sem a interface,
/// sobra a identidade do aparelho.
pub fn physical_id(path: &str) -> String {
    let p = path.to_ascii_uppercase();
    let parts: Vec<&str> = p.split('#').filter(|s| !s.starts_with('{')).collect();
    let mut out = Vec::with_capacity(parts.len());
    for (i, part) in parts.iter().enumerate() {
        let mut seg = part.to_string();
        if let Some(c) = seg.find("&COL") {
            let tail = &seg[c + 4..];
            let skip = tail.find(|ch: char| !ch.is_ascii_hexdigit()).unwrap_or(tail.len());
            seg = format!("{}{}", &seg[..c], &tail[skip..]);
        }
        if i == 2 {
            if let Some(last) = seg.rfind('&') {
                seg.truncate(last);
            }
        }
        out.push(seg);
    }
    out.join("#")
}

pub fn classify(path: &str) -> Bus {
    let p = path.to_ascii_uppercase();
    if p.contains("BTHENUM") || p.contains("BTHLEDEVICE") || p.contains("{00001124-0000-1000-8000-00805F9B34FB}") {
        Bus::Bluetooth
    } else if p.contains("VID_") {
        Bus::Usb
    } else {
        // ACPI (PS/2 do notebook), HID sobre I²C sem VID: o teclado de fábrica
        Bus::Notebook
    }
}

pub fn display_name(path: &str, bus: Bus) -> String {
    if bus == Bus::Notebook {
        return "Teclado do notebook".into();
    }
    let p = path.to_ascii_uppercase();
    let vid = p.split("VID_").nth(1).map(|v| v.chars().take(4).collect::<String>());
    let brand = match vid.as_deref() {
        Some("046D") => Some("Logitech"),
        Some("045E") => Some("Microsoft"),
        Some("1532") => Some("Razer"),
        Some("1B1C") => Some("Corsair"),
        Some("1038") => Some("SteelSeries"),
        Some("0951") | Some("03F0") => Some("HyperX"),
        Some("258A") | Some("0C45") => Some("Redragon"),
        Some("05AC") => Some("Apple"),
        Some("04F2") => Some("Chicony"),
        Some("413C") => Some("Dell"),
        _ => None,
    };
    match (brand, bus) {
        (Some(b), _) => format!("Teclado {b}"),
        (None, Bus::Bluetooth) => "Teclado Bluetooth".into(),
        _ => "Teclado USB".into(),
    }
}

pub fn via(bus: Bus) -> &'static str {
    match bus {
        Bus::Notebook => "Do notebook",
        Bus::Usb => "Com fio",
        Bus::Bluetooth => "Bluetooth",
    }
}

/// Teclas → controle de Xbox (docs/ARCHITECTURE.md): WASD analógico esquerdo, setas o direito,
/// Espaço A, Shift B, E X, Q Y, Enter Start, Tab Back, R/F bumpers, Z/C gatilhos.
pub fn pad_from_keys(down: &[bool; 256]) -> Pad {
    let k = |vk: usize| down[vk];
    let axis = |neg: usize, pos: usize| (k(pos) as i8 - k(neg) as i8) as f32;
    let mut b = 0;
    for (vk, bit) in [
        (0x20, buttons::A),
        (0x10, buttons::B),
        (0x45, buttons::X),
        (0x51, buttons::Y),
        (0x0D, buttons::START),
        (0x09, buttons::BACK),
        (0x52, buttons::LB),
        (0x46, buttons::RB),
    ] {
        if k(vk) {
            b |= bit;
        }
    }
    let (lx, ly) = (axis(0x41, 0x44), axis(0x53, 0x57));
    let (rx, ry) = (axis(0x25, 0x27), axis(0x28, 0x26));
    let norm = |x: f32, y: f32| {
        let m = (x * x + y * y).sqrt();
        if m > 1.0 {
            (x / m, y / m)
        } else {
            (x, y)
        }
    };
    let (lx, ly) = norm(lx, ly);
    let (rx, ry) = norm(rx, ry);
    Pad { buttons: b, lx, ly, rx, ry, lt: k(0x5A) as u8 as f32, rt: k(0x43) as u8 as f32 }
}

pub fn list(hub: &Hub) -> Vec<KeyboardView> {
    let lobby = hub.lobby.lock().unwrap();
    let mut out: Vec<KeyboardView> = known()
        .lock()
        .unwrap()
        .iter()
        .map(|(id, (name, bus))| {
            let key = DeviceKey(format!("kb:{id}"));
            KeyboardView { key: key.0.clone(), device_name: name.clone(), via: via(*bus), seat: lobby.seat_of_device(&key) }
        })
        .collect();
    out.sort_by_key(|k| (k.via != "Do notebook", k.device_name.clone()));
    out
}

/// "Conectar" na tela, ou Enter no teclado.
pub fn join(hub: &Hub, key: &str) -> Option<usize> {
    let key = DeviceKey(key.to_string());
    let outcome = hub.lobby.lock().unwrap().add(&key)?;
    let seat = match outcome {
        JoinOutcome::Joined(i) | JoinOutcome::Rejoined(i) => i,
        _ => return None,
    };
    hub.plug(seat);
    hub.emit_lobby();
    emit(hub);
    Some(seat)
}

fn emit(hub: &Hub) {
    let _ = hub.app.emit("keyboards", list(hub));
}

fn arrived(hub: &Hub, path: &str) -> String {
    let id = physical_id(path);
    let bus = classify(path);
    let name = display_name(path, bus);
    let fresh = known().lock().unwrap().insert(id.clone(), (name.clone(), bus)).is_none();
    let key = DeviceKey(format!("kb:{id}"));
    if fresh {
        hub.meta.lock().unwrap().insert(
            key.clone(),
            DeviceMeta { brand: "keyboard".into(), device_name: name.clone(), via: via(bus).into(), ..Default::default() },
        );
        hub.lobby.lock().unwrap().connected(Device { key, kind: DeviceKind::Keyboard, name: None });
        emit(hub);
    }
    id
}

fn removed(hub: &Hub, id: &str) {
    if known().lock().unwrap().remove(id).is_some() {
        let key = DeviceKey(format!("kb:{id}"));
        let seat = hub.lobby.lock().unwrap().seat_of_device(&key);
        hub.lobby.lock().unwrap().disconnected(&key, hub.now_ms());
        if let Some(s) = seat {
            hub.set_pad(s, Pad::NEUTRAL);
            hub.unplug(s);
        }
        hub.emit_lobby();
        emit(hub);
    }
}

/// Uma tecla de um teclado: atualiza a vaga dele, ou faz entrar se for Enter com a tela aberta.
fn key_event(hub: &Hub, id: &str, state: &mut [bool; 256], vk: u16, down: bool) {
    if (vk as usize) < 256 {
        state[vk as usize] = down;
    }
    let key = DeviceKey(format!("kb:{id}"));
    let seat = hub.lobby.lock().unwrap().seat_of_device(&key);
    match seat {
        Some(s) => hub.set_pad(s, pad_from_keys(state)),
        None if down && vk == 0x0D && LISTENING.load(Ordering::Relaxed) => {
            join(hub, &key.0);
        }
        None => {}
    }
}

#[cfg(windows)]
pub fn start(hub: Arc<Hub>) {
    std::thread::Builder::new().name("pa-keyboards".into()).spawn(move || win::run(hub)).expect("thread dos teclados");
}

#[cfg(not(windows))]
pub fn start(_hub: Arc<Hub>) {}

#[cfg(windows)]
mod win {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::{Arc, OnceLock};

    use windows_sys::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::{
        GetRawInputData, GetRawInputDeviceInfoW, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
        RAWINPUTHEADER, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDI_DEVICENAME, RID_INPUT, RIM_TYPEKEYBOARD,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW, HWND_MESSAGE, MSG, WM_INPUT,
        WM_INPUT_DEVICE_CHANGE, WNDCLASSW,
    };

    use super::*;

    const RI_KEY_BREAK: u16 = 1;
    const GIDC_ARRIVAL: usize = 1;
    const GIDC_REMOVAL: usize = 2;

    static HUB: OnceLock<Arc<Hub>> = OnceLock::new();

    /// hDevice → (id físico, teclas apertadas).
    type Devices = HashMap<isize, (String, Box<[bool; 256]>)>;

    thread_local! {
        /// Só esta thread mexe.
        static DEVICES: RefCell<Devices> = RefCell::new(HashMap::new());
    }

    pub fn run(hub: Arc<Hub>) {
        let _ = HUB.set(hub);
        unsafe {
            let class: Vec<u16> = "PartyAnywhereRawInput\0".encode_utf16().collect();
            let instance = GetModuleHandleW(std::ptr::null());
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return;
            }
            let rid = RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x06,
                dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                hwndTarget: hwnd,
            };
            if RegisterRawInputDevices(&rid, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32) == 0 {
                return;
            }
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                DispatchMessageW(&msg);
            }
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let Some(hub) = HUB.get() else { return DefWindowProcW(hwnd, msg, wparam, lparam) };
        match msg {
            WM_INPUT => on_input(hub, lparam as HRAWINPUT),
            WM_INPUT_DEVICE_CHANGE => {
                let device = lparam as HANDLE;
                if wparam == GIDC_ARRIVAL {
                    if let Some(path) = device_path(device) {
                        let id = arrived(hub, &path);
                        DEVICES.with(|d| d.borrow_mut().insert(device as isize, (id, Box::new([false; 256]))));
                    }
                } else if wparam == GIDC_REMOVAL {
                    let gone = DEVICES.with(|d| d.borrow_mut().remove(&(device as isize)));
                    if let Some((id, _)) = gone {
                        // outra coleção do mesmo teclado ainda ligada? então ele não saiu
                        let still = DEVICES.with(|d| d.borrow().values().any(|(other, _)| *other == id));
                        if !still {
                            removed(hub, &id);
                        }
                    }
                }
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    unsafe fn on_input(hub: &Hub, handle: HRAWINPUT) {
        let mut raw: RAWINPUT = std::mem::zeroed();
        let mut size = std::mem::size_of::<RAWINPUT>() as u32;
        let got = GetRawInputData(
            handle,
            RID_INPUT,
            &mut raw as *mut RAWINPUT as *mut _,
            &mut size,
            std::mem::size_of::<RAWINPUTHEADER>() as u32,
        );
        if got == u32::MAX || raw.header.dwType != RIM_TYPEKEYBOARD || raw.header.hDevice.is_null() {
            return; // tecla injetada por programa (sem aparelho): não é um jogador
        }
        let device = raw.header.hDevice;
        let kb = raw.data.keyboard;
        let down = kb.Flags & RI_KEY_BREAK == 0;
        let known = DEVICES.with(|d| d.borrow().contains_key(&(device as isize)));
        if !known {
            // teclado que já estava ligado antes do Hub abrir
            let Some(path) = device_path(device) else { return };
            let id = arrived(hub, &path);
            DEVICES.with(|d| d.borrow_mut().insert(device as isize, (id, Box::new([false; 256]))));
        }
        DEVICES.with(|d| {
            if let Some((id, state)) = d.borrow_mut().get_mut(&(device as isize)) {
                key_event(hub, id, state, kb.VKey, down);
            }
        });
    }

    unsafe fn device_path(device: HANDLE) -> Option<String> {
        let mut len = 0u32;
        GetRawInputDeviceInfoW(device, RIDI_DEVICENAME, std::ptr::null_mut(), &mut len);
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        let n = GetRawInputDeviceInfoW(device, RIDI_DEVICENAME, buf.as_mut_ptr() as *mut _, &mut len);
        if n == u32::MAX {
            return None;
        }
        Some(String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collections_of_one_keyboard_share_an_id() {
        let a = r"\\?\HID#VID_046D&PID_C52B&MI_00&Col01#8&2a1&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}";
        let b = r"\\?\HID#VID_046D&PID_C52B&MI_00&Col02#8&2a1&0&0001#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}";
        assert_eq!(physical_id(a), physical_id(b));
        assert_eq!(physical_id(a), r"\\?\HID#VID_046D&PID_C52B&MI_00#8&2A1&0");
        let other = r"\\?\HID#VID_046D&PID_C52B&MI_00&Col01#8&99f&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}";
        assert_ne!(physical_id(a), physical_id(other), "outro teclado igual tem outra instância");
    }

    #[test]
    fn tells_notebook_usb_and_bluetooth_apart() {
        let laptop = r"\\?\ACPI#MSF0001#4&2c5b1d02&0#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}";
        let usb = r"\\?\HID#VID_046D&PID_C52B&MI_00#8&2a1&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}";
        let bt = r"\\?\HID#{00001124-0000-1000-8000-00805f9b34fb}_VID&0002046d_PID&b342&Col01#9&1&0&0000#{884b96c3}";
        assert_eq!(classify(laptop), Bus::Notebook);
        assert_eq!(classify(usb), Bus::Usb);
        assert_eq!(classify(bt), Bus::Bluetooth);
        assert_eq!(display_name(laptop, Bus::Notebook), "Teclado do notebook");
        assert_eq!(display_name(usb, Bus::Usb), "Teclado Logitech");
    }

    #[test]
    fn wasd_and_arrows_become_sticks() {
        let mut down = [false; 256];
        down[0x57] = true; // W
        down[0x44] = true; // D
        down[0x20] = true; // Espaço
        let p = pad_from_keys(&down);
        assert!((p.lx - p.ly).abs() < 1e-6 && p.lx > 0.7 && p.lx < 0.71, "diagonal normalizada");
        assert_eq!(p.buttons, buttons::A);
        down = [false; 256];
        down[0x25] = true; // ←
        assert_eq!(pad_from_keys(&down).rx, -1.0);
    }
}
