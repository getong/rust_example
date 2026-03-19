use plotters::prelude::*;
use rand::RngExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut rng = rand::rng();

  // Generate 25 red and 25 blue points
  let red_points: Vec<(f64, f64)> = (0 .. 25)
    .map(|_| (rng.random_range(0.0 .. 5.0), rng.random_range(0.0 .. 5.0)))
    .collect();

  let blue_points: Vec<(f64, f64)> = (0 .. 25)
    .map(|_| (rng.random_range(5.0 .. 10.0), rng.random_range(5.0 .. 10.0)))
    .collect();

  // Setup drawing area
  let root = BitMapBackend::new("scatter_classified.png", (640, 480)).into_drawing_area();
  root.fill(&WHITE)?;

  let mut chart = ChartBuilder::on(&root)
    .caption("Scatter Plot by Category", ("sans-serif", 30))
    .margin(20)
    .x_label_area_size(40)
    .y_label_area_size(40)
    .build_cartesian_2d(0.0 .. 10.0, 0.0 .. 10.0)?;

  chart.configure_mesh().x_desc("X").y_desc("Y").draw()?;

  // Draw red points
  chart
    .draw_series(
      red_points
        .iter()
        .map(|(x, y)| Circle::new((*x, *y), 4, RED.filled())),
    )?
    .label("Red Class")
    .legend(|(x, y)| Circle::new((x, y), 4, RED.filled()));

  // Draw blue points
  chart
    .draw_series(
      blue_points
        .iter()
        .map(|(x, y)| Circle::new((*x, *y), 4, BLUE.filled())),
    )?
    .label("Blue Class")
    .legend(|(x, y)| Circle::new((x, y), 4, BLUE.filled()));

  // Draw legend
  chart
    .configure_series_labels()
    .border_style(&BLACK)
    .background_style(&WHITE.mix(0.8))
    .draw()?;

  println!("Scatter plot saved to scatter_classified.png");
  Ok(())
}
