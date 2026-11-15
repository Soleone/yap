//! The voice model, loaded once and kept with a primed session, turning reads into audio for the player.

use crate::library::{self, Library, Source};
use crate::player::Player;
use crate::text;
use ptts::checkpoint::{Checkpoint, DEFAULT_MODEL, ResolveOptions};
use ptts::preprocess::Lang;
use ptts::synth::{DeviceKind, Quant, Session, SpeechOptions, Synth};
use ptts::voice_catalog::{DEFAULT_VOICE_CATALOG, VoiceCatalog};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

pub const DEFAULT_VOICE: &str = "Miles";

/// Generation stops running ahead of playback once this much audio waits, so a long text doesn't fill memory.
const AHEAD_SECS: f32 = 30.0;

#[derive(Clone)]
pub struct Engine {
  jobs: Sender<Job>,
  player: Player,
  library: Arc<Library>,
}

struct Job {
  read: u64,
  voice: String,
  /// Each piece's text and its hash, in order.
  pieces: Vec<(String, String)>,
}

impl Engine {
  /// Starts the engine thread, which loads the model and primes the default voice right away so the first read
  /// starts at once.
  pub fn start(player: Player, library: Arc<Library>) -> Engine {
    let (jobs, rx) = channel();
    let engine = Engine { jobs, player, library };
    let worker = engine.clone();
    std::thread::spawn(move || worker.run(rx));
    engine
  }

  /// Stops what plays and starts reading `text`. Returns the player's id for the read.
  pub fn speak(&self, text: &str, source: &Source, voice: Option<String>) -> Result<u64, String> {
    let voice = voice.unwrap_or_else(|| DEFAULT_VOICE.to_string());
    let voice_id = format!("{DEFAULT_MODEL} {voice}");
    let pieces: Vec<(String, String)> = text::pieces(&text::prepare(text))
      .into_iter()
      .map(|piece| {
        let hash = library::piece_hash(&voice_id, &text::words(&piece));
        (piece, hash)
      })
      .collect();
    let hashes: Vec<String> = pieces.iter().map(|(_, hash)| hash.clone()).collect();
    self.library.add(text, source, &voice, &hashes)?;
    let read = self.player.begin();
    self.jobs.send(Job { read, voice, pieces }).map_err(|_| "the engine stopped".to_string())?;
    Ok(read)
  }

  fn run(self, jobs: Receiver<Job>) {
    let mut model = Model::load(DEFAULT_VOICE).map_err(|error| eprintln!("loading the voice model: {error}")).ok();
    for job in jobs {
      let result = self.generate(&mut model, &job);
      self.player.finish(job.read, result.err());
    }
  }

  /// Plays each piece from disk when it was read before, and generates and stores it otherwise. Returns early once
  /// another read took over.
  fn generate(&self, model: &mut Option<Model>, job: &Job) -> Result<(), String> {
    let current = || self.player.current() == job.read;
    for (piece, hash) in &job.pieces {
      while current() && self.player.backlog() > AHEAD_SECS {
        std::thread::sleep(Duration::from_millis(100));
      }
      if !current() {
        return Ok(());
      }
      if let Some(audio) = self.library.read_piece(hash) {
        self.player.push(job.read, &audio);
        continue;
      }
      if model.is_none() {
        *model = Some(Model::load(&job.voice)?);
      }
      let session = model.as_mut().expect("loaded above").session(&job.voice)?;
      let mut audio = Vec::new();
      for chunk in session.stream(piece).map_err(|error| error.to_string())? {
        let chunk = chunk.map_err(|error| error.to_string())?;
        if !current() {
          return Ok(());
        }
        self.player.push(job.read, &chunk);
        audio.extend_from_slice(&chunk);
      }
      self.library.write_piece(hash, &audio)?;
    }
    Ok(())
  }
}

struct Model {
  synth: Synth,
  catalog: VoiceCatalog,
  /// Priming a voice takes about half a second; a session keeps that done for every read in the voice.
  primed: Option<(String, Session)>,
}

impl Model {
  fn load(voice: &str) -> Result<Model, String> {
    let checkpoint = Checkpoint::locate(DEFAULT_MODEL, ResolveOptions { quant: Quant::Q80, weights: None })
      .map_err(|error| error.to_string())?;
    let mut synth = checkpoint.builder(Lang::En).device(DeviceKind::Cpu).build().map_err(|error| error.to_string())?;
    checkpoint.register_voices(&mut synth);
    let catalog = VoiceCatalog::new(DEFAULT_VOICE_CATALOG).map_err(|error| error.to_string())?;
    let mut model = Model { synth, catalog, primed: None };
    model.session(voice)?;
    Ok(model)
  }

  fn session(&mut self, voice: &str) -> Result<&Session, String> {
    if self.primed.as_ref().is_none_or(|(primed, _)| primed != voice) {
      let name = self.catalog.register(&mut self.synth, voice).map_err(|error| error.to_string())?;
      let session =
        self.synth.session_default(&SpeechOptions::default().voice(name)).map_err(|error| error.to_string())?;
      self.primed = Some((voice.to_string(), session));
    }
    Ok(&self.primed.as_ref().expect("primed above").1)
  }
}
