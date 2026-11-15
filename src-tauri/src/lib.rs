mod engine;
mod library;
mod player;
mod popup;
mod registry;
mod server;
mod settings;
mod text;

use engine::{CatalogVoice, Engine, Status};
use settings::{Settings, Store};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{
  AppHandle, Manager, State, WindowEvent,
  menu::{Menu, MenuItem},
  tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// The `yap` command starts the app with this when it isn't running, so it goes straight to the tray.
const START_HIDDEN: &str = "--start-hidden";

/// Models, pieces, the library and the port file.
fn data_dir() -> PathBuf {
  dirs::data_local_dir().expect("the OS has a local data folder").join("yap")
}

/// Settings and the Hugging Face token.
fn config_dir() -> PathBuf {
  dirs::config_dir().expect("the OS has a config folder").join("yap")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let (data, config) = (data_dir(), config_dir());
  // Before any thread starts, so nothing reads the environment while it changes. Models go in yap's own folder,
  // not the shared Hugging Face cache, so removing yap removes them.
  unsafe {
    std::env::set_var("HF_HUB_CACHE", data.join("models"));
    std::env::set_var("HF_TOKEN_PATH", settings::token_file(&config));
  }

  tauri::Builder::default()
    .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
    .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec![START_HIDDEN])))
    .invoke_handler(tauri::generate_handler![
      popup::toggle_pause,
      popup::stop,
      popup::fit_popup,
      get_settings,
      set_settings,
      models,
      voices,
      preview,
      engine_status,
      has_token,
      set_token,
      library_list,
      library_play,
      library_delete,
    ])
    .on_window_event(|window, event| {
      if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        window.hide().ok();
      }
    })
    .setup(move |app| {
      std::fs::create_dir_all(&data)?;
      std::fs::create_dir_all(&config)?;
      let store = Store::open(&config);
      let player = player::Player::start()?;
      let library = Arc::new(library::Library::open(&data)?);
      let engine = Engine::start(player.clone(), library.clone(), store.clone());
      server::start(&data, engine.clone(), player.clone())?;
      popup::start(app.handle().clone(), player.clone(), store.clone());
      app.manage(player);
      app.manage(store);
      app.manage(engine);
      app.manage(library);
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

#[tauri::command]
fn get_settings(store: State<Store>) -> Settings {
  store.get()
}

/// Saves the settings and gets their voice ready, so the next read starts at once.
#[tauri::command]
fn set_settings(store: State<Store>, engine: State<Engine>, settings: Settings) -> Result<(), String> {
  store.set(settings)?;
  engine.prime();
  Ok(())
}

#[tauri::command]
fn models() -> &'static [registry::Model] {
  registry::MODELS
}

/// Async so the first catalog download doesn't hold up the window.
#[tauri::command]
async fn voices() -> Result<Vec<CatalogVoice>, String> {
  engine::voices()
}

/// Reads a line in the voice and speed of the settings, without keeping it in the library.
#[tauri::command]
fn preview(engine: State<Engine>) -> Result<(), String> {
  let voice = engine.voice(None, None);
  let line = format!("Hi, I'm {}. This is how I sound reading to you.", voice.name);
  engine.speak(&line, None, voice).map(drop)
}

#[tauri::command]
fn engine_status(engine: State<Engine>) -> Status {
  engine.status()
}

/// Whether a Hugging Face token is saved. The token itself never goes back to the window.
#[tauri::command]
fn has_token() -> bool {
  settings::token_file(&config_dir()).exists()
}

/// Saves the token for gated models, or removes it when empty.
#[tauri::command]
fn set_token(engine: State<Engine>, token: String) -> Result<(), String> {
  let file = settings::token_file(&config_dir());
  let token = token.trim();
  let result = if token.is_empty() { std::fs::remove_file(&file) } else { std::fs::write(&file, token) };
  result.map_err(|error| error.to_string())?;
  engine.prime();
  Ok(())
}

#[tauri::command]
fn library_list(library: State<Arc<library::Library>>) -> Result<Vec<library::Read>, String> {
  library.list()
}

/// Plays a read from the library from piece number `from`.
#[tauri::command]
fn library_play(engine: State<Engine>, id: i64, from: usize) -> Result<(), String> {
  engine.replay(id, from).map(drop)
}

/// Removes a read, stopping it first when it plays.
#[tauri::command]
fn library_delete(library: State<Arc<library::Library>>, player: State<player::Player>, id: i64) -> Result<(), String> {
  if player.now().is_some_and(|now| now.library_id == Some(id)) {
    player.stop();
  }
  library.delete(id)
}
