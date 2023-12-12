pub struct Ui {}

macro_rules! memoize {
    ($ui:expr, $f:path, ($($p:expr),*), $content:expr) => {
        $f($ui, $($p,)* $content);
    };
}

pub fn comp0(ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
  f(ui);
}

pub fn comp1(ui: &mut Ui, a: u8, f: impl FnOnce(&mut Ui)) {
  println!("a: {:?}", a);
  f(ui);
}

pub fn comp_(ui: &mut Ui, a: &str, f: impl FnOnce(&mut Ui)) {
  println!("a: {:?}", a);
  f(ui);
}

pub fn comp2(ui: &mut Ui, a: u8, b: u32, f: impl FnOnce(&mut Ui)) {
  println!("a: {:?}, b: {:?}", a, b);
  f(ui);
}

pub fn comp3(ui: &mut Ui, a: u8, b: u32, c: u64, f: impl FnOnce(&mut Ui)) {
  println!("a: {:?}, b: {:?}, c : {:?}", a, b, c);
  f(ui);
}

pub fn comp4(ui: &mut Ui, a: u8, b: u32, c: u64, d: usize, f: impl FnOnce(&mut Ui)) {
  println!("a: {:?}, b: {:?}, c : {:?}, d: {:?}", a, b, c, d);
  f(ui);
}

// #[allow(clippy::too_many_arguments)]
fn comp12(
  ui: &mut Ui,
  p1: u8,
  p2: u8,
  p3: u8,
  p4: u8,
  p5: u8,
  p6: u8,
  p7: u8,
  p8: u8,
  p9: u8,
  p10: u8,
  p11: u8,
  p12: u8,
  f: impl FnOnce(&mut Ui),
) {
  println!(
        "p1: {:?}, p2: {:?}, p3 : {:?}, p4: {:?}, p5: {:?}, p6: {:?}, p7 : {:?}, p8: {:?}, p9: {:?}, p10: {:?}, p11 : {:?}, p12: {:?}",
        p1, p2, p3, p4, p5, p6, p7, p8, p9, p10, p11, p12
    );
  f(ui);
}

fn main() {
  let mut ui = Ui {};
  memoize!(&mut ui, comp0, (), |_| {});
  memoize!(&mut ui, comp_, (""), |_| {});
  memoize!(&mut ui, comp4, (0, 1, 2, 3), |_| {});
  memoize!(
    &mut ui,
    comp12,
    (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0),
    |_| {}
  );
}
