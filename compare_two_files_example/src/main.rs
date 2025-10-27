use std::{
  collections::HashMap,
  env,
  fs::File,
  io::{BufRead, BufReader},
  process,
};

fn main() {
  let args: Vec<String> = env::args().collect();

  if args.len() != 3 {
    eprintln!("Usage: {} <old.txt> <new.txt>", args[0]);
    process::exit(1);
  }

  let old_file = &args[1];
  let new_file = &args[2];

  match find_similar_lines(old_file, new_file) {
    Ok(matches) => {
      if matches.is_empty() {
        println!("No similar lines found between the files.");
      } else {
        println!("Similar lines found (ignoring timestamps):");
        for (old_line, new_line) in matches {
          println!("old.txt: {}", old_line);
          println!("new.txt: {}", new_line);
          println!("---");
        }
      }
    }
    Err(e) => {
      eprintln!("Error: {}", e);
      process::exit(1);
    }
  }
}

fn read_lines_to_vec(filename: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
  let file = File::open(filename)?;
  let reader = BufReader::new(file);
  let mut lines = Vec::new();

  for line in reader.lines() {
    let line = line?;
    let trimmed = line.trim();
    if !trimmed.is_empty() {
      lines.push(trimmed.to_string());
    }
  }

  Ok(lines)
}

fn normalize_line(line: &str) -> String {
  // Remove timestamp patterns like "2022-5", "2025-2", etc.
  let re = regex::Regex::new(r"\d{4}-\d+").unwrap();
  re.replace_all(line, "TIMESTAMP").to_string()
}

fn find_similar_lines(
  old_file: &str,
  new_file: &str,
) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
  let old_lines = read_lines_to_vec(old_file)?;
  let new_lines = read_lines_to_vec(new_file)?;

  let mut old_normalized: HashMap<String, String> = HashMap::new();
  for line in &old_lines {
    let normalized = normalize_line(line);
    old_normalized.insert(normalized, line.clone());
  }

  let mut matches = Vec::new();
  for new_line in &new_lines {
    let normalized = normalize_line(new_line);
    if let Some(old_line) = old_normalized.get(&normalized) {
      matches.push((old_line.clone(), new_line.clone()));
    }
  }

  Ok(matches)
}
