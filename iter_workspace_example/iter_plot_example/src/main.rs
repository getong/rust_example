mod feigenbaum;

fn plot_feigenbaum_diag(a: f64, b: f64, dark_iters: usize, n_r: usize) -> (Vec<f64>, Vec<f64>) {
  let delta = (b - a) / n_r as f64;
  feigenbaum::plot(a, b, dark_iters, 1000, delta, 1)
}

fn main() {
  // println!("Hello, world!");
  let (a, b) = plot_feigenbaum_diag(1.0, 2.0, 3, 4);
  println!("a: {:#?}, b: {:#?}", a, b);
}
