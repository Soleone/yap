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

  /// Stops whatever plays and starts a new read, returning its id.
  pub fn begin(&self) -> u64 {
    let mut queue = self.queue();
    queue.samples.clear();
    queue.read += 1;
    queue.generating = true;
    queue.paused = false;
    queue.error = None;
    queue.read
  }

  pub fn current(&self) -> u64 {
    self.queue().read
  }

  /// Queues 24 kHz audio for `read`, if it is still the current one.
  pub fn push(&self, read: u64, audio: &[f32]) {
    let mut queue = self.queue();
    if queue.read != read || audio.is_empty() {
      return;
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
          let sample = if queue.paused { 0.0 } else { queue.samples.pop_front().unwrap_or(0.0) };
          frame.fill(sample);
        }
      },
      |error| eprintln!("audio output: {error}"),
      None,
    )
    .map_err(|error| error.to_string())?;
  stream.play().map_err(|error| error.to_string())?;
  Ok(stream)
}
