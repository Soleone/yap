mod engine;
mod library;
mod player;
mod server;
mod text;

use std::path::PathBuf;
use std::sync::Arc;
use tauri::{
  AppHandle, Manager, WindowEvent,
  menu::{Menu, MenuItem},
  tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// The `yap` command starts the app with this when it isn't running, so it goes straight to the tray.
const START_HIDDEN: &str = "--start-hidden";

/// Models, pieces, the library and the port file. Settings will live in the config folder instead.
fn data_dir() -> PathBuf {
  dirs::data_local_dir().expect("the OS has a local data folder").join("yap")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let data = data_dir();
  // Before any thread starts, so nothing reads the environment while it changes. Models go in yap's own folder,
  // not the shared Hugging Face cache, so removing yap removes them.
  unsafe { std::env::set_var("HF_HUB_CACHE", data.join("models")) };

  tauri::Builder::default()
    .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
    .on_window_event(|window, event| {
      if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        window.hide().ok();
      }
    })
    .setup(move |app| {
      std::fs::create_dir_all(&data)?;
      let player = player::Player::start()?;
      let library = Arc::new(library::Library::open(&data)?);
      let engine = engine::Engine::start(player.clone(), library);
      server::start(&data, engine, player)?;
      tray(app.handle())?;
      if !std::env::args().any(|arg| arg == START_HIDDEN) {
        show(app.handle());
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

fn show(app: &AppHandle) {
  if let Some(window) = app.get_webview_window("main") {
    window.show().ok();
    window.set_focus().ok();
  }
}

fn tray(app: &AppHandle) -> tauri::Result<()> {
  let open = MenuItem::with_id(app, "open", "Open Yap", true, None::<&str>)?;
  let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
  TrayIconBuilder::new()
    .icon(app.default_window_icon().cloned().expect("the bundle has an icon"))
    .tooltip("Yap")
    .menu(&Menu::with_items(app, &[&open, &quit])?)
    .show_menu_on_left_click(false)
    .on_menu_event(|app, event| match event.id.as_ref() {
      "open" => show(app),
      "quit" => app.exit(0),
      _ => {}
    })
    .on_tray_icon_event(|tray, event| {
      if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
        show(tray.app_handle());
      }
    })
    .build(app)?;
  Ok(())
}
