use std::{env, error::Error, fs};

pub struct GrepOpts<'a> {
  query: &'a str,
  filename: &'a str,
  insensitive: bool,
  inverse: bool,
}

// I don't fully understand '_ here; rust told me to do it.
impl GrepOpts<'_> {
  pub fn build(query: &str, insensitive: bool, inverse: bool) -> GrepOpts {
    GrepOpts {
      query,
      insensitive,
      inverse,
      filename: "",
    }
  }

  pub fn from(argv: &[String]) -> Result<GrepOpts, String> {
    let mut opts = GrepOpts {
      query: "",
      filename: "",
      insensitive: false,
      inverse: false,
    };

    for arg in argv {
      let mut flags = arg.chars();
      if flags.next().unwrap() == '-' {
        for flag in flags {
          match flag {
            'v' => opts.inverse = true,
            'i' => opts.insensitive = true,
            _ => (),
          }
        }
      } else if opts.query == "" {
        opts.query = arg.as_str();
      } else if opts.filename == "" {
        opts.filename = arg.as_str()
      } else {
        return Err(format!("Unknown option: {}", arg));
      }
    }

    if opts.query == "" {
      return Err("No search query provided.".to_string());
    } else if opts.filename == "" {
      return Err("No filename provided.".to_string());
    }

    Ok(opts)
  }
}

pub fn run(opts: GrepOpts) -> Result<bool, Box<dyn Error>> {
  let contents = fs::read_to_string(opts.filename)?;
  let quiet = env::var("QUIET").is_ok();
  let results = if opts.insensitive {
    search_case_insensitive(&contents, opts)
  } else {
    search_case_sensitive(&contents, opts)
  };

  if !quiet {
    for &line in &results {
      println!("{}", line);
    }
  }

  Ok(results.len() > 0)
}

fn search_case_sensitive<'a>(contents: &'a str, opts: GrepOpts) -> Vec<&'a str> {
  let mut results = Vec::new();
  for line in contents.lines() {
    if line.contains(opts.query) != opts.inverse {
      results.push(line);
    }
  }

  results
}

fn search_case_insensitive<'a>(contents: &'a str, opts: GrepOpts) -> Vec<&'a str> {
  let mut results = Vec::new();
  let query = opts.query.to_lowercase();
  for line in contents.lines() {
    if line.to_lowercase().contains(&query) != opts.inverse {
      results.push(line);
    }
  }

  results
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_str<'a>() -> &'a str {
    "Rust:
safe, fast, productive
Pick three.
Trust me."
  }

  #[test]
  fn case_sensitive() {
    let query = "duct";
    let contents = test_str();

    assert_eq!(
      vec!["safe, fast, productive"],
      search_case_sensitive(contents, GrepOpts::build(query, false, false))
    );
  }

  #[test]
  fn inv_case_sensitive() {
    let query = "duct";
    let contents = test_str();

    assert_eq!(
      vec!["Rust:", "Pick three.", "Trust me."],
      search_case_sensitive(contents, GrepOpts::build(query, false, true))
    );
  }

  #[test]
  fn case_insensitive() {
    let query = "rUsT";
    let contents = test_str();

    assert_eq!(
      vec!["Rust:", "Trust me."],
      search_case_insensitive(contents, GrepOpts::build(query, true, false))
    )
  }

  #[test]
  fn inv_case_insensitive() {
    let query = "rUsT";
    let contents = test_str();

    assert_eq!(
      vec!["safe, fast, productive", "Pick three."],
      search_case_insensitive(contents, GrepOpts::build(query, true, true))
    )
  }
}
