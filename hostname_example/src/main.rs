use std::io;

fn main() -> io::Result<()> {
  // Retrieve the hostname
  dbg!(hostname::get()?);

  // And set a new one
  hostname::set("potato")?;

  Ok(())
}
