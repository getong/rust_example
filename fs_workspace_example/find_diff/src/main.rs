use std::{
  cmp::Ordering,
  collections::{HashMap, HashSet},
  fs::{self, File},
  io::{self, BufWriter, Write},
  iter::Peekable,
  path::{Path, PathBuf},
  str::Chars,
  sync::Arc,
};

use merkle_search_tree::MerkleSearchTree;
use strsim::normalized_levenshtein;

const INPUT_DIR: &str = "files";
const OUTPUT_FILE: &str = "out.txt";
const SIMILARITY_THRESHOLD: f64 = 0.85;
const MAX_BROAD_KEY_POSTINGS: usize = 256;

#[derive(Debug)]
struct Document {
  name: String,
  lines: Vec<Line>,
}

#[derive(Debug)]
struct Line {
  text: String,
  comparison_text: String,
  comparison_char_len: usize,
  candidate_keys: Vec<String>,
}

#[derive(Debug)]
struct IndexedDocuments {
  documents: Vec<Document>,
  lines: Vec<LineRef>,
}

#[derive(Clone, Copy, Debug)]
struct LineRef {
  document: usize,
  line: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct LinePair {
  left: usize,
  right: usize,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> io::Result<()> {
  let documents = read_documents(INPUT_DIR)?;
  let documents = Arc::new(IndexedDocuments::new(documents));
  let matches = find_similar_lines(Arc::clone(&documents)).await?;
  let groups = group_matches(&matches);
  write_groups(OUTPUT_FILE, &documents, &groups)?;

  println!("wrote {} groups to {}", groups.len(), OUTPUT_FILE);
  Ok(())
}

impl IndexedDocuments {
  fn new(documents: Vec<Document>) -> Self {
    let mut lines = Vec::new();

    for (document, item) in documents.iter().enumerate() {
      lines.extend((0 .. item.lines.len()).map(|line| LineRef { document, line }));
    }

    Self { documents, lines }
  }

  fn line(&self, line_index: usize) -> &Line {
    let line_ref = self.lines[line_index];
    &self.documents[line_ref.document].lines[line_ref.line]
  }

  fn file_name(&self, line_index: usize) -> &str {
    let line_ref = self.lines[line_index];
    &self.documents[line_ref.document].name
  }
}

fn read_documents(dir: impl AsRef<Path>) -> io::Result<Vec<Document>> {
  let mut paths = txt_files(dir.as_ref())?;
  paths.sort();

  paths.into_iter().map(|path| read_document(&path)).collect()
}

fn txt_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
  let mut paths = Vec::new();

  for entry in fs::read_dir(dir)? {
    let path = entry?.path();
    if path.extension().is_some_and(|extension| extension == "txt") {
      paths.push(path);
    }
  }

  Ok(paths)
}

fn read_document(path: &Path) -> io::Result<Document> {
  let content = fs::read_to_string(path)?;
  let name = path
    .file_stem()
    .unwrap_or_default()
    .to_string_lossy()
    .into_owned();

  let mut seen = HashSet::new();
  let mut lines = Vec::new();

  for raw_line in content.lines() {
    let text = raw_line.trim();
    if text.is_empty() || !seen.insert(text.to_owned()) {
      continue;
    }

    let comparison_text = comparison_slice(text);
    lines.push(Line {
      text: text.to_owned(),
      comparison_text: comparison_text.to_owned(),
      comparison_char_len: comparison_text.chars().count(),
      candidate_keys: candidate_keys(comparison_text),
    });
  }

  Ok(Document { name, lines })
}

async fn find_similar_lines(documents: Arc<IndexedDocuments>) -> io::Result<Vec<LinePair>> {
  let candidate_index = build_global_candidate_index(&documents);
  let shard_keys = mst_ordered_keys(candidate_index.keys());
  let shard_count = worker_count().min(shard_keys.len().max(1));
  let key_shards = split_evenly(&shard_keys, shard_count);
  let mut handles = Vec::with_capacity(key_shards.len());

  for keys in key_shards {
    let documents = Arc::clone(&documents);
    let shard = keys
      .into_iter()
      .filter_map(|key| {
        candidate_index
          .get(&key)
          .map(|indices| (key.clone(), indices.clone()))
      })
      .collect::<Vec<_>>();

    handles.push(tokio::task::spawn_blocking(move || {
      collect_shard_matches(&documents, shard)
    }));
  }

  let mut matches = Vec::new();
  for handle in handles {
    let mut shard_matches = handle.await.map_err(io::Error::other)?;
    matches.append(&mut shard_matches);
  }

  matches.sort_unstable();
  matches.dedup();
  Ok(matches)
}

fn build_global_candidate_index(documents: &IndexedDocuments) -> HashMap<String, Vec<usize>> {
  let mut index: HashMap<String, Vec<usize>> = HashMap::new();

  for (line_index, line_ref) in documents.lines.iter().enumerate() {
    let line = &documents.documents[line_ref.document].lines[line_ref.line];
    for key in &line.candidate_keys {
      index.entry(key.clone()).or_default().push(line_index);
    }
  }

  index
    .into_iter()
    .filter_map(|(key, mut line_indices)| {
      line_indices.sort_unstable();
      line_indices.dedup();

      if (2 ..= MAX_BROAD_KEY_POSTINGS).contains(&line_indices.len()) {
        Some((key, line_indices))
      } else {
        None
      }
    })
    .collect()
}

fn mst_ordered_keys<'a>(keys: impl Iterator<Item = &'a String>) -> Vec<String> {
  let mut tree = MerkleSearchTree::<String, ()>::default();

  for key in keys {
    tree.upsert(key.clone(), &());
  }
  let _ = tree.root_hash();

  tree.node_iter().map(|node| node.key().clone()).collect()
}

fn split_evenly<T: Clone>(items: &[T], shard_count: usize) -> Vec<Vec<T>> {
  if items.is_empty() {
    return Vec::new();
  }

  let shard_count = shard_count.max(1);
  let chunk_size = items.len().div_ceil(shard_count);

  items
    .chunks(chunk_size)
    .map(<[T]>::to_vec)
    .collect::<Vec<_>>()
}

fn worker_count() -> usize {
  std::thread::available_parallelism()
    .map(usize::from)
    .unwrap_or(1)
}

fn collect_shard_matches(
  documents: &IndexedDocuments,
  shard: Vec<(String, Vec<usize>)>,
) -> Vec<LinePair> {
  let mut seen = HashSet::new();
  let mut matches = Vec::new();

  for (_key, line_indices) in shard {
    for left_pos in 0 .. line_indices.len() {
      for right_pos in (left_pos + 1) .. line_indices.len() {
        let left = line_indices[left_pos];
        let right = line_indices[right_pos];
        let pair = LinePair { left, right };

        if !seen.insert(pair) || !is_similar_pair(documents, pair) {
          continue;
        }

        matches.push(pair);
      }
    }
  }

  matches
}

fn is_similar_pair(documents: &IndexedDocuments, pair: LinePair) -> bool {
  let left = documents.line(pair.left);
  let right = documents.line(pair.right);

  if left.text == right.text {
    return false;
  }

  if !can_reach_threshold(left.comparison_char_len, right.comparison_char_len) {
    return false;
  }

  normalized_levenshtein(&left.comparison_text, &right.comparison_text) >= SIMILARITY_THRESHOLD
}

fn comparison_slice(text: &str) -> &str {
  text.rsplit_once('/').map_or(text, |(_, tail)| tail)
}

fn candidate_keys(text: &str) -> Vec<String> {
  let tokens = significant_tokens(text);
  let mut keys = Vec::new();

  let normalized = normalize_digits(text);
  if !normalized.is_empty() {
    keys.push(format!("full:{normalized}"));
  }

  for pair in tokens.windows(2) {
    keys.push(format!("pair:{}|{}", pair[0], pair[1]));
  }

  if tokens.len() <= 2 {
    for token in &tokens {
      keys.push(format!("tok:{token}"));
    }
  }

  keys.sort();
  keys.dedup();
  keys
}

fn normalize_digits(text: &str) -> String {
  let mut normalized = String::with_capacity(text.len());
  let mut in_digits = false;
  let mut last_was_separator = true;

  for ch in text.chars() {
    if ch.is_ascii_digit() {
      if !in_digits {
        normalized.push('#');
      }
      in_digits = true;
      last_was_separator = false;
      continue;
    }

    in_digits = false;

    if ch.is_alphabetic() {
      normalized.extend(ch.to_lowercase());
      last_was_separator = false;
    } else if !last_was_separator {
      normalized.push('.');
      last_was_separator = true;
    }
  }

  if normalized.ends_with('.') {
    normalized.pop();
  }

  normalized
}

fn significant_tokens(text: &str) -> Vec<String> {
  let mut tokens = Vec::new();
  let mut current = String::new();

  for ch in text.chars() {
    if ch.is_alphabetic() {
      current.extend(ch.to_lowercase());
    } else {
      push_significant_token(&mut tokens, &mut current);
    }
  }
  push_significant_token(&mut tokens, &mut current);

  if tokens.iter().any(|token| token.chars().count() >= 4) {
    tokens.retain(|token| token.chars().count() >= 4);
  } else {
    tokens.retain(|token| token.chars().count() >= 3);
  }

  tokens
}

fn push_significant_token(tokens: &mut Vec<String>, current: &mut String) {
  if !current.is_empty() {
    tokens.push(std::mem::take(current));
  }
}

fn can_reach_threshold(left_len: usize, right_len: usize) -> bool {
  let max_len = left_len.max(right_len);
  if max_len == 0 {
    return false;
  }

  let min_distance = left_len.abs_diff(right_len);
  let best_possible_score = 1.0 - (min_distance as f64 / max_len as f64);

  best_possible_score >= SIMILARITY_THRESHOLD
}

fn group_matches(matches: &[LinePair]) -> Vec<Vec<usize>> {
  let mut indices = HashMap::new();
  let mut lines = Vec::new();
  let mut parents = Vec::new();

  for item in matches {
    let left_index = match_line_index(item.left, &mut indices, &mut lines, &mut parents);
    let right_index = match_line_index(item.right, &mut indices, &mut lines, &mut parents);

    union(&mut parents, left_index, right_index);
  }

  let mut groups_by_root: HashMap<usize, Vec<usize>> = HashMap::new();
  for (index, line) in lines.iter().copied().enumerate() {
    let root = find_root(&mut parents, index);
    groups_by_root.entry(root).or_default().push(line);
  }

  groups_by_root.into_values().collect()
}

fn match_line_index(
  line: usize,
  indices: &mut HashMap<usize, usize>,
  lines: &mut Vec<usize>,
  parents: &mut Vec<usize>,
) -> usize {
  if let Some(&index) = indices.get(&line) {
    return index;
  }

  let index = lines.len();
  indices.insert(line, index);
  lines.push(line);
  parents.push(index);
  index
}

fn union(parents: &mut [usize], left: usize, right: usize) {
  let left_root = find_root(parents, left);
  let right_root = find_root(parents, right);

  if left_root != right_root {
    parents[right_root] = left_root;
  }
}

fn find_root(parents: &mut [usize], index: usize) -> usize {
  let parent = parents[index];
  if parent == index {
    return index;
  }

  let root = find_root(parents, parent);
  parents[index] = root;
  root
}

fn match_group_cmp(documents: &IndexedDocuments, left: &[usize], right: &[usize]) -> Ordering {
  match (left.first(), right.first()) {
    (Some(left), Some(right)) => match_line_cmp(documents, *left, *right),
    (None, Some(_)) => Ordering::Less,
    (Some(_), None) => Ordering::Greater,
    (None, None) => Ordering::Equal,
  }
}

fn match_line_cmp(documents: &IndexedDocuments, left: usize, right: usize) -> Ordering {
  let left_text = &documents.line(left).text;
  let right_text = &documents.line(right).text;

  natural_cmp(left_text, right_text)
    .then_with(|| documents.file_name(left).cmp(documents.file_name(right)))
}

fn write_groups(
  path: impl AsRef<Path>,
  documents: &IndexedDocuments,
  groups: &[Vec<usize>],
) -> io::Result<()> {
  let file = File::create(path)?;
  let mut writer = BufWriter::new(file);
  let mut groups = groups.to_vec();

  for group in &mut groups {
    group.sort_by(|left, right| match_line_cmp(documents, *left, *right));
    group.dedup();
  }
  groups.sort_by(|left, right| match_group_cmp(documents, left, right));

  for group in &groups {
    for &line_index in group {
      let line = documents.line(line_index);
      let file = documents.file_name(line_index);
      writeln!(writer, "{} : {}", line.text, file)?;
    }
    writeln!(writer)?;
  }

  writer.flush()
}

fn natural_cmp(left: &str, right: &str) -> Ordering {
  let mut left_chars = left.chars().peekable();
  let mut right_chars = right.chars().peekable();

  loop {
    match (left_chars.peek(), right_chars.peek()) {
      (None, None) => return Ordering::Equal,
      (None, Some(_)) => return Ordering::Less,
      (Some(_), None) => return Ordering::Greater,
      (Some(left_ch), Some(right_ch)) => {
        let ordering = if left_ch.is_ascii_digit() && right_ch.is_ascii_digit() {
          compare_digit_runs(&mut left_chars, &mut right_chars)
        } else {
          let left_lower = left_chars.next().into_iter().flat_map(char::to_lowercase);
          let right_lower = right_chars.next().into_iter().flat_map(char::to_lowercase);
          left_lower.cmp(right_lower)
        };

        if ordering != Ordering::Equal {
          return ordering;
        }
      }
    }
  }
}

fn compare_digit_runs(
  left_chars: &mut Peekable<Chars<'_>>,
  right_chars: &mut Peekable<Chars<'_>>,
) -> Ordering {
  let left_digits = take_digit_run(left_chars);
  let right_digits = take_digit_run(right_chars);
  let left_trimmed = left_digits.trim_start_matches('0');
  let right_trimmed = right_digits.trim_start_matches('0');
  let left_number = if left_trimmed.is_empty() {
    "0"
  } else {
    left_trimmed
  };
  let right_number = if right_trimmed.is_empty() {
    "0"
  } else {
    right_trimmed
  };

  left_number
    .len()
    .cmp(&right_number.len())
    .then_with(|| left_number.cmp(right_number))
    .then_with(|| left_digits.len().cmp(&right_digits.len()))
}

fn take_digit_run(chars: &mut Peekable<Chars<'_>>) -> String {
  let mut digits = String::new();

  while let Some(ch) = chars.next_if(char::is_ascii_digit) {
    digits.push(ch);
  }

  digits
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn normalizes_numbers_and_separators() {
    assert_eq!(normalize_digits("abc.2025"), "abc.#");
    assert_eq!(
      normalize_digits("Object-Oriented 2026.pdf"),
      "object.oriented.#.pdf"
    );
  }

  #[test]
  fn finds_lines_that_only_differ_by_year() {
    let documents = test_documents(&[("a", &["abc.2025"][..]), ("b", &["abc.2026"][..])]);
    let pair = LinePair { left: 0, right: 1 };

    assert!(is_similar_pair(&documents, pair));
  }

  #[test]
  fn compares_path_lines_by_file_name() {
    let documents = test_documents(&[(
      "a",
      &[
        "./unreal-engine-video/Unreal.Engine.for.Indie.Filmmakers",
        "Unreal.Engine.for.Indie.Filmmakers",
      ][..],
    )]);
    let pair = LinePair { left: 0, right: 1 };

    assert!(is_similar_pair(&documents, pair));
  }

  #[test]
  fn keeps_full_path_text_for_output() {
    let documents = test_documents(&[(
      "backup",
      &[
        "./unreal-engine-video/Unreal.Engine.for.Indie.Filmmakers",
        "Unreal.Engine.for.Indie.Filmmakers",
      ][..],
    )]);
    let groups = vec![vec![0, 1]];
    let path = std::env::temp_dir().join("find_diff_output_test.txt");

    write_groups(&path, &documents, &groups).unwrap();

    let output = fs::read_to_string(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert!(output.contains("./unreal-engine-video/Unreal.Engine.for.Indie.Filmmakers : backup"));
  }

  #[tokio::test]
  async fn finds_similar_lines_inside_same_document() {
    let documents = Arc::new(test_documents(&[(
      "a",
      &["Object-Oriented 2025.pdf", "Object-Oriented 2026.pdf"][..],
    )]));

    let matches = find_similar_lines(Arc::clone(&documents)).await.unwrap();

    assert_eq!(matches, vec![LinePair { left: 0, right: 1 }]);
  }

  #[test]
  fn natural_sort_orders_numbers_by_value() {
    assert_eq!(
      natural_cmp("course.2024-12", "course.2026-2"),
      Ordering::Less
    );
    assert_eq!(
      natural_cmp("course.2026-2", "course.2026-10"),
      Ordering::Less
    );
  }

  #[test]
  fn groups_transitive_matches() {
    let matches = vec![
      LinePair { left: 0, right: 1 },
      LinePair { left: 1, right: 2 },
    ];

    let mut groups = group_matches(&matches);
    for group in &mut groups {
      group.sort_unstable();
    }

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0], vec![0, 1, 2]);
  }

  fn test_documents(input: &[(&str, &[&str])]) -> IndexedDocuments {
    let documents = input
      .iter()
      .map(|(name, lines)| Document {
        name: (*name).to_owned(),
        lines: lines
          .iter()
          .map(|text| {
            let comparison_text = comparison_slice(text);

            Line {
              text: (*text).to_owned(),
              comparison_text: comparison_text.to_owned(),
              comparison_char_len: comparison_text.chars().count(),
              candidate_keys: candidate_keys(comparison_text),
            }
          })
          .collect(),
      })
      .collect();

    IndexedDocuments::new(documents)
  }
}
