"""What every model script shares: the line protocol with yap, downloads, and writing pieces.

yap runs `uv run --script <model>.py <source>`. Once the model is loaded the script prints {"ready": true}; then
each line in is one request and each line out its answer, or {"error": "..."}:

- A piece, {"text", "language", "speed", "out"} with the model's own {"voice", "description"} or a cloned voice's
  {"clone": {"audio", "text"}}, the WAV it was cloned from and what it says. {"done": true} once `out` holds the piece
  as 24 kHz mono 16-bit WAV.
- {"transcribe": path, "keep": bool}: {"done": true, "text": "..."}, what the 24 kHz WAV at `path` says.
"""

import json
import os
import sys
import traceback
import urllib.request
import wave
import zipfile
from pathlib import Path

import numpy as np

RATE = 24_000
# Speech recognition for clips to clone from, loaded only for that, since it is rare.
WHISPER = "hf://openai/whisper-large-v3-turbo@41f01f3fe87f28c78e2fbf8b568835947dd65ed9"
# Weights go in its `models` folder, Python code in `runtimes`.
FOLDER = Path(os.environ["YAP_MODELS"])

# Libraries print progress and warnings to stdout, so the protocol keeps the real stdout to itself.
_protocol = os.fdopen(os.dup(1), "w", encoding="utf-8")
os.dup2(2, 1)
sys.stdout = sys.stderr


def _send(message):
    _protocol.write(json.dumps(message) + "\n")
    _protocol.flush()


def run(load):
    """Loads the model with `load`, which returns `say(request) -> (samples, rate)`, then serves pieces."""
    try:
        say = load(sys.argv[1])
    except Exception as error:
        traceback.print_exc()
        _send({"error": f"loading the model: {error}"})
        sys.exit(1)
    _send({"ready": True})
    for line in sys.stdin:
        request = json.loads(line)
        try:
            if "transcribe" in request:
                _send({"done": True, "text": _transcribe(Path(request["transcribe"]), request["keep"])})
                continue
            samples, rate = say(request)
            _write(Path(request["out"]), np.asarray(samples, dtype=np.float32).reshape(-1), rate)
            _send({"done": True})
        except Exception as error:
            traceback.print_exc()
            _send({"error": str(error)})


def _write(path, samples, rate):
    if rate != RATE:
        from math import gcd

        from scipy.signal import resample_poly

        divisor = gcd(RATE, rate)
        samples = resample_poly(samples, RATE // divisor, rate // divisor)
    with wave.open(str(path), "wb") as file:
        file.setnchannels(1)
        file.setsampwidth(2)
        file.setframerate(RATE)
        file.writeframes((np.clip(samples, -1, 1) * 32767).astype("<i2").tobytes())


def _transcribe(path, keep):
    """What the 24 kHz WAV at `path` says, by Whisper, which downloads the first time and is let go of after, and
    deleted unless `keep`."""
    import shutil

    import torch
    from scipy.signal import resample_poly
    from transformers import pipeline

    with wave.open(str(path)) as file:
        samples = np.frombuffer(file.readframes(file.getnframes()), dtype="<i2").astype(np.float32) / 32768
    folder = hugging_face(WHISPER)
    recognizer = pipeline(
        "automatic-speech-recognition", model=str(folder), device=cuda(), torch_dtype=torch.float16
    )
    try:
        # Whisper hears at 16 kHz.
        return recognizer({"raw": resample_poly(samples, 2, 3), "sampling_rate": 16_000})["text"].strip()
    finally:
        del recognizer
        torch.cuda.empty_cache()
        if not keep:
            shutil.rmtree(folder, ignore_errors=True)


def hugging_face(source):
    """The local folder of `hf://owner/repo@revision`, downloaded into yap's models folder the first time. A plain
    folder rather than the Hugging Face cache, whose symlinks Windows refuses without developer mode."""
    from huggingface_hub import snapshot_download

    repo, revision = source.removeprefix("hf://").split("@")
    folder = FOLDER / "models" / f"{repo.replace('/', '--')}-{revision[:12]}"
    return Path(snapshot_download(repo, revision=revision, local_dir=folder))


def download(url, path):
    """Downloads `url` to `path` unless it is there, under a temporary name first so a stopped download never counts."""
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        partial = path.with_suffix(path.suffix + ".partial")
        urllib.request.urlretrieve(url, partial)
        partial.rename(path)
    return path


def github(repo, commit):
    """The code of a GitHub repo at `commit`, for models whose inference code is a checkout rather than a package."""
    folder = FOLDER / "runtimes" / "code" / f"{repo.replace('/', '-')}-{commit}"
    if not folder.exists():
        archive = download(f"https://github.com/{repo}/archive/{commit}.zip", folder.with_suffix(".zip"))
        with zipfile.ZipFile(archive) as zip:
            zip.extractall(folder.parent / "unpacking")
        (folder.parent / "unpacking" / f"{repo.split('/')[1]}-{commit}").rename(folder)
        (folder.parent / "unpacking").rmdir()
        archive.unlink()
    return folder


def cuda():
    import torch

    if not torch.cuda.is_available():
        raise RuntimeError("this model needs an NVIDIA GPU with a current driver")
    return "cuda:0"
