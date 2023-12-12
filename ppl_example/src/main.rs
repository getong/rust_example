use std::env;
// use ppl::prelude::*;

use std::{
  collections::HashMap,
  fs::File,
  io::{BufRead, BufReader},
  usize,
};

use ppl::{
  prelude::*,
  templates::map::{Map, MapReduce, Reduce},
};

struct Source {
  reader: BufReader<File>,
}
impl Out<Vec<String>> for Source {
  fn run(&mut self) -> Option<Vec<String>> {
    let mut tmp = String::new();
    let res = self.reader.read_line(&mut tmp);
    match res {
      Ok(len) => {
        if len > 0 {
          Some(
            tmp
              .split_whitespace()
              .map(|s| {
                s.to_lowercase()
                  .chars()
                  .filter(|c| c.is_alphabetic())
                  .collect::<String>()
              })
              .collect(),
          )
        } else {
          None
        }
      }
      Err(e) => panic!("{}", e.to_string()),
    }
  }
}

struct Sink {
  counter: HashMap<String, usize>,
}
impl In<Vec<(String, usize)>, Vec<(String, usize)>> for Sink {
  fn run(&mut self, input: Vec<(String, usize)>) {
    // Increment value for key in hashmap
    // If key does not exist, insert it with value 1
    for (key, value) in input {
      let counter = self.counter.entry(key).or_insert(0);
      *counter += value;
    }
  }
  fn finalize(self) -> Option<Vec<(String, usize)>> {
    Some(self.counter.into_iter().collect())
  }
}

pub fn ppl(dataset: &str, threads: usize) {
  let file = File::open(dataset).expect("no such file");
  let reader = BufReader::new(file);

  let mut p = pipeline![
    Source { reader },
    Map::build::<Vec<String>, Vec<(String, usize)>>(threads / 2, |str| -> (String, usize) {
      (str, 1)
    }),
    Reduce::build(threads / 2, |str, count| {
      let mut sum = 0;
      for c in count {
        sum += c;
      }
      (str, sum)
    }),
    Sink {
      counter: HashMap::new()
    }
  ];

  p.start();
  let res = p.wait_and_collect();

  let mut total_words = 0;
  for (_key, value) in res.unwrap() {
    total_words += value;
  }
  println!("[PIPELINE] Total words: {}", total_words);
}

// Version that use a node that combine map and reduce
pub fn ppl_combined_map_reduce(dataset: &str, threads: usize) {
  let file = File::open(dataset).expect("no such file");
  let reader = BufReader::new(file);

  let mut p = pipeline![
    Source { reader },
    MapReduce::build_with_replicas(
      threads / 2,
      |str| -> (String, usize) { (str, 1) },
      |str, count| {
        let mut sum = 0;
        for c in count {
          sum += c;
        }
        (str, sum)
      },
      2
    ),
    Sink {
      counter: HashMap::new()
    }
  ];

  p.start();
  let res = p.wait_and_collect();

  let mut total_words = 0;
  for (_key, value) in res.unwrap() {
    total_words += value;
  }
  println!(
    "[PIPELINE MAP REDUCE COMBINED] Total words: {}",
    total_words
  );
}
// Version that use par_map_reduce instead of the pipeline
pub fn ppl_map(dataset: &str, threads: usize) {
  let file = File::open(dataset).expect("no such file");
  let reader = BufReader::new(file);

  let mut tp = ThreadPool::with_capacity(threads);

  let mut words = Vec::new();

  reader
    .lines()
    .map(|s| s.unwrap())
    .for_each(|s| words.push(s));

  let res = tp.par_map_reduce(
    words // Collect all the lines in a vector
      .iter()
      .flat_map(|s| s.split_whitespace())
      .map(|s| {
        s.to_lowercase()
          .chars()
          .filter(|c| c.is_alphabetic())
          .collect::<String>()
      })
      .collect::<Vec<String>>(),
    |str| -> (String, usize) { (str, 1) },
    |str, count| {
      let mut sum = 0;
      for c in count {
        sum += c;
      }
      (str, sum)
    },
  );

  let mut total_words = 0;
  for (_str, count) in res {
    //println!("{}: {}", str, count);
    total_words += count;
  }

  println!("[MAP] Total words: {}", total_words);
}

// Take a function and calculate its execution time
fn timeit<F>(f: F)
where
  F: FnOnce(),
{
  let start = std::time::Instant::now();
  f();
  let end = std::time::Instant::now();
  let duration = end.duration_since(start);
  println!("Time: {}", duration.as_secs_f64());
}

fn main() {
  env_logger::init();

  let args: Vec<String> = env::args().collect();
  if args.len() < 4 {
    println!();
    panic!(
      "Correct usage: $ ./{:?} <backend> <nthreads> <dataset.txt>",
      args[0]
    );
  }
  let backend = &args[1];
  let threads = args[2].parse::<usize>().unwrap();
  let dataset = &args[3];

  // TODO: add more backends? In that case this can be a benchmark instead than an example
  match backend.as_str() {
    //"sequential" => sequential::sequential(dir_name),
    //"rust-ssp" => rust_ssp::rust_ssp(dir_name, threads),
    //"rayon" => rayon::rayon(dir_name, threads),
    //"std-threads" => std_threads::std_threads(dir_name, threads),
    "ppl" => {
      timeit(|| ppl(dataset, threads));
      timeit(|| ppl_combined_map_reduce(dataset, threads));
      timeit(|| ppl_map(dataset, threads));
    }
    _ => println!("Invalid run_mode, use: ppl "),
  }
}
