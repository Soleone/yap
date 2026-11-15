# /// script
# requires-python = "==3.12.*"
# dependencies = [
#   "torch==2.9.1", "torchaudio==2.9.1", "qwen-tts==0.1.1", "transformers==4.57.3",
#   "huggingface-hub>=0.34", "numpy>=2.0", "soundfile>=0.13", "scipy>=1.14",
# ]
# [tool.uv.sources]
# torch = { index = "pytorch-cu128" }
# torchaudio = { index = "pytorch-cu128" }
# [[tool.uv.index]]
# name = "pytorch-cu128"
# url = "https://download.pytorch.org/whl/cu128"
# explicit = true
# ///
"""Qwen3-TTS 1.7B CustomVoice on CUDA. Its dependencies match breeze.py's, so the two share one environment."""

import serve
import torch
from qwen_tts import Qwen3TTSModel

LANGUAGES = {
    "en": "English", "zh": "Chinese", "ja": "Japanese", "ko": "Korean", "de": "German",
    "fr": "French", "ru": "Russian", "pt": "Portuguese", "es": "Spanish", "it": "Italian",
}
SEED = 240808


def load(source):
    tts = Qwen3TTSModel.from_pretrained(str(serve.hugging_face(source)), device_map=serve.cuda(), dtype=torch.bfloat16)

    def say(request):
        # A fixed seed, so a piece sounds the same however often it is made.
        torch.manual_seed(SEED)
        wavs, rate = tts.generate_custom_voice(
            text=request["text"], speaker=request["voice"], language=LANGUAGES[request["language"]]
        )
        return wavs[0], rate

    return say


serve.run(load)
