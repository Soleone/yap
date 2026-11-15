//! Reading the selected text, and which app and window it is in, when the hotkey is pressed. From Pluck's grab
//! module, without pasting or focusing.

use crate::library::Source;
#[cfg(not(target_os = "linux"))]
use std::{thread::sleep, time::Duration};

/// The selected text of the app in front, and where it is. Empty text when nothing is selected.
pub fn selection() -> Result<(String, Source), String> {
  let mut source = platform::source();
  let (text, url) = copy_selection()?;
  source.url = url;
  Ok((text, source))
}

/// Copies the selection with the copy shortcut and puts the previous clipboard text back. An unchanged clipboard
/// means nothing was selected. Only text survives the round trip.
#[cfg(not(target_os = "linux"))]
fn copy_selection() -> Result<(String, String), String> {
  let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
  let previous = clipboard.get_text().ok();
  let before = platform::change_count();

  platform::wait_for_modifiers_released();
  platform::send_copy();
  let copied = (0..50).any(|_| {
    sleep(Duration::from_millis(10));
    before != platform::change_count()
  });
  if !copied {
    return Ok(Default::default());
  }

  let selection = (clipboard.get_text().unwrap_or_default(), platform::source_url());
  let restored = match previous {
    Some(previous) => clipboard.set_text(previous),
    None => clipboard.clear(),
  };
  restored.map_err(|error| format!("Couldn't restore the clipboard: {error}"))?;
  Ok(selection)
}

/// The primary selection holds what was last highlighted, so the clipboard is never touched.
#[cfg(target_os = "linux")]
fn copy_selection() -> Result<(String, String), String> {
  let output = std::process::Command::new("wl-paste")
    .args(["--primary", "--no-newline", "--type", "text"])
    .output()
    .map_err(|error| format!("Reading the selection needs wl-clipboard: {error}"))?;
  Ok((String::from_utf8_lossy(&output.stdout).into_owned(), String::new()))
}

#[cfg(windows)]
mod platform {
  use crate::library::Source;
  use std::{mem::size_of, thread::sleep, time::Duration};
  use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND},
    System::{
      DataExchange::GetClipboardSequenceNumber,
      Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW},
    },
    UI::{
      Input::KeyboardAndMouse::{
        GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_C,
        VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
      },
      WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId},
    },
  };

  pub fn source() -> Source {
    let window = unsafe { GetForegroundWindow() };
    Source { app: process_name(window), title: window_title(window), url: String::new() }
  }

  pub fn change_count() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
  }

  /// Browsers put a `SourceURL:` line in the header of the clipboard's HTML.
  pub fn source_url() -> String {
    let Some(format) = clipboard_win::register_format("HTML Format") else { return String::new() };
    let Ok(_open) = clipboard_win::Clipboard::new_attempts(10) else { return String::new() };
    let mut html = Vec::new();
    if clipboard_win::raw::get_vec(format.get(), &mut html).is_err() {
      return String::new();
    }
    let html = String::from_utf8_lossy(&html);
    let header = html.lines().take_while(|line| !line.starts_with('<'));
    let url = header.filter_map(|line| line.strip_prefix("SourceURL:")).map(str::trim).next().unwrap_or_default();
    // Electron apps like Mailspring name their own files here, which says nothing about the text.
    if url.starts_with("http://") || url.starts_with("https://") { url.to_string() } else { String::new() }
  }

  /// A shortcut sent while the hotkey's Alt is still held arrives as Ctrl+Alt+C.
  pub fn wait_for_modifiers_released() {
    let modifiers = [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN];
    for _ in 0..100 {
      if modifiers.iter().all(|&key| unsafe { GetAsyncKeyState(key as i32) } >= 0) {
        return;
      }
      sleep(Duration::from_millis(10));
    }
  }

  pub fn send_copy() {
    let inputs = [key(VK_CONTROL, false), key(VK_C, false), key(VK_C, true), key(VK_CONTROL, true)];
    unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
  }

  fn key(key: VIRTUAL_KEY, up: bool) -> INPUT {
    let flags = if up { KEYEVENTF_KEYUP } else { 0 };
    let ki = KEYBDINPUT { wVk: key, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 };
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki } }
  }

  fn window_title(window: HWND) -> String {
    let mut buffer = [0u16; 512];
    let length = unsafe { GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
  }

  /// The executable's name without extension, like "chrome" or "Code".
  fn process_name(window: HWND) -> String {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
      return String::new();
    }
    let mut buffer = [0u16; 1024];
    let mut length = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut length) };
    unsafe { CloseHandle(process) };
    if ok == 0 {
      return String::new();
    }
    let path = String::from_utf16_lossy(&buffer[..length as usize]);
    std::path::Path::new(&path).file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default()
  }
}

/// Sending keys needs the Accessibility permission, which macOS asks the user for once.
#[cfg(target_os = "macos")]
mod platform {
  use crate::library::Source;
  use objc2_app_kit::{NSPasteboard, NSWorkspace};
  use objc2_core_graphics::{CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation};
  use std::{thread::sleep, time::Duration};

  /// Virtual key code of C on an ANSI layout.
  const KEY_C: u16 = 8;

  /// The window title needs the Accessibility API, which yap doesn't ask for.
  pub fn source() -> Source {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication();
    let name = app.and_then(|app| app.localizedName()).map(|name| name.to_string()).unwrap_or_default();
    Source { app: name, ..Source::default() }
  }

  pub fn change_count() -> isize {
    NSPasteboard::generalPasteboard().changeCount()
  }

  pub fn source_url() -> String {
    String::new()
  }

  pub fn wait_for_modifiers_released() {
    let held =
      CGEventFlags::MaskCommand | CGEventFlags::MaskAlternate | CGEventFlags::MaskShift | CGEventFlags::MaskControl;
    for _ in 0..100 {
      if !CGEventSource::flags_state(CGEventSourceStateID::HIDSystemState).intersects(held) {
        return;
      }
      sleep(Duration::from_millis(10));
    }
  }

  pub fn send_copy() {
    for down in [true, false] {
      if let Some(event) = CGEvent::new_keyboard_event(None, KEY_C, down) {
        CGEvent::set_flags(Some(&event), CGEventFlags::MaskCommand);
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
      }
    }
  }
}

/// Hyprland on Wayland names the window; the selection comes from `wl-paste`.
#[cfg(target_os = "linux")]
mod platform {
  use crate::library::Source;

  pub fn source() -> Source {
    let window = std::process::Command::new("hyprctl")
      .args(["activewindow", "-j"])
      .output()
      .ok()
      .and_then(|output| serde_json::from_slice::<serde_json::Value>(&output.stdout).ok())
      .unwrap_or_default();
    let field = |name: &str| window[name].as_str().unwrap_or_default().to_string();
    Source { app: field("class"), title: field("title"), url: String::new() }
  }
}
