use ndarray::{Array2, s};

/// Perform valid 2D convolution: output size is (H - k_h + 1, W - k_w + 1)
fn convolve2d(input: &Array2<f64>, kernel: &Array2<f64>) -> Array2<f64> {
  let (h, w) = input.dim();
  let (k_h, k_w) = kernel.dim();

  // Output dimensions based on valid convolution
  let out_h = h - k_h + 1;
  let out_w = w - k_w + 1;

  let mut output = Array2::<f64>::zeros((out_h, out_w));

  for i in 0 .. out_h {
    for j in 0 .. out_w {
      // Extract submatrix of shape (k_h, k_w) from input
      let window = input.slice(s![i .. i + k_h, j .. j + k_w]);
      let sum = (&window * kernel).sum();
      output[(i, j)] = sum;
    }
  }

  output
}

use ndarray::array;

fn main() {
  let input = array![
    [1.0, 2.0, 3.0, 4.0],
    [5.0, 6.0, 7.0, 8.0],
    [9.0, 10.0, 11.0, 12.0],
    [13.0, 14.0, 15.0, 16.0],
  ];

  let kernel = array![[1.0, 0.0], [0.0, -1.0],];

  let output = convolve2d(&input, &kernel);
  println!("Output:\n{:#?}", output);
}
