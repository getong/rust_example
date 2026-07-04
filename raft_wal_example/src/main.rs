use raft_wal::AsyncRaftWal;

#[tokio::main]
async fn main() {
  let mut wal = AsyncRaftWal::open("./my-raft-data").await.unwrap();

  wal.append(1, b"entry-1").await.unwrap();
  wal.set_meta("term", b"1").await.unwrap();

  // Must call close() — tokio can't flush in Drop
  wal.close().await.unwrap();
}
