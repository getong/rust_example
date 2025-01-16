use std::{cell::LazyCell, collections::HashMap, sync::OnceLock};

const MEM: LazyCell<HashMap<usize, usize>> = LazyCell::new(|| {
  let mut m = HashMap::new();
  m.insert(0, 0);
  m.insert(1, 1);
  m.insert(2, 2);
  m.insert(3, 6);
  m.insert(4, 24);
  m
});

static MEM2: OnceLock<Vec<usize>> = OnceLock::new();

fn factorial(i: usize) -> usize {
  MEM[&i]
}

#[tokio::main]
async fn main() {
  println!("{}", factorial(0));
  println!("{}", factorial(1));
  println!("{}", factorial(2));
  println!("{}", factorial(3));
  println!("{}", factorial(4));

  let handler = tokio::spawn(async {
    if let Some(mem2_ref) = MEM2.get() {
      println!(
        "before initialize, mem2 contains 2: {:?}",
        mem2_ref.contains(&2)
      );
    } else {
      println!("before initialize, mem2 does not contain 2");
    }
  });
  _ = handler.await;

  mem2();

  // Access the Vec inside the OnceLock and check for the element
  if let Some(mem2_ref) = MEM2.get() {
    println!(
      "after initialized, mem2 contains 2: {:?}",
      mem2_ref.contains(&2)
    );
  }

  let handler = tokio::spawn(async {
    if let Some(mem2_ref) = MEM2.get() {
      println!(
        "in the future object, after initialized, mem2 contains 2: {:?}",
        mem2_ref.contains(&2)
      );
    }
  });
  _ = handler.await;
  _ = tokio::time::sleep(std::time::Duration::from_millis(1000));
}

fn mem2() -> &'static Vec<usize> {
  MEM2.get_or_init(|| {
    let mut m = vec![];
    m.push(0);
    m.push(1);
    m.push(2);
    m.push(3);
    m.push(4);
    m.push(5);
    m
  })
}
