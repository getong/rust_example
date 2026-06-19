use smartcore::{linalg::basic::matrix::DenseMatrix, neighbors::knn_classifier::KNNClassifier};

fn main() {
  // Turn vector slices into a matrix
  let x =
    DenseMatrix::from_2d_array(&[&[1., 2.], &[3., 4.], &[5., 6.], &[7., 8.], &[9., 10.]]).unwrap();

  // Class labels
  let y = vec![2, 2, 2, 3, 3];

  // Train classifier
  let knn = KNNClassifier::fit(&x, &y, Default::default()).unwrap();

  // Predict
  let test_x = DenseMatrix::from_2d_array(&[&[2., 3.], &[8., 9.]]).unwrap();
  let yhat = knn.predict(&test_x).unwrap();
  println!("{yhat:?}");
}
