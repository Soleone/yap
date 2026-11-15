//! The status popup: what plays, with subtitles, at the bottom center of the screen. It never takes focus.

use crate::player::{Now, Player};
use crate::settings::{PopupStyle, Position, Store};
use std::time::Duration;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State};

const WIDTH: f64 = 480.0;
/// Space between the popup and the edge of the work area, in logical pixels.
const MARGIN: f64 = 40.0;
const TICK: Duration = Duration::from_millis(50);
/// A read that ends and a new one starting right after keep the popup up in between.
const HIDE_AFTER_TICKS: u32 = 6;

/// Sends what plays to the popup every tick while a read plays, and `null` once nothing played for a moment. The page
/// sizes the window to its content through `fit_popup`, which also shows it.
pub fn start(app: AppHandle, player: Player, settings: Store) {
  std::thread::spawn(move || {
    let mut idle = HIDE_AFTER_TICKS + 1;
    loop {
      std::thread::sleep(TICK);
      match player.now() {
        Some(now) => {
          idle = 0;
          let style = settings.get().popup;
          if style != PopupStyle::Off {
            app.emit_to("popup", "yap://now", Some(Shown { style, now })).ok();
          }
        }
        None => {
          idle = idle.saturating_add(1);
          if idle == HIDE_AFTER_TICKS {
            app.emit_to("popup", "yap://now", None::<Shown>).ok();
          }
        }
      }
    }
  });
}

#[derive(Clone, serde::Serialize)]
struct Shown {
  style: PopupStyle,
  #[serde(flatten)]
  now: Now,
}

#[tauri::command]
pub fn toggle_pause(player: State<Player>) {
  player.toggle_pause();
}

#[tauri::command]
pub fn stop(player: State<Player>) {
  player.stop();
}

/// Sizes the popup to its content, `height` logical pixels, centered at the top or bottom of the screen under the
/// cursor. With no content it hides.
#[tauri::command]
pub fn fit_popup(app: AppHandle, settings: State<Store>, height: f64) -> Result<(), String> {
  let window = app.get_webview_window("popup").ok_or("the popup window is in tauri.conf.json")?;
  if height <= 0.0 {
    return window.hide().map_err(|error| error.to_string());
  }
  let place = || -> tauri::Result<()> {
    let cursor = app.cursor_position()?;
    let Some(monitor) = app.monitor_from_point(cursor.x, cursor.y)?.or(app.primary_monitor()?) else { return Ok(()) };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let x = area.position.x + (area.size.width as i32 - (WIDTH * scale) as i32) / 2;
    let y = match settings.get().position {
      Position::Top => area.position.y + (MARGIN * scale) as i32,
      Position::Bottom => area.position.y + area.size.height as i32 - ((height + MARGIN) * scale) as i32,
    };
    window.set_size(LogicalSize::new(WIDTH, height))?;
    window.set_position(PhysicalPosition::new(x, y))?;
    window.show()
  };
  place().map_err(|error| error.to_string())
}
