// #![allow(unused)]
use std::fmt::Debug;

pub struct Ui {}

impl Ui {
  pub fn update<T>(&mut self, a: String, b: Vec<T>, _f: impl FnOnce(&mut Ui))
  where
    T: PartialEq + Clone + 'static + Debug,
  {
    println!("a:{:?}, b: {:?}", a, b);
  }
}

pub trait Component<Params, Content> {
  fn call(&self, ui: &mut Ui, params: Params, content: Content);
}

impl<F, P1, P2, Content> Component<(P1, P2), Content> for F
where
  P1: PartialEq + Clone + 'static,
  P2: PartialEq + Clone + 'static,
  Content: FnOnce(&mut Ui),
  F: Fn(&mut Ui, P1, P2, Content),
{
  fn call(&self, ui: &mut Ui, params: (P1, P2), content: Content) {
    let (p1, p2) = params;
    self(ui, p1, p2, content)
  }
}

pub fn memoize<
  Params: PartialEq + Clone + 'static,
  Content: FnOnce(&mut Ui),
  Comp: Component<Params, Content>,
>(
  ui: &mut Ui,
  component: Comp,
  params: Params,
  content: Content,
) {
  component.call(ui, params, content);
}

fn comp2(ui: &mut Ui, a: u8, b: u32, f: impl FnOnce(&mut Ui)) {
  println!("a:{}, b: {}", a, b);
  f(ui);
}

fn main() {
  let mut ui = Ui {};

  memoize(&mut ui, comp2, (2, 3), |_| {});

  let args = (String::new(), vec![(1usize, 1.0f64)]);
  memoize(&mut ui, Ui::update, args, |_| {});
}
