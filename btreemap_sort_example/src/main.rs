use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::iter::FromIterator;

fn fill_tree() -> BTreeMap<String, i32> {
    let file = match File::open("values.txt") {
        Err(e) => panic!("Unable to open file: {}", e),
        Ok(file) => file,
    };
    let mut movie_entries: BTreeMap<String, i32> = BTreeMap::new();
    let lines = io::BufReader::new(file).lines();
    for line in lines {
        let line = line.unwrap();
        let mut split_line = line.as_str().split('\t');
        let left = split_line.next().unwrap();
        let right = split_line.next().unwrap();
        let year = right.parse::<i32>().unwrap();
        movie_entries.insert(String::from(left), year);
    }
    movie_entries
}

fn main() {
    let movies: BTreeMap<String, i32> = fill_tree();
    println!("We have {} movies", movies.len());
    match movies.get("Captain America") {
        Some(year) => println!("{}", year),
        None => println!("Unable to find that movie"),
    }
    for (movie, year) in &movies {
        println!("{}: {}", movie, year);
    }
    let mut movie_vec = Vec::from_iter(movies);
    movie_vec.sort_by(|&(_, a), &(_, b)| a.cmp(&b));
    println!("{:?}", movie_vec);
}
