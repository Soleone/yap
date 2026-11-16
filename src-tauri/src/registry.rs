//! The models yap can download, and what each offers. Pinned to a revision, so a model never changes under the cache.

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
  pub id: &'static str,
  pub name: &'static str,
  /// The weights at a pinned revision: a Hugging Face repo at a commit, or a release download.
  #[serde(skip)]
  pub source: &'static str,
  #[serde(skip)]
  pub runtime: Runtime,
  /// Everything it downloads, its Python runtime included, measured on Windows. Breeze and Qwen3 share their 5 GB
  /// runtime, so the second of them takes that much less.
  pub size_mb: u32,
  /// Language codes; the first is the default.
  pub languages: &'static [&'static str],
  pub license: &'static str,
  /// Gated models need a Hugging Face token whose account accepted the model's terms.
  pub gated: bool,
  /// Needs an NVIDIA GPU and reads slower than real time, so it suits saved reads.
  pub gpu: bool,
  /// The speeds the slider offers, from slowest to fastest.
  pub speeds: &'static [Speed],
  /// Reads in voices cloned from a recording, as well as its own.
  pub clones: bool,
  /// Its voices; Phonon's come from the Phonon catalog instead.
  #[serde(skip)]
  pub voices: &'static [Voice],
}

/// What runs a model.
#[derive(Clone, Copy, PartialEq)]
pub enum Runtime {
  /// In process, through `ptts`.
  Phonon,
  /// A Python script in `src-tauri/workers`, run by uv.
  Python(&'static str),
}

/// A speed the model can read at: the multiplier the slider shows, and the value the model's own control takes for it
/// (Phonon's `duration_delta` condition, Kokoro's speed).
#[derive(Serialize)]
pub struct Speed {
  pub speed: f32,
  #[serde(skip)]
  pub control: &'static str,
}

/// A voice of a model outside the Phonon catalog. An empty language means it reads every language of its model.
pub struct Voice {
  pub name: &'static str,
  pub language: &'static str,
  pub description: &'static str,
}

/// Measured on Phonon 31M English with Miles and Maeve, three takes each: the condition reaches 0.82x at 0.9 and 1.3x at
/// -0.6, and past -0.6 reads slower again.
const PHONON_SPEEDS: &[Speed] = &[
  Speed { speed: 0.8, control: "0.9" },
  Speed { speed: 0.9, control: "0.3" },
  Speed { speed: 1.0, control: "0.0" },
  Speed { speed: 1.1, control: "-0.15" },
  Speed { speed: 1.2, control: "-0.3" },
  Speed { speed: 1.3, control: "-0.6" },
];

const KOKORO_SPEEDS: &[Speed] = &[
  Speed { speed: 0.8, control: "0.8" },
  Speed { speed: 0.9, control: "0.9" },
  Speed { speed: 1.0, control: "1.0" },
  Speed { speed: 1.1, control: "1.1" },
  Speed { speed: 1.2, control: "1.2" },
  Speed { speed: 1.3, control: "1.3" },
];

/// For models without a speed control.
const NORMAL_SPEED: &[Speed] = &[Speed { speed: 1.0, control: "1.0" }];

const fn phonon_40m(
  id: &'static str,
  name: &'static str,
  source: &'static str,
  languages: &'static [&'static str],
) -> Model {
  Model {
    id,
    name,
    source,
    runtime: Runtime::Phonon,
    size_mb: 75,
    languages,
    license: "CC BY-NC 4.0",
    gated: true,
    gpu: false,
    speeds: PHONON_SPEEDS,
    clones: false,
    voices: &[],
  }
}

const fn voice(name: &'static str, language: &'static str, description: &'static str) -> Voice {
  Voice { name, language, description }
}

pub const MODELS: &[Model] = &[
  Model {
    id: "phonon-31m-en",
    name: "Phonon 31M English",
    source: "hf://gradium/phonon-31m-en@92741c5b07d085ff840e26fea8ce43110b87852f",
    runtime: Runtime::Phonon,
    size_mb: 65,
    languages: &["en"],
    license: "Apache 2.0",
    gated: false,
    gpu: false,
    speeds: PHONON_SPEEDS,
    clones: false,
    voices: &[],
  },
  phonon_40m(
    "phonon-40m-en",
    "Phonon 40M English",
    "hf://gradium/phonon-40m-en@bc7e8ae7e543a7a3e4604753e42fbda76ddf0781",
    &["en"],
  ),
  phonon_40m(
    "phonon-40m-multilingual",
    "Phonon 40M Multilingual",
    "hf://gradium/phonon-40m-multilingual@1537ce46c64f42903e0d608b132ccb073dac46d2",
    &["en", "fr", "de", "es", "pt"],
  ),
  phonon_40m(
    "phonon-40m-fr",
    "Phonon 40M French",
    "hf://gradium/phonon-40m-fr@d91086970206b88efd2c88ca971d0628b5968f0f",
    &["fr"],
  ),
  phonon_40m(
    "phonon-40m-de",
    "Phonon 40M German",
    "hf://gradium/phonon-40m-de@79cfc7b47fcfc0b666657b981eb366415023a862",
    &["de"],
  ),
  phonon_40m(
    "phonon-40m-es",
    "Phonon 40M Spanish",
    "hf://gradium/phonon-40m-es@e5e052b26e31abd5918ade71d3135e69d51a66f8",
    &["es"],
  ),
  phonon_40m(
    "phonon-40m-pt",
    "Phonon 40M Portuguese",
    "hf://gradium/phonon-40m-pt@4cbfd742a1555d60eb5399c6d296e9f2e07389e9",
    &["pt"],
  ),
  Model {
    id: "kokoro-82m",
    name: "Kokoro 82M",
    source: "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0",
    runtime: Runtime::Python("kokoro"),
    size_mb: 550,
    languages: &["en", "fr", "es", "it", "pt", "hi"],
    license: "Apache 2.0",
    gated: false,
    gpu: false,
    speeds: KOKORO_SPEEDS,
    clones: false,
    voices: &[
      voice("af_heart", "en", "American woman, warm; Kokoro's best"),
      voice("af_bella", "en", "American woman, bright"),
      voice("af_nicole", "en", "American woman, soft and close"),
      voice("af_sarah", "en", "American woman, even"),
      voice("am_michael", "en", "American man, calm"),
      voice("am_fenrir", "en", "American man, deep"),
      voice("am_puck", "en", "American man, lively"),
      voice("bf_emma", "en", "British woman"),
      voice("bf_isabella", "en", "British woman, light"),
      voice("bm_george", "en", "British man, deep"),
      voice("bm_fable", "en", "British man, storyteller"),
      voice("ff_siwis", "fr", "French woman"),
      voice("ef_dora", "es", "Spanish woman"),
      voice("em_alex", "es", "Spanish man"),
      voice("if_sara", "it", "Italian woman"),
      voice("im_nicola", "it", "Italian man"),
      voice("pf_dora", "pt", "Brazilian woman"),
      voice("pm_alex", "pt", "Brazilian man"),
      voice("hf_alpha", "hi", "Hindi woman"),
      voice("hm_omega", "hi", "Hindi man"),
    ],
  },
  Model {
    id: "ming-tts-0.5b",
    name: "Ming TTS 0.5B",
    source: "hf://inclusionAI/Ming-3.0-tts-0.5B@7247f4c94a5b2e987404ed28a5cfec41b1a4ca61",
    runtime: Runtime::Python("ming"),
    size_mb: 7_600,
    languages: &["en"],
    license: "MIT",
    gated: false,
    gpu: true,
    speeds: NORMAL_SPEED,
    clones: true,
    voices: &[
      voice("US_Emma", "en", "American woman"),
      voice("UK_Charlotte", "en", "British woman"),
      voice("UK_George", "en", "British man"),
    ],
  },
  Model {
    id: "qwen3-tts-1.7b",
    name: "Qwen3-TTS 1.7B",
    source: "hf://Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice@0c0e3051f131929182e2c023b9537f8b1c68adfe",
    runtime: Runtime::Python("qwen3"),
    size_mb: 9_300,
    languages: &["en", "zh", "ja", "ko", "de", "fr", "ru", "pt", "es", "it"],
    license: "Apache 2.0",
    gated: false,
    gpu: true,
    speeds: NORMAL_SPEED,
    clones: false,
    voices: &[
      voice("Ryan", "", "Man, English native"),
      voice("Aiden", "", "Man, English native"),
      voice("Vivian", "", "Woman, Chinese native"),
      voice("Serena", "", "Woman, Chinese native"),
      voice("Uncle_Fu", "", "Older man, Chinese native"),
      voice("Dylan", "", "Man, Beijing dialect"),
      voice("Eric", "", "Man, Sichuan dialect"),
      voice("Ono_Anna", "", "Woman, Japanese native"),
      voice("Sohee", "", "Woman, Korean native"),
    ],
  },
  Model {
    id: "qwen3-tts-1.7b-base",
    name: "Qwen3-TTS 1.7B Base",
    source: "hf://Qwen/Qwen3-TTS-12Hz-1.7B-Base@fd4b254389122332181a7c3db7f27e918eec64e3",
    runtime: Runtime::Python("qwen3"),
    size_mb: 9_300,
    languages: &["en", "zh", "ja", "ko", "de", "fr", "ru", "pt", "es", "it"],
    license: "Apache 2.0",
    gated: false,
    gpu: true,
    speeds: NORMAL_SPEED,
    clones: true,
    // The Base model has no voices of its own; it reads only in cloned ones.
    voices: &[],
  },
  Model {
    id: "breeze-tts-2",
    name: "Breeze TTS 2 3B",
    source: "hf://BreezeBlue/Breeze-TTS-2@3e28c5151381a722f1d8661b4118c298caa77aa4",
    runtime: Runtime::Python("breeze"),
    size_mb: 12_200,
    languages: &["en", "zh"],
    license: "BreezeBlue non-commercial",
    gated: false,
    gpu: true,
    speeds: NORMAL_SPEED,
    clones: true,
    // Breeze designs each voice from its description, so the description is the voice.
    voices: &[
      voice(
        "radio-man",
        "",
        "A relaxed American man in his forties with a deep, smooth voice, like a public radio host.",
      ),
      voice(
        "radio-woman",
        "",
        "A relaxed American woman in her forties with a low, smooth voice, like a public radio host.",
      ),
      voice(
        "teacher-man",
        "",
        "A warm, clear man in his thirties, calm and friendly, like a good teacher explaining something.",
      ),
      voice(
        "teacher-woman",
        "",
        "A warm, clear woman in her thirties, calm and friendly, like a good teacher explaining something.",
      ),
    ],
  },
];

/// The model `id` names, or the first one for an id no longer in the registry.
pub fn model(id: &str) -> &'static Model {
  MODELS.iter().find(|model| model.id == id).unwrap_or(&MODELS[0])
}

impl Model {
  /// The offered speed closest to `speed`.
  pub fn speed(&self, speed: f32) -> &Speed {
    self
      .speeds
      .iter()
      .min_by(|a, b| (a.speed - speed).abs().total_cmp(&(b.speed - speed).abs()))
      .expect("every model has speeds")
  }

  /// `wanted` when this model reads it, else its first language.
  pub fn language(&self, wanted: &str) -> &'static str {
    self.languages.iter().copied().find(|&code| code == wanted).unwrap_or(self.languages[0])
  }

  /// A voice of this model outside the Phonon catalog.
  pub fn voice(&self, name: &str) -> Option<&Voice> {
    self.voices.iter().find(|voice| voice.name == name)
  }
}
