//! Telling which of the languages set up in the settings a text is in, so it reads in that language's voice.

use crate::settings::Settings;
use whatlang::{Detector, Lang};

/// whatlang's own reliable flag wants 0.9, which most one-line reads never reach. Choosing between two or three
/// languages, full sentences score near 1.0 and "Hallo, wie geht es dir?" 0.33, while "Run cargo test." scores 0.08
/// and "Thanks!" 0.15 and stay in the main language.
const CONFIDENT: f64 = 0.25;

/// The language of `text` among the main one and the others in the settings. Text too short or unclear to tell is
/// in the main language.
pub fn detect(text: &str, settings: &Settings) -> String {
  let main = settings.language.clone();
  let candidates: Vec<(&str, Lang)> = std::iter::once(main.as_str())
    .chain(settings.other_languages.iter().map(|other| other.language.as_str()))
    .filter_map(|code| Some((code, lang(code)?)))
    .collect();
  if candidates.len() < 2 {
    return main;
  }
  let detector = Detector::with_allowlist(candidates.iter().map(|&(_, lang)| lang).collect());
  detector
    .detect(text)
    .filter(|info| info.confidence() >= CONFIDENT)
    .and_then(|info| candidates.iter().find(|&&(_, lang)| lang == info.lang()))
    .map_or(main.clone(), |&(code, _)| code.to_string())
}

/// whatlang's language for a code in the registry.
fn lang(code: &str) -> Option<Lang> {
  Some(match code {
    "en" => Lang::Eng,
    "fr" => Lang::Fra,
    "de" => Lang::Deu,
    "es" => Lang::Spa,
    "pt" => Lang::Por,
    "it" => Lang::Ita,
    "ru" => Lang::Rus,
    "zh" => Lang::Cmn,
    "ja" => Lang::Jpn,
    "ko" => Lang::Kor,
    "hi" => Lang::Hin,
    _ => return None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::registry::MODELS;
  use crate::settings::OtherLanguage;

  fn with_german() -> Settings {
    let german = OtherLanguage { language: "de".into(), model: "phonon-40m-de".into(), voice: "Anna".into() };
    Settings { other_languages: vec![german], ..Settings::default() }
  }

  // speak.detect
  #[test]
  fn reads_in_the_language_of_the_text() {
    let settings = with_german();
    assert_eq!(detect("Ich habe das Problem mit dem Server gelöst.", &settings), "de");
    assert_eq!(detect("The build failed on Windows again.", &settings), "en");
  }

  // speak.detect
  #[test]
  fn unclear_text_reads_in_the_main_language() {
    assert_eq!(detect("Run cargo test.", &with_german()), "en");
  }

  // speak.detect
  #[test]
  fn without_other_languages_everything_reads_in_the_main_one() {
    assert_eq!(detect("Ich habe das Problem mit dem Server gelöst.", &Settings::default()), "en");
  }

  #[test]
  fn every_registry_language_can_be_told() {
    for code in MODELS.iter().flat_map(|model| model.languages) {
      assert!(lang(code).is_some(), "{code} has no whatlang language");
    }
  }
}
