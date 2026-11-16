//! The `yap` command: hands text to the running app and waits while it plays. Starts the app when it isn't running.

use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::ExitCode;
#[cfg(not(windows))]
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const USAGE: &str = "usage:
  yap \"text\"            read the text aloud, and wait while it plays
  ... | yap             read stdin
  yap --save ...        make the audio in the background without playing it;
                        it plays from the library once ready
  yap --voice Maeve ... read in another voice
  yap --speed 1.2 ...   read faster or slower; the model's closest speed is used
  yap --language de ... read in that language's voice from the settings,
                        instead of telling the language from the text
  yap --app Firefox --title \"Docs\" --url https://... ...
                        where the text came from, kept with the read
  yap --source-env PLUCK ...
                        the same from PLUCK_APP, PLUCK_TITLE and PLUCK_URL, for
                        callers that pass them in the environment
  yap --stop            stop what plays
  yap --pause           pause, or resume
  yap                   open the window
  yap -- \"-5 degrees\"   text after -- is never a flag";

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("yap: {error}");
      ExitCode::FAILURE
    }
  }
}

fn run() -> Result<(), String> {
  let mut args = std::env::args().skip(1);
  let mut voice = None;
  let mut speed = None;
  let mut language = None;
  let mut source = serde_json::json!({});
  let mut words = Vec::new();
  let mut command = "speak";
  while let Some(arg) = args.next() {
    match arg.as_str() {
      "--" => words.extend(args.by_ref()),
      "--stop" => command = "stop",
      "--pause" => command = "pause",
      "--save" => command = "save",
      "--voice" => voice = Some(args.next().ok_or("--voice needs a name")?),
      "--language" => language = Some(args.next().ok_or("--language needs a code, like de")?),
      "--speed" => {
        let value = args.next().ok_or("--speed needs a number, like 1.2")?;
        speed = Some(value.parse::<f32>().map_err(|_| format!("--speed needs a number, like 1.2, not {value}"))?);
      }
      "--app" | "--title" | "--url" => {
        source[&arg[2..]] = args.next().ok_or(format!("{arg} needs a value"))?.into();
      }
      // Values in the environment never pass through a shell, so a quote in a window title can't break the command.
      "--source-env" => {
        let prefix = args.next().ok_or("--source-env needs a prefix, like PLUCK")?;
        for field in ["app", "title", "url"] {
          if let Ok(value) = std::env::var(format!("{prefix}_{}", field.to_uppercase())) {
            source[field] = value.into();
          }
        }
      }
      "-h" | "--help" => {
        println!("{USAGE}");
        return Ok(());
      }
      flag if flag.starts_with("--") => return Err(format!("unknown flag {flag}\n\n{USAGE}")),
      _ => words.push(arg),
    }
  }

  let request = match command {
    "speak" | "save" => {
      let text = if !words.is_empty() {
        words.join(" ")
      } else if !std::io::stdin().is_terminal() {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).map_err(|error| format!("reading stdin: {error}"))?;
        text
      } else {
        // The app's single instance shows its window when started again.
        return launch(&[]);
      };
      if text.trim().is_empty() {
        return Ok(());
      }
      serde_json::json!({
        "command": command, "text": text, "voice": voice, "speed": speed, "language": language, "source": source
      })
    }
    command => serde_json::json!({ "command": command }),
  };
  send(request)
}

/// Sends one request and waits for its one-line reply. A speak replies once it has finished playing.
fn send(mut request: serde_json::Value) -> Result<(), String> {
  let (stream, token) = connect()?;
  request["token"] = token.into();
  writeln!(&stream, "{request}").map_err(|error| error.to_string())?;
  let mut reply = String::new();
  BufReader::new(&stream).read_line(&mut reply).map_err(|error| error.to_string())?;
  let reply: serde_json::Value = serde_json::from_str(&reply).map_err(|_| "yap closed the connection".to_string())?;
  match reply["error"].as_str() {
    Some(error) => Err(error.to_string()),
    None => Ok(()),
  }
}

/// Connects to the running app, starting it in the tray first when it isn't running.
fn connect() -> Result<(TcpStream, String), String> {
  if let Some(connected) = try_connect() {
    return Ok(connected);
  }
  launch(&["--start-hidden"])?;
  let started = Instant::now();
  while started.elapsed() < Duration::from_secs(20) {
    std::thread::sleep(Duration::from_millis(100));
    if let Some(connected) = try_connect() {
      return Ok(connected);
    }
  }
  Err("the app didn't start in time".to_string())
}

/// The port and token the app wrote when it started. A stale file from an earlier run fails to connect.
fn try_connect() -> Option<(TcpStream, String)> {
  let dir = dirs::data_local_dir()?.join("yap");
  let file: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("server.json")).ok()?).ok()?;
  let port = file["port"].as_u64()?;
  let stream = TcpStream::connect(("127.0.0.1", port as u16)).ok()?;
  Some((stream, file["token"].as_str()?.to_string()))
}

/// Starts `yap-app` from the folder this command is in, detached so it outlives this command.
fn launch(args: &[&str]) -> Result<(), String> {
  let exe = std::env::current_exe().map_err(|error| error.to_string())?;
  let app = exe.with_file_name(format!("yap-app{}", std::env::consts::EXE_SUFFIX));
  spawn_detached(&app, args).map_err(|error| format!("starting {}: {error}", app.display()))
}

#[cfg(not(windows))]
fn spawn_detached(app: &Path, args: &[&str]) -> std::io::Result<()> {
  Command::new(app).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(drop)
}

/// Windows hands a child every inheritable handle its parent has, not just stdio, so the app would hold open pipes
/// of whatever ran this command, and Pluck, waiting for this command's output to end, would wait until the app quits.
/// `std::process::Command` always inherits, so this calls `CreateProcessW` with inheritance off.
#[cfg(windows)]
fn spawn_detached(app: &Path, args: &[&str]) -> std::io::Result<()> {
  use std::ptr::null;
  use windows_sys::Win32::Foundation::CloseHandle;
  use windows_sys::Win32::System::Threading::{CreateProcessW, DETACHED_PROCESS, PROCESS_INFORMATION, STARTUPINFOW};
  // The arguments are this command's own flags, so they need no quoting.
  let line = format!("\"{}\" {}", app.display(), args.join(" "));
  let mut line: Vec<u16> = line.encode_utf16().chain([0]).collect();
  let startup = STARTUPINFOW { cb: size_of::<STARTUPINFOW>() as u32, ..unsafe { std::mem::zeroed() } };
  let mut process: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
  let started = unsafe {
    CreateProcessW(
      null(),
      line.as_mut_ptr(),
      null(),
      null(),
      0,
      DETACHED_PROCESS,
      null(),
      null(),
      &startup,
      &mut process,
    )
  };
  if started == 0 {
    return Err(std::io::Error::last_os_error());
  }
  unsafe {
    CloseHandle(process.hProcess);
    CloseHandle(process.hThread);
  }
  Ok(())
}
