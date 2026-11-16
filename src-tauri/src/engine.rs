//! The voice models, loaded once and kept, turning reads into audio for the player, and saved reads into pieces in
//! the library. Reads and saved reads each have a thread and models of their own, so a long save never holds up a
//! read.

use crate::cloned::Cloned;
use crate::language;
use crate::library::{self, Library, Sound, Source};
use crate::models_folder;
use crate::phonon::{self, Phonon, VoiceInfo};
use crate::player::Player;
use crate::registry::{self, Model as ModelInfo, Runtime, Speed};
use crate::settings::{Settings, Store};
use crate::text;
use crate::worker::{Speaker, Stopper, Worker};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
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

/// What a read asks for instead of the settings.
#[derive(Default, Deserialize)]
pub struct Wanted {
  /// Reads in this language's voice without telling it from the text.
  pub language: Option<String>,
  pub voice: Option<String>,
  pub speed: Option<f32>,
}

/// What the settings window shows about the model.
#[derive(Clone, Default, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Status {
  /// A cancellable load is a Python model starting, which may be downloading gigabytes.
  Loading { model: String, cancellable: bool },
  #[default]
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
  cloned: Arc<Cloned>,
  shown: Arc<Shown>,
  /// The last settings whose voices all got ready, which cancelling goes back to.
  settled: Arc<Mutex<Option<Settings>>>,
  folder: Arc<Folder>,
}

/// What the settings window sees of the reading thread's models, and the Python model starting, which it can cancel.
#[derive(Default)]
struct Shown {
  status: Mutex<Status>,
  starting: Mutex<Option<Stopper>>,
}

/// Where models download, which every model is let go of for while they move to another folder.
struct Folder {
  state: Mutex<FolderState>,
  settled: Condvar,
  /// Each loaded model holds a clone, so a move can tell once all of them are gone.
  lease: Arc<()>,
}

struct FolderState {
  path: PathBuf,
  moving: bool,
}

impl Folder {
  /// The folder to load a model from once no move is under way, letting go of `loaded` while one is, and the lease the
  /// model holds. Taken together, so no move starts while it loads.
  fn path(&self, loaded: &mut Vec<Loaded>) -> (PathBuf, Arc<()>) {
    let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if state.moving {
      loaded.clear();
    }
    let state = self.settled.wait_while(state, |state| state.moving).unwrap_or_else(|poisoned| poisoned.into_inner());
    (state.path.clone(), self.lease.clone())
  }
}

enum Job {
  Speak {
    read: u64,
    voice: Voice,
    /// Each piece's text and its hash, in order, and the piece to start from.
    pieces: Vec<(String, String)>,
    from: usize,
  },
  /// Loads the models and primes the voices of every language in the settings ahead of the next read, and lets go of
  /// the rest.
  Prime,
  /// Tells what the clip about to be cloned says, in a model that clones.
  Transcribe { model: &'static ModelInfo, keep: bool, reply: Sender<Result<String, String>> },
}

impl Engine {
  /// Starts the reading thread, which loads the model and primes the voice in the settings right away so the first
  /// read starts at once, and the saving thread, which makes the reads saved for later, those from before a restart
  /// first, and prunes old reads.
  pub fn start(player: Player, library: Arc<Library>, settings: Store, cloned: Arc<Cloned>, models: PathBuf) -> Engine {
    let (jobs, job_rx) = channel();
    let (saves, save_rx) = channel();
    let folder = Arc::new(Folder {
      state: Mutex::new(FolderState { path: models, moving: false }),
      settled: Condvar::new(),
      lease: Arc::new(()),
    });
    let engine = Engine {
      jobs,
      saves,
      player,
      library,
      settings,
      cloned,
      shown: Arc::default(),
      settled: Arc::default(),
      folder,
    };
    let reader = engine.clone();
    std::thread::spawn(move || reader.read(job_rx));
    let saver = engine.clone();
    std::thread::spawn(move || saver.make_saved(save_rx));
    engine.prime();
    engine
  }

  /// The voice in the settings for the language of `text`, with what `wanted` asks for instead.
  pub fn voice(&self, text: &str, wanted: Wanted) -> Voice {
    let settings = self.settings.get();
    let language = wanted.language.clone().unwrap_or_else(|| language::detect(&text::prepare(text), &settings));
    reading_voice(&settings, language, wanted)
  }

  /// The voice of the main language in the settings.
  pub fn main_voice(&self) -> Voice {
    let settings = self.settings.get();
    reading_voice(&settings, settings.language.clone(), Wanted::default())
  }

  /// The voice saved reads of `text` use: the one in the settings for them, or the reading voice when none is set or
  /// the text's language has a voice of its own.
  pub fn save_voice(&self, text: &str, wanted: Wanted) -> Voice {
    let settings = self.settings.get();
    let language = wanted.language.clone().unwrap_or_else(|| language::detect(&text::prepare(text), &settings));
    if settings.save_model.is_empty() || settings.other_languages.iter().any(|other| other.language == language) {
      return reading_voice(&settings, language, wanted);
    }
    // Empty when the model's voices couldn't be listed as it was chosen.
    let saved = Some(settings.save_voice).filter(|voice| !voice.is_empty());
    let name = wanted.voice.or(saved).unwrap_or(settings.voice);
    Voice::new(&settings.save_model, &language, name, wanted.speed.unwrap_or(settings.speed))
  }

  /// Gets the voices in the settings ready, after they changed.
  pub fn prime(&self) {
    self.jobs.send(Job::Prime).ok();
  }

  /// Stops the Python model that is starting and goes back to the models and voices of the settings that were last
  /// ready, or the default ones. Returns the settings.
  pub fn cancel(&self) -> Result<Settings, String> {
    let ready = lock(&self.settled).clone().unwrap_or_default();
    let mut settings = self.settings.get();
    settings.language = ready.language;
    settings.model = ready.model;
    settings.voice = ready.voice;
    settings.other_languages = ready.other_languages;
    self.settings.set(settings.clone())?;
    // Queued before the stop, so the reading thread loads them right after the start fails.
    self.prime();
    if let Some(starting) = lock(&self.shown.starting).take() {
      starting.stop();
    }
    Ok(settings)
  }

  /// The ids of the models in the models folder that loaded before.
  pub fn downloaded(&self) -> Vec<String> {
    models_folder::downloaded(&self.models_folder())
  }

  pub fn models_folder(&self) -> PathBuf {
    self.folder.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).path.clone()
  }

  /// Stops what plays, lets go of every model, moves them all into `to` and saves it as the models folder in the
  /// settings. Reads wait meanwhile, and load from `to` after. What fails to move stays where it was, which stays the
  /// models folder.
  pub fn move_models(&self, to: PathBuf) -> Result<(), String> {
    let from = {
      let mut state = self.folder.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
      if state.moving {
        return Err("The models are already moving.".to_string());
      }
      if state.path == to {
        return Ok(());
      }
      state.moving = true;
      state.path.clone()
    };
    self.player.stop();
    // The reading thread lets go of its models on its next job, and the saving thread before its next piece. It holds
    // none while nothing waits.
    self.prime();
    while Arc::strong_count(&self.folder.lease) > 1 {
      std::thread::sleep(Duration::from_millis(50));
    }
    let result = models_folder::move_models(&from, &to).map_err(|error| format!("Moving the models: {error}"));
    let moved = result.is_ok();
    let result = result.and_then(|()| {
      // SAFETY: no model loads while they move.
      unsafe { models_folder::point_cache(&to) };
      let mut settings = self.settings.get();
      settings.models_folder = to.to_string_lossy().into_owned();
      self.settings.set(settings)
    });
    let mut state = self.folder.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if moved {
      state.path = to;
    }
    state.moving = false;
    self.folder.settled.notify_all();
    result
  }

  /// What the clip staged for cloning says, told by speech recognition in `model`'s Python runtime. The recognition
  /// model is deleted again after unless `keep`. Reads wait meanwhile.
  pub fn transcribe(&self, model: &str, keep: bool) -> Result<String, String> {
    let (reply, answer) = channel();
    let job = Job::Transcribe { model: registry::model(model), keep, reply };
    self.jobs.send(job).map_err(|_| "the engine stopped".to_string())?;
    answer.recv().map_err(|_| "the engine stopped".to_string())?
  }

  pub fn status(&self) -> Status {
    lock(&self.shown.status).clone()
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
    let mut maker = Maker::new(self.folder.clone(), self.cloned.clone(), "read", Some(self.shown.clone()));
    for job in jobs {
      match job {
        Job::Speak { read, voice, pieces, from } => {
          let result = self.generate(&mut maker, read, &voice, &pieces, from);
          self.player.finish(read, result.err());
        }
        Job::Prime => {
          let settings = self.settings.get();
          if maker.prime(settings_voices(&settings)) {
            *lock(&self.settled) = Some(settings);
          }
        }
        Job::Transcribe { model, keep, reply } => {
          reply.send(maker.transcribe(model, &self.cloned.clip(), keep)).ok();
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
    let mut maker = Maker::new(self.folder.clone(), self.cloned.clone(), "save", None);
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
        maker.loaded.clear();
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

/// The models one thread uses, each loaded on first use.
struct Maker {
  folder: Arc<Folder>,
  cloned: Arc<Cloned>,
  /// Keeps the files of the reading and the saving workers apart.
  slot: &'static str,
  /// The settings window shows the reading model's status; the saving thread reports through the library instead.
  shown: Option<Arc<Shown>>,
  loaded: Vec<Loaded>,
  /// The voices of the settings, whose models stay loaded. Any other model goes once another loads, so replaying an
  /// old read in a big model doesn't keep it, and the saving thread, which primes nothing, holds one at a time.
  kept: Vec<Voice>,
}

struct Loaded {
  model: &'static str,
  language: &'static str,
  runtime: Loader,
  _lease: Arc<()>,
}

impl Loaded {
  /// Phonon loads per language; a Python model reads every language of its own.
  fn serves(&self, voice: &Voice) -> bool {
    self.model == voice.model.id && (voice.model.runtime != Runtime::Phonon || self.language == voice.language)
  }
}

enum Loader {
  Phonon(Phonon),
  Python(Worker),
}

impl Maker {
  fn new(folder: Arc<Folder>, cloned: Arc<Cloned>, slot: &'static str, shown: Option<Arc<Shown>>) -> Maker {
    Maker { folder, cloned, slot, shown, loaded: Vec::new(), kept: Vec::new() }
  }

  fn set(&self, status: Status) {
    if let Some(shown) = &self.shown {
      *lock(&shown.status) = status;
    }
  }

  /// Loads the models for `voices` and primes the voices, letting go of models none of them use and keeping the
  /// status up to date. The status shows the first that failed. Returns whether all got ready.
  fn prime(&mut self, voices: Vec<Voice>) -> bool {
    self.kept = voices.clone();
    self.let_go();
    let failed = voices.iter().find_map(|voice| self.ready(voice).err());
    let ready = failed.is_none();
    self.set(match failed {
      None => Status::Ready,
      Some(error) => Status::Failed { error },
    });
    ready
  }

  /// The model for `voice`, loading it first when needed, with a Phonon voice primed.
  fn ready(&mut self, voice: &Voice) -> Result<&mut Loader, String> {
    let (models, lease) = self.folder.path(&mut self.loaded);
    let index = match self.loaded.iter().position(|loaded| loaded.serves(voice)) {
      Some(index) => index,
      None => {
        self.let_go();
        self.set(Status::Loading { model: voice.model.name.to_string(), cancellable: false });
        let runtime = match voice.model.runtime {
          Runtime::Phonon => Loader::Phonon(Phonon::load(voice.model, voice.language)?),
          Runtime::Python(script) => Loader::Python(self.start(&models, script, voice.model)?),
        };
        // Only the settings window's label depends on it.
        models_folder::mark_downloaded(&models, voice.model.id).ok();
        self.loaded.push(Loaded { model: voice.model.id, language: voice.language, runtime, _lease: lease });
        self.loaded.len() - 1
      }
    };
    let runtime = &mut self.loaded[index].runtime;
    if let Loader::Phonon(phonon) = runtime {
      phonon.session(&voice.name, voice.speed.control)?;
    }
    Ok(runtime)
  }

  /// Starts a Python model, which the settings window can cancel until it is ready.
  fn start(&self, models: &std::path::Path, script: &str, model: &'static ModelInfo) -> Result<Worker, String> {
    let mut worker = Worker::spawn(models, self.slot, script, model)?;
    if let Some(shown) = &self.shown {
      *lock(&shown.starting) = Some(worker.stopper());
    }
    self.set(Status::Loading { model: model.name.to_string(), cancellable: true });
    let ready = worker.wait_ready();
    if let Some(shown) = &self.shown {
      lock(&shown.starting).take();
    }
    ready.map(|()| worker)
  }

  /// Drops the models no voice of the settings uses.
  fn let_go(&mut self) {
    let kept = &self.kept;
    self.loaded.retain(|loaded| kept.iter().any(|voice| loaded.serves(voice)));
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
    self.loaded.retain_mut(|loaded| match &mut loaded.runtime {
      Loader::Python(worker) => worker.alive(),
      Loader::Phonon(_) => true,
    });
    result
  }

  fn make(
    &mut self,
    voice: &Voice,
    piece: &str,
    push: &mut dyn FnMut(&[f32]) -> bool,
  ) -> Result<Option<Vec<f32>>, String> {
    let cloned = self.cloned.clone();
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
        let speaker = match voice.model.voice(&voice.name) {
          Some(stock) => Speaker::Stock(stock),
          None => {
            let cloned = cloned.find(&voice.name).filter(|_| voice.model.clones);
            let (audio, transcript) = cloned
              .ok_or_else(|| format!("{} has no voice {}; choose one in the settings", voice.model.name, voice.name))?;
            Speaker::Cloned { audio, transcript }
          }
        };
        let audio = worker.say(piece, &speaker, voice.language, voice.speed.control)?;
        Ok(push(&audio).then_some(audio))
      }
    }
  }

  /// What the WAV at `audio` says, in the Python runtime of `model`, loaded first when needed.
  fn transcribe(&mut self, model: &'static ModelInfo, audio: &std::path::Path, keep: bool) -> Result<String, String> {
    let voice = Voice::new(model.id, model.languages[0], String::new(), 1.0);
    match self.ready(&voice)? {
      Loader::Python(worker) => worker.transcribe(audio, keep),
      Loader::Phonon(_) => Err(format!("{} can't clone voices", model.name)),
    }
  }
}

/// The voices of every language in `settings`.
fn settings_voices(settings: &Settings) -> Vec<Voice> {
  std::iter::once(settings.language.clone())
    .chain(settings.other_languages.iter().map(|other| other.language.clone()))
    .map(|language| reading_voice(settings, language, Wanted::default()))
    .collect()
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
  mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The voice `settings` read `language` in: its own when it has one, else the main one, with what `wanted` asks for
/// instead. A language the main model doesn't read reads in the model's first.
fn reading_voice(settings: &Settings, language: String, wanted: Wanted) -> Voice {
  let (model, voice) = match settings.other_languages.iter().find(|other| other.language == language) {
    Some(other) => (&other.model, &other.voice),
    None => (&settings.model, &settings.voice),
  };
  Voice::new(model, &language, wanted.voice.unwrap_or_else(|| voice.clone()), wanted.speed.unwrap_or(settings.speed))
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

/// The voices of `model` for the settings window, the cloned ones last when it clones. Phonon's come from its catalog,
/// downloaded the first time.
pub fn voices(model: &ModelInfo, cloned: &Cloned) -> Result<Vec<VoiceInfo>, String> {
  let mut voices = match model.runtime {
    Runtime::Phonon => phonon::voices()?,
    Runtime::Python(_) => model
      .voices
      .iter()
      .map(|voice| VoiceInfo {
        name: voice.name.to_string(),
        language: voice.language.to_string(),
        description: voice.description.to_string(),
        cloned: false,
      })
      .collect(),
  };
  if model.clones {
    voices.extend(cloned.list().into_iter().map(|voice| VoiceInfo {
      name: voice.name,
      language: String::new(),
      description: "Cloned from your clip".to_string(),
      cloned: true,
    }));
  }
  Ok(voices)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::settings::OtherLanguage;

  // speak.language
  #[test]
  fn a_language_reads_in_its_own_voice_or_else_the_main_one() {
    let german = OtherLanguage { language: "de".into(), model: "phonon-40m-de".into(), voice: "Anna".into() };
    let settings = Settings { other_languages: vec![german], ..Settings::default() };
    let voice = reading_voice(&settings, "de".into(), Wanted::default());
    assert_eq!((voice.model.id, voice.language, voice.name.as_str()), ("phonon-40m-de", "de", "Anna"));
    let voice = reading_voice(&settings, "fr".into(), Wanted::default());
    assert_eq!((voice.model.id, voice.language, voice.name.as_str()), ("phonon-31m-en", "en", "Miles"));
  }
}
