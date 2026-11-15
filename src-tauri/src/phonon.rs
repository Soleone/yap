//! Phonon models, run in process through `ptts`, with their voices from the Phonon catalog.

use crate::registry::Model;
use ptts::checkpoint::{Checkpoint, ResolveOptions};
use ptts::preprocess::Lang;
use ptts::synth::{DeviceKind, Quant, Session, SpeechOptions, Synth};
use ptts::voice_catalog::{DEFAULT_VOICE_CATALOG, VoiceCatalog};
use std::collections::HashMap;
use std::str::FromStr;

pub struct Phonon {
  synth: Synth,
  catalog: VoiceCatalog,
  /// Priming a voice takes about half a second; a session keeps that done for every read in the voice and speed.
  primed: Option<(String, &'static str, Session)>,
}

fn error(error: ptts::Error) -> String {
  error.to_string()
}

impl Phonon {
  pub fn load(model: &Model, language: &str) -> Result<Phonon, String> {
    let checkpoint =
      Checkpoint::locate(model.source, ResolveOptions { quant: Quant::Q80, weights: None }).map_err(error)?;
    let mut synth =
      checkpoint.builder(Lang::from_str(language).map_err(error)?).device(DeviceKind::Cpu).build().map_err(error)?;
    checkpoint.register_voices(&mut synth);
    let catalog = VoiceCatalog::new(DEFAULT_VOICE_CATALOG).map_err(error)?;
    Ok(Phonon { synth, catalog, primed: None })
  }

  /// A session primed for the catalog voice `name` at the speed `delta`, the `duration_delta` condition.
  pub fn session(&mut self, name: &str, delta: &'static str) -> Result<&Session, String> {
    if self.primed.as_ref().is_none_or(|(primed, primed_delta, _)| primed != name || *primed_delta != delta) {
      self.synth.set_conditions(HashMap::from([("duration_delta".to_string(), delta.to_string())])).map_err(error)?;
      let registered = self.catalog.register(&mut self.synth, name).map_err(error)?;
      let session = self.synth.session_default(&SpeechOptions::default().voice(registered)).map_err(error)?;
      self.primed = Some((name.to_string(), delta, session));
    }
    Ok(&self.primed.as_ref().expect("primed above").2)
  }
}

/// A voice for the settings window.
#[derive(serde::Serialize)]
pub struct VoiceInfo {
  pub name: String,
  pub language: String,
  pub description: String,
}

/// Every voice in the Phonon catalog. Downloads the catalog the first time.
pub fn voices() -> Result<Vec<VoiceInfo>, String> {
  let catalog = VoiceCatalog::new(DEFAULT_VOICE_CATALOG).map_err(error)?;
  let voices = catalog.voices().map_err(error)?;
  Ok(
    voices
      .iter()
      .map(|voice| VoiceInfo {
        name: voice.name.clone(),
        language: voice.language.clone().unwrap_or_default(),
        description: voice.description.clone().unwrap_or_default(),
      })
      .collect(),
  )
}
