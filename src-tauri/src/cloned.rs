//! Voices cloned from a recording or an uploaded clip, which every model that clones reads in. Each is a 24 kHz WAV and
//! what it says, in the `clones` folder of the data folder, listed in `clones.json` there.

use crate::registry;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Shorter clips give too little of the voice to clone.
const MIN_SECS: usize = 3;
const RATE: u32 = 24_000;

#[derive(Clone, Serialize, Deserialize)]
pub struct ClonedVoice {
  pub name: String,
  /// What the recording says, which the models read it with.
  pub transcript: String,
  file: String,
}

pub struct Cloned {
  folder: PathBuf,
  voices: Mutex<Vec<ClonedVoice>>,
}

impl Cloned {
  /// Reads the list in `data`. A missing or unreadable list has no voices.
  pub fn open(data: &Path) -> Cloned {
    let folder = data.join("clones");
    let voices = std::fs::read_to_string(folder.join("clones.json"))
      .ok()
      .and_then(|json| serde_json::from_str(&json).ok())
      .unwrap_or_default();
    Cloned { folder, voices: Mutex::new(voices) }
  }

  pub fn list(&self) -> Vec<ClonedVoice> {
    self.lock().clone()
  }

  /// The recording of the voice `name` and what it says.
  pub fn find(&self, name: &str) -> Option<(PathBuf, String)> {
    let voices = self.lock();
    let voice = voices.iter().find(|voice| voice.name == name)?;
    Some((self.folder.join(&voice.file), voice.transcript.clone()))
  }

  /// Keeps `pcm`, 16-bit little-endian mono samples at 24 kHz, as the clip a voice is about to be cloned from.
  pub fn stage(&self, pcm: &[u8]) -> Result<(), String> {
    if pcm.len() / 2 < MIN_SECS * RATE as usize {
      return Err(format!("The clip is too short; record at least {MIN_SECS} seconds."));
    }
    std::fs::create_dir_all(&self.folder).map_err(|error| error.to_string())?;
    let spec =
      hound::WavSpec { channels: 1, sample_rate: RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut writer = hound::WavWriter::create(self.clip(), spec).map_err(|error| error.to_string())?;
    for sample in pcm.chunks_exact(2) {
      writer.write_sample(i16::from_le_bytes([sample[0], sample[1]])).map_err(|error| error.to_string())?;
    }
    writer.finalize().map_err(|error| error.to_string())
  }

  /// The clip `stage` kept.
  pub fn clip(&self) -> PathBuf {
    self.folder.join("clip.wav")
  }

  /// Adds the staged clip as the voice `name`, saying `transcript`.
  pub fn add(&self, name: &str, transcript: &str) -> Result<(), String> {
    let (name, transcript) = (name.trim(), transcript.trim());
    if name.is_empty() || transcript.is_empty() {
      return Err("A cloned voice needs a name and what its clip says.".to_string());
    }
    let mut voices = self.lock();
    let stock = registry::MODELS.iter().filter(|model| model.clones).any(|model| model.voice(name).is_some());
    if stock || voices.iter().any(|voice| voice.name == name) {
      return Err(format!("There is already a voice named {name}."));
    }
    // Named by time rather than by name, which may hold anything.
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
    let file = format!("{millis}.wav");
    std::fs::rename(self.clip(), self.folder.join(&file)).map_err(|error| error.to_string())?;
    voices.push(ClonedVoice { name: name.to_string(), transcript: transcript.to_string(), file });
    self.save(&voices)
  }

  /// Removes the voice `name` and its recording.
  pub fn delete(&self, name: &str) -> Result<(), String> {
    let mut voices = self.lock();
    if let Some(index) = voices.iter().position(|voice| voice.name == name) {
      let voice = voices.remove(index);
      self.save(&voices)?;
      std::fs::remove_file(self.folder.join(voice.file)).ok();
    }
    Ok(())
  }

  fn save(&self, voices: &[ClonedVoice]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(voices).map_err(|error| error.to_string())?;
    std::fs::write(self.folder.join("clones.json"), json).map_err(|error| error.to_string())
  }

  fn lock(&self) -> std::sync::MutexGuard<'_, Vec<ClonedVoice>> {
    self.voices.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // settings.clone
  #[test]
  fn a_cloned_voice_needs_a_long_enough_clip_and_a_name_of_its_own() {
    let dir = std::env::temp_dir().join(format!("yap-cloned-test-{}", std::process::id()));
    let cloned = Cloned::open(&dir);
    assert!(cloned.stage(&[0; 2 * 24_000]).is_err());
    let clip = [0; 2 * 4 * 24_000];

    cloned.stage(&clip).unwrap();
    cloned.add(" Dad ", "Hello there.").unwrap();
    cloned.stage(&clip).unwrap();
    assert!(cloned.add("Dad", "Hello again.").is_err());
    assert!(cloned.add("radio-man", "Hello again.").is_err());

    let reopened = Cloned::open(&dir);
    let (audio, transcript) = reopened.find("Dad").unwrap();
    assert!(audio.exists());
    assert_eq!(transcript, "Hello there.");
    reopened.delete("Dad").unwrap();
    assert!(!audio.exists() && Cloned::open(&dir).find("Dad").is_none());
    std::fs::remove_dir_all(dir).ok();
  }
}
