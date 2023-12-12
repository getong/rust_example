use std::collections::HashMap;

type Table = HashMap<String, Vec<String>>;

fn show(table: &Table) {
  for (artist, works) in table {
    println!("works by {}: ", artist);
    for work in works {
      println!("     {}", work);
    }
  }
}

fn sort_works(table: &mut Table) {
  for (_artist, works) in table {
    works.sort();
  }
}

fn main() {
  // println!("Hello, world!");
  let mut table = Table::new();

  table.insert(
    "Gesualdo".to_string(),
    vec![
      "many madrigals".to_string(),
      "Tenebrae Responsoria".to_string(),
    ],
  );

  table.insert(
    "Caravaggio".to_string(),
    vec![
      "The Musicians".to_string(),
      "The Calling of St. Matthew".to_string(),
    ],
  );

  table.insert(
    "Cellini".to_string(),
    vec![
      "Perseus with the head of Medusa".to_string(),
      "a salt cellar".to_string(),
    ],
  );

  println!("before sorting\n");
  show(&table);

  sort_works(&mut table);
  println!("after sorting\n");

  show(&table);

  assert_eq!(table["Gesualdo"][1], "many madrigals");
}
