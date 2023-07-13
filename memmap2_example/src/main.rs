use std::fs::File;
use std::io::Read;

use memmap2::Mmap;

fn main() -> Result<(), Box<dyn std::error::Error>>{
    // println!("Hello, world!");

    let mut file = File::open("Cargo.toml")?;

    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;

    let mmap = unsafe { Mmap::map(&file)?  };

    assert_eq!(&contents[..], &mmap[..]);
    Ok(())
}
