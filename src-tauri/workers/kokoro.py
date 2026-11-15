# /// script
# requires-python = "==3.12.*"
# dependencies = ["kokoro-onnx==0.5.0", "onnxruntime==1.22.1", "numpy==2.3.2"]
# ///
"""Kokoro 82M through ONNX on the CPU. The source is the release folder holding the model and its voices."""

import serve
from kokoro_onnx import Kokoro

# A Kokoro voice's first letter says its accent.
LANGS = {"a": "en-us", "b": "en-gb", "e": "es", "f": "fr-fr", "h": "hi", "i": "it", "p": "pt-br"}


def load(source):
    folder = serve.DATA / "models" / "kokoro-v1.0"
    model = serve.download(f"{source}/kokoro-v1.0.onnx", folder / "kokoro-v1.0.onnx")
    voices = serve.download(f"{source}/voices-v1.0.bin", folder / "voices-v1.0.bin")
    tts = Kokoro(str(model), str(voices))

    def say(request):
        voice = request["voice"]
        return tts.create(request["text"], voice=voice, speed=float(request["speed"]), lang=LANGS[voice[0]])

    return say


serve.run(load)
