//! The models yap can download, and what each offers. Pinned to a revision, so a model never changes under the cache.

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
  pub id: &'static str,
  pub name: &'static str,
  /// What `ptts` resolves: a Hugging Face repo at a commit.
  #[serde(skip)]
  pub source: &'static str,
  pub size_mb: u32,
  /// Language codes; the first is the default, more than one lets the settings choose.
  pub languages: &'static [&'static str],
  pub license: &'static str,
  /// Gated models need a Hugging Face token whose account accepted the model's terms.
  pub gated: bool,
  /// The speeds the slider offers, from slowest to fastest.
  pub speeds: &'static [Speed],
}

/// A speed the model can read at: the multiplier the slider shows, and the condition value that makes it.
#[derive(Serialize)]
pub struct Speed {
  pub speed: f32,
  #[serde(skip)]
  pub duration_delta: &'static str,
}

/// Measured on Phonon 31M English with Miles and Maeve, three takes each: the condition reaches 0.82x at 0.9 and 1.3x at
/// -0.6, and past -0.6 reads slower again.
const PHONON_SPEEDS: &[Speed] = &[
  Speed { speed: 0.8, duration_delta: "0.9" },
  Speed { speed: 0.9, duration_delta: "0.3" },
  Speed { speed: 1.0, duration_delta: "0.0" },
  Speed { speed: 1.1, duration_delta: "-0.15" },
  Speed { speed: 1.2, duration_delta: "-0.3" },
  Speed { speed: 1.3, duration_delta: "-0.6" },
];

const fn phonon_40m(
  id: &'static str,
  name: &'static str,
  source: &'static str,
  languages: &'static [&'static str],
) -> Model {
  Model { id, name, source, size_mb: 75, languages, license: "CC BY-NC 4.0", gated: true, speeds: PHONON_SPEEDS }
}

pub const MODELS: &[Model] = &[
  Model {
    id: "phonon-31m-en",
    name: "Phonon 31M English",
    source: "hf://gradium/phonon-31m-en@92741c5b07d085ff840e26fea8ce43110b87852f",
    size_mb: 65,
    languages: &["en"],
    license: "Apache 2.0",
    gated: false,
    speeds: PHONON_SPEEDS,
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
}
