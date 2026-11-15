//! Turning text into the sentences yap speaks, caches and shows as subtitles.

/// Shorter sentences join the next one, so a stray "Yes." or list number isn't a piece of its own.
const MIN_PIECE: usize = 40;

/// The text as it should sound. Markdown reads badly aloud: table rows become sentences, code blocks are announced
/// instead of read, and markup like `**`, `#` and link targets is dropped.
pub fn prepare(text: &str) -> String {
  let mut out = Vec::new();
  let mut header: Option<Vec<String>> = None;
  let mut in_code = false;
  for line in text.lines() {
    let line = line.trim();
    if line.starts_with("```") {
      if !in_code {
        out.push("A code block, skipped.".to_string());
      }
      in_code = !in_code;
      continue;
    }
    if in_code {
      continue;
    }
    if line.starts_with('|') {
      let cells = table_cells(line);
      if cells.iter().all(|cell| cell.chars().all(|c| matches!(c, '-' | ':' | ' '))) {
        continue;
      }
      match &header {
        None => header = Some(cells),
        Some(names) => out.push(table_row(names, &cells)),
      }
      continue;
    }
    header = None;
    out.push(sentence(&inline(line.trim_start_matches(['#', '>', ' ']))));
  }
  out.join("\n")
}

fn table_cells(line: &str) -> Vec<String> {
  line.trim_matches('|').split('|').map(|cell| inline(cell.trim())).collect()
}

/// "Name: Miles, Language: English." Empty cells are left out.
fn table_row(names: &[String], cells: &[String]) -> String {
  let pairs: Vec<String> = names
    .iter()
    .zip(cells)
    .filter(|(_, cell)| !cell.is_empty())
    .map(|(name, cell)| if name.is_empty() { cell.clone() } else { format!("{name}: {cell}") })
    .collect();
  sentence(&pairs.join(", "))
}

/// Ends a line that has no closing punctuation with a period, so headings and list items get a pause.
fn sentence(line: &str) -> String {
  let line = line.trim_start_matches(['-', '*', '+', ' ']).trim();
  match line.chars().last() {
    None => String::new(),
    Some(c) if c.is_alphanumeric() || c == ')' => format!("{line}."),
    Some(_) => line.to_string(),
  }
}

/// Drops emphasis and inline code marks, and keeps only the text of `[text](url)` links.
fn inline(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  let mut rest = text;
  while let Some(start) = rest.find('[') {
    let Some(close) = rest[start..].find("](").map(|i| start + i) else { break };
    let Some(end) = rest[close..].find(')').map(|i| close + i) else { break };
    out.push_str(&rest[..start]);
    out.push_str(&rest[start + 1..close]);
    rest = &rest[end + 1..];
  }
  out.push_str(rest);
  out.replace("**", "").replace("__", "").replace('`', "")
}

/// Splits prepared text into sentences, the unit yap generates, caches and subtitles. Whitespace is collapsed first,
/// so line wrapping can't move the split points.
pub fn pieces(text: &str) -> Vec<String> {
  let mut pieces = Vec::new();
  let mut piece = String::new();
  for paragraph in text.split("\n\n") {
    let words: Vec<&str> = paragraph.split_whitespace().collect();
    for (i, word) in words.iter().enumerate() {
      if !piece.is_empty() {
        piece.push(' ');
      }
      piece.push_str(word);
      let ends = word.ends_with(['.', '!', '?', '…']) || i == words.len() - 1;
      if ends && piece.len() >= MIN_PIECE {
        pieces.push(std::mem::take(&mut piece));
      }
    }
  }
  if !piece.is_empty() {
    pieces.push(piece);
  }
  pieces
}

/// The text without the punctuation apps disagree on, like quote styles and dashes, so they hit the same cache entry.
/// A "?" changes how it sounds, and so does punctuation between digits, as in 3.5, so both stay.
pub fn words(text: &str) -> String {
  let chars: Vec<char> = text.chars().collect();
  let kept: String = chars
    .iter()
    .enumerate()
    .filter(|&(i, &c)| {
      c.is_alphanumeric()
        || c.is_whitespace()
        || c == '?'
        || c == '_'
        || (i > 0 && chars[i - 1].is_ascii_digit() && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit()))
    })
    .map(|(_, &c)| c)
    .collect();
  kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  // text.tables
  fn text_prepare_tables_become_sentences() {
    let table = "| Voice | Language |\n|---|---|\n| Miles | English |\n| Maeve | |";
    assert_eq!(prepare(table), "Voice: Miles, Language: English.\nVoice: Maeve.");
  }

  #[test]
  // text.code
  fn text_prepare_code_blocks_are_announced() {
    assert_eq!(prepare("Run this:\n```sh\nrm -rf /\n```\nDone"), "Run this:\nA code block, skipped.\nDone.");
  }

  #[test]
  // text.markup
  fn text_prepare_drops_markup() {
    assert_eq!(prepare("## Setup\n- see **the** [docs](https://x.y) and `yap`"), "Setup.\nsee the docs and yap.");
  }

  #[test]
  // text.pieces
  fn text_pieces_split_at_sentences_and_join_short_ones() {
    let text = "Yes. This sentence is long enough to stand on its own.\nIt wraps  here.\n\nNew paragraph";
    assert_eq!(
      pieces(text),
      ["Yes. This sentence is long enough to stand on its own.", "It wraps here. New paragraph"]
    );
  }

  #[test]
  // library.replay
  fn text_words_ignore_quotes_and_dashes_but_keep_numbers_and_questions() {
    assert_eq!(words("“Version 3.5” — ready?"), words("\"Version 3.5\" - ready?"));
    assert_eq!(words("3.5 or 35?"), "3.5 or 35?");
  }
}
