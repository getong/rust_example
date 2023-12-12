use glommio::{
  io::{DmaFile, DmaStreamReaderBuilder},
  LocalExecutor,
};

fn main() {
  // println!("Hello, world!");
  let ex = LocalExecutor::default();
  ex.run(async {
    let file = DmaFile::open("myfile.txt").await.unwrap();
    let mut reader = DmaStreamReaderBuilder::new(file).build();
    assert_eq!(reader.current_pos(), 0);
    let result = reader.get_buffer_aligned(512).await.unwrap();
    assert_eq!(result.len(), 512);
    println!("First 512 bytes: {:?}", &*result);
    reader.close().await.unwrap();
  });
}
