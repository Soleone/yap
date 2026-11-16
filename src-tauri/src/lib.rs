mod engine;
mod grab;
mod language;
mod library;
mod phonon;
mod player;
mod popup;
mod registry;
mod server;
mod settings;
mod text;
mod worker;

use engine::{Engine, Status, Wanted};
use phonon::VoiceInfo;
use settings::{Settings, Store};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
  AppHandle, Manager, State, WindowEvent,
  menu::{Menu, MenuItem},
  tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// The `yap` command starts the app with this when it isn't running, so it goes straight to the tray.
const START_HIDDEN: &str = "--start-hidden";

/// Holding the hotkey repeats it; one grab at a time.
static GRABBING: AtomicBool = AtomicBool::new(false);

/// Pieces, the library, the port file, and the models unless the settings put them elsewhere.
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
  let store = Store::open(&config);
  let chosen = store.get().models_folder;
  let models_dir = if chosen.is_empty() { data.clone() } else { PathBuf::from(chosen) };
  // Before any thread starts, so nothing reads the environment while it changes. That's also why a new models
  // folder waits for a restart. Models go in yap's own folder, not the shared Hugging Face cache, so removing yap
  // removes them.
  unsafe {
    std::env::set_var("HF_HUB_CACHE", models_dir.join("models"));
    std::env::set_var("HF_TOKEN_PATH", settings::token_file(&config));
  }

  tauri::Builder::default()
    .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
    .plugin(
      tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _, event| {
          if event.state == ShortcutState::Pressed {
            let app = app.clone();
            std::thread::spawn(move || read_selection(&app));
          }
        })
        .build(),
    )
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_process::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
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
      prime,
      has_token,
      set_token,
      models_folder,
      choose_folder,
      restart,
      library_list,
      library_play,
      library_delete,
      library_keep,
      set_hotkey,
      pause_hotkey,
      platform,
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
      let hotkey = store.get().hotkey;
      if !hotkey.is_empty() {
        // A hotkey another app took meanwhile stays in the settings, where it can be changed.
        app.global_shortcut().register(hotkey.as_str()).ok();
      }
      let player = player::Player::start()?;
      let library = Arc::new(library::Library::open(&data)?);
      let engine = Engine::start(player.clone(), library.clone(), store.clone(), models_dir.clone());
      server::start(&data, engine.clone(), player.clone())?;
      popup::start(app.handle().clone(), player.clone(), store.clone());
      app.manage(player);
      app.manage(store);
      app.manage(engine);
      app.manage(library);
      app.manage(ModelsFolder(models_dir));
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

/// The voices of `model`. Async so the first catalog download doesn't hold up the window.
#[tauri::command]
async fn voices(model: String) -> Result<Vec<VoiceInfo>, String> {
  engine::voices(registry::model(&model))
}

/// Reads a line in the main voice and speed of the settings, without keeping it in the library.
#[tauri::command]
fn preview(engine: State<Engine>) -> Result<(), String> {
  let voice = engine.main_voice();
  let line = format!("Hi, I'm {}. This is how I sound reading to you.", voice.name);
  engine.speak(&line, None, voice).map(drop)
}

#[tauri::command]
fn engine_status(engine: State<Engine>) -> Status {
  engine.status()
}

/// Gets the voices of the settings ready again, after loading one failed.
#[tauri::command]
fn prime(engine: State<Engine>) {
  engine.prime();
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

/// Keeps a read from pruning, or lets it be pruned again.
#[tauri::command]
fn library_keep(library: State<Arc<library::Library>>, id: i64, kept: bool) -> Result<(), String> {
  library.keep(id, kept)
}

/// Reads the selected text of the app in front. With nothing selected, the hotkey stops what plays instead.
fn read_selection(app: &AppHandle) {
  if GRABBING.swap(true, Ordering::SeqCst) {
    return;
  }
  let selection = grab::selection();
  GRABBING.store(false, Ordering::SeqCst);
  let engine = app.state::<Engine>();
  match selection {
    Ok((text, source)) if !text.trim().is_empty() => {
      if let Err(error) = engine.speak(&text, Some(&source), engine.voice(&text, Wanted::default())) {
        eprintln!("reading the selection: {error}");
      }
    }
    Ok(_) => app.state::<player::Player>().stop(),
    Err(error) => eprintln!("reading the selection: {error}"),
  }
}

/// Registers `hotkey` and saves it, or turns the hotkey off when empty. A hotkey that can't be registered leaves the
/// current one in place.
#[tauri::command]
fn set_hotkey(app: AppHandle, store: State<Store>, hotkey: String) -> Result<(), String> {
  let shortcuts = app.global_shortcut();
  shortcuts.unregister_all().ok();
  let mut settings = store.get();
  if !hotkey.is_empty() && shortcuts.register(hotkey.as_str()).is_err() {
    if !settings.hotkey.is_empty() {
      shortcuts.register(settings.hotkey.as_str()).ok();
    }
    return Err("That hotkey is taken by the system or another app.".to_string());
  }
  settings.hotkey = hotkey;
  store.set(settings)
}

/// Registered hotkeys never reach the page, so recording a new one turns the current one off.
#[tauri::command]
fn pause_hotkey(app: AppHandle) {
  app.global_shortcut().unregister_all().ok();
}

/// "windows", "macos" or "linux".
#[tauri::command]
fn platform() -> &'static str {
  std::env::consts::OS
}

/// The models folder in use, which a new one in the settings replaces at the next start.
struct ModelsFolder(PathBuf);

#[tauri::command]
fn models_folder(folder: State<ModelsFolder>) -> PathBuf {
  folder.0.clone()
}

/// The folder picked in the system's folder dialog, which opens at `start`; none when it was cancelled. Async so the
/// open dialog doesn't hold up the window.
#[tauri::command]
async fn choose_folder(app: AppHandle, start: PathBuf) -> Option<PathBuf> {
  app.dialog().file().set_title("Choose a models folder").set_directory(start).blocking_pick_folder()?.into_path().ok()
}

#[tauri::command]
fn restart(app: AppHandle) {
  app.restart();
}
