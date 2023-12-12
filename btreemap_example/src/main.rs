use std::collections::BTreeMap; // Just change HashMap to BTreeMap

struct City {
  name: String,
  population: BTreeMap<u32, u32>, // Just change HashMap to BTreeMap
}

fn main() {
  let mut tallinn = City {
    name: "Tallinn".to_string(),
    population: BTreeMap::new(), // Just change HashMap to BTreeMap
  };

  tallinn.population.insert(1372, 3_250);
  tallinn.population.insert(1851, 24_000);
  tallinn.population.insert(2020, 437_619);

  for (year, population) in tallinn.population {
    println!(
      "In the year {} the city of {} had a population of {}.",
      year, tallinn.name, population
    );
  }

  entry_example();
}

fn entry_example() {
  let mut count: BTreeMap<&str, usize> = BTreeMap::new();

  // count the number of occurrences of letters in the vec
  for x in ["a", "b", "a", "c", "a", "b"] {
    count.entry(x).and_modify(|curr| *curr += 1).or_insert(1);
  }

  assert_eq!(count["a"], 3);
  assert_eq!(count["b"], 2);
  assert_eq!(count["c"], 1);
}
