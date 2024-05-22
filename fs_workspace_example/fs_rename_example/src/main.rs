use std::fs;

// mkdir -p /tmp/a/b/
// touch /tmp/a/b/c.txt
fn main() -> std::io::Result<()> {
  fs::rename("/tmp/a/b", "/tmp/b")?; // Rename a.txt to b.txt
  fs::remove_dir("/tmp/a")?;
  Ok(())
}
