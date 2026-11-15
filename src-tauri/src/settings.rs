//! What the settings window changes, kept as JSON in the config folder.

use crate::registry;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
  pub model: String,
  /// For a model with several languages.
  pub language: String,
  pub voice: String,
  /// A multiplier; reads use the model's offered speed closest to it.
  pub speed: f32,
  pub popup: PopupStyle,
  pub position: Position,
  /// Reads the selected text of any app, like `Ctrl+Alt+R`. Empty is off.
  pub hotkey: String,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PopupStyle {
  Subtitles,
  Minimal,
  Off,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Position {
  Top,
  Bottom,
}

impl Default for Settings {
  fn default() -> Self {
    Settings {
      model: registry::MODELS[0].id.to_string(),
      language: "en".to_string(),
      voice: "Miles".to_string(),
      speed: 1.0,
      popup: PopupStyle::Subtitles,
      position: Position::Bottom,
      hotkey: String::new(),
    }
  }
}

impl Settings {
  /// The language reads use: the chosen one when the model has it, else the model's first.
  pub fn language(&self) -> &str {
    let languages = registry::model(&self.model).languages;
    languages.iter().find(|&&code| code == self.language).unwrap_or(&languages[0])
  }
}

/// The settings shared by the engine, the popup and the window, saved on every change.
#[derive(Clone)]
pub struct Store {
  settings: Arc<Mutex<Settings>>,
  file: PathBuf,
}

impl Store {
  /// Reads `settings.json` in `dir`. A missing or unreadable file gives the defaults.
  pub fn open(dir: &Path) -> Store {
    let file = dir.join("settings.json");
    let settings =
      std::fs::read_to_string(&file).ok().and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default();
    Store { settings: Arc::new(Mutex::new(settings)), file }
  }

  pub fn get(&self) -> Settings {
    self.settings.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
  }

  pub fn set(&self, settings: Settings) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&settings).map_err(|error| error.to_string())?;
    std::fs::write(&self.file, json).map_err(|error| error.to_string())?;
    *self.settings.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = settings;
    Ok(())
  }
}

/// Where the Hugging Face token for gated models is kept, apart from the settings so they never carry it.
pub fn token_file(config: &Path) -> PathBuf {
  config.join("hf-token")
}
