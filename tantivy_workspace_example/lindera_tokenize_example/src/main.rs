use lindera::{
  LinderaResult, dictionary::load_dictionary, mode::Mode, segmenter::Segmenter,
  tokenizer::Tokenizer,
};

fn main() -> LinderaResult<()> {
  let dictionary = load_dictionary("embedded://ipadic")?;
  let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
  let tokenizer = Tokenizer::new(segmenter);

  let text = "関西国際空港限定トートバッグ";
  let mut tokens = tokenizer.tokenize(text)?;
  println!("text:\t{}", text);
  for token in tokens.iter_mut() {
    let details = token.details().join(",");
    println!("token:\t{}\t{}", token.surface.as_ref(), details);
  }

  Ok(())
}

// copy from https://github.com/lindera/lindera/blob/main/examples/tokenizer_example.rs
