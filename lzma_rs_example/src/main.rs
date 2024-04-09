fn main() {
  let filename = "foo.xz";
  let mut f = std::io::BufReader::new(std::fs::File::open(filename).unwrap());
  // "decomp" can be anything that implements "std::io::Write"
  let mut decomp: Vec<u8> = Vec::new();
  lzma_rs::xz_decompress(&mut f, &mut decomp).unwrap();
  // Decompressed content is now in "decomp"
}
