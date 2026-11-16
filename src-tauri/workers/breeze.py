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
"""Breeze TTS 2 on CUDA. Every voice of its own is designed from its description; cloned voices read from their clip. Its inference code is a checkout, not a
package, so it comes from GitHub at a pinned commit."""

import hashlib
import os
import sys

import numpy as np
import serve
import soundfile

CODE = ("breezeblue-ai/breeze-tts", "daa6cb2efe5471f09369229f322d20bb956cb3c3")
SEED = 42
# Designing from a description gives a different speaker for every text, so each voice is designed once, reading
# this line, and every piece clones that recording.
REFERENCE = {
    "en": "Hello there. I will be reading to you today, so sit back and listen for a while.",
    "zh": "你好。今天由我来为你朗读，请放松下来，慢慢听。",
}


def load(source):
    code = serve.github(*CODE)
    os.chdir(code)
    sys.path.insert(0, str(code))
    from breeze_infer.runtime import load_runtime, set_all_seeds, update_generation_config_for_breeze
    from breeze_infer.templates import get_template, prepare_inputs, select_template_name
    from models.fast_streaming import FastBreezeStreamingRuntime, FastStreamingConfig

    tokenizer, model, audio_tokenizer = load_runtime(
        serve.hugging_face(source), device=serve.cuda(), attn_implementation="eager"
    )
    update_generation_config_for_breeze(model)
    runtime = FastBreezeStreamingRuntime(
        model,
        audio_tokenizer,
        FastStreamingConfig(max_new_tokens=1500, max_seq_len=2048, repetition_penalty=1.1),
        tokenizer=tokenizer,
    )

    def speak(piece, guidance_scale):
        # A fixed seed, so a piece sounds the same however often it is made.
        set_all_seeds(SEED)
        inputs = prepare_inputs(
            tokenizer, audio_tokenizer, model, [piece], get_template(select_template_name(piece)),
            guidance_scale=guidance_scale, guidance_scale_ref=None, guidance_scale_ins=None,
        )
        chunks = [chunk.audio for chunk in runtime.iter_audio_chunks(inputs, request_id="piece", seed=SEED)]
        return np.concatenate(chunks)

    def reference(description, language):
        """The recording of `description` reading the reference line, designed the first time it is asked for."""
        key = hashlib.sha256(f"{SEED} {language} {description}".encode()).hexdigest()[:16]
        path = serve.FOLDER / "voices" / f"breeze-{key}.wav"
        if not path.exists():
            piece = {"id": "piece", "text": REFERENCE[language], "speaker": "S0", "instruction": description}
            path.parent.mkdir(parents=True, exist_ok=True)
            soundfile.write(path, speak(piece, guidance_scale=4.0), runtime.sample_rate)
        return path

    def say(request):
        if "clone" in request:
            audio, said = request["clone"]["audio"], request["clone"]["text"]
        else:
            audio, said = str(reference(request["description"], request["language"])), REFERENCE[request["language"]]
        piece = {"id": "piece", "text": request["text"], "speaker": "S0", "ref_audio_path": audio, "ref_text": said}
        return speak(piece, guidance_scale=1.0), runtime.sample_rate

    return say


serve.run(load)
