//! The local port the `yap` command talks to. A client sends one JSON line and gets one back once its command is
//! done. A speak holds the connection while it plays, and a dropped connection stops it, so killing the client
//! stops the read.

use crate::engine::Engine;
use crate::library::Source;
use crate::player::{Player, Progress};
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::Duration;

#[derive(serde::Deserialize)]
struct Request {
  token: String,
  #[serde(flatten)]
  command: Command,
}

#[derive(serde::Deserialize)]
#[serde(tag = "command", rename_all = "lowercase")]
enum Command {
  Speak {
    text: String,
    voice: Option<String>,
    speed: Option<f32>,
    #[serde(default)]
    source: Source,
  },
  Stop,
  Pause,
}

/// Listens on a random local port and writes it with a fresh token to `server.json` in `dir`, where the client finds
/// them. The token keeps other users on the same machine out.
pub fn start(dir: &Path, engine: Engine, player: Player) -> Result<(), String> {
  let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
  let port = listener.local_addr().map_err(|error| error.to_string())?.port();
  let token = token();
  let file = serde_json::json!({ "port": port, "token": token }).to_string();
  std::fs::write(dir.join("server.json"), file).map_err(|error| error.to_string())?;
  std::thread::spawn(move || {
    for stream in listener.incoming().flatten() {
      let (engine, player, token) = (engine.clone(), player.clone(), token.clone());
      std::thread::spawn(move || {
        let reply = match serve(&stream, &token, &engine, &player) {
          Ok(()) => serde_json::json!({ "done": true }),
          Err(error) => serde_json::json!({ "error": error }),
        };
        writeln!(&stream, "{reply}").ok();
      });
    }
  });
  Ok(())
}

fn serve(stream: &TcpStream, token: &str, engine: &Engine, player: &Player) -> Result<(), String> {
  let mut line = String::new();
  BufReader::new(stream).read_line(&mut line).map_err(|error| error.to_string())?;
  let request: Request = serde_json::from_str(&line).map_err(|error| format!("unreadable request: {error}"))?;
  if request.token != token {
    return Err("wrong token".to_string());
  }
  match request.command {
    Command::Speak { text, voice, speed, source } => {
      let read = engine.speak(&text, Some(&source), engine.voice(voice, speed))?;
      stop_when_dropped(stream, player, read)?;
      loop {
        match player.progress(read) {
          Progress::Playing => std::thread::sleep(Duration::from_millis(50)),
          Progress::Done => return Ok(()),
          Progress::Failed(error) => return Err(error),
        }
      }
    }
    Command::Stop => Ok(player.stop()),
    Command::Pause => Ok(player.toggle_pause()),
  }
}

/// The client sends nothing after its request, so a read returning means it went away.
fn stop_when_dropped(stream: &TcpStream, player: &Player, read: u64) -> Result<(), String> {
  let mut watch = stream.try_clone().map_err(|error| error.to_string())?;
  let player = player.clone();
  std::thread::spawn(move || {
    watch.read(&mut [0]).ok();
    if player.current() == read {
      player.stop();
    }
  });
  Ok(())
}

/// 64 random bits from the standard library's hash seed, which comes from the OS. Plenty for a local token.
fn token() -> String {
  format!("{:016x}", std::collections::hash_map::RandomState::new().build_hasher().finish())
}
