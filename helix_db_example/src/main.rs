use helix_db_example::{add_user, sample_batches};

fn main() {
  let (read_batches, write_batches) = sample_batches();
  let dynamic_request = add_user("John".to_string());

  println!(
    "built {} read batches, {} write batches, and dynamic query {:?}",
    read_batches.len(),
    write_batches.len(),
    dynamic_request.query_name
  );
}
