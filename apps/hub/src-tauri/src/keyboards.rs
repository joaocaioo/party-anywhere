//! Teclados como jogadores. Raw Input separa cada
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

/// Um teclado físico que o Hub viu.
struct Known {
    name: String,
    bus: Bus,
    /// Alguém digitou nele desde que o Hub abriu. Só esses aparecem nas listas: mouses gamer e receptores
    /// sem fio também se apresentam como "teclado" ao Windows (teclas de macro), mas nunca digitam.
    typed: bool,
}

/// Teclados conhecidos (chave do aparelho físico → nome, barramento, se já digitou).
static KNOWN: OnceLock<Mutex<HashMap<String, Known>>> = OnceLock::new();

fn known() -> &'static Mutex<HashMap<String, Known>> {
    KNOWN.get_or_init(|| Mutex::new(HashMap::new()))
}

/// O que o Windows diz do aparelho físico: o ContainerId (todas as interfaces de um mesmo aparelho têm o
/// mesmo), se é embutido no PC e o nome que o próprio aparelho informa.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Identity {
    pub container: Option<String>,
    pub builtin: bool,
    pub name: Option<String>,
}

/// ContainerId que o Windows dá aos aparelhos embutidos no próprio PC (teclado do notebook, PS/2).
pub const BUILTIN_CONTAINER: &str = "00000000-0000-0000-ffff-ffffffffffff";

/// Nomes genéricos do Windows não ajudam a pessoa a achar o teclado: ficam de fora.
pub fn useful_name(name: &str) -> Option<String> {
    let n = name.trim();
    let lower = n.to_lowercase();
    let generic = ["hid", "usb input device", "dispositivo de entrada", "teclado padrão", "standard ps/2", "keyboard device"];
    (!n.is_empty() && !generic.iter().any(|g| lower.contains(g))).then(|| n.chars().take(32).collect())
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
        Some("05AC") => Some("Apple"),
        Some("413C") => Some("Dell"),
        // VIDs de fabricantes de chip (Sinowealth 258A, SONiX 0C45, Chicony 04F2...) são usados por dezenas
        // de marcas: dizer a marca a partir deles é inventar
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

/// Teclas → controle de Xbox: WASD analógico esquerdo, setas o direito,
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
        .filter_map(|(id, k)| {
            let key = DeviceKey(format!("kb:{id}"));
            let seat = lobby.seat_of_device(&key);
            (k.typed || seat.is_some()).then(|| KeyboardView { key: key.0.clone(), device_name: k.name.clone(), via: via(k.bus), seat })
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

fn arrived(hub: &Hub, path: &str, ident: &Identity) -> String {
    // o aparelho físico: o ContainerId do Windows; sem ele, o caminho sem coleção e sem interface
    let id = ident.container.clone().unwrap_or_else(|| physical_id(path));
    let bus = if ident.builtin { Bus::Notebook } else { classify(path) };
    let name = match (bus, &ident.name) {
        (Bus::Notebook, _) => "Teclado do notebook".to_string(),
        (_, Some(n)) => n.clone(),
        _ => display_name(path, bus),
    };
    let mut map = known().lock().unwrap();
    if map.contains_key(&id) {
        return id;
    }
    map.insert(id.clone(), Known { name: name.clone(), bus, typed: false });
    drop(map);
    let key = DeviceKey(format!("kb:{id}"));
    hub.meta.lock().unwrap().insert(
        key.clone(),
        DeviceMeta { brand: "keyboard".into(), device_name: name, via: via(bus).into(), ..Default::default() },
    );
    hub.lobby.lock().unwrap().connected(Device { key, kind: DeviceKind::Keyboard, name: None });
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
    // a primeira tecla de verdade: agora ele aparece nas listas
    if down {
        let first = known().lock().unwrap().get_mut(id).is_some_and(|k| !std::mem::replace(&mut k.typed, true));
        if first {
            emit(hub);
        }
    }
    let key = DeviceKey(format!("kb:{id}"));
    if down {
        hub.activity(&key);
    }
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
                        let id = arrived(hub, &path, &identity(&path));
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
            let id = arrived(hub, &path, &identity(&path));
            DEVICES.with(|d| d.borrow_mut().insert(device as isize, (id, Box::new([false; 256]))));
        }
        DEVICES.with(|d| {
            if let Some((id, state)) = d.borrow_mut().get_mut(&(device as isize)) {
                key_event(hub, id, state, kb.VKey, down);
            }
        });
    }

    /// `\\?\HID#VID_..&MI_00#7&1a2b&0&0000#{guid}` → `HID\VID_..&MI_00\7&1a2b&0&0000`, e do nó do aparelho
    /// (ou dos pais dele) o ContainerId e o nome que o aparelho informa.
    fn identity(path: &str) -> Identity {
        use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
            CM_Get_DevNode_PropertyW, CM_Get_Parent, CM_Locate_DevNodeW, CM_LOCATE_DEVNODE_NORMAL, CR_SUCCESS,
        };
        use windows_sys::Win32::Devices::Properties::{
            DEVPKEY_Device_BusReportedDeviceDesc, DEVPKEY_Device_ContainerId, DEVPKEY_Device_FriendlyName, DEVPROPKEY,
            DEVPROPTYPE, DEVPROP_TYPE_GUID, DEVPROP_TYPE_STRING,
        };
        let trimmed = path.strip_prefix("\\\\?\\").unwrap_or(path);
        let instance = trimmed.rsplit_once("#{").map(|(a, _)| a).unwrap_or(trimmed).replace('#', "\\");
        let wide: Vec<u16> = instance.encode_utf16().chain(Some(0)).collect();
        let mut node = 0u32;
        if unsafe { CM_Locate_DevNodeW(&mut node, wide.as_ptr(), CM_LOCATE_DEVNODE_NORMAL) } != CR_SUCCESS {
            return Identity::default();
        }
        let prop = |node: u32, key: &DEVPROPKEY, want: DEVPROPTYPE| -> Option<Vec<u8>> {
            let mut kind: DEVPROPTYPE = 0;
            let mut size = 0u32;
            unsafe { CM_Get_DevNode_PropertyW(node, key, &mut kind, std::ptr::null_mut(), &mut size, 0) };
            if size == 0 || size > 4096 {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            let ok = unsafe { CM_Get_DevNode_PropertyW(node, key, &mut kind, buf.as_mut_ptr(), &mut size, 0) } == CR_SUCCESS;
            (ok && kind == want).then_some(buf)
        };
        let text = |b: Vec<u8>| -> String {
            let w: Vec<u16> = b.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).take_while(|&c| c != 0).collect();
            String::from_utf16_lossy(&w)
        };
        let container = prop(node, &DEVPKEY_Device_ContainerId, DEVPROP_TYPE_GUID).filter(|g| g.len() == 16).map(|g| {
            let d1 = u32::from_le_bytes([g[0], g[1], g[2], g[3]]);
            let d2 = u16::from_le_bytes([g[4], g[5]]);
            let d3 = u16::from_le_bytes([g[6], g[7]]);
            format!(
                "{d1:08x}-{d2:04x}-{d3:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                g[8], g[9], g[10], g[11], g[12], g[13], g[14], g[15]
            )
        });
        // o nome que o aparelho informa costuma estar no nó USB/Bluetooth, um ou dois acima do HID
        let mut name = None;
        let mut cur = node;
        for _ in 0..4 {
            name = prop(cur, &DEVPKEY_Device_BusReportedDeviceDesc, DEVPROP_TYPE_STRING)
                .or_else(|| prop(cur, &DEVPKEY_Device_FriendlyName, DEVPROP_TYPE_STRING))
                .map(text)
                .and_then(|n| useful_name(&n));
            if name.is_some() {
                break;
            }
            let mut parent = 0u32;
            if unsafe { CM_Get_Parent(&mut parent, cur, 0) } != CR_SUCCESS {
                break;
            }
            cur = parent;
        }
        let builtin = container.as_deref() == Some(BUILTIN_CONTAINER);
        Identity { container, builtin, name }
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
    fn generic_windows_names_are_dropped_and_no_brand_is_invented() {
        assert_eq!(useful_name("Dispositivo de teclado HID"), None);
        assert_eq!(useful_name("USB Input Device"), None);
        assert_eq!(useful_name("K380 Multi-Device Keyboard").as_deref(), Some("K380 Multi-Device Keyboard"));
        let sinowealth = r"\\?\HID#VID_258A&PID_002A&MI_00#8&1&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}";
        assert_eq!(display_name(sinowealth, Bus::Usb), "Teclado USB", "chip genérico não vira marca");
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
