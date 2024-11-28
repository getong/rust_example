use std::time::Duration;

use spin_sleep::LoopHelper;

fn main() {
  let mut loop_helper = LoopHelper::builder()
    .report_interval_s(0.5) // report every half a second
    .build_with_target_rate(250.0); // limit to 250 FPS if possible

  let mut current_fps = None;

  loop {
    let delta = loop_helper.loop_start(); // or .loop_start_s() for f64 seconds

    compute_something(delta);

    if let Some(fps) = loop_helper.report_rate() {
      current_fps = Some(fps);
    }

    render_fps(current_fps);

    loop_helper.loop_sleep(); // sleeps to achieve a 250 FPS rate
  }
}

fn compute_something(delta: Duration) {
  println!("delta is {:?}", delta);
}

fn render_fps(current_fps: Option<f64>) {
  println!("current_fps is {:?}", current_fps);
}
