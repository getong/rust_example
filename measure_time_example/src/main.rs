use measure_time::{debug_time, error_time, info_time, print_time, trace_time};
fn main() {
  info_time!("measure function");
  {
    debug_time!("{:?}", "measuring block");
    let mut sum = 0;
    for el in 0 .. 50000 {
      sum += el;
    }
    println!("{:?}", sum);
  }
  trace_time!("{:?}", "trace");
  print_time!("print");
  error_time!(target: "measure_time", "custom target");
}
