use std::{
  env,
  fs::{self, File, OpenOptions},
  io::{self, Write},
  path::Path,
  process,
};

use memmap2::{Mmap, MmapMut};

const LARGE_DATA_CONTENT: &[u8] =
  b"0123456789abcdefghijklmnopqrstuvwxyz\nThis file is read through memmap2.\n";
const OUTPUT_CONTENT: &[u8] = b"xxxx will be replaced by Rust through MmapMut.\n";

fn main() -> io::Result<()> {
  println!("memmap2 的作用：把文件的一段内容映射成内存里的字节切片。");
  println!(
    "这样可以像访问 &[u8] 或 &mut [u8] 一样访问文件内容，适合大文件、随机访问和零拷贝风格读取。"
  );

  let large_data_path = env::temp_dir().join(format!("large_data-{}.bin", process::id()));
  let output_path = env::temp_dir().join(format!("output-{}.bin", process::id()));

  create_demo_file(&large_data_path, LARGE_DATA_CONTENT)?;
  create_demo_file(&output_path, OUTPUT_CONTENT)?;

  print_first_ten_bytes_with_mmap(&large_data_path)?;
  write_rust_prefix_with_mmap_mut(&output_path)?;

  let updated = fs::read_to_string(&output_path)?;
  println!("\noutput.bin 修改后的内容：\n{updated}");

  fs::remove_file(large_data_path)?;
  fs::remove_file(output_path)?;

  Ok(())
}

fn create_demo_file(path: &Path, content: &[u8]) -> io::Result<()> {
  let mut file = OpenOptions::new()
    .read(true)
    .write(true)
    .create(true)
    .truncate(true)
    .open(path)?;

  file.write_all(content)?;
  file.flush()
}

fn print_first_ten_bytes_with_mmap(path: &Path) -> io::Result<()> {
  let file = File::open(path)?;

  // SAFETY: The file has been fully initialized before mapping, and this
  // function does not mutate the file while the read-only mapping is alive.
  let mmap = unsafe { Mmap::map(&file)? };

  let preview_len = mmap.len().min(10);
  println!("\n只读映射 Mmap：");
  println!("- 文件路径：{}", path.display());
  println!("- 映射长度：{} bytes", mmap.len());
  println!("- 文件前 {preview_len} 个字节：{:?}", &mmap[.. preview_len]);

  Ok(())
}

fn write_rust_prefix_with_mmap_mut(path: &Path) -> io::Result<()> {
  let file = OpenOptions::new().read(true).write(true).open(path)?;
  let replacement = b"Rust";

  if file.metadata()?.len() < replacement.len() as u64 {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "file is too small to write the Rust prefix",
    ));
  }

  // SAFETY: The file is opened for reading and writing, its mapped range is
  // initialized, and this function does not access the same bytes through
  // another mapping while the mutable mapping is alive.
  let mut mmap = unsafe { MmapMut::map_mut(&file)? };

  mmap[.. replacement.len()].copy_from_slice(replacement);
  mmap.flush()?;

  println!("\n可写映射 MmapMut：");
  println!("- 文件路径：{}", path.display());
  println!("- 已通过 mmap[0..4].copy_from_slice(b\"Rust\") 修改文件开头。");
  println!("- 已调用 flush() 将修改显式同步到磁盘。");

  Ok(())
}
