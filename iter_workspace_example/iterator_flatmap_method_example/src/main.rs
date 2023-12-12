// copy from https://bean-li.github.io/Iterator-Adapter/
use std::collections::HashMap;

fn main() {
  let mut major_cities = HashMap::new();

  major_cities.insert("China", vec!["Beijing", "Shanghai", "Nanjing"]);
  major_cities.insert("Japan", vec!["Tokyo", "Kyoto"]);
  major_cities.insert("USA", vec!["Portland", "New York"]);

  let counties = ["China", "Japan", "USA"];

  for &city in counties.iter().flat_map(|country| &major_cities[country]) {
    println!("{}", city);
  }

  let all_countries: Vec<&str> = counties
    .iter()
    .flat_map(|country| major_cities.get(country).unwrap().to_vec())
    .collect();
  println!("all_countries: {:?}", all_countries);

  let all_countries2: Vec<&str> = counties
    .iter()
    .flat_map(|country| major_cities.get(country).unwrap().to_owned())
    .collect();
  // println!("all_countries2: {:?}", all_countries2);
  assert_eq!(all_countries, all_countries2);

  let city_list: Vec<&str> = major_cities.get("China").unwrap().to_vec();
  println!("city list : {:?}", city_list);
}
