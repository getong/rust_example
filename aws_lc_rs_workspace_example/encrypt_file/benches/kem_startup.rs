use std::{hint::black_box, process::Command, time::Instant};

use aws_lc_rs::kem::{DecapsulationKey, ML_KEM_1024};
fn main() -> Result<(), Box<dyn std::error::Error>> {
  if std::env::args().any(|arg| arg == "--child") {
    let first = Instant::now();
    let key = DecapsulationKey::generate(&ML_KEM_1024)?;
    let cold = first.elapsed().as_secs_f64() * 1000.0;
    black_box(&key);
    let later = Instant::now();
    for _ in 0 .. 100 {
      black_box(DecapsulationKey::generate(&ML_KEM_1024)?);
    }
    println!(
      "{cold:.6} {:.6}",
      later.elapsed().as_secs_f64() * 1000.0 / 100.0
    );
    return Ok(());
  }
  if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
    println!(
      "cargo bench --bench kem_startup: 3 fresh processes, first keygen vs mean of next 100"
    );
    return Ok(());
  }
  let mut cold = Vec::new();
  let mut warm = Vec::new();
  for _ in 0 .. 3 {
    let output = Command::new(std::env::current_exe()?)
      .arg("--child")
      .output()?;
    if !output.status.success() {
      return Err(String::from_utf8_lossy(&output.stderr).to_string().into());
    }
    let values = String::from_utf8(output.stdout)?
      .split_whitespace()
      .map(str::parse::<f64>)
      .collect::<Result<Vec<_>, _>>()?;
    if values.len() != 2 {
      return Err("unexpected child measurements".into());
    }
    cold.push(values[0]);
    warm.push(values[1]);
  }
  cold.sort_by(f64::total_cmp);
  warm.sort_by(f64::total_cmp);
  println!(
    "ML-KEM-1024 keygen: 3 fresh processes; median of first calls and median of per-process warm \
     means"
  );
  println!(
    "first call: {:.3} ms; subsequent call: {:.3} ms",
    cold[1], warm[1]
  );
  println!(
    "Includes process-local first-use costs; moving initialization earlier does not remove it \
     from CLI latency."
  );
  Ok(())
}
