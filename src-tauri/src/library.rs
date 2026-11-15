//! What yap has read: one row per read in SQLite, one WAV file per piece, shared between reads.

use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Library {
  db: Mutex<Connection>,
  pieces: PathBuf,
}

/// Where a read's text came from, as the client says.
#[derive(Default, serde::Deserialize)]
#[serde(default)]
pub struct Source {
  pub app: String,
  pub title: String,
  pub url: String,
}

impl Library {
  pub fn open(dir: &Path) -> Result<Library, String> {
    let pieces = dir.join("pieces");
    std::fs::create_dir_all(&pieces).map_err(|error| error.to_string())?;
    let db = Connection::open(dir.join("library.db")).map_err(|error| error.to_string())?;
    db.execute_batch(
      "create table if not exists reads (
        id integer primary key,
        created integer not null,
        title text not null,
        text text not null,
        source_app text not null,
        source_title text not null,
        source_url text not null,
        voice text not null,
        pieces text not null
      )",
    )
    .map_err(|error| error.to_string())?;
    Ok(Library { db: Mutex::new(db), pieces })
  }

  /// Records a read with the hashes of its pieces, in order, and returns its id.
  pub fn add(&self, text: &str, source: &Source, voice: &str, pieces: &[String]) -> Result<i64, String> {
    let created = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let title: String = text.split_whitespace().take(12).collect::<Vec<_>>().join(" ");
    let db = self.db.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    db.execute(
      "insert into reads (created, title, text, source_app, source_title, source_url, voice, pieces)
       values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
      params![created, title, text, source.app, source.title, source.url, voice, pieces.join(" ")],
    )
    .map_err(|error| error.to_string())?;
    Ok(db.last_insert_rowid())
  }

  pub fn piece_path(&self, hash: &str) -> PathBuf {
    self.pieces.join(format!("{hash}.wav"))
  }

  /// The stored audio of a piece, if it was generated before.
  pub fn read_piece(&self, hash: &str) -> Option<Vec<f32>> {
    let reader = hound::WavReader::open(self.piece_path(hash)).ok()?;
    reader.into_samples::<i16>().map(|sample| sample.map(|s| s as f32 / 32768.0)).collect::<Result<_, _>>().ok()
  }

  /// Stores a piece under a temporary name first, so a stopped read never leaves half a piece.
  pub fn write_piece(&self, hash: &str, audio: &[f32]) -> Result<(), String> {
    let path = self.piece_path(hash);
    let partial = path.with_extension("partial");
    let spec = hound::WavSpec {
      channels: 1,
      sample_rate: crate::player::RATE,
      bits_per_sample: 16,
      sample_format: hound::SampleFormat::Int,
    };
    let write = || -> Result<(), hound::Error> {
      let mut writer = hound::WavWriter::create(&partial, spec)?;
      audio.iter().try_for_each(|&sample| writer.write_sample((sample.clamp(-1.0, 1.0) * 32767.0) as i16))?;
      writer.finalize()
    };
    write().map_err(|error| error.to_string())?;
    std::fs::rename(&partial, &path).map_err(|error| error.to_string())
  }
}

/// A piece's file name: the hash of what makes its audio, so the same words in the same voice play from disk.
pub fn piece_hash(voice_id: &str, words: &str) -> String {
  let digest = Sha256::digest(format!("{voice_id}\n{words}"));
  digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
