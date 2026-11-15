//! Plays one read at a time on the default output device, as the engine hands it audio.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, mpsc};

/// The sample rate pieces are generated and stored at.
pub const RATE: u32 = 24_000;

#[derive(Clone)]
pub struct Player(Arc<Mutex<Queue>>);

#[derive(Default)]
struct Queue {
  /// Mono samples at the device rate.
  samples: VecDeque<f32>,
  device_rate: u32,
  /// The read being played; audio pushed for any other read is dropped.
  read: u64,
  generating: bool,
  paused: bool,
  error: Option<String>,
  /// What the popup shows about the read.
  pieces: Vec<String>,
  voice: String,
  app: String,
  /// Device samples queued and played since the read began, and where each piece starts.
  queued: u64,
  played: u64,
  starts: Vec<u64>,
}

/// What plays right now, for the popup.
#[derive(Clone, serde::Serialize)]
pub struct Now {
  text: String,
  piece: usize,
  pieces: usize,
  /// Seconds into the piece, and its length once all of it is generated.
  elapsed: f32,
  length: Option<f32>,
  paused: bool,
  voice: String,
  app: String,
}

/// How a read is doing, for the client waiting on it.
pub enum Progress {
  Playing,
  Done,
  Failed(String),
}

impl Player {
  /// Opens the default output device on a thread of its own, since a cpal stream can't move between threads on every
  /// platform. Returns once the device is open.
  pub fn start() -> Result<Player, String> {
    let player = Player(Arc::default());
    let queue = player.0.clone();
    let (opened, result) = mpsc::channel();
    std::thread::spawn(move || match open(queue) {
      Ok(_stream) => {
        opened.send(Ok(())).ok();
        loop {
          std::thread::park();
        }
      }
      Err(error) => {
        opened.send(Err(error)).ok();
      }
    });
    result.recv().map_err(|error| error.to_string())??;
    Ok(player)
  }

  fn queue(&self) -> MutexGuard<'_, Queue> {
    self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  /// Stops whatever plays and starts a new read of `pieces`, returning its id.
  pub fn begin(&self, pieces: Vec<String>, voice: &str, app: &str) -> u64 {
    let mut queue = self.queue();
    let device_rate = queue.device_rate;
    *queue = Queue {
      device_rate,
      read: queue.read + 1,
      generating: true,
      pieces,
      voice: voice.to_string(),
      app: app.to_string(),
      ..Queue::default()
    };
    queue.read
  }

  pub fn current(&self) -> u64 {
    self.queue().read
  }

  /// Queues 24 kHz audio of piece number `piece` of `read`, if it is still the current one.
  pub fn push(&self, read: u64, piece: usize, audio: &[f32]) {
    let mut queue = self.queue();
    if queue.read != read || audio.is_empty() {
      return;
    }
    if queue.starts.len() == piece {
      let start = queue.queued;
      queue.starts.push(start);
    }
    let ratio = queue.device_rate as f32 / RATE as f32;
    // Linear interpolation is enough for speech going up to 44.1 or 48 kHz.
    let n = (audio.len() as f32 * ratio) as usize;
    for i in 0..n {
      let x = i as f32 / ratio;
      let (a, f) = (x as usize, x.fract());
      let b = (a + 1).min(audio.len() - 1);
      queue.samples.push_back(audio[a] * (1.0 - f) + audio[b] * f);
    }
    queue.queued += n as u64;
  }

  /// Marks `read` as fully generated, or failed.
  pub fn finish(&self, read: u64, error: Option<String>) {
    let mut queue = self.queue();
    if queue.read == read {
      queue.generating = false;
      queue.error = error;
    }
  }

  pub fn stop(&self) {
    let mut queue = self.queue();
    queue.samples.clear();
    queue.generating = false;
    queue.read += 1;
  }

  /// Seconds of audio queued but not yet played.
  pub fn backlog(&self) -> f32 {
    let queue = self.queue();
    queue.samples.len() as f32 / queue.device_rate as f32
  }

  pub fn toggle_pause(&self) {
    let mut queue = self.queue();
    queue.paused = !queue.paused;
  }

  /// The piece playing and how far into it, while a read plays.
  pub fn now(&self) -> Option<Now> {
    let queue = self.queue();
    if queue.samples.is_empty() && !queue.generating {
      return None;
    }
    let piece = queue.starts.iter().rposition(|&start| start <= queue.played)?;
    let start = queue.starts[piece];
    let end = match queue.starts.get(piece + 1) {
      Some(&next) => Some(next),
      None if !queue.generating => Some(queue.queued),
      None => None,
    };
    let seconds = |samples: u64| samples as f32 / queue.device_rate as f32;
    Some(Now {
      text: queue.pieces[piece].clone(),
      piece,
      pieces: queue.pieces.len(),
      elapsed: seconds(queue.played - start),
      length: end.map(|end| seconds(end - start)),
      paused: queue.paused,
      voice: queue.voice.clone(),
      app: queue.app.clone(),
    })
  }

  pub fn progress(&self, read: u64) -> Progress {
    let queue = self.queue();
    if queue.read != read {
      return Progress::Done;
    }
    match &queue.error {
      Some(error) => Progress::Failed(error.clone()),
      None if queue.generating || !queue.samples.is_empty() => Progress::Playing,
      None => Progress::Done,
    }
  }
}

fn open(queue: Arc<Mutex<Queue>>) -> Result<cpal::Stream, String> {
  let device = cpal::default_host().default_output_device().ok_or("no audio output device")?;
  let config = device.default_output_config().map_err(|error| error.to_string())?;
  let channels = config.channels() as usize;
  queue.lock().unwrap().device_rate = config.sample_rate().0;
  let stream = device
    .build_output_stream(
      &config.into(),
      move |out: &mut [f32], _| {
        let mut queue = queue.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for frame in out.chunks_mut(channels) {
          let sample = if queue.paused { None } else { queue.samples.pop_front() };
          queue.played += sample.is_some() as u64;
          frame.fill(sample.unwrap_or(0.0));
        }
      },
      |error| eprintln!("audio output: {error}"),
      None,
    )
    .map_err(|error| error.to_string())?;
  stream.play().map_err(|error| error.to_string())?;
  Ok(stream)
}
