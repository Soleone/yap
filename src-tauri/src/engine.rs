//! The voice model, loaded once and kept with a primed session, turning reads into audio for the player.

use crate::library::{self, Library, Source};
use crate::player::Player;
use crate::registry::{self, Model as ModelInfo, Speed};
use crate::settings::Store;
use crate::text;
use ptts::checkpoint::{Checkpoint, ResolveOptions};
use ptts::preprocess::Lang;
use ptts::synth::{DeviceKind, Quant, Session, SpeechOptions, Synth};
use ptts::voice_catalog::{DEFAULT_VOICE_CATALOG, VoiceCatalog};
use serde::Serialize;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Generation stops running ahead of playback once this much audio waits, so a long text doesn't fill memory.
const AHEAD_SECS: f32 = 30.0;

/// Everything that decides how a piece sounds, and so which stored file it plays from.
#[derive(Clone)]
pub struct Voice {
  model: &'static ModelInfo,
  language: String,
  pub name: String,
  speed: &'static Speed,
}

impl Voice {
  fn id(&self) -> String {
    format!("{} {} {} {}", self.model.source, self.language, self.name, self.speed.duration_delta)
  }
}

/// What the settings window shows about the model.
#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Status {
  Loading { model: String },
  Ready,
  Failed { error: String },
}

#[derive(Clone)]
pub struct Engine {
  jobs: Sender<Job>,
  player: Player,
  library: Arc<Library>,
  settings: Store,
  status: Arc<Mutex<Status>>,
}

enum Job {
  Speak {
    read: u64,
    voice: Voice,
    /// Each piece's text and its hash, in order.
    pieces: Vec<(String, String)>,
  },
  /// Loads the model and primes the voice ahead of the next read.
  Prime(Voice),
}

impl Engine {
  /// Starts the engine thread, which loads the model and primes the voice in the settings right away, so the first
  /// read starts at once.
  pub fn start(player: Player, library: Arc<Library>, settings: Store) -> Engine {
    let (jobs, rx) = channel();
    let status = Arc::new(Mutex::new(Status::Ready));
    let engine = Engine { jobs, player, library, settings, status };
    let worker = engine.clone();
    std::thread::spawn(move || worker.run(rx));
    engine.prime();
    engine
  }

  /// The voice in the settings, with `name` or `speed` instead when given.
  pub fn voice(&self, name: Option<String>, speed: Option<f32>) -> Voice {
    let settings = self.settings.get();
    let model = registry::model(&settings.model);
    Voice {
      model,
      language: settings.language().to_string(),
      name: name.unwrap_or(settings.voice),
      speed: model.speed(speed.unwrap_or(settings.speed)),
    }
  }

  /// Gets the voice in the settings ready, after they changed.
  pub fn prime(&self) {
    self.jobs.send(Job::Prime(self.voice(None, None))).ok();
  }

  pub fn status(&self) -> Status {
    self.status.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
  }

  /// Stops what plays and starts reading `text`, recorded in the library when it has a `source`. Returns the
  /// player's id for the read.
  pub fn speak(&self, text: &str, source: Option<&Source>, voice: Voice) -> Result<u64, String> {
    let voice_id = voice.id();
    let pieces: Vec<(String, String)> = text::pieces(&text::prepare(text))
      .into_iter()
      .map(|piece| {
        let hash = library::piece_hash(&voice_id, &text::words(&piece));
        (piece, hash)
      })
      .collect();
    if let Some(source) = source {
      let hashes: Vec<String> = pieces.iter().map(|(_, hash)| hash.clone()).collect();
      self.library.add(text, source, &voice.name, &hashes)?;
    }
    let texts = pieces.iter().map(|(piece, _)| piece.clone()).collect();
    let app = source.map_or("", |source| &source.app);
    let read = self.player.begin(texts, &voice.name, app);
    self.jobs.send(Job::Speak { read, voice, pieces }).map_err(|_| "the engine stopped".to_string())?;
    Ok(read)
  }

  fn run(self, jobs: Receiver<Job>) {
    let mut model = None;
    for job in jobs {
      match job {
        Job::Speak { read, voice, pieces } => {
          let result = self.generate(&mut model, read, &voice, &pieces);
          self.player.finish(read, result.err());
        }
        Job::Prime(voice) => {
          self.session(&mut model, &voice).ok();
        }
      }
    }
  }

  /// Plays each piece from disk when it was read before, and generates and stores it otherwise. Returns early once
  /// another read took over.
  fn generate(
    &self,
    model: &mut Option<Model>,
    read: u64,
    voice: &Voice,
    pieces: &[(String, String)],
  ) -> Result<(), String> {
    let current = || self.player.current() == read;
    for (index, (piece, hash)) in pieces.iter().enumerate() {
      while current() && self.player.backlog() > AHEAD_SECS {
        std::thread::sleep(Duration::from_millis(100));
      }
      if !current() {
        return Ok(());
      }
      if let Some(audio) = self.library.read_piece(hash) {
        self.player.push(read, index, &audio);
        continue;
      }
      let mut audio = Vec::new();
      for chunk in self.session(model, voice)?.stream(piece).map_err(|error| error.to_string())? {
        let chunk = chunk.map_err(|error| error.to_string())?;
        if !current() {
          return Ok(());
        }
        self.player.push(read, index, &chunk);
        audio.extend_from_slice(&chunk);
      }
      self.library.write_piece(hash, &audio)?;
    }
    Ok(())
  }

  /// A session primed for `voice`, loading or switching the model first when needed. Keeps the status up to date.
  fn session<'a>(&self, model: &'a mut Option<Model>, voice: &Voice) -> Result<&'a Session, String> {
    let set = |status| *self.status.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = status;
    if model.as_ref().is_none_or(|model| !model.serves(voice)) {
      set(Status::Loading { model: voice.model.name.to_string() });
      *model = None;
      match Model::load(voice) {
        Ok(loaded) => *model = Some(loaded),
        Err(error) => {
          set(Status::Failed { error: error.clone() });
          return Err(error);
        }
      }
    }
    let session = model.as_mut().expect("loaded above").session(voice);
    set(match &session {
      Ok(_) => Status::Ready,
      Err(error) => Status::Failed { error: error.clone() },
    });
    session
  }
}

struct Model {
  model: &'static str,
  language: String,
  synth: Synth,
  catalog: VoiceCatalog,
  /// Priming a voice takes about half a second; a session keeps that done for every read in the voice and speed.
  primed: Option<(String, &'static str, Session)>,
}

impl Model {
  fn load(voice: &Voice) -> Result<Model, String> {
    let error = |error: ptts::Error| error.to_string();
    let checkpoint =
      Checkpoint::locate(voice.model.source, ResolveOptions { quant: Quant::Q80, weights: None }).map_err(error)?;
    let language = Lang::from_str(&voice.language).map_err(error)?;
    let mut synth = checkpoint.builder(language).device(DeviceKind::Cpu).build().map_err(error)?;
    checkpoint.register_voices(&mut synth);
    let catalog = VoiceCatalog::new(DEFAULT_VOICE_CATALOG).map_err(error)?;
    Ok(Model { model: voice.model.id, language: voice.language.clone(), synth, catalog, primed: None })
  }

  fn serves(&self, voice: &Voice) -> bool {
    self.model == voice.model.id && self.language == voice.language
  }

  fn session(&mut self, voice: &Voice) -> Result<&Session, String> {
    let delta = voice.speed.duration_delta;
    if self.primed.as_ref().is_none_or(|(name, primed_delta, _)| *name != voice.name || *primed_delta != delta) {
      let error = |error: ptts::Error| error.to_string();
      self.synth.set_conditions(HashMap::from([("duration_delta".to_string(), delta.to_string())])).map_err(error)?;
      let name = self.catalog.register(&mut self.synth, &voice.name).map_err(error)?;
      let session = self.synth.session_default(&SpeechOptions::default().voice(name)).map_err(error)?;
      self.primed = Some((voice.name.clone(), delta, session));
    }
    Ok(&self.primed.as_ref().expect("primed above").2)
  }
}

/// A voice in the catalog, for the settings window.
#[derive(Serialize)]
pub struct CatalogVoice {
  name: String,
  language: String,
  description: String,
}

/// Every voice in the catalog. Downloads the catalog the first time.
pub fn voices() -> Result<Vec<CatalogVoice>, String> {
  let catalog = VoiceCatalog::new(DEFAULT_VOICE_CATALOG).map_err(|error| error.to_string())?;
  let voices = catalog.voices().map_err(|error| error.to_string())?;
  Ok(
    voices
      .iter()
      .map(|voice| CatalogVoice {
        name: voice.name.clone(),
        language: voice.language.clone().unwrap_or_default(),
        description: voice.description.clone().unwrap_or_default(),
      })
      .collect(),
  )
}
