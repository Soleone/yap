//! The voice models, loaded once and kept, turning reads into audio for the player, and saved reads into pieces in
//! the library. Reads and saved reads each have a thread and a model of their own, so a long save never holds up a
//! read.

use crate::library::{self, Library, Sound, Source};
use crate::phonon::{self, Phonon, VoiceInfo};
use crate::player::Player;
use crate::registry::{self, Model as ModelInfo, Runtime, Speed};
use crate::settings::Store;
use crate::text;
use crate::worker::Worker;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Generation stops running ahead of playback once this much audio waits, so a long text doesn't fill memory.
const AHEAD_SECS: f32 = 30.0;

/// How often old reads are pruned while the app runs.
const PRUNE_EVERY: Duration = Duration::from_secs(60 * 60);

/// Everything that decides how a piece sounds, and so which stored file it plays from.
#[derive(Clone)]
pub struct Voice {
  model: &'static ModelInfo,
  language: &'static str,
  pub name: String,
  speed: &'static Speed,
}

impl Voice {
  fn new(model: &str, language: &str, name: String, speed: f32) -> Voice {
    let model = registry::model(model);
    Voice { model, language: model.language(language), name, speed: model.speed(speed) }
  }

  fn id(&self) -> String {
    format!("{} {} {} {}", self.model.source, self.language, self.name, self.speed.control)
  }

  fn sound(&self) -> Sound {
    Sound {
      model: self.model.id.to_string(),
      language: self.language.to_string(),
      voice: self.name.clone(),
      speed: self.speed.speed,
    }
  }

  fn of(sound: Sound) -> Voice {
    Voice::new(&sound.model, &sound.language, sound.voice, sound.speed)
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
  /// Tells the saving thread a read is waiting.
  saves: Sender<()>,
  player: Player,
  library: Arc<Library>,
  settings: Store,
  status: Arc<Mutex<Status>>,
  data: PathBuf,
}

enum Job {
  Speak {
    read: u64,
    voice: Voice,
    /// Each piece's text and its hash, in order, and the piece to start from.
    pieces: Vec<(String, String)>,
    from: usize,
  },
  /// Loads the model and primes the voice ahead of the next read.
  Prime(Voice),
}

impl Engine {
  /// Starts the reading thread, which loads the model and primes the voice in the settings right away so the first
  /// read starts at once, and the saving thread, which makes the reads saved for later, those from before a restart
  /// first, and prunes old reads.
  pub fn start(player: Player, library: Arc<Library>, settings: Store, data: PathBuf) -> Engine {
    let (jobs, job_rx) = channel();
    let (saves, save_rx) = channel();
    let status = Arc::new(Mutex::new(Status::Ready));
    let engine = Engine { jobs, saves, player, library, settings, status, data };
    let reader = engine.clone();
    std::thread::spawn(move || reader.read(job_rx));
    let saver = engine.clone();
    std::thread::spawn(move || saver.make_saved(save_rx));
    engine.prime();
    engine
  }

  /// The voice in the settings, with `name` or `speed` instead when given.
  pub fn voice(&self, name: Option<String>, speed: Option<f32>) -> Voice {
    let settings = self.settings.get();
    Voice::new(&settings.model, &settings.language, name.unwrap_or(settings.voice), speed.unwrap_or(settings.speed))
  }

  /// The voice saved reads use: the one in the settings for them, or the reading voice when none is set.
  pub fn save_voice(&self, name: Option<String>, speed: Option<f32>) -> Voice {
    let settings = self.settings.get();
    if settings.save_model.is_empty() {
      return self.voice(name, speed);
    }
    // Empty when the model's voices couldn't be listed as it was chosen.
    let saved = Some(settings.save_voice).filter(|voice| !voice.is_empty());
    let name = name.or(saved).unwrap_or(settings.voice);
    Voice::new(&settings.save_model, &settings.language, name, speed.unwrap_or(settings.speed))
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
    let pieces = pieces(text, &voice);
    let library_id = match source {
      Some(source) => Some(self.library.add(text, source, &voice.sound(), &hashes(&pieces), false)?),
      None => None,
    };
    self.play(library_id, pieces, source.map_or("", |source| &source.app), voice, 0)
  }

  /// Records `text` in the library to be made in the background without playing, and returns its id.
  pub fn save(&self, text: &str, source: &Source, voice: Voice) -> Result<i64, String> {
    let id = self.library.add(text, source, &voice.sound(), &hashes(&pieces(text, &voice)), true)?;
    self.saves.send(()).ok();
    Ok(id)
  }

  /// Plays a read from the library again from piece number `from`, the way it sounded the first time.
  pub fn replay(&self, id: i64, from: usize) -> Result<u64, String> {
    let (text, source, sound) = self.library.get(id)?;
    self.library.used(id)?;
    let voice = Voice::of(sound);
    self.play(Some(id), pieces(&text, &voice), &source.app, voice, from)
  }

  fn play(
    &self,
    library_id: Option<i64>,
    pieces: Vec<(String, String)>,
    app: &str,
    voice: Voice,
    from: usize,
  ) -> Result<u64, String> {
    let texts = pieces.iter().map(|(piece, _)| piece.clone()).collect();
    let read = self.player.begin(library_id, texts, &voice.name, app);
    self.jobs.send(Job::Speak { read, voice, pieces, from }).map_err(|_| "the engine stopped".to_string())?;
    Ok(read)
  }

  fn read(self, jobs: Receiver<Job>) {
    let mut maker = Maker::new(self.data.clone(), "read", Some(self.status.clone()));
    for job in jobs {
      match job {
        Job::Speak { read, voice, pieces, from } => {
          let result = self.generate(&mut maker, read, &voice, &pieces, from);
          self.player.finish(read, result.err());
        }
        Job::Prime(voice) => {
          maker.prime(&voice).ok();
        }
      }
    }
  }

  /// Plays each piece from disk when it was read before, and generates and stores it otherwise. Returns early once
  /// another read took over.
  fn generate(
    &self,
    maker: &mut Maker,
    read: u64,
    voice: &Voice,
    pieces: &[(String, String)],
    from: usize,
  ) -> Result<(), String> {
    let current = || self.player.current() == read;
    for (index, (piece, hash)) in pieces.iter().enumerate().skip(from) {
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
      let made = maker.piece(voice, piece, |chunk| {
        self.player.push(read, index, chunk);
        current()
      })?;
      match made {
        Some(audio) => self.library.write_piece(hash, &audio)?,
        None => return Ok(()),
      }
    }
    Ok(())
  }

  /// Makes saved reads one at a time, oldest first, and prunes old reads every so often. Waits for `wake` when
  /// nothing is left.
  fn make_saved(self, wake: Receiver<()>) {
    let mut maker = Maker::new(self.data.clone(), "save", None);
    let mut pruned: Option<Instant> = None;
    loop {
      if pruned.is_none_or(|at| at.elapsed() > PRUNE_EVERY) {
        if let Err(error) = self.library.prune(self.settings.get().keep_days) {
          eprintln!("pruning the library: {error}");
        }
        pruned = Some(Instant::now());
      }
      let next = match self.library.next_saving() {
        Ok(next) => next,
        Err(error) => {
          eprintln!("finding saved reads: {error}");
          None
        }
      };
      let Some((id, text, sound)) = next else {
        // A GPU model holds gigabytes of video memory, so it goes while nothing waits.
        maker.loaded = None;
        match wake.recv_timeout(PRUNE_EVERY) {
          Ok(()) | Err(RecvTimeoutError::Timeout) => continue,
          Err(RecvTimeoutError::Disconnected) => return,
        }
      };
      let voice = Voice::of(sound);
      let result = self.make_pieces(&mut maker, id, &voice, &pieces(&text, &voice));
      if let Err(error) = self.library.saved(id, result.err()) {
        eprintln!("finishing saved read {id}: {error}");
      }
    }
  }

  /// Makes and stores every piece of saved read `id` that isn't stored yet. Stops once the read is deleted.
  fn make_pieces(&self, maker: &mut Maker, id: i64, voice: &Voice, pieces: &[(String, String)]) -> Result<(), String> {
    for (piece, hash) in pieces {
      if !self.library.is_saving(id)? {
        return Ok(());
      }
      if self.library.has_piece(hash) {
        continue;
      }
      let audio = maker.piece(voice, piece, |_| true)?.expect("a save is never cancelled");
      self.library.write_piece(hash, &audio)?;
    }
    Ok(())
  }
}

/// The model one thread uses, loaded on first use and switched when a voice needs another.
struct Maker {
  data: PathBuf,
  /// Keeps the files of the reading and the saving workers apart.
  slot: &'static str,
  /// The settings window shows the reading model's status; the saving thread reports through the library instead.
  status: Option<Arc<Mutex<Status>>>,
  loaded: Option<Loaded>,
}

struct Loaded {
  model: &'static str,
  language: &'static str,
  runtime: Loader,
}

enum Loader {
  Phonon(Phonon),
  Python(Worker),
}

impl Maker {
  fn new(data: PathBuf, slot: &'static str, status: Option<Arc<Mutex<Status>>>) -> Maker {
    Maker { data, slot, status, loaded: None }
  }

  fn set(&self, status: Status) {
    if let Some(shared) = &self.status {
      *shared.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = status;
    }
  }

  /// Loads the model for `voice` and primes the voice, keeping the status up to date.
  fn prime(&mut self, voice: &Voice) -> Result<(), String> {
    let result = self.ready(voice).map(drop);
    self.set(match &result {
      Ok(()) => Status::Ready,
      Err(error) => Status::Failed { error: error.clone() },
    });
    result
  }

  /// The model for `voice`, loading or switching it first when needed, with a Phonon voice primed.
  fn ready(&mut self, voice: &Voice) -> Result<&mut Loader, String> {
    // Phonon loads per language; a Python model reads every language of its own.
    let serves = |loaded: &Loaded| {
      loaded.model == voice.model.id && (voice.model.runtime != Runtime::Phonon || loaded.language == voice.language)
    };
    if self.loaded.as_ref().is_none_or(|loaded| !serves(loaded)) {
      self.set(Status::Loading { model: voice.model.name.to_string() });
      self.loaded = None;
      let runtime = match voice.model.runtime {
        Runtime::Phonon => Loader::Phonon(Phonon::load(voice.model, voice.language)?),
        Runtime::Python(script) => Loader::Python(Worker::start(&self.data, self.slot, script, voice.model)?),
      };
      self.loaded = Some(Loaded { model: voice.model.id, language: voice.language, runtime });
    }
    let runtime = &mut self.loaded.as_mut().expect("loaded above").runtime;
    if let Loader::Phonon(phonon) = runtime {
      phonon.session(&voice.name, voice.speed.control)?;
    }
    Ok(runtime)
  }

  /// Makes `piece` in `voice`, handing `push` the audio as it comes. Returns all of it, or `None` once `push` says
  /// to stop.
  fn piece(
    &mut self,
    voice: &Voice,
    piece: &str,
    mut push: impl FnMut(&[f32]) -> bool,
  ) -> Result<Option<Vec<f32>>, String> {
    let result = self.make(voice, piece, &mut push);
    self.set(match &result {
      Ok(_) => Status::Ready,
      Err(error) => Status::Failed { error: error.clone() },
    });
    // A Python model that stopped would fail every piece after, so the next one starts it again.
    let stopped = match &mut self.loaded {
      Some(Loaded { runtime: Loader::Python(worker), .. }) => !worker.alive(),
      _ => false,
    };
    if stopped {
      self.loaded = None;
    }
    result
  }

  fn make(
    &mut self,
    voice: &Voice,
    piece: &str,
    push: &mut dyn FnMut(&[f32]) -> bool,
  ) -> Result<Option<Vec<f32>>, String> {
    match self.ready(voice)? {
      Loader::Phonon(phonon) => {
        let session = phonon.session(&voice.name, voice.speed.control)?;
        let mut audio = Vec::new();
        for chunk in session.stream(piece).map_err(|error| error.to_string())? {
          let chunk = chunk.map_err(|error| error.to_string())?;
          if !push(&chunk) {
            return Ok(None);
          }
          audio.extend_from_slice(&chunk);
        }
        Ok(Some(audio))
      }
      Loader::Python(worker) => {
        let info = voice
          .model
          .voice(&voice.name)
          .ok_or_else(|| format!("{} has no voice {}; choose one in the settings", voice.model.name, voice.name))?;
        let audio = worker.say(piece, info, voice.language, voice.speed.control)?;
        Ok(push(&audio).then_some(audio))
      }
    }
  }
}

/// The sentences of `text` as `voice` reads them, each with the hash of its stored audio.
fn pieces(text: &str, voice: &Voice) -> Vec<(String, String)> {
  let voice_id = voice.id();
  text::pieces(&text::prepare(text))
    .into_iter()
    .map(|piece| {
      let hash = library::piece_hash(&voice_id, &text::words(&piece));
      (piece, hash)
    })
    .collect()
}

fn hashes(pieces: &[(String, String)]) -> Vec<String> {
  pieces.iter().map(|(_, hash)| hash.clone()).collect()
}

/// The voices of `model` for the settings window. Phonon's come from its catalog, downloaded the first time.
pub fn voices(model: &ModelInfo) -> Result<Vec<VoiceInfo>, String> {
  match model.runtime {
    Runtime::Phonon => phonon::voices(),
    Runtime::Python(_) => Ok(
      model
        .voices
        .iter()
        .map(|voice| VoiceInfo {
          name: voice.name.to_string(),
          language: voice.language.to_string(),
          description: voice.description.to_string(),
        })
        .collect(),
    ),
  }
}
