use alcs::Alcs;

fn main() {
  let a = "word";
  let b = "hello world";
  let va = a.chars().collect::<Vec<char>>();
  let vb = b.chars().collect::<Vec<char>>();
  let alcs = Alcs::new(&va, &vb);
  for i in 0..b.len() {
    for (i, j, cij) in alcs.suffix(i) {
      println!(
        r#"LCS between "{}" and "{}" has length {}"#,
        a,
        &b[i..j],
        cij
      );
    }
  }
}
