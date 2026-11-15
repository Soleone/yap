//! What yap has read: one row per read in SQLite, one WAV file per piece, shared between reads.

use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// A read's title is its first words.
const TITLE_WORDS: usize = 12;

pub struct Library {
  db: Mutex<Connection>,
  pieces: PathBuf,
}

/// Where a read's text came from, as the client says.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Source {
  pub app: String,
  pub title: String,
  pub url: String,
}

/// How a read sounds, kept so a replay sounds the same whatever the settings are by then.
pub struct Sound {
  pub model: String,
  pub language: String,
  pub voice: String,
  pub speed: f32,
}

/// A read as the library window lists it.
#[derive(serde::Serialize)]
pub struct Read {
  id: i64,
  /// Seconds since 1970.
  created: i64,
  title: String,
  source: Source,
  voice: String,
  pieces: usize,
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
        model text not null,
        language text not null,
        voice text not null,
        speed real not null,
        pieces text not null
      )",
    )
    .map_err(|error| error.to_string())?;
    Ok(Library { db: Mutex::new(db), pieces })
  }

  fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
    self.db.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  /// Records a read with the hashes of its pieces, in order, and returns its id. The same text read again replaces
  /// its earlier read, so it moves to the top instead of showing twice.
  pub fn add(&self, text: &str, source: &Source, sound: &Sound, pieces: &[String]) -> Result<i64, String> {
    let created = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let words: Vec<&str> = text.split_whitespace().collect();
    let title =
      if words.len() > TITLE_WORDS { format!("{}…", words[..TITLE_WORDS].join(" ")) } else { words.join(" ") };
    let db = self.db();
    db.execute("delete from reads where text = ?1", [text]).map_err(|error| error.to_string())?;
    db.execute(
      "insert into reads (created, title, text, source_app, source_title, source_url, model, language, voice, speed, pieces)
       values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
      params![
        created,
        title,
        text,
        source.app,
        source.title,
        source.url,
        sound.model,
        sound.language,
        sound.voice,
        sound.speed,
        pieces.join(" ")
      ],
    )
    .map_err(|error| error.to_string())?;
    Ok(db.last_insert_rowid())
  }

  /// Every read, newest first.
  pub fn list(&self) -> Result<Vec<Read>, String> {
    let db = self.db();
    let mut query = db
      .prepare(
        "select id, created, title, source_app, source_title, source_url, voice, pieces from reads order by id desc",
      )
      .map_err(|error| error.to_string())?;
    query
      .query_map([], |row| {
        Ok(Read {
          id: row.get(0)?,
          created: row.get(1)?,
          title: row.get(2)?,
          source: Source { app: row.get(3)?, title: row.get(4)?, url: row.get(5)? },
          voice: row.get(6)?,
          pieces: row.get::<_, String>(7)?.split(' ').filter(|hash| !hash.is_empty()).count(),
        })
      })
      .and_then(|rows| rows.collect())
      .map_err(|error| error.to_string())
  }

  /// A read's text and how it sounded, to play it again.
  pub fn get(&self, id: i64) -> Result<(String, Source, Sound), String> {
    self
      .db()
      .query_row(
        "select text, source_app, source_title, source_url, model, language, voice, speed from reads where id = ?1",
        [id],
        |row| {
          let source = Source { app: row.get(1)?, title: row.get(2)?, url: row.get(3)? };
          let sound = Sound { model: row.get(4)?, language: row.get(5)?, voice: row.get(6)?, speed: row.get(7)? };
          Ok((row.get(0)?, source, sound))
        },
      )
      .map_err(|error| error.to_string())
  }

  /// Removes a read, and the stored pieces no other read uses.
  pub fn delete(&self, id: i64) -> Result<(), String> {
    let db = self.db();
    let pieces: String = db
      .query_row("select pieces from reads where id = ?1", [id], |row| row.get(0))
      .map_err(|error| error.to_string())?;
    db.execute("delete from reads where id = ?1", [id]).map_err(|error| error.to_string())?;
    let mut query = db.prepare("select pieces from reads").map_err(|error| error.to_string())?;
    let kept: HashSet<String> = query
      .query_map([], |row| row.get::<_, String>(0))
      .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
      .map_err(|error| error.to_string())?
      .iter()
      .flat_map(|pieces| pieces.split(' ').map(str::to_string))
      .collect();
    for hash in pieces.split(' ').filter(|hash| !hash.is_empty() && !kept.contains(*hash)) {
      std::fs::remove_file(self.piece_path(hash)).ok();
    }
    Ok(())
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  // library.delete
  fn library_delete_keeps_pieces_other_reads_share() {
    let dir = std::env::temp_dir().join(format!("yap-library-test-{}", std::process::id()));
    let library = Library::open(&dir).unwrap();
    for hash in ["a", "b", "c"] {
      library.write_piece(hash, &[0.0; 10]).unwrap();
    }
    let sound = Sound { model: "m".into(), language: "en".into(), voice: "Miles".into(), speed: 1.0 };
    let first = library.add("one", &Source::default(), &sound, &["a".into(), "b".into()]).unwrap();
    library.add("two", &Source::default(), &sound, &["b".into(), "c".into()]).unwrap();

    library.delete(first).unwrap();

    let stored: Vec<bool> = ["a", "b", "c"].iter().map(|hash| library.piece_path(hash).exists()).collect();
    assert_eq!(stored, [false, true, true]);
    assert_eq!(library.list().unwrap().len(), 1);
    drop(library);
    std::fs::remove_dir_all(dir).ok();
  }

  #[test]
  // library.record
  fn library_add_moves_the_same_text_to_the_top() {
    let dir = std::env::temp_dir().join(format!("yap-library-test-again-{}", std::process::id()));
    let library = Library::open(&dir).unwrap();
    let sound = Sound { model: "m".into(), language: "en".into(), voice: "Miles".into(), speed: 1.0 };
    for text in ["one", "two", "one"] {
      library.add(text, &Source::default(), &sound, &[]).unwrap();
    }
    let titles: Vec<String> = library.list().unwrap().into_iter().map(|read| read.title).collect();
    assert_eq!(titles, ["one", "two"]);
    drop(library);
    std::fs::remove_dir_all(dir).ok();
  }
}
