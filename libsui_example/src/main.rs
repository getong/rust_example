use libsui::{Macho, PortableExecutable};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let exe = std::fs::read("tests/exec_mach64")?;

  let mut out = std::fs::File::create("out")?;

  Macho::from(exe)?
    .write_section("__hello", b"Hello, World!".to_vec())?
    .build(&mut out)?;

  let exe = std::fs::read("tests/exec_pe64")?;

  let mut out = std::fs::File::create("out.exe")?;

  PortableExecutable::from(&exe)?
    .write_resource("hello.txt", b"Hello, World!".to_vec())?
        .build(&mut out)?;

  Ok(())
}
