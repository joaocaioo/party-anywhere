// sem janela de console no executável final
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Party Anywhere: o Hub. Junta celulares, controles e teclados num lobby P1..P10, transforma celulares e
//! teclados em controles de Xbox para qualquer jogo (Modo Universal) e mantém a Biblioteca ligada ao GitHub.

mod commands;
mod foreground;
mod gamepads;
mod hub;
mod keyboards;
mod nav;
mod sdk;
mod sdk_phone;
mod settings;
mod view;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use commands::LibraryState;
use pa_library::{Library, LoadNote};
use tauri::{Emitter, Manager};

/// `%APPDATA%\PartyAnywhere` (o que a pessoa montou) e `%LOCALAPPDATA%\PartyAnywhere` (jogos e caches).
fn dirs() -> (PathBuf, PathBuf) {
    let base = |var: &str| std::env::var_os(var).map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("PartyAnywhere");
    (base("APPDATA"), base("LOCALAPPDATA"))
}

/// A janela abre do tamanho da tela útil (sem a barra de tarefas): até 1280×820, nunca maior que 92%
/// da área de trabalho, centralizada. Abre invisível e aparece já no tamanho certo.
fn fit_to_screen(app: &tauri::App) {
    let Some(w) = app.get_webview_window("main") else { return };
    if let Ok(Some(m)) = w.current_monitor() {
        let scale = m.scale_factor();
        let area = m.work_area();
        let (aw, ah) = (area.size.width as f64 / scale, area.size.height as f64 / scale);
        let width = (aw * 0.92).clamp(860.0, 1280.0).min(aw);
        let height = (ah * 0.92).clamp(560.0, 820.0).min(ah);
        let _ = w.set_size(tauri::LogicalSize::new(width, height));
        let _ = w.center();
    }
    let _ = w.show();
}

/// Bibliotecas de antes da arte sob demanda guardavam a capa da Steam inteira no library.json: troca pela
/// referência `steam:<appid>` (capa e arte larga lidas na hora), se a pessoa não editou a capa.
fn migrate_steam_art(lib: &mut Library) -> bool {
    let mut changed = false;
    for e in lib.games.iter_mut().filter(|e| !e.edited) {
        if let pa_library::Source::Pc { launch, .. } = &e.source {
            let appid = launch.strip_prefix("steam://rungameid/");
            if let (Some(id), true) = (appid, e.cover.as_deref().is_some_and(|c| c.starts_with("data:"))) {
                e.cover = Some(format!("steam:{id}"));
                changed = true;
            }
        }
    }
    changed
}

/// Jogos e Hub: confere ao abrir e a cada 6 horas.
const UPDATE_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

fn main() {
    tauri::Builder::default()
        // abrir o Hub de novo traz a janela que já está aberta (um lobby, uma porta do celular, um SDK)
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
            // `party-anywhere --add dono/repo` com o Hub já aberto (a `pa add`, um agente de IA)
            if let Some(link) = commands::add_arg(&args) {
                commands::request_add(app, link);
            }
        }))
        // capas da Steam lidas do disco pelo navegador, sob demanda e com cache (sem base64 pela ponte):
        // pa-art://localhost/steam/<appid>/cover|hero (no Windows: http://pa-art.localhost/...)
        .register_asynchronous_uri_scheme_protocol("pa-art", |_ctx, request, responder| {
            let path = request.uri().path().to_string();
            std::thread::spawn(move || responder.respond(commands::art_response(&path)));
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            fit_to_screen(app);
            // aberto com `--add dono/repo`: a UI abre o "Adicionar por link" assim que carregar
            if let Some(link) = commands::add_arg(&std::env::args().collect::<Vec<_>>()) {
                commands::request_add(app.handle(), link);
            }
            let (roaming, local) = dirs();
            let hub = hub::Hub::new(app.handle().clone(), settings::path(&roaming));
            // o idioma escolhido no instalador (ou por quem instalou com /LANG=en), uma vez só
            let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf()));
            if let Some(lang) = exe_dir.as_deref().and_then(settings::take_installer_lang) {
                hub.settings.lock().unwrap().lang = Some(lang.into());
                hub.save_settings();
            }
            if let Err(e) = hub.start_phones() {
                eprintln!("party-anywhere: celulares indisponíveis: {e}");
            }
            gamepads::start(hub.clone());
            keyboards::start(hub.clone());
            sdk::start(hub.clone());
            nav::start(hub.clone());
            view::start(hub.clone());
            app.manage(hub);

            let path = roaming.join("library.json");
            let (mut lib, note) = Library::load(&path);
            if migrate_steam_art(&mut lib) {
                let _ = lib.save(&path);
            }
            let libs = Arc::new(LibraryState {
                lib: Mutex::new(lib),
                path,
                games_root: local.join("games"),
                hub_etag: Mutex::new(None),
                trash: Mutex::new(Vec::new()),
            });
            app.manage(libs.clone());

            let handle = app.handle().clone();
            std::thread::Builder::new().name("pa-updates".into()).spawn(move || loop {
                if note == LoadNote::RestoredFromBackup {
                    let _ = handle.emit("library-restored", ());
                }
                let found = commands::check_game_updates(&libs);
                if !found.is_empty() {
                    let _ = handle.emit("game-updates", &found);
                }
                let etag = libs.hub_etag.lock().unwrap().clone();
                if let Ok(Some(u)) = pa_library::check_hub(&pa_library::http::Ureq::default(), env!("CARGO_PKG_VERSION"), etag.as_deref()) {
                    *libs.hub_etag.lock().unwrap() = u.etag.clone();
                    let _ = handle.emit("hub-update", &u);
                }
                std::thread::sleep(UPDATE_EVERY);
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::firewall_allow,
            commands::hub_info,
            commands::network_addresses,
            commands::settings_get,
            commands::set_join_ip,
            commands::set_lang,
            commands::layouts_list,
            commands::layout_save,
            commands::layout_delete,
            commands::layout_reset,
            commands::devices_list,
            commands::devices_watch,
            commands::game_stopped,
            commands::game_layout_set,
            commands::virtual_pads_install,
            commands::virtual_pads_retry,
            commands::lobby_snapshot,
            commands::seat_remove,
            commands::seat_undo_remove,
            commands::seat_rename,
            commands::seat_move,
            commands::seat_identify,
            commands::set_join_policy,
            commands::keyboards_list,
            commands::keyboards_listen,
            commands::keyboard_add,
            commands::library_list,
            commands::library_inspect,
            commands::library_install,
            commands::library_add_pc,
            commands::library_sync,
            commands::pick_game_file,
            commands::base_game_check,
            commands::take_pending_add,
            commands::desktop_shortcut,
            commands::base_game_pick,
            commands::open_bluetooth_settings,
            commands::library_remove,
            commands::library_restore,
            commands::library_move,
            commands::library_edit,
            commands::library_discover,
            commands::library_check_updates,
            commands::pc_scan,
            commands::game_play,
            commands::hub_check_update,
            commands::hub_apply_update,
            commands::installed_info,
            commands::open_url,
            commands::game_art,
        ])
        .run(tauri::generate_context!())
        .expect("o Hub não conseguiu abrir");
}
