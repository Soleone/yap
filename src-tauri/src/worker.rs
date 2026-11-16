//! Models that run in Python: one long-running script per model, started through uv, which makes the pieces.
//! The protocol is in `workers/serve.py`.

use crate::registry::{Model, Voice};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

/// The scripts, written next to each other into the models folder so each can import `serve`.
const SCRIPTS: &[(&str, &str)] = &[
  ("serve.py", include_str!("../workers/serve.py")),
  ("kokoro.py", include_str!("../workers/kokoro.py")),
  ("ming.py", include_str!("../workers/ming.py")),
  ("qwen3.py", include_str!("../workers/qwen3.py")),
  ("breeze.py", include_str!("../workers/breeze.py")),
];

pub struct Worker {
  child: Arc<Mutex<Child>>,
  input: ChildStdin,
  output: BufReader<ChildStdout>,
  /// Where the script writes each piece, and its log.
  piece: PathBuf,
  log: PathBuf,
}

impl Worker {
  /// Starts `script` for `model`, which loads it once `wait_ready` is called. The first start downloads Python, the
  /// model's packages and its weights, which can take a long while. `slot` keeps the files of two workers apart.
  pub fn spawn(models: &Path, slot: &str, script: &str, model: &Model) -> Result<Worker, String> {
    let folder = models.join("runtimes").join("workers");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    for (name, code) in SCRIPTS {
      let path = folder.join(name);
      // Another worker may be running this script, so it is only written when it changed.
      if std::fs::read_to_string(&path).ok().as_deref() != Some(*code) {
        std::fs::write(path, code).map_err(|error| error.to_string())?;
      }
    }
    let log = folder.join(format!("{script}-{slot}.log"));
    let stderr = std::fs::File::create(&log).map_err(|error| error.to_string())?;
    let runtimes = models.join("runtimes");
    let mut command = Command::new("uv");
    command
      .args(["run", "--script"])
      .arg(folder.join(format!("{script}.py")))
      .arg(model.source)
      .env("YAP_MODELS", models)
      // Python, packages and environments stay in yap's folder, so removing yap removes them.
      .env("UV_CACHE_DIR", runtimes.join("uv"))
      .env("UV_PYTHON_INSTALL_DIR", runtimes.join("python"))
      .env("PYTHONUNBUFFERED", "1")
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(stderr);
    #[cfg(windows)]
    {
      use std::os::windows::process::CommandExt;
      const CREATE_NO_WINDOW: u32 = 0x0800_0000;
      command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(|error| match error.kind() {
      std::io::ErrorKind::NotFound => {
        format!("{} runs in Python through uv. Install uv from docs.astral.sh/uv, then try again.", model.name)
      }
      _ => format!("starting uv: {error}"),
    })?;
    let input = child.stdin.take().expect("stdin is piped");
    let output = BufReader::new(child.stdout.take().expect("stdout is piped"));
    let child = Arc::new(Mutex::new(child));
    Ok(Worker { child, input, output, piece: folder.join(format!("{script}-{slot}.wav")), log })
  }

  /// Waits until the model is loaded, or fails once the worker stopped.
  pub fn wait_ready(&mut self) -> Result<(), String> {
    self.reply()
  }

  /// Stops the worker from another thread, even while it loads.
  pub fn stopper(&self) -> Stopper {
    Stopper(self.child.clone())
  }

  /// Makes one piece and returns its 24 kHz audio.
  pub fn say(&mut self, text: &str, voice: &Voice, language: &str, speed: &str) -> Result<Vec<f32>, String> {
    let request = serde_json::json!({
      "text": text,
      "voice": voice.name,
      "description": voice.description,
      "language": language,
      "speed": speed,
      "out": self.piece,
    });
    writeln!(self.input, "{request}").map_err(|_| self.died())?;
    self.reply()?;
    let reader = hound::WavReader::open(&self.piece).map_err(|error| error.to_string())?;
    reader
      .into_samples::<i16>()
      .map(|sample| sample.map(|s| s as f32 / 32768.0))
      .collect::<Result<_, _>>()
      .map_err(|error| error.to_string())
  }

  pub fn alive(&mut self) -> bool {
    matches!(lock(&self.child).try_wait(), Ok(None))
  }

  /// Waits for the script's next line: ready, done or an error.
  fn reply(&mut self) -> Result<(), String> {
    let mut line = String::new();
    if self.output.read_line(&mut line).map_err(|error| error.to_string())? == 0 {
      return Err(self.died());
    }
    let reply: serde_json::Value = serde_json::from_str(&line).map_err(|_| format!("unreadable reply: {line}"))?;
    match reply["error"].as_str() {
      Some(error) => Err(error.to_string()),
      None => Ok(()),
    }
  }

  /// The end of the log, which says why the script stopped.
  fn died(&self) -> String {
    let log = std::fs::read_to_string(&self.log).unwrap_or_default();
    let last: Vec<&str> = log.lines().filter(|line| !line.trim().is_empty()).collect();
    format!("the model stopped: {}", last[last.len().saturating_sub(3)..].join(" "))
  }
}

impl Drop for Worker {
  fn drop(&mut self) {
    Stopper(self.child.clone()).stop();
  }
}

pub struct Stopper(Arc<Mutex<Child>>);

impl Stopper {
  pub fn stop(&self) {
    let mut child = lock(&self.0);
    child.kill().ok();
    child.wait().ok();
  }
}

fn lock(child: &Mutex<Child>) -> std::sync::MutexGuard<'_, Child> {
  child.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
