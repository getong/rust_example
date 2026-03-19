use ndarray::prelude::*;
use rand::rng;
use rand_distr::{Distribution, Normal};

fn relu(x: f64) -> f64 {
  x.max(0.0)
}

// Applies activation elementwise
fn relu_layer(x: Array1<f64>) -> Array1<f64> {
  x.mapv(relu)
}

fn forward_pass(
  x: Array1<f64>,
  w1: Array2<f64>,
  b1: Array1<f64>,
  w2: Array2<f64>,
  b2: Array1<f64>,
) -> f64 {
  let hidden = relu_layer(w1.dot(&x) + &b1); // Hidden layer
  println!("the sorted hidden list is {:?}", hidden);
  let output = w2.dot(&hidden) + &b2; // Output layer (no activation)
  println!("the sorted output list is {:?}", output);
  output[0]
}

fn main() {
  let input = array![0.3, 0.8, 0.5];

  let mut rng = rng();
  let normal = Normal::new(0.0, 1.0).unwrap();

  // Layer 1: 4 hidden neurons, 3 inputs
  let w1 = Array::from_shape_fn((4, 3), |_| normal.sample(&mut rng));
  let b1 = Array::from_shape_fn(4, |_| normal.sample(&mut rng));

  // Output layer: 1 neuron, 4 hidden units
  let w2 = Array::from_shape_fn((1, 4), |_| normal.sample(&mut rng));
  let b2 = Array::from_shape_fn(1, |_| normal.sample(&mut rng));

  let result = forward_pass(input, w1, b1, w2, b2);
  println!("Predicted output: {}", result);
}
