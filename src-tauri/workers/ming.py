# /// script
# requires-python = "==3.12.*"
# dependencies = [
#   "torch==2.6.0", "torchaudio==2.6.0", "transformers==4.46.3", "tokenizers", "accelerate==1.3.0",
#   "huggingface-hub", "numpy", "soundfile==0.12.1", "tqdm", "loguru", "x-transformers==2.9.2",
#   "diffusers==0.33.0", "onnxruntime", "inflect", "einops", "packaging", "scipy>=1.14",
# ]
# [tool.uv.sources]
# torch = { index = "pytorch-cu124" }
# torchaudio = { index = "pytorch-cu124" }
# [[tool.uv.index]]
# name = "pytorch-cu124"
# url = "https://download.pytorch.org/whl/cu124"
# explicit = true
# ///
"""Ming TTS 0.5B on CUDA, reading with its stock voices or cloned ones. Its inference code is a checkout, not a package, so it
comes from GitHub at a pinned commit."""

import os
import sys

import serve

CODE = ("inclusionAI/Ming-3.0-tts", "a39b80660bdc5a4394d5e1d41af6dbfd9c443a54")


def load(source):
    code = serve.github(*CODE)
    os.chdir(code)
    sys.path.insert(0, str(code))
    from ming_tts import MingTTS

    # Text normalization needs pynini, which has no reliable Windows wheels.
    tts = MingTTS(str(serve.hugging_face(source)), device=serve.cuda(), text_normalization=False)

    def say(request):
        if "clone" in request:
            # Ming reads only English in yap.
            audio = tts.synthesize(
                task="TTS", text=request["text"], language_id="English", prompt_wav_path=request["clone"]["audio"],
                prompt_text=request["clone"]["text"], use_spk_emb=True,
            )
        else:
            audio = tts.synthesize(
                task="IP", text=request["text"], controls={"ip": request["voice"]}, use_zero_spk_emb=True
            )
        return audio.squeeze(0).float().cpu().numpy(), tts.sample_rate

    return say


serve.run(load)
