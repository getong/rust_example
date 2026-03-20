use plotters::prelude::*;
use rand::RngExt;
use rand_distr::{Distribution, StandardNormal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let root = BitMapBackend::new("gan_scatter.png", (800, 600)).into_drawing_area();
  root.fill(&WHITE)?;

  let mut chart = ChartBuilder::on(&root)
    .margin(20)
    .caption("GAN samples vs target mixture", ("sans-serif", 24))
    .x_label_area_size(35)
    .y_label_area_size(35)
    .build_cartesian_2d(-4.0f32 .. 4.0f32, -3.0f32 .. 3.0f32)?;

  chart.configure_mesh().draw()?;

  // Generated points (your GAN outputs)
  let samples: Vec<(f32, f32)> = vec![
    (-0.416, 0.173),
    (-0.323, 0.237),
    (1.351, -0.047),
    (-0.558, 0.086),
    (0.371, 0.288),
    (-0.972, 0.005),
    (0.307, 0.357),
    (-1.749, 0.895),
    (-2.599, -0.228),
    (2.095, 0.641),
  ];

  // Reference samples from the target mixture
  let centers = [(-2.0f32, 0.0f32), (2.0f32, 0.0f32)];
  let sigma = 0.5f32;
  let mut rng = rand::rng();
  let normal = StandardNormal;

  let mut ref_pts: Vec<(f32, f32)> = Vec::with_capacity(1500);
  for _ in 0 .. 1500 {
    let c = if rng.random_bool(0.5) {
      centers[0]
    } else {
      centers[1]
    };

    // --- Fix: sample as f64, then cast to f32 ---
    let dx64: f64 = normal.sample(&mut rng);
    let dy64: f64 = normal.sample(&mut rng);
    let dx: f32 = (dx64 as f32) * sigma;
    let dy: f32 = (dy64 as f32) * sigma;

    ref_pts.push((c.0 + dx, c.1 + dy));
  }

  // Target mixture (light points)
  chart.draw_series(
    ref_pts
      .iter()
      .map(|&(x, y)| Circle::new((x, y), 2, RGBColor(180, 180, 255).mix(0.4).filled())),
  )?;

  // Generated samples (emphasized markers)
  chart
    .draw_series(samples.iter().map(|&(x, y)| Cross::new((x, y), 6, &RED)))?
    .label("Generated")
    .legend(|(x, y)| Cross::new((x, y), 6, &RED));

  // Mark the two Gaussian centers
  chart.draw_series(std::iter::once(Circle::new(centers[0], 4, BLUE.filled())))?;
  chart.draw_series(std::iter::once(Circle::new(centers[1], 4, BLUE.filled())))?;

  chart
    .configure_series_labels()
    .border_style(&BLACK)
    .draw()?;
  println!("Saved plot to gan_scatter.png");
  Ok(())
}
