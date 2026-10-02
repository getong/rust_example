use std::{
  collections::BTreeMap,
  env, fs,
  io::Write,
  path::{Path, PathBuf},
  process::{Command, Stdio},
  time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const PASSWORD: &[u8] = b"benchmark-only-password\n";
const ENCRYPT: &str = env!("CARGO_BIN_EXE_encrypt");
const DECRYPT: &str = env!("CARGO_BIN_EXE_decrypt");

struct TempDir(PathBuf);

impl TempDir {
  fn new() -> Result<Self> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = env::temp_dir().join(format!("encrypt-bench-{}-{stamp}", std::process::id()));
    fs::create_dir(&path)?;
    Ok(Self(path))
  }
}

impl Drop for TempDir {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

struct Sample {
  elapsed: Duration,
  stages: BTreeMap<String, f64>,
}

// Time the real process, including startup, sync_all, and buffer destruction.
// Passwords are piped so measurements never include human input delays.
fn run(binary: &str, args: &[&Path], input: &[u8]) -> Result<Sample> {
  let start = Instant::now();
  let mut child = Command::new(binary)
    .args(args)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
  child
    .stdin
    .take()
    .ok_or("stdin pipe unavailable")?
    .write_all(input)?;
  let output = child.wait_with_output()?;
  let elapsed = start.elapsed();
  if !output.status.success() {
    return Err(format!("CLI failed: {}", String::from_utf8_lossy(&output.stderr)).into());
  }
  let mut stages = BTreeMap::new();
  // A password prompt may precede BENCH on the same line.
  for line in String::from_utf8_lossy(&output.stderr).lines() {
    if let Some((_, record)) = line.split_once("BENCH\t") {
      let (name, ns) = record.split_once('\t').ok_or("invalid timing record")?;
      *stages.entry(name.to_owned()).or_default() += ns.parse::<f64>()? / 1_000_000.0;
    }
  }
  #[cfg(feature = "perf-trace")]
  if stages.is_empty() {
    return Err("perf-trace enabled but CLI timing records are missing".into());
  }
  Ok(Sample { elapsed, stages })
}

fn median(values: &mut [f64]) -> f64 {
  values.sort_by(f64::total_cmp);
  let middle = values.len() / 2;
  if values.len().is_multiple_of(2) {
    (values[middle - 1] + values[middle]) / 2.0
  } else {
    values[middle]
  }
}

fn measure(
  name: &str,
  filter: &str,
  samples: usize,
  mut operation: impl FnMut() -> Result<Sample>,
) -> Result<usize> {
  if !name.contains(filter) {
    return Ok(0);
  }
  println!("\n{name}: warming up...");
  operation()?;
  let measurements: Vec<_> = (0 .. samples).map(|_| operation()).collect::<Result<_>>()?;
  let mut times: Vec<_> = measurements
    .iter()
    .map(|s| s.elapsed.as_secs_f64() * 1000.0)
    .collect();
  let mid = median(&mut times);
  println!(
    "{name}: median {mid:.3} ms | min {:.3} ms | max {:.3} ms | n={samples}",
    times[0],
    times[times.len() - 1]
  );
  for label in measurements[0].stages.keys() {
    let mut values = measurements
      .iter()
      .map(|s| s.stages[label])
      .collect::<Vec<_>>();
    println!("  {label:<28} {:>12.3} ms", median(&mut values));
  }
  Ok(1)
}

fn main() -> Result<()> {
  let mut samples = 3;
  let mut filter = String::new();
  let mut args = env::args().skip(1);
  while let Some(arg) = args.next() {
    match arg.as_str() {
      "--bench" => {} // Cargo supplies this to custom harnesses.
      "--samples" => {
        samples = args
          .next()
          .ok_or("--samples requires a positive integer")?
          .parse()?;
        if samples == 0 {
          return Err("--samples must be positive".into());
        }
      }
      "--help" | "-h" => {
        println!("cargo bench --bench cli -- [FILTER] [--samples N]");
        println!("Filters: protected, plain, keygen, encrypt, decrypt, 4B, 1MiB, 64MiB");
        println!("Add --features perf-trace before -- for per-stage timing.");
        return Ok(());
      }
      _ if !arg.starts_with('-') && filter.is_empty() => filter = arg,
      _ => return Err(format!("unknown argument: {arg}").into()),
    }
  }

  println!("CLI benchmark: 1 warmup + {samples} samples per case, sequential processes.");
  println!(
    "Times include process startup, disk sync, and zeroization; exclude fixture setup and \
     verification."
  );
  println!("Synthetic files only; passwords piped; no cache flush or CPU/RSS sampling.");
  #[cfg(feature = "perf-trace")]
  println!(
    "Stage times are inclusive; repeated stages are summed per process. Do not add parent and \
     child stages."
  );
  #[cfg(not(feature = "perf-trace"))]
  println!("For stage details: cargo bench --features perf-trace");

  let dir = TempDir::new()?;
  let mut measured = 0;
  for (mode, password) in [("plain", &b""[..]), ("protected", PASSWORD)] {
    let public = dir.0.join(format!("{mode}.public"));
    let private = dir.0.join(format!("{mode}.private"));
    let keygen_input = password.repeat(2);
    let mut keygen_args = vec![Path::new("--keygen")];
    if password.is_empty() {
      keygen_args.push(Path::new("--no-password"));
    }
    keygen_args.extend([public.as_path(), private.as_path()]);
    let keygen_name = format!("{mode}/keygen");
    measured += measure(&keygen_name, &filter, samples, || {
      let sample = run(DECRYPT, &keygen_args, &keygen_input)?;
      // Validate and remove fixtures outside the measured interval.
      let overhead = if password.is_empty() { 0 } else { 67 + 16 };
      if fs::metadata(&public)?.len() != (encrypt_file::PUBLIC_KEY_LEN + overhead) as u64
        || fs::metadata(&private)?.len() != (encrypt_file::PRIVATE_KEY_LEN + overhead) as u64
      {
        return Err("generated key size mismatch".into());
      }
      fs::remove_file(&public)?;
      fs::remove_file(&private)?;
      Ok(sample)
    })?;

    let cases: Vec<_> = [
      ("4B", 4),
      ("1MiB", 1024 * 1024),
      ("64MiB", 64 * 1024 * 1024),
    ]
    .into_iter()
    .filter(|(label, _)| {
      format!("{mode}/encrypt/{label}").contains(&filter)
        || format!("{mode}/decrypt/{label}").contains(&filter)
    })
    .collect();
    if cases.is_empty() {
      continue;
    }
    run(DECRYPT, &keygen_args, &keygen_input)?;
    let sender = dir.0.join(format!("{mode}.sender"));
    let signer = dir.0.join(format!("{mode}.signer"));
    let mut signgen_args = vec![Path::new("--sign-keygen")];
    if password.is_empty() {
      signgen_args.push(Path::new("--no-password"));
    }
    signgen_args.extend([sender.as_path(), signer.as_path()]);
    run(DECRYPT, &signgen_args, &keygen_input)?;
    let encryption_passwords = password.repeat(2);
    for (label, size) in cases {
      let content: Vec<_> = (0 .. size).map(|n| n as u8).collect();
      let source = dir.0.join("input");
      let encrypted = dir.0.join("encrypted");
      let recovered = dir.0.join("recovered");
      fs::write(&source, &content)?;
      measured += measure(&format!("{mode}/encrypt/{label}"), &filter, samples, || {
        let sample = run(
          ENCRYPT,
          &[
            Path::new("--sign-key"),
            &signer,
            &public,
            &source,
            &encrypted,
          ],
          &encryption_passwords,
        )?;
        run(
          DECRYPT,
          &[
            Path::new("--verify-key"),
            &sender,
            &private,
            &encrypted,
            &recovered,
          ],
          password,
        )?;
        if fs::read(&recovered)? != content {
          return Err("encryption round-trip mismatch".into());
        }
        fs::remove_file(&encrypted)?;
        fs::remove_file(&recovered)?;
        Ok(sample)
      })?;
      if format!("{mode}/decrypt/{label}").contains(&filter) {
        run(
          ENCRYPT,
          &[
            Path::new("--sign-key"),
            &signer,
            &public,
            &source,
            &encrypted,
          ],
          &encryption_passwords,
        )?;
        measured += measure(&format!("{mode}/decrypt/{label}"), &filter, samples, || {
          let sample = run(
            DECRYPT,
            &[
              Path::new("--verify-key"),
              &sender,
              &private,
              &encrypted,
              &recovered,
            ],
            password,
          )?;
          if fs::read(&recovered)? != content {
            return Err("decryption round-trip mismatch".into());
          }
          fs::remove_file(&recovered)?;
          Ok(sample)
        })?;
        fs::remove_file(&encrypted)?;
      }
    }
  }
  if measured == 0 {
    return Err(format!("no benchmarks match filter {filter:?}").into());
  }
  println!("\n{measured} benchmark cases completed; all output checks passed.");
  Ok(())
}
