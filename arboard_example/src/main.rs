use std::{error::Error, io, path::Path};

use arboard::Clipboard;
use image::{ImageBuffer, Rgba};

fn main() -> Result<(), Box<dyn Error>> {
  let mut clipboard = Clipboard::new()?;

  if let Ok(text) = clipboard.get_text() {
    println!("Clipboard text was: {text}");
    return Ok(());
  }

  let image = clipboard.get_image()?;
  let path = Path::new("clipboard.png");
  save_clipboard_image(image, path)?;
  println!("saved clipboard image to {}", path.display());

  // 原来的写入剪贴板示例保留在这里，默认不执行，避免覆盖用户已有剪贴板内容。
  // let the_string = "Hello, world!";
  // clipboard.set_text(the_string)?;
  // println!("But now the clipboard text should be: \"{}\"", the_string);

  Ok(())
}

fn save_clipboard_image(
  image: arboard::ImageData<'static>,
  path: &Path,
) -> Result<(), Box<dyn Error>> {
  let width = u32::try_from(image.width)?;
  let height = u32::try_from(image.height)?;

  let buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, image.bytes.into_owned())
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid RGBA image buffer"))?;

  buffer.save(path)?;

  Ok(())
}
