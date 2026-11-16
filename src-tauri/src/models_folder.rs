//! Moves the downloaded models and their Python runtimes into a newly chosen models folder, so none are left behind
//! to download again.

use std::fs;
use std::io;
use std::path::Path;

/// What a models folder holds. The rest of the data folder, the default models folder, stays where it is.
const PARTS: [&str; 2] = ["models", "runtimes"];

/// Moves every model and runtime in `from` into `to`, keeping what `to` already has. What fails to move stays in
/// `from`, and moving again finishes it.
pub fn move_models(from: &Path, to: &Path) -> io::Result<()> {
  for part in PARTS {
    let source = from.join(part);
    if to.starts_with(&source) {
      return Err(io::Error::other(format!("{} is inside {}", to.display(), source.display())));
    }
    if fs::symlink_metadata(&source).is_ok() {
      move_into(&source, &to.join(part))?;
    }
  }
  Ok(())
}

/// Points the Hugging Face cache, which `ptts` reads from the environment on every load, at `folder`. Models go in
/// yap's own folder, not the shared cache, so removing yap removes them.
///
/// # Safety
///
/// No other thread may read the environment outside Rust's own lock meanwhile, so no model may be loading.
pub unsafe fn point_cache(folder: &Path) {
  unsafe { std::env::set_var("HF_HUB_CACHE", folder.join("models")) };
}

/// Notes that model `id` loaded from `folder`, so the settings window lists it as downloaded. Inside `models`, so it
/// moves with the model.
pub fn mark_downloaded(folder: &Path, id: &str) -> io::Result<()> {
  let marks = folder.join("models").join("downloaded");
  fs::create_dir_all(&marks)?;
  fs::write(marks.join(id), "")
}

/// The ids of the models in `folder` that loaded before.
pub fn downloaded(folder: &Path) -> Vec<String> {
  let Ok(marks) = fs::read_dir(folder.join("models").join("downloaded")) else { return Vec::new() };
  marks.flatten().map(|mark| mark.file_name().to_string_lossy().into_owned()).collect()
}

/// Moves `from` to `to`, merging it into what's there. A rename does it at once on the same drive; across drives each
/// file is copied, under a temporary name until it's whole, and then deleted.
fn move_into(from: &Path, to: &Path) -> io::Result<()> {
  if fs::symlink_metadata(to).is_err() {
    fs::create_dir_all(to.parent().expect("a models folder part has a parent"))?;
    if fs::rename(from, to).is_ok() {
      return Ok(());
    }
  }
  let kind = fs::symlink_metadata(from)?.file_type();
  if kind.is_dir() {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
      let entry = entry?;
      move_into(&entry.path(), &to.join(entry.file_name()))?;
    }
    return fs::remove_dir(from);
  }
  if fs::symlink_metadata(to).is_err() {
    // The Hugging Face cache links each model's files to its blobs with relative links, which still point right in
    // the new folder. Windows gets copies instead of links.
    #[cfg(unix)]
    if kind.is_symlink() {
      std::os::unix::fs::symlink(fs::read_link(from)?, to)?;
      return fs::remove_file(from);
    }
    let mut part = to.as_os_str().to_owned();
    part.push(".moving");
    fs::copy(from, &part)?;
    fs::rename(part, to)?;
  }
  fs::remove_file(from)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::PathBuf;

  fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("yap-move-{name}-{}", std::process::id()));
    fs::remove_dir_all(&folder).ok();
    fs::create_dir_all(&folder).unwrap();
    folder
  }

  // settings.folder
  #[test]
  fn moves_models_and_runtimes_and_leaves_the_rest() {
    let from = folder("from");
    fs::create_dir_all(from.join("models/kokoro")).unwrap();
    fs::write(from.join("models/kokoro/weights"), "w").unwrap();
    fs::create_dir_all(from.join("runtimes/python")).unwrap();
    fs::write(from.join("library.db"), "db").unwrap();
    let to = folder("to");

    move_models(&from, &to).unwrap();

    assert_eq!(fs::read_to_string(to.join("models/kokoro/weights")).unwrap(), "w");
    assert!(to.join("runtimes/python").is_dir());
    assert!(!from.join("models").exists() && !from.join("runtimes").exists());
    assert!(from.join("library.db").exists());
  }

  // settings.folder
  #[test]
  fn merges_into_a_folder_that_has_models_and_keeps_its_copy() {
    let from = folder("merge-from");
    let to = folder("merge-to");
    fs::create_dir_all(from.join("models/kokoro")).unwrap();
    fs::write(from.join("models/kokoro/weights"), "old").unwrap();
    fs::write(from.join("models/kokoro/voices"), "v").unwrap();
    fs::create_dir_all(to.join("models/kokoro")).unwrap();
    fs::write(to.join("models/kokoro/weights"), "new").unwrap();

    move_models(&from, &to).unwrap();

    assert_eq!(fs::read_to_string(to.join("models/kokoro/weights")).unwrap(), "new");
    assert_eq!(fs::read_to_string(to.join("models/kokoro/voices")).unwrap(), "v");
    assert!(!from.join("models").exists());
  }

  // settings.model
  #[test]
  fn a_model_that_loaded_lists_as_downloaded_and_moves_with_the_models() {
    let from = folder("marks-from");
    assert!(downloaded(&from).is_empty());
    mark_downloaded(&from, "kokoro").unwrap();
    assert_eq!(downloaded(&from), ["kokoro"]);
    let to = folder("marks-to");

    move_models(&from, &to).unwrap();

    assert_eq!(downloaded(&to), ["kokoro"]);
  }

  // settings.folder
  #[test]
  fn refuses_a_folder_inside_the_models() {
    let from = folder("inside");
    fs::create_dir_all(from.join("models")).unwrap();

    assert!(move_models(&from, &from.join("models/more")).is_err());
    assert!(from.join("models").is_dir());
  }
}
