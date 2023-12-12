use tokio::fs::File;
use tokio::io::{self, AsyncReadExt};

#[tokio::main]
async fn main() -> io::Result<()> {
  let a = 1;
  let b = 2;
  let c = 3;
  let list = vec![&a, &b, &c];
  println!("list: {:?}", list);

  // let list2 = vec![&1, &2, &3];

  let filenames = vec!["file1.txt", "file2.txt", "file3.txt"];

  let mut tasks = Vec::new();

  for filename in filenames {
    let task = tokio::spawn(async move {
      // println!("list: {:?}", list2);
      let mut file = File::open(filename).await?;
      let mut contents = Vec::new();
      file.read_to_end(&mut contents).await?;
      Ok::<_, io::Error>(contents)
    });

    tasks.push(task);
  }

  for task in tasks {
    let contents = task.await??;
    println!("File contents: {:?}", contents);
  }

  Ok(())
}
