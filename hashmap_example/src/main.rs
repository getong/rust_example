use std::collections::HashMap; // This is so we can just write HashMap instead of std::collections::HashMap every time

struct City {
  name: String,
  population: HashMap<u32, u32>, // This will have the year and the population for the year
}

macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert($key, $value);
            )+
            m
        }
     };
);

fn main() {
  let mut tallinn = City {
    name: "Tallinn".to_string(),
    population: HashMap::new(), // So far the HashMap is empty
  };

  tallinn.population.insert(1372, 3_250); // insert three dates
  tallinn.population.insert(1851, 24_000);
  tallinn.population.insert(2020, 437_619);

  for (year, population) in tallinn.population {
    // The HashMap is HashMap<u32, u32> so it returns a two items each time
    println!(
      "In the year {} the city of {} had a population of {}.",
      year, tallinn.name, population
    );
  }

  let mut book_hashmap = HashMap::new();
  book_hashmap.insert(1, "L'Allemagne Moderne");
  if book_hashmap.get(&1).is_none() {
    // is_none() returns a bool: true if it's None, false if it's Some
    book_hashmap.insert(1, "Le Petit Prince");
  }
  println!("{:?}", book_hashmap.get(&1));

  let book_collection = vec![
    "L'Allemagne Moderne",
    "Le Petit Prince",
    "Eye of the World",
    "Eye of the World",
  ]; // Eye of the World appears twice
  let mut book_hashmap = HashMap::new();
  for book in book_collection {
    book_hashmap.entry(book).or_insert(true);
  }
  for (book, true_or_false) in book_hashmap {
    println!("Do we have {}? {}", book, true_or_false);
  }

  let book_collection = vec![
    "L'Allemagne Moderne",
    "Le Petit Prince",
    "Eye of the World",
    "Eye of the World",
  ];
  let mut book_hashmap = HashMap::new();
  for book in book_collection {
    let return_value = book_hashmap.entry(book).or_insert(0); // return_value is a mutable reference. If nothing is there, it will be 0
    *return_value += 1; // Now return_value is at least 1. And if there was another book, it will go
                        // up by 1
  }
  for (book, number) in book_hashmap {
    println!("{}, {}", book, number);
  }

  let data = vec![
    // This is the raw data
    ("male", 9),
    ("female", 5),
    ("male", 0),
    ("female", 6),
    ("female", 5),
    ("male", 10),
  ];

  let mut survey_hash = HashMap::new();
  for item in data {
    // This gives a tuple of (&str, i32)
    survey_hash.entry(item.0).or_insert(Vec::new()).push(item.1); // This pushes the number into the
                                                                  // Vec inside
  }
  for (male_or_female, numbers) in survey_hash {
    println!("{:?}: {:?}", male_or_female, numbers);
  }

  let names = map! { 1 => "one", 2 => "two" };
  println!("{} -> {:?}", 1, names.get(&1));
  println!("{} -> {:?}", 10, names.get(&10));

  let mut map: HashMap<&str, i32> = HashMap::new();
  map.insert("zhangsan", 97);
  map.insert("lisi", 86);
  map.insert("wangwu", 55);
  println!("{:?}", map);

  for (_, val) in map.iter_mut() {
    *val += 2;
  }

  println!("{:?}", map);

  let result = map.remove("wangwu");
  println!("{:?}", map);
  println!("{:?}", result);

  println!("zhangsan: {}", map["zhangsan"]);

  hashmap_example1();
}

fn hashmap_example1() {
  let mut countries: HashMap<&str, &str> = HashMap::new();
  countries.insert("US", "United States");
  countries.insert("FR", "France");
  countries.insert("UK", "United Kingdom");
  countries.insert("FR", "France");
  countries.insert("FL", "Finland");

  for (key, value) in &countries {
    println!("{} => {}", key, value);
  }

  for key in countries.keys() {
    println!("{}", key);
  }

  for value in countries.values() {
    println!("{}", value);
  }
}
