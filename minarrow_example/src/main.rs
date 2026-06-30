use minarrow::{MaskedArray, Print, arr_f64, arr_i32, fa_i32, fa_str32, tbl};

fn main() {
  // Create arrays with macros
  let ids = arr_i32![1, 2, 3, 4];
  let prices = arr_f64![10.5, 20.0, 15.75];
  // let names = arr_str32!["alice", "bob", "charlie"];
  // let flags = arr_bool![true, false, true];

  // Direct typed access - no downcasting
  assert_eq!(ids.len(), 4);
  assert_eq!(prices.num().f64().get(0), Some(10.5));

  // Build tables via FieldArrays with constructor macros
  let table = tbl!(
    "users",
    fa_i32!("id", 1, 2, 3),
    fa_str32!("name", "alice", "bob", "charlie"),
  );
  table.print();
}
