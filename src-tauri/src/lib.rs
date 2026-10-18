use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;

mod error;
mod game;
mod mods;
mod nexus;
mod smapi;

/// Event the frontend listens on when the system hands us an `nxm://` link.
const NXM_EVENT: &str = "nxm://received";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Registered first on purpose: a second launch (which is what a browser does when
        // the user clicks "Mod Manager Download") hands its arguments to the running
        // instance instead of starting a second copy.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            // Links that arrive while the app is running are delivered as an event.
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    let _ = handle.emit(NXM_EVENT, url.to_string());
                }
            });
            // Registering at runtime makes the scheme work for a running-from-source or
            // AppImage build too, not just an installed package. Failing here must not
            // stop the app; the worst case is that clicking a link does nothing.
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            if let Err(error) = app.deep_link().register_all() {
                eprintln!("deep-link registration failed: {error}");
            }
            // A link that *started* the app arrives as a command-line argument instead.
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    let _ = app.emit(NXM_EVENT, url.to_string());
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            game::commands::discover_game_installations,
            game::commands::validate_game_directory,
            game::commands::pick_game_directory,
            smapi::commands::smapi_status,
            smapi::commands::fetch_smapi_release,
            smapi::commands::smapi_log,
            smapi::commands::install_smapi,
            smapi::commands::uninstall_smapi,
            mods::commands::list_mods,
            mods::commands::set_mod_enabled,
            mods::commands::remove_mod,
            mods::commands::pick_mod_archive,
            mods::commands::install_mod,
            nexus::commands::search_nexus_mods,
            nexus::commands::nexus_api_key_status,
            nexus::commands::set_nexus_api_key,
            nexus::commands::install_mod_from_nxm,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
