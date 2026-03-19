use plotters::{element::PathElement, prelude::*};
use rand::Rng;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut rng = rand::rng();
  let loss_values: Vec<f64> = (0 .. 100)
    .map(|epoch| {
      let base_loss = 1.0 / (epoch as f64 + 1.0);
      base_loss + rng.gen_range(-0.01 .. 0.01)
    })
    .collect();

  let root = BitMapBackend::new("training_loss.png", (800, 600)).into_drawing_area();
  root.fill(&WHITE)?;

  let max_loss = loss_values.iter().cloned().fold(0. / 0., f64::max);

  let mut chart = ChartBuilder::on(&root)
    .caption("Simulated Training Loss", ("sans-serif", 30))
    .margin(20)
    .x_label_area_size(40)
    .y_label_area_size(50)
    .build_cartesian_2d(0 .. 100, 0.0 .. max_loss)?;

  chart
    .configure_mesh()
    .x_desc("Epoch")
    .y_desc("Loss")
    .draw()?;

  chart
    .draw_series(LineSeries::new(
      loss_values.iter().enumerate().map(|(x, y)| (x as i32, *y)),
      &BLUE,
    ))?
    .label("Loss")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));

  chart
    .configure_series_labels()
    .background_style(&WHITE.mix(0.8))
    .border_style(&BLACK)
    .draw()?;

  println!("Loss plot saved to training_loss.png");
  Ok(())
}
