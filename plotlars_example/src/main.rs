use plotlars::{Plot, ScatterPlot};
use polars::prelude::*;

fn main() {
  let dataset = LazyCsvReader::new("penguins.csv")
    .finish()
    .unwrap()
    .select([
      col("species").cast(DataType::Categorical(None, CategoricalOrdering::default())),
      col("flipper_length_mm").cast(DataType::Int16),
      col("body_mass_g").cast(DataType::Int16),
    ])
    .collect()
    .unwrap();

  ScatterPlot::builder()
    .data(&dataset)
    .x("body_mass_g")
    .y("flipper_length_mm")
    .group("species")
    .size(10)
    .opacity(0.5)
    .plot_title("Penguin Flipper Length vs Body Mass")
    .x_title("Body Mass (g)")
    .y_title("Flipper Length (mm)")
    .legend_title("Species")
    .build()
    .plot();
}
