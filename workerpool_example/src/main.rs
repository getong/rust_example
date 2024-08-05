use std::io::prelude::*;
use std::io::{self, BufReader};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::channel;
use workerpool::{Pool, Worker};

struct LineDelimitedProcess {
  stdin: ChildStdin,
  stdout: BufReader<ChildStdout>,
}
impl Default for LineDelimitedProcess {
  fn default() -> Self {
    let child = Command::new("cat")
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::inherit())
      .spawn()
      .unwrap();
    Self {
      stdin: child.stdin.unwrap(),
      stdout: BufReader::new(child.stdout.unwrap()),
    }
  }
}
impl Worker for LineDelimitedProcess {
  type Input = Box<[u8]>;
  type Output = io::Result<String>;

  fn execute(&mut self, inp: Self::Input) -> Self::Output {
    self.stdin.write_all(&*inp)?;
    self.stdin.write_all(b"\n")?;
    self.stdin.flush()?;
    let mut s = String::new();
    self.stdout.read_line(&mut s)?;
    s.pop(); // exclude newline
    Ok(s)
  }
}

fn main() {
  let n_workers = 4;
  let n_jobs = 8;
  let pool = Pool::<LineDelimitedProcess>::new(n_workers);

  let (tx, rx) = channel();
  for i in 0 .. n_jobs {
    let inp = Box::new([97 + i]);
    pool.execute_to(tx.clone(), inp);
  }

  // output is a permutation of "abcdefgh"
  let mut output = rx
    .iter()
    .take(n_jobs as usize)
    .fold(String::new(), |mut a, b| {
      a.push_str(&b.unwrap());
      a
    })
    .into_bytes();
  output.sort();
  assert_eq!(output, b"abcdefgh");
}
