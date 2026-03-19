use linfa::Dataset;
use ndarray::array;

fn main() {
  let features = array![[1.0, 2.0], [3.0, 4.0]];
  let targets = array![0, 1];
  let dataset = Dataset::new(features, targets);
  println!("Linfa dataset: {:?}", dataset);
}
