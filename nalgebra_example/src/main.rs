use nalgebra::{Matrix3, Point3, Rotation3, Translation3, Vector3};

fn main() {
  vector_demo();
  matrix_demo();
  transform_demo();
  solve_linear_system_demo();
}

fn vector_demo() {
  let a = Vector3::new(1.0, 2.0, 3.0);
  let b = Vector3::new(4.0, 5.0, 6.0);

  println!("== Vector ==");
  println!("a = {a}");
  println!("b = {b}");
  println!("a + b = {}", a + b);
  println!("dot(a, b) = {}", a.dot(&b));
  println!("cross(a, b) = {}", a.cross(&b));
  println!("|a| = {:.3}", a.norm());
  println!();
}

fn matrix_demo() {
  let m = Matrix3::new(
    1.0, 2.0, 3.0, //
    0.0, 1.0, 4.0, //
    5.0, 6.0, 0.0,
  );
  let v = Vector3::new(1.0, 2.0, 3.0);

  println!("== Matrix ==");
  println!("m = {m}");
  println!("v = {v}");
  println!("m * v = {}", m * v);
  println!("det(m) = {:.3}", m.determinant());

  match m.try_inverse() {
    Some(inverse) => println!("inverse(m) = {inverse}"),
    None => println!("m is not invertible"),
  }

  println!();
}

fn transform_demo() {
  let point = Point3::new(1.0, 0.0, 0.0);
  let rotation = Rotation3::from_axis_angle(&Vector3::z_axis(), std::f64::consts::FRAC_PI_2);
  let translation = Translation3::new(0.0, 2.0, 0.0);

  let rotated = rotation * point;
  let transformed = translation * rotated;

  println!("== Transform ==");
  println!("point = {point}");
  println!("rotate 90 degrees around z = {rotated}");
  println!("then translate by (0, 2, 0) = {transformed}");
  println!();
}

fn solve_linear_system_demo() {
  // Solve:
  // 2x + y - z = 8
  // -3x - y + 2z = -11
  // -2x + y + 2z = -3
  let coefficients = Matrix3::new(
    2.0, 1.0, -1.0, //
    -3.0, -1.0, 2.0, //
    -2.0, 1.0, 2.0,
  );
  let constants = Vector3::new(8.0, -11.0, -3.0);

  println!("== Linear system ==");
  match coefficients.lu().solve(&constants) {
    Some(solution) => {
      println!("solution = {solution}");
      println!("check A * x = {}", coefficients * solution);
    }
    None => println!("the system has no unique solution"),
  }
}
