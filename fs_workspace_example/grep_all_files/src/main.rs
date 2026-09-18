use std::{
  env, fs, io,
  path::{Path, PathBuf},
};

use aho_corasick::AhoCorasick;
use walkdir::WalkDir;

const INCREMENT_DIR_DEFAULT: &str = "~/abc";
const TARGET_DIR_DEFAULT: &str = "~/cde/";

#[derive(Debug)]
struct Pattern {
  name: String,
  lowercase_name: String,
}

#[derive(Debug)]
struct Match {
  path: PathBuf,
  line_number: usize,
  text: String,
}

fn main() -> io::Result<()> {
  let increment_dir = dir_from_env("GREP_ALL_FILES_INCREMENT_DIR", INCREMENT_DIR_DEFAULT)?;
  let target_dir = dir_from_env("GREP_ALL_FILES_TARGET_DIR", TARGET_DIR_DEFAULT)?;

  println!("increment dir: {}", increment_dir.display());
  println!("target dir:    {}", target_dir.display());

  let patterns = read_patterns(&increment_dir)?;

  if patterns.is_empty() {
    println!(
      "no files or directories found in {}",
      increment_dir.display()
    );
    return Ok(());
  }

  let matches = grep_dir(&target_dir, &patterns)?;
  let mut matched_patterns = 0;

  for (pattern, pattern_matches) in patterns.iter().zip(&matches) {
    if pattern_matches.is_empty() {
      continue;
    }

    matched_patterns += 1;
    print_matches(pattern, pattern_matches);
  }

  println!(
    "{matched_patterns} of {} file/directory names found in {}",
    patterns.len(),
    target_dir.display()
  );
  Ok(())
}

fn dir_from_env(key: &str, default: &str) -> io::Result<PathBuf> {
  let dir = match env::var_os(key) {
    Some(value) => match value.to_str() {
      Some(value) => expand_home(value)?,
      None => PathBuf::from(value),
    },
    None => expand_home(default)?,
  };

  if !dir.is_dir() {
    return Err(io::Error::new(
      io::ErrorKind::NotADirectory,
      format!("{key} is not a directory: {}", dir.display()),
    ));
  }

  Ok(dir)
}

fn expand_home(path: &str) -> io::Result<PathBuf> {
  let Some(stripped) = path.strip_prefix("~/") else {
    return Ok(PathBuf::from(path));
  };

  let home = env::var_os("HOME")
    .map(PathBuf::from)
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is not set"))?;

  Ok(home.join(stripped))
}

fn read_patterns(dir: impl AsRef<Path>) -> io::Result<Vec<Pattern>> {
  let mut names = Vec::new();

  for entry in fs::read_dir(dir.as_ref())? {
    let entry = entry?;
    let file_type = entry.file_type()?;
    if !file_type.is_file() && !file_type.is_dir() {
      continue;
    }

    let name = entry.file_name().to_string_lossy().into_owned();
    if name.starts_with('.') {
      continue;
    }

    names.push(name);
  }

  names.sort();
  names.dedup();

  Ok(names.into_iter().map(Pattern::new).collect())
}

impl Pattern {
  fn new(name: String) -> Self {
    Self {
      lowercase_name: search_title(&name).to_lowercase(),
      name,
    }
  }
}

fn search_title(name: &str) -> &str {
  let mut title = name;
  if let Some((stem, extension)) = title.rsplit_once('.')
    && !stem.is_empty()
    && (extension.eq_ignore_ascii_case("pdf") || extension.eq_ignore_ascii_case("epub"))
  {
    title = stem;
  }

  while let Some((prefix, suffix)) = title.rsplit_once('.') {
    // Remove trailing dates and editions without changing numbers inside the title.
    let numeric = suffix.split('-').all(is_digits);
    let ordinal = ["st", "nd", "rd", "th"].iter().any(|ending| {
      suffix.len() > ending.len()
        && suffix
          .get(suffix.len() - ending.len() ..)
          .is_some_and(|tail| tail.eq_ignore_ascii_case(ending))
        && suffix
          .get(.. suffix.len() - ending.len())
          .is_some_and(is_digits)
    });
    if prefix.is_empty() || !(numeric || ordinal) {
      break;
    }
    title = prefix;
  }
  title
}

fn is_digits(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn grep_dir(dir: impl AsRef<Path>, patterns: &[Pattern]) -> io::Result<Vec<Vec<Match>>> {
  let mut matches = (0 .. patterns.len())
    .map(|_| Vec::new())
    .collect::<Vec<_>>();
  if patterns.is_empty() {
    return Ok(matches);
  }
  // Build once and reuse for every file; keep the existing Unicode lowercasing.
  let matcher = AhoCorasick::new(patterns.iter().map(|pattern| &pattern.lowercase_name))
    .map_err(io::Error::other)?;

  for entry in WalkDir::new(dir.as_ref()).sort_by_file_name() {
    let entry = match entry {
      Ok(entry) => entry,
      Err(error) => {
        eprintln!("skip: {error}");
        continue;
      }
    };

    if !entry.file_type().is_file() {
      continue;
    }

    let Some(content) = read_text_file(entry.path()) else {
      continue;
    };

    collect_file_matches(entry.path(), &content, &matcher, &mut matches);
  }

  Ok(matches)
}

fn read_text_file(path: &Path) -> Option<String> {
  match fs::read(path) {
    Ok(bytes) => String::from_utf8(bytes).ok(),
    Err(error) => {
      eprintln!("skip {}: {error}", path.display());
      None
    }
  }
}

fn collect_file_matches(
  path: &Path,
  content: &str,
  matcher: &AhoCorasick,
  matches: &mut [Vec<Match>],
) {
  let mut last_matched_line = vec![0; matches.len()];
  for (line_index, text) in content.lines().enumerate() {
    let lowercase_text = text.to_lowercase();
    let line_number = line_index + 1;

    // Overlapping matches preserve titles that are substrings of other titles.
    for found in matcher.find_overlapping_iter(&lowercase_text) {
      let index = found.pattern().as_usize();
      if last_matched_line[index] == line_number {
        continue;
      }
      last_matched_line[index] = line_number;

      matches[index].push(Match {
        path: path.to_path_buf(),
        line_number,
        text: text.to_owned(),
      });
    }
  }
}

fn print_matches(pattern: &Pattern, matches: &[Match]) {
  println!("{}", pattern.name);

  for item in matches {
    println!(
      "  {}:{}: {}",
      item.path.display(),
      item.line_number,
      item.text
    );
  }

  println!();
}

#[cfg(test)]
mod tests {
  use super::*;

  struct TestDir {
    path: PathBuf,
  }

  impl TestDir {
    fn new(name: &str) -> Self {
      let path = env::temp_dir().join(format!("grep_all_files_{name}"));
      let _ = fs::remove_dir_all(&path);
      fs::create_dir_all(&path).unwrap();

      Self { path }
    }

    fn write(&self, name: &str, content: impl AsRef<[u8]>) -> PathBuf {
      let path = self.path.join(name);
      if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
      }
      fs::write(&path, content).unwrap();

      path
    }
  }

  impl Drop for TestDir {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.path);
    }
  }

  #[test]
  fn reads_sorted_file_and_directory_names_without_hidden_entries() {
    let dir = TestDir::new("patterns");
    dir.write("b.pdf", "");
    dir.write("a.pdf", "");
    dir.write(".DS_Store", "");
    dir.write(".hidden/ignored.pdf", "");
    dir.write("nested/c.pdf", "");

    let patterns = read_patterns(&dir.path).unwrap();

    let names = patterns
      .iter()
      .map(|pattern| pattern.name.as_str())
      .collect::<Vec<_>>();
    assert_eq!(names, vec!["a.pdf", "b.pdf", "nested"]);
  }

  #[test]
  fn finds_file_names_in_nested_target_files() {
    let dir = TestDir::new("target");
    let list = dir.write(
      "lists/english.txt",
      "Learn.to.Code.with.Rust.2026.pdf\nother line\n",
    );
    dir.write("lists/binary.bin", [0xff, 0xfe, 0x00]);

    let patterns = vec![
      Pattern::new("Learn.to.Code.with.Rust.2026.pdf".to_owned()),
      Pattern::new("missing.pdf".to_owned()),
    ];
    let matches = grep_dir(&dir.path, &patterns).unwrap();

    assert_eq!(matches[0].len(), 1);
    assert_eq!(matches[0][0].path, list);
    assert_eq!(matches[0][0].line_number, 1);
    assert_eq!(matches[0][0].text, "Learn.to.Code.with.Rust.2026.pdf");
    assert!(matches[1].is_empty());
  }

  #[test]
  fn matches_file_names_ignoring_case_and_surrounding_text() {
    let dir = TestDir::new("case");
    dir.write(
      "english.txt",
      "./books/learn.to.code.with.RUST.2026.pdf : disk",
    );

    let patterns = vec![Pattern::new("Learn.to.Code.with.Rust.2026.pdf".to_owned())];
    let matches = grep_dir(&dir.path, &patterns).unwrap();

    assert_eq!(matches[0].len(), 1);
    assert_eq!(matches[0][0].line_number, 1);
  }

  #[test]
  fn strips_extensions_and_trailing_numbers_and_editions() {
    for (name, expected) in [
      (
        "CC++.Pointers.&.Applications.1080.2025-9",
        "CC++.Pointers.&.Applications",
      ),
      ("Rust3.Guide.2025", "Rust3.Guide"),
      ("Chapter.2.Guide.1080.2025-9", "Chapter.2.Guide"),
      ("book.pdf", "book"),
      ("book.2025.pdf", "book"),
      (
        "Building.Data-Driven.Applications.with.LlamaIndex.2nd.2026.pdf",
        "Building.Data-Driven.Applications.with.LlamaIndex",
      ),
      ("book.1st.2026.epub", "book"),
      ("book.3RD.2026.PDF", "book"),
      ("book.11th.EPUB", "book"),
      ("Guide.2nd.Chapter.2026.pdf", "Guide.2nd.Chapter"),
      ("book.second.pdf", "book.second"),
      ("book.2026.txt", "book.2026.txt"),
      ("中文书名.pdf", "中文书名"),
      ("book.1080p", "book.1080p"),
      ("123", "123"),
      (".123", ".123"),
    ] {
      assert_eq!(search_title(name), expected);
    }
  }

  #[test]
  fn searches_nested_files_using_directory_title_without_numeric_suffix() {
    let increment = TestDir::new("numeric_increment");
    let name = "CC++.Pointers.&.Applications.1080.2025-9";
    fs::create_dir(increment.path.join(name)).unwrap();
    let target = TestDir::new("numeric_target");
    let list = target.write(
      "archive/nested/list.txt",
      "CCxx.Pointers.&.Applications\n/books/cc++.pointers.&.applications.720.2024-1\n",
    );

    let patterns = read_patterns(&increment.path).unwrap();
    let matches = grep_dir(&target.path, &patterns).unwrap();

    assert_eq!(patterns[0].name, name);
    assert_eq!(matches[0].len(), 1);
    assert_eq!(matches[0][0].path, list);
    assert_eq!(matches[0][0].line_number, 2);
  }

  #[test]
  fn matches_books_with_different_extensions_and_editions() {
    let dir = TestDir::new("editions");
    dir.write(
      "list.txt",
      "Building.Data-Driven.Applications.with.LlamaIndex.1st.2024.epub\n",
    );
    let patterns = vec![Pattern::new(
      "Building.Data-Driven.Applications.with.LlamaIndex.2nd.2026.pdf".to_owned(),
    )];
    let matches = grep_dir(&dir.path, &patterns).unwrap();
    assert_eq!(matches[0].len(), 1);
    assert_eq!(matches[0][0].line_number, 1);
  }

  #[test]
  fn multi_pattern_search_matches_naive_search() {
    let dir = TestDir::new("multi_pattern");
    let content = "RUST.Book rust.book RUST\nÄPFEL 中文书\ncc++.a ccxxza\nRust.Book\nnone\n";
    dir.write("list.txt", content);
    let patterns = [
      "Rust",
      "Rust.Book",
      "Rust.2026.pdf",
      "äpfel",
      "中文书",
      "CC++.a",
      "missing",
    ]
    .into_iter()
    .map(|name| Pattern::new(name.to_owned()))
    .collect::<Vec<_>>();
    let matches = grep_dir(&dir.path, &patterns).unwrap();

    for (pattern, found) in patterns.iter().zip(&matches) {
      let expected = content
        .lines()
        .enumerate()
        .filter(|(_, text)| text.to_lowercase().contains(&pattern.lowercase_name))
        .map(|(index, text)| (index + 1, text))
        .collect::<Vec<_>>();
      let actual = found
        .iter()
        .map(|item| (item.line_number, item.text.as_str()))
        .collect::<Vec<_>>();
      assert_eq!(actual, expected, "{}", pattern.name);
    }
    assert!(grep_dir(&dir.path, &[]).unwrap().is_empty());
  }

  #[test]
  fn expands_home_prefixed_paths() {
    let Some(home) = env::var_os("HOME") else {
      return;
    };

    assert_eq!(
      expand_home("~/cde").unwrap(),
      PathBuf::from(home).join("cde")
    );
    assert_eq!(expand_home("files").unwrap(), PathBuf::from("files"));
  }
}
