use std::fs;

// mkdir -p /tmp/a/b/c

fn main() -> std::io::Result<()> {
  fs::rename("/tmp/a/b/c", "/tmp/a/c")?;
  Ok(())
}

// copy from https://thats-it-code.com/rust/rust__operating-files-and-folders/
