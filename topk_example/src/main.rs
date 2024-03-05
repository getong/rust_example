use topk::FilteredSpaceSaving;

fn main() {
  let mut topk = FilteredSpaceSaving::new(3);
  topk.insert("1", 10);
  topk.insert("2", 20);
  topk.insert("3", 1);
  topk.insert("4", 2);
  let topk_result = topk.into_sorted_vec();
  assert_eq!(topk_result.len(), 3);
  assert_eq!(topk_result[0].0, "2");

  let mut fss1 = FilteredSpaceSaving::new(3);
  fss1.insert("1", 10);
  fss1.insert("2", 20);
  fss1.insert("3", 2);
  fss1.insert("4", 1);
  fss1.insert("4", 3);
  fss1.insert("5", 3);
  let mut fss2 = FilteredSpaceSaving::new(3);
  fss2.insert("1", 10);
  fss2.insert("2", 20);
  fss2.insert("3", 20);
  fss2.insert("4", 10);
  fss1.merge(&fss2).unwrap();
  let result = fss1.into_sorted_vec();
  assert_eq!(result[0].0, "2");
}
