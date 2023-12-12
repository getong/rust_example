mod my_macro;

fn main() {
  println!("{}", square!(2));
  let fx;
  let fy;
  let fz;
  let tx;
  let ty;
  let tz;

  make_forces_torques_zero!((fx, fy, fz, tx, ty, tz));

  assert_eq!(fx, 0.0);
  assert_eq!(fy, 0.0);
  assert_eq!(fz, 0.0);
  assert_eq!(tx, 0.0);
  assert_eq!(ty, 0.0);
  assert_eq!(tz, 0.0);
}
