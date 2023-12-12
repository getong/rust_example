use unicode_segmentation::UnicodeSegmentation;

fn main() {
  let s = "a̐éö̲\r\n";
  let g = s.graphemes(true).collect::<Vec<&str>>();
  let b: &[_] = &["a̐", "é", "ö̲", "\r\n"];
  assert_eq!(g, b);

  let s = "The quick (\"brown\") fox can't jump 32.3 feet, right?";
  let w = s.unicode_words().collect::<Vec<&str>>();
  let b: &[_] = &[
    "The", "quick", "brown", "fox", "can't", "jump", "32.3", "feet", "right",
  ];
  assert_eq!(w, b);

  let s = "The quick (\"brown\")  fox";
  let w = s.split_word_bounds().collect::<Vec<&str>>();
  let b: &[_] = &[
    "The", " ", "quick", " ", "(", "\"", "brown", "\"", ")", "  ", "fox",
  ];
  assert_eq!(w, b);
}
