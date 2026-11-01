//! O que a UI pode pedir ao Hub (`invoke`). Rede e disco rodam fora da thread da UI.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pa_core::{DeviceKind, JoinPolicy};
use pa_library::http::Ureq;
use pa_library::{install::install, Discovered, GameUpdate, HubUpdate, InspectReport, Installed, Library, LibraryEntry, Source};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use tauri_plugin_opener::OpenerExt;

use crate::hub::{Hub, LobbyView};
use crate::keyboards::{self, KeyboardView};
use pa_library::pcgames::{self, Art, PcGame};

pub struct LibraryState {
    pub lib: Mutex<Library>,
    pub path: PathBuf,
    pub games_root: PathBuf,
    pub hub_etag: Mutex<Option<String>>,
    /// Removidos há pouco, para o "Desfazer" devolver tudo (inclusive o que está instalado).
    pub trash: Mutex<Vec<LibraryEntry>>,
}

impl LibraryState {
    fn save(&self, lib: &Library) {
        if let Err(e) = lib.save(&self.path) {
            eprintln!("party-anywhere: não consegui salvar a biblioteca: {e}");
        }
    }
}

type Hubs<'a> = State<'a, Arc<Hub>>;
type Libs<'a> = State<'a, Arc<LibraryState>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HubInfo {
    version: &'static str,
    join_url: String,
}

#[tauri::command]
pub fn hub_info(hub: Hubs) -> HubInfo {
    HubInfo { version: env!("CARGO_PKG_VERSION"), join_url: hub.join_url.lock().unwrap().clone() }
}

/// "Não conecta?": os endereços deste PC que o celular pode tentar, do mais provável ao menos.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    url: String,
    interface: String,
    current: bool,
}

#[tauri::command]
pub fn network_addresses(hub: Hubs) -> Vec<Address> {
    let current = hub.join_url.lock().unwrap().clone();
    let port = hub.phone.lock().unwrap().as_ref().map(|p| p.local_addr().port()).unwrap_or(crate::hub::PHONE_PORT);
    pa_phone::net::lan_candidates()
        .into_iter()
        .map(|c| {
            let url = format!("http://{}:{port}/", c.ip);
            Address { current: url == current, url, interface: c.interface }
        })
        .collect()
}

// ---------------------------------------------------------------- lobby

#[tauri::command]
pub fn lobby_snapshot(hub: Hubs) -> LobbyView {
    hub.snapshot()
}

#[tauri::command]
pub fn seat_remove(hub: Hubs, index: usize) -> bool {
    hub.remove_seat(index)
}

#[tauri::command]
pub fn seat_undo_remove(hub: Hubs, index: usize) -> bool {
    hub.undo_remove(index)
}

#[tauri::command]
pub fn seat_rename(hub: Hubs, index: usize, name: String) -> bool {
    let ok = hub.lobby.lock().unwrap().rename(index, &name);
    hub.emit_lobby();
    ok
}

#[tauri::command]
pub fn seat_move(hub: Hubs, from: usize, to: usize) -> bool {
    hub.move_seat(from, to)
}

/// Identificar. Controle Xbox: vibra. Celular: a tela pisca na cor do jogador (e vibra no Android; o iPhone
/// não deixa uma página vibrar sem um toque na tela). Devolve o que aconteceu, para o aviso não prometer
/// uma vibração que não houve.
#[tauri::command]
pub fn seat_identify(hub: Hubs, index: usize) -> &'static str {
    let Some(seat) = hub.lobby.lock().unwrap().seat(index).cloned() else { return "none" };
    match seat.kind {
        DeviceKind::Phone => {
            if let Some(p) = hub.phone.lock().unwrap().as_ref() {
                p.identify(index as u8);
            }
            "phone"
        }
        DeviceKind::Gamepad => {
            let brand = hub.meta.lock().unwrap().get(&seat.device).map(|m| m.brand.clone()).unwrap_or_default();
            if let Some(tx) = hub.gamepad_tx.lock().unwrap().as_ref() {
                let _ = tx.send(crate::gamepads::Command::Rumble(seat.device));
            }
            if crate::gamepads::can_rumble(&brand) {
                "rumble"
            } else {
                "flash"
            }
        }
        DeviceKind::Keyboard => "flash",
    }
}

#[tauri::command]
pub fn set_join_policy(hub: Hubs, on_button: bool) {
    hub.lobby.lock().unwrap().policy = if on_button { JoinPolicy::OnButtonPress } else { JoinPolicy::Automatic };
    hub.settings.lock().unwrap().join_on_button = on_button;
    hub.save_settings();
    hub.emit_lobby();
}

// ---------------------------------------------------------------- ajustes

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    version: &'static str,
    join_on_button: bool,
    preferred_ip: Option<String>,
    join_url: String,
    virtual_pads: bool,
    virtual_error: Option<String>,
}

#[tauri::command]
pub fn settings_get(hub: Hubs) -> SettingsView {
    let st = hub.settings.lock().unwrap().clone();
    SettingsView {
        version: env!("CARGO_PKG_VERSION"),
        join_on_button: st.join_on_button,
        preferred_ip: st.preferred_ip,
        join_url: hub.join_url.lock().unwrap().clone(),
        virtual_pads: hub.virtual_pads.lock().unwrap().is_some(),
        virtual_error: hub.virtual_error.lock().unwrap().clone(),
    }
}

/// Endereço do QR: um IP deste PC, ou `None` para o Hub escolher.
#[tauri::command]
pub fn set_join_ip(hub: Hubs, ip: Option<String>) -> String {
    hub.set_join_ip(ip);
    hub.join_url.lock().unwrap().clone()
}

/// Baixa o instalador oficial do ViGEmBus (o componente de controle virtual) do GitHub e abre: o Windows
/// pede permissão de administrador. Depois, "Conferir" liga os controles virtuais sem reiniciar o Hub.
#[tauri::command]
pub async fn virtual_pads_install(app: tauri::AppHandle, hub: Hubs<'_>) -> Result<bool, String> {
    // cada passo vai para a UI (evento `virtual-install`): baixando N%, instalador aberto, pronto ou o erro
    let step = |stage: &'static str, pct: Option<u8>, message: Option<String>| {
        let _ = app.emit("virtual-install", VirtualStep { stage, pct, message });
    };
    step("download", Some(0), None);
    let progress_app = app.clone();
    let downloaded = tauri::async_runtime::spawn_blocking(move || -> Result<PathBuf, String> {
        use pa_library::vigem;
        let mut last = 0u8;
        let bin = Ureq::default().download(vigem::URL, 50 * 1024 * 1024, |done, total| {
            let pct = total.map(|t| (done * 100 / t.max(1)).min(100) as u8).unwrap_or(0);
            if pct != last {
                last = pct;
                let _ = progress_app.emit("virtual-install", VirtualStep { stage: "download", pct: Some(pct), message: None });
            }
        })?;
        vigem::verify(&bin)?;
        let path = std::env::temp_dir().join(vigem::FILE);
        std::fs::write(&path, bin).map_err(|e| e.to_string())?;
        Ok(path)
    })
    .await
    .map_err(|e| e.to_string())?;
    let path = match downloaded {
        Ok(p) => p,
        Err(e) => {
            step("failed", None, Some(format!("Não deu para baixar: {e}")));
            return Ok(false);
        }
    };
    // o instalador de driver precisa de administrador: o Windows mostra o aviso (UAC) e o Hub espera terminar
    step("installer", None, None);
    let ran = tauri::async_runtime::spawn_blocking(move || run_elevated_and_wait(&path)).await.map_err(|e| e.to_string())?;
    if let Err(e) = ran {
        step("failed", None, Some(e));
        return Ok(false);
    }
    step("check", None, None);
    let hub = hub.inner().clone();
    let ok = tauri::async_runtime::spawn_blocking(move || {
        // o driver pode levar um instante para aparecer depois que o instalador fecha
        (0..12).any(|_| hub.retry_virtual_pads() || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            false
        })
    })
    .await
    .unwrap_or(false);
    if ok {
        step("done", None, None);
    } else {
        step("failed", None, Some("O instalador fechou, mas o componente não apareceu. Se ele pediu, reinicie o PC; senão, tente de novo.".into()));
    }
    Ok(ok)
}

#[derive(Clone, Serialize)]
struct VirtualStep {
    stage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

/// Abre um instalador pedindo administrador (o aviso do Windows) e espera ele fechar.
#[cfg(windows)]
fn run_elevated_and_wait(path: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::System::Threading::{WaitForSingleObject, INFINITE};
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let file: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let verb: Vec<u16> = "runas".encode_utf16().chain(Some(0)).collect();
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.nShow = SW_SHOWNORMAL;
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let code = unsafe { GetLastError() };
        return Err(if code == 1223 {
            "Você recusou o aviso do Windows. Sem ele o componente não instala.".into()
        } else {
            format!("Não consegui abrir o instalador (erro {code}).")
        });
    }
    if !info.hProcess.is_null() {
        unsafe {
            WaitForSingleObject(info.hProcess, INFINITE);
            CloseHandle(info.hProcess);
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn run_elevated_and_wait(_: &std::path::Path) -> Result<(), String> {
    Err("só no Windows.".into())
}

#[tauri::command]
pub fn virtual_pads_retry(hub: Hubs) -> bool {
    hub.retry_virtual_pads()
}

// ---------------------------------------------------------------- controles

#[tauri::command]
pub fn layouts_list(hub: Hubs) -> Vec<crate::settings::Layout> {
    hub.settings.lock().unwrap().all_layouts()
}

#[tauri::command]
pub fn layout_save(hub: Hubs, id: Option<String>, name: String, modes: Vec<crate::settings::Mode>, game: Option<String>) -> Result<crate::settings::Layout, String> {
    let saved = hub.settings.lock().unwrap().upsert_layout(id.as_deref(), &name, modes, game)?;
    hub.save_settings();
    hub.broadcast_layouts();
    Ok(saved)
}

/// Na biblioteca: o controle do celular que entra sozinho quando este jogo abre (`None`: nenhum).
#[tauri::command]
pub fn game_layout_set(hub: Hubs, game: String, layout: Option<String>) -> Result<(), String> {
    hub.settings.lock().unwrap().set_game(&game, layout.as_deref())?;
    hub.save_settings();
    hub.broadcast_layouts();
    Ok(())
}

/// "Restaurar": o controle do jogo volta a ser o que veio com ele.
#[tauri::command]
pub fn layout_reset(hub: Hubs, id: String) -> Result<crate::settings::Layout, String> {
    let saved = hub.settings.lock().unwrap().reset_game_layout(&id)?;
    hub.save_settings();
    hub.broadcast_layouts();
    Ok(saved)
}

#[tauri::command]
pub fn layout_delete(hub: Hubs, id: String) -> bool {
    let ok = hub.settings.lock().unwrap().delete_layout(&id);
    if ok {
        hub.save_settings();
        hub.broadcast_layouts();
    }
    ok
}

/// Aparelho conectado agora, com ou sem vaga (os que esperam: teclados, controles parados, removidos).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    key: String,
    kind: &'static str,
    brand: String,
    device_name: String,
    via: String,
    battery: Option<u8>,
    ping: Option<u32>,
    seat: Option<usize>,
    layout: Option<String>,
}

/// A aba Controles abriu ou fechou: liga ou desliga o aviso "este aparelho apertou algo".
#[tauri::command]
pub fn devices_watch(hub: Hubs, on: bool) {
    hub.watch_input.store(on, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
pub fn devices_list(hub: Hubs) -> Vec<DeviceView> {
    let lobby = hub.lobby.lock().unwrap();
    let meta = hub.meta.lock().unwrap();
    let st = hub.settings.lock().unwrap();
    let kind = |k: DeviceKind| match k {
        DeviceKind::Phone => "phone",
        DeviceKind::Gamepad => "gamepad",
        DeviceKind::Keyboard => "keyboard",
    };
    let view = |key: &pa_core::DeviceKey, k: DeviceKind, seat: Option<usize>| {
        let m = meta.get(key).cloned().unwrap_or_default();
        DeviceView {
            key: key.0.clone(),
            kind: kind(k),
            brand: m.brand,
            device_name: m.device_name,
            via: m.via,
            battery: m.battery,
            ping: m.ping_ms,
            seat,
            layout: (k == DeviceKind::Phone).then(|| st.layout_for(&key.0)),
        }
    };
    lobby
        .seats()
        .map(|s| view(&s.device, s.kind, Some(s.index)))
        .chain(lobby.waiting().iter().map(|d| view(&d.key, d.kind, None)))
        .collect()
}

// ---------------------------------------------------------------- teclados

#[tauri::command]
pub fn keyboards_list(hub: Hubs) -> Vec<KeyboardView> {
    keyboards::list(&hub)
}

/// A tela "Teclado" abriu ou fechou: Enter num teclado só faz entrar com ela aberta.
#[tauri::command]
pub fn keyboards_listen(on: bool) {
    keyboards::LISTENING.store(on, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
pub fn keyboard_add(hub: Hubs, key: String) -> Option<usize> {
    keyboards::join(&hub, &key)
}

// ---------------------------------------------------------------- biblioteca

/// O que a capa e a tela do fliperama mostram.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiGame {
    id: String,
    name: String,
    tagline: String,
    players: String,
    badge: &'static str,
    store: Option<String>,
    tags: Vec<String>,
    art: [String; 3],
    image: Option<String>,
    installed: Option<String>,
    update: Option<String>,
    repo: Option<String>,
}

fn ui_game(e: &LibraryEntry) -> UiGame {
    let (badge, store, tags, repo) = match &e.source {
        Source::Repo { repo } if repo.starts_with(pa_library::github::LOCAL_PREFIX) => ("comm", None, vec!["Seu mod (do PC)".into()], Some(repo.clone())),
        Source::Repo { repo } => ("comm", None, vec!["Da comunidade".into()], Some(repo.clone())),
        Source::Pc { store, .. } => ("pc", Some(store.clone()), vec!["Seu PC".into(), "Modo Universal".into()], None),
    };
    UiGame {
        id: e.id.clone(),
        name: e.name.clone(),
        tagline: e.tagline.clone(),
        players: e.players.clone(),
        badge,
        store,
        tags,
        art: palette(&e.id),
        image: e.cover.clone(),
        installed: e.installed.as_ref().map(|i| i.version.clone()),
        update: e.update.clone(),
        repo,
    }
}

/// Cores da capa gerada (quando o jogo não tem imagem), estáveis por jogo.
fn palette(id: &str) -> [String; 3] {
    const P: [[&str; 3]; 6] = [
        ["#4c1d95", "#ec4899", "#f97316"],
        ["#082f49", "#06b6d4", "#facc15"],
        ["#14532d", "#22c55e", "#fde047"],
        ["#450a0a", "#dc2626", "#f59e0b"],
        ["#1e1b4b", "#a855f7", "#22c55e"],
        ["#064e3b", "#10b981", "#a3e635"],
    ];
    let h = id.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    P[(h % 6) as usize].map(String::from)
}

#[tauri::command]
pub fn library_list(libs: Libs) -> Vec<UiGame> {
    libs.lib.lock().unwrap().games.iter().map(ui_game).collect()
}

#[tauri::command]
pub async fn library_inspect(url: String) -> Result<InspectReport, String> {
    tauri::async_runtime::spawn_blocking(move || pa_library::inspect(&Ureq::default(), &url)).await.map_err(|e| e.to_string())?
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    id: String,
    stage: &'static str,
}

/// Instala (ou atualiza) um jogo publicado. `base_dir`: pasta do jogo-base, para mods.
#[tauri::command]
pub async fn library_install(
    app: tauri::AppHandle,
    hub: Hubs<'_>,
    libs: Libs<'_>,
    url: String,
    base_dir: Option<String>,
) -> Result<Vec<UiGame>, String> {
    let libs = libs.inner().clone();
    let hub = hub.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let http = Ureq::default();
        let report = pa_library::inspect(&http, &url)?;
        let (Some(manifest), Some(latest), true) = (&report.manifest, &report.latest, report.installable) else {
            let why = report.issues.iter().find(|i| i.is_error()).map(|i| i.message.clone());
            return Err(why.unwrap_or(report.summary));
        };
        let base_dir = base_dir.map(PathBuf::from).or_else(|| {
            let appid = manifest.requires.as_ref()?.base_game.as_ref()?.steam_app_id?;
            pcgames::steam_app_dir(appid)
        });
        let _ = app.emit("install-progress", InstallProgress { id: manifest.id.clone(), stage: "download" });
        let installed = install(&http, manifest, latest, &libs.games_root, base_dir.as_deref())?;
        let _ = app.emit("install-progress", InstallProgress { id: manifest.id.clone(), stage: "done" });
        // o jogo trouxe o controle do celular dele: vira o "Padrão do jogo"
        if let Some(c) = &manifest.controls {
            let saved = hub.settings.lock().unwrap().set_game_layout(&manifest.id, &manifest.name, c.modes.clone());
            match saved {
                Ok(_) => {
                    hub.save_settings();
                    hub.broadcast_layouts();
                }
                Err(e) => eprintln!("party-anywhere: controle de {} ignorado: {e}", manifest.id),
            }
        }
        let mut entry = pa_library::entry_for(&report).ok_or("manifesto ausente")?;
        entry.installed = Some(installed);
        let mut lib = libs.lib.lock().unwrap();
        lib.upsert(entry);
        libs.save(&lib);
        Ok(lib.games.iter().map(ui_game).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Um jogo como a UI tem: a lista inteira chega depois de qualquer mudança (mover, editar, remover,
/// desfazer, adicionar do PC). O Hub aplica a diferença; quem saiu vai para a lixeira do "Desfazer".
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncGame {
    id: String,
    name: String,
    #[serde(default)]
    tagline: String,
    #[serde(default)]
    players: String,
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    store: Option<String>,
    #[serde(default)]
    launch: Option<String>,
}

#[tauri::command]
pub fn library_sync(libs: Libs, games: Vec<SyncGame>) -> Vec<UiGame> {
    let mut lib = libs.lib.lock().unwrap();
    let mut trash = libs.trash.lock().unwrap();
    let mut next = Vec::with_capacity(games.len());
    for g in games {
        let existing = lib.remove(&g.id).or_else(|| trash.iter().position(|e| e.id == g.id).map(|i| trash.remove(i)));
        let entry = match existing {
            Some(mut e) => {
                let image = g.image.filter(|i| !i.is_empty());
                if e.name != g.name || e.tagline != g.tagline || (!g.players.is_empty() && e.players != g.players) || e.cover != image {
                    e.name = g.name.chars().take(40).collect();
                    e.tagline = g.tagline;
                    if !g.players.is_empty() {
                        e.players = g.players;
                    }
                    e.cover = image;
                    e.edited = true;
                }
                e
            }
            // novo jogo do PC (Steam, Epic, .exe): só com como abrir
            None => match (g.store, g.launch) {
                (Some(store), Some(launch)) => LibraryEntry {
                    id: g.id,
                    name: g.name,
                    tagline: g.tagline,
                    players: if g.players.is_empty() { "Modo Universal".into() } else { g.players },
                    source: Source::Pc { store, launch },
                    cover: g.image.filter(|i| !i.is_empty()),
                    installed: None,
                    update: None,
                    etag: None,
                    edited: false,
                },
                _ => continue,
            },
        };
        next.push(entry);
    }
    trash.extend(lib.games.drain(..));
    let excess = trash.len().saturating_sub(20);
    trash.drain(..excess);
    lib.games = next;
    libs.save(&lib);
    lib.games.iter().map(ui_game).collect()
}

// ---------------------------------------------------------------- adicionar pelo link, vindo de fora

static PENDING_ADD: Mutex<Option<String>> = Mutex::new(None);

/// O link de `--add <link>` (repositório de um mod ou jogo), se veio.
pub fn add_arg(args: &[String]) -> Option<String> {
    let i = args.iter().position(|a| a == "--add")?;
    args.get(i + 1).map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s.len() < 300)
}

/// Pede à UI para abrir o "Adicionar por link" com este link. Fica guardado também, para o caso de a UI
/// ainda não ter carregado (ela pega com `take_pending_add`).
pub fn request_add(app: &tauri::AppHandle, link: String) {
    *PENDING_ADD.lock().unwrap() = Some(link.clone());
    let _ = app.emit("add-repo", link);
}

#[tauri::command]
pub fn take_pending_add() -> Option<String> {
    PENDING_ADD.lock().unwrap().take()
}

/// Ajustes → "Criar atalho na área de trabalho" (o instalador já oferece; isto é para quem pulou).
/// `create: false` só conta se já existe.
#[tauri::command]
pub fn desktop_shortcut(create: bool) -> Result<bool, String> {
    const NO_SHORTCUT: &str = "Não deu para criar o atalho. Abra o Party Anywhere pelo menu Iniciar.";
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let desktop = desktop_dir().ok_or(NO_SHORTCUT)?;
    let lnk = desktop.join("Party Anywhere.lnk");
    if lnk.exists() || !create {
        return Ok(lnk.exists());
    }
    let ps = format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.WorkingDirectory='{}';$s.Save()",
        lnk.display().to_string().replace('\'', "''"),
        exe.display().to_string().replace('\'', "''"),
        exe.parent().map(|p| p.display().to_string()).unwrap_or_default().replace('\'', "''"),
    );
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // sem janela de console
    }
    let ok = cmd.status().map(|s| s.success()).unwrap_or(false);
    if ok && lnk.exists() {
        Ok(true)
    } else {
        Err(NO_SHORTCUT.into())
    }
}

/// A área de trabalho de verdade, como o Windows diz: com o OneDrive ela muda de lugar e de nome
/// ("OneDrive\Área de Trabalho" num Windows em português), então `%USERPROFILE%\Desktop` não serve.
#[cfg(windows)]
fn desktop_dir() -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{FOLDERID_Desktop, SHGetKnownFolderPath};
    let mut raw: windows_sys::core::PWSTR = std::ptr::null_mut();
    // SAFETY: `raw` recebe um texto alocado pelo Windows, lido até o zero final e liberado com CoTaskMemFree.
    let path = unsafe {
        let ok = SHGetKnownFolderPath(&FOLDERID_Desktop, 0, std::ptr::null_mut(), &mut raw) >= 0 && !raw.is_null();
        let path = ok.then(|| {
            let len = (0..).take_while(|&i| *raw.add(i) != 0).count();
            PathBuf::from(std::ffi::OsString::from_wide(std::slice::from_raw_parts(raw, len)))
        });
        CoTaskMemFree(raw as *const _);
        path
    };
    path.filter(|d| d.is_dir())
}

#[cfg(not(windows))]
fn desktop_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Desktop")).filter(|d| d.is_dir())
}

// ---------------------------------------------------------------- jogo original de um mod

/// Onde está o jogo que o mod precisa (Steam, Epic, qualquer jogo do PC, ou `dir` que a pessoa escolheu),
/// e como conseguir se não estiver. A tela chama de novo a cada poucos segundos enquanto a Steam ou a Epic
/// instalam: quando o jogo aparece, ela segue sozinha.
#[tauri::command]
pub async fn base_game_check(base: pa_manifest::BaseGame, dir: Option<String>) -> Result<pa_library::basegame::BaseCheck, String> {
    tauri::async_runtime::spawn_blocking(move || pa_library::basegame::check(&base, dir.as_deref().map(std::path::Path::new)))
        .await
        .map_err(|e| e.to_string())
}

/// "Já tenho o jogo": a pessoa aponta o .exe ou a pasta (vale cópia de qualquer lugar: GOG, CD, outra
/// pasta). `None` se ela fechou a janela.
#[tauri::command]
pub async fn base_game_pick(app: tauri::AppHandle, base: pa_manifest::BaseGame, folder: bool) -> Option<pa_library::basegame::BaseCheck> {
    use tauri_plugin_dialog::DialogExt;
    let dialog = app.dialog().file().set_title(format!("Onde está o {}?", base.name));
    let picked = if folder {
        dialog.blocking_pick_folder()
    } else {
        dialog.add_filter(&base.exe, &["exe"]).blocking_pick_file()
    }?;
    let path = picked.into_path().ok()?;
    tauri::async_runtime::spawn_blocking(move || pa_library::basegame::check(&base, Some(&path))).await.ok()
}

/// "Escolher pasta ou atalho": a janela de arquivo do Windows.
#[tauri::command]
pub async fn pick_game_file(app: tauri::AppHandle) -> Option<PcGame> {
    use tauri_plugin_dialog::DialogExt;
    let file = app
        .dialog()
        .file()
        .set_title("Escolha o jogo")
        .add_filter("Jogo ou atalho", &["exe", "lnk", "url"])
        .blocking_pick_file()?;
    let path = file.into_path().ok()?;
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let name = pretty_name(&stem);
    let slug: String = stem.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    Some(PcGame {
        id: format!("pc-{}", slug.trim_matches('-')),
        name,
        store: "PC".into(),
        players: "Modo Universal".into(),
        tagline: "Jogo do seu PC, com os celulares como controles.".into(),
        launch: path.to_string_lossy().to_string(),
        image: None,
        install_dir: path.parent().map(|p| p.to_path_buf()),
        steam_app_id: None,
        epic_app_name: None,
    })
}

/// "GTA_SanAndreas" → "GTA San Andreas".
fn pretty_name(stem: &str) -> String {
    let mut out = String::new();
    let mut prev: Option<char> = None;
    for c in stem.chars() {
        let c = if matches!(c, '_' | '-' | '.') { ' ' } else { c };
        if let Some(p) = prev {
            if (p.is_lowercase() && c.is_uppercase()) || (p.is_alphabetic() && c.is_ascii_digit()) {
                out.push(' ');
            }
        }
        if !(c == ' ' && out.ends_with(' ')) {
            out.push(c);
        }
        prev = Some(c);
    }
    let mut words: Vec<String> = Vec::new();
    for w in out.split_whitespace() {
        let mut cs = w.chars();
        words.push(cs.next().map(|f| f.to_uppercase().collect::<String>() + cs.as_str()).unwrap_or_default());
    }
    words.join(" ")
}

#[tauri::command]
pub fn open_bluetooth_settings(app: tauri::AppHandle) -> Result<(), String> {
    app.opener().open_url("ms-settings:bluetooth", None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_add_pc(libs: Libs, games: Vec<PcGame>) -> Vec<UiGame> {
    let mut lib = libs.lib.lock().unwrap();
    for g in games {
        lib.upsert(LibraryEntry {
            id: g.id,
            name: g.name,
            tagline: g.tagline,
            players: g.players,
            source: Source::Pc { store: g.store, launch: g.launch },
            cover: g.image,
            installed: None,
            update: None,
            etag: None,
            edited: false,
        });
    }
    libs.save(&lib);
    lib.games.iter().map(ui_game).collect()
}

/// Devolve a entrada removida para o "Desfazer" (com a posição).
#[tauri::command]
pub fn library_remove(libs: Libs, id: String) -> Option<(usize, LibraryEntry)> {
    let mut lib = libs.lib.lock().unwrap();
    let pos = lib.games.iter().position(|g| g.id == id)?;
    let entry = lib.games.remove(pos);
    libs.save(&lib);
    Some((pos, entry))
}

#[tauri::command]
pub fn library_restore(libs: Libs, index: usize, entry: LibraryEntry) -> Vec<UiGame> {
    let mut lib = libs.lib.lock().unwrap();
    if lib.get(&entry.id).is_none() {
        let i = index.min(lib.games.len());
        lib.games.insert(i, entry);
        libs.save(&lib);
    }
    lib.games.iter().map(ui_game).collect()
}

#[tauri::command]
pub fn library_move(libs: Libs, from: usize, to: usize) -> bool {
    let mut lib = libs.lib.lock().unwrap();
    let ok = lib.move_game(from, to);
    if ok {
        libs.save(&lib);
    }
    ok
}

#[derive(Deserialize)]
pub struct GameEdit {
    name: Option<String>,
    tagline: Option<String>,
    players: Option<String>,
    /// data URL ou caminho; `Some("")` tira a capa.
    image: Option<String>,
}

#[tauri::command]
pub fn library_edit(libs: Libs, id: String, edit: GameEdit) -> bool {
    let mut lib = libs.lib.lock().unwrap();
    let Some(e) = lib.get_mut(&id) else { return false };
    if let Some(n) = edit.name.filter(|n| !n.trim().is_empty()) {
        e.name = n.trim().chars().take(40).collect();
    }
    if let Some(t) = edit.tagline {
        e.tagline = t.trim().chars().take(140).collect();
    }
    if let Some(p) = edit.players.filter(|p| !p.trim().is_empty()) {
        e.players = p.trim().to_string();
    }
    if let Some(img) = edit.image {
        e.cover = (!img.is_empty()).then_some(img);
    }
    e.edited = true;
    libs.save(&lib);
    true
}

#[tauri::command]
pub async fn library_discover() -> Result<Vec<Discovered>, String> {
    tauri::async_runtime::spawn_blocking(|| pa_library::discover(&Ureq::default())).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn pc_scan() -> Vec<PcGame> {
    tauri::async_runtime::spawn_blocking(pcgames::scan).await.unwrap_or_default()
}

/// Confere atualizações dos jogos instalados por link; avisa a UI com o evento `game-updates`.
#[tauri::command]
pub async fn library_check_updates(app: tauri::AppHandle, libs: Libs<'_>) -> Result<Vec<GameUpdate>, String> {
    let libs = libs.inner().clone();
    let found = tauri::async_runtime::spawn_blocking(move || check_game_updates(&libs)).await.map_err(|e| e.to_string())?;
    if !found.is_empty() {
        let _ = app.emit("game-updates", &found);
    }
    Ok(found)
}

pub fn check_game_updates(libs: &LibraryState) -> Vec<GameUpdate> {
    // trabalha numa cópia: a rede não segura a trava da biblioteca
    let mut copy = libs.lib.lock().unwrap().clone();
    let found = pa_library::check_games(&Ureq::default(), &mut copy);
    let mut lib = libs.lib.lock().unwrap();
    for g in &copy.games {
        if let Some(e) = lib.get_mut(&g.id) {
            e.update = g.update.clone();
            e.etag = g.etag.clone();
        }
    }
    libs.save(&lib);
    found
}

#[tauri::command]
pub fn game_play(app: tauri::AppHandle, hub: Hubs, libs: Libs, id: String, players: Option<Vec<usize>>) -> Result<(), String> {
    let entry = libs.lib.lock().unwrap().get(&id).cloned().ok_or("jogo não encontrado na biblioteca")?;
    // mais gente do que o jogo aceita: só quem foi escolhido aparece para ele
    hub.set_players(players.as_deref());
    // o jogo tem um controle próprio ("Padrão do jogo"): os celulares trocam para ele
    let game_layout = hub.settings.lock().unwrap().layout_for_game(&id);
    hub.set_session_layout(game_layout);
    match (&entry.source, &entry.installed) {
        (Source::Pc { launch, .. }, _) => {
            if launch.contains("://") {
                app.opener().open_url(launch, None::<&str>).map_err(|e| format!("não consegui abrir {}: {e}", entry.name))
            } else {
                spawn(&PathBuf::from(launch), &[], None)
            }
        }
        (Source::Repo { .. }, Some(inst)) => {
            let exe = pa_library::install::exe_path(inst);
            // mod: o jogo original saiu da pasta (desinstalado ou movido)
            if !exe.is_file() {
                return Err(match &inst.base_dir {
                    Some(dir) => format!(
                        "O jogo original não está mais em {}. Instale de novo pelo link do mod e mostre onde ele está agora.",
                        dir.display()
                    ),
                    None => format!("{} não está mais instalado. Instale de novo pelo link.", entry.name),
                });
            }
            spawn(&exe, &inst.args, Some(inst.dir.join(&inst.working_dir)))
        }
        (Source::Repo { .. }, None) => Err(format!("{} ainda não está instalado.", entry.name)),
    }
}

/// Voltou do jogo para o Hub: todo mundo volta a contar.
#[tauri::command]
pub fn game_stopped(hub: Hubs) {
    hub.set_players(None);
    hub.set_session_layout(None);
}

fn spawn(exe: &std::path::Path, args: &[String], cwd: Option<PathBuf>) -> Result<(), String> {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    if let Some(dir) = cwd.or_else(|| exe.parent().map(|p| p.to_path_buf())) {
        cmd.current_dir(dir);
    }
    cmd.spawn().map(|_| ()).map_err(|e| format!("não consegui abrir o jogo ({}): {e}", exe.display()))
}

// ---------------------------------------------------------------- atualização do Hub

#[tauri::command]
pub async fn hub_check_update(app: tauri::AppHandle, libs: Libs<'_>) -> Result<Option<HubUpdate>, String> {
    let libs = libs.inner().clone();
    let etag = libs.hub_etag.lock().unwrap().clone();
    let found = tauri::async_runtime::spawn_blocking(move || {
        pa_library::check_hub(&Ureq::default(), env!("CARGO_PKG_VERSION"), etag.as_deref())
    })
    .await
    .map_err(|e| e.to_string())??;
    if let Some(u) = &found {
        *libs.hub_etag.lock().unwrap() = u.etag.clone();
        let _ = app.emit("hub-update", u);
    }
    Ok(found)
}

/// Baixa o instalador novo (conferindo o hash publicado), abre e fecha o Hub; o instalador substitui a
/// versão e reabre.
#[tauri::command]
pub async fn hub_apply_update(app: tauri::AppHandle, update: HubUpdate) -> Result<(), String> {
    let path = tauri::async_runtime::spawn_blocking(move || pa_library::hubapp::download_installer(&Ureq::default(), &update))
        .await
        .map_err(|e| e.to_string())??;
    std::process::Command::new(&path).spawn().map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}

/// Arte de um jogo do PC sob demanda: `steam:<appid>` vira a capa ou a arte larga (hero) da Steam.
#[tauri::command]
pub async fn game_art(reference: String, hero: bool) -> Option<String> {
    let art = if hero { Art::Hero } else { Art::Cover };
    tauri::async_runtime::spawn_blocking(move || pcgames::art_data_url(&reference, art)).await.ok().flatten()
}

/// Responde `pa-art`: `/steam/<appid>/cover` ou `/hero` (o caminho pode vir codificado).
pub fn art_response(path: &str) -> tauri::http::Response<Vec<u8>> {
    let decoded = path.replace("%2F", "/").replace("%2f", "/");
    let parts: Vec<&str> = decoded.trim_matches('/').split('/').collect();
    let file = match parts.as_slice() {
        ["steam", appid, kind] => pcgames::steam_art_file(appid, if *kind == "hero" { Art::Hero } else { Art::Cover }),
        _ => None,
    };
    let builder = tauri::http::Response::builder().header("Access-Control-Allow-Origin", "*");
    match file.and_then(|f| std::fs::read(f).ok()) {
        Some(bytes) => builder
            .header("Content-Type", "image/jpeg")
            .header("Cache-Control", "max-age=86400")
            .body(bytes)
            .unwrap_or_default(),
        None => builder.status(404).body(Vec::new()).unwrap_or_default(),
    }
}

/// Instalado na pasta de um jogo-base? A UI pergunta a pasta quando o mod precisa.
#[tauri::command]
pub fn installed_info(libs: Libs, id: String) -> Option<Installed> {
    libs.lib.lock().unwrap().get(&id).and_then(|e| e.installed.clone())
}

#[cfg(test)]
mod tests {
    use super::pretty_name;

    /// A área de trabalho vem do Windows (com o OneDrive ela fica em "OneDrive\Área de Trabalho", não em
    /// `%USERPROFILE%\Desktop`), e existe.
    #[cfg(windows)]
    #[test]
    fn the_desktop_is_the_one_windows_says() {
        let d = super::desktop_dir().expect("área de trabalho");
        assert!(d.is_dir() && d.is_absolute(), "{}", d.display());
    }

    #[test]
    fn names_from_files() {
        assert_eq!(pretty_name("GTA_SanAndreas"), "GTA San Andreas");
        assert_eq!(pretty_name("hades2"), "Hades 2");
        assert_eq!(pretty_name("overcooked-all-you-can-eat"), "Overcooked All You Can Eat");
    }
}

#[tauri::command]
pub fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    // links da web e os pedidos de instalação para a Steam e a Epic (o jogo original de um mod)
    let allowed = ["https://", "http://", "steam://install/", "steam://store/", "com.epicgames.launcher://apps/"];
    if !allowed.iter().any(|p| url.starts_with(p)) {
        return Err("só links da web, da Steam e da Epic".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}
